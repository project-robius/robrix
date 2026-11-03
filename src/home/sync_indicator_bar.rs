//! `SyncIndicatorBar`, a thin bar along the top of a room in the mobile view mode,
//! which shows a streak zipping across it while we're syncing and a red "Offline" bar while we're offline.

use makepad_widgets::*;

use crate::home::rooms_list_header::{RoomsListHeaderAction, SyncStatus, get_sync_status};

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    // We draw just the bottom of a tall box that's as rounded as the header,
    // so the bar's ends follow the header's bottom corners.
    let SyncIndicatorBarDraw = mod.draw.DrawColor {
        border_radius: uniform(4.0)
        get_color: fn() {
            return self.color
        }
        pixel: fn() {
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.box(0.0, self.rect_size.y - 100.0, self.rect_size.x, 100.0, self.border_radius)
            return sdf.fill(self.get_color())
        }
    }

    mod.widgets.SyncIndicatorBar = #(SyncIndicatorBar::register_widget(vm)) {
        width: Fill, height: 3
        // The offline bar is taller, to fit its label.
        offline_height: 17
        align: Align{x: 0.5, y: 0.5}

        draw_syncing: SyncIndicatorBarDraw {
            color: (COLOR_ROBRIX_CYAN)
            // A cyan streak runs along a faint cyan track: it fades in at the left, speeds up until it's halfway across,
            // then eases out. The faster it goes, the longer its tail.
            get_color: fn() {
                let phase = fract(self.draw_pass.time / 1.3)
                let past_middle = step(2.0 / 3.0, phase)
                let eased = mix(0.5 * pow(phase * 1.5, 4.0), 1.0 - 0.5 * pow((1.0 - phase) * 3.0, 2.0), past_middle)
                let speed = mix(pow(phase * 1.5, 3.0), (1.0 - phase) * 3.0, past_middle)
                let d = (self.pos.x - mix(-0.03, 1.2, eased)) * self.rect_size.x
                let front = d / 8.0
                let tail = pow(max(0.0, 1.0 + d / (60.0 + 220.0 * speed)), 1.3)
                let glow = mix(tail, exp(-front * front), step(0.0, d)) * smoothstep(0.0, 0.1, phase)
                return vec4(self.color.rgb, self.color.a * mix(0.3, 1.0, glow))
            }
        }
        draw_offline: SyncIndicatorBarDraw { color: (COLOR_BG_DANGER_RED_SOLID) }
        draw_offline_text +: {
            ink_centered: true
            color: (COLOR_TEXT_ON_ACCENT)
            text_style: theme.font_regular { font_size: 8.5 }
        }
    }
}

#[derive(Script, ScriptHook, Widget)]
pub struct SyncIndicatorBar {
    #[uid] uid: WidgetUid,
    #[walk] walk: Walk,
    #[layout] layout: Layout,
    #[live] offline_height: f64,
    #[live] draw_syncing: DrawColor,
    #[live] draw_offline: DrawColor,
    #[live] draw_offline_text: DrawText,
    #[redraw] #[rust] area: Area,
}

impl Widget for SyncIndicatorBar {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, _scope: &mut Scope) {
        if let Event::Actions(actions) = event
            && actions.iter().any(|action| action.downcast_ref::<RoomsListHeaderAction>().is_some())
        {
            self.redraw(cx);
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, _scope: &mut Scope, walk: Walk) -> DrawStep {
        match get_sync_status(cx) {
            SyncStatus::Offline => {
                self.draw_offline.begin(cx, Walk { height: Size::Fixed(self.offline_height), ..walk }, self.layout);
                self.draw_offline_text.draw_walk(cx, Walk::fit(), Align::default(), "Offline");
                self.draw_offline.end(cx);
                self.area = self.draw_offline.area();
            }
            status => {
                // Our area must exist even when we draw nothing, so that a status change can redraw us.
                let rect = cx.walk_turtle_with_area(&mut self.area, walk);
                if status == SyncStatus::Syncing {
                    self.draw_syncing.draw_abs(cx, rect);
                }
            }
        }
        DrawStep::done()
    }
}

impl SyncIndicatorBarRef {
    /// Returns how much taller we make our header, which we only do while showing the offline bar.
    pub fn extra_height(&self, cx: &mut Cx) -> f64 {
        let Some(inner) = self.borrow() else { return 0.0 };
        if get_sync_status(cx) == SyncStatus::Offline {
            inner.offline_height + inner.walk.margin.top
        } else {
            0.0
        }
    }
}
