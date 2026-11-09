//! The go-back and go-forward buttons in the window's title/caption bar,
//! or next to the rooms filter bar if there's no caption bar.

use makepad_widgets::*;

use crate::{
    home::{
        nav_history::{GoBackAction, GoForwardAction},
        nav_history_menu::{LongPressWatcher, NavHistoryDirection, request_nav_history_menu},
    },
    room::room_action_bar::RoomActionTooltip,
};

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.NavHistoryButton = RobrixIconButton {
        width: 32, height: 26
        margin: 0
        padding: 0
        spacing: 0
        align: Align{x: 0.5, y: 0.5}
        enabled: false
        // Don't grab focus so that so pressing Space/Enter after a click won't go back/forward again.
        grab_key_focus: false
        // Long-presses and drags don't count as clicks (only regular taps), so enable this.
        enable_long_press: true
        draw_bg +: {
            border_radius: 5.0
            color: (COLOR_TRANSPARENT)
            color_hover: (COLOR_NAV_HISTORY_BUTTON_BG_HOVER)
            color_down: (COLOR_NAV_HISTORY_BUTTON_BG_DOWN)
        }
        draw_icon.color: (COLOR_FG_DISABLED)
        icon_walk: Walk{width: 18, height: 18}
    }

    mod.widgets.NavHistoryButtons = #(NavHistoryButtons::register_widget(vm)) {
        width: Fit, height: Fit
        spacing: 2
        align: Align{y: 0.5}

        go_back_button := mod.widgets.NavHistoryButton {
            draw_icon +: { svg: (ICON_ARROW_LEFT) }
        }
        go_forward_button := mod.widgets.NavHistoryButton {
            draw_icon +: { svg: (ICON_ARROW_RIGHT) }
        }
    }
}

/// The go-back and go-forward buttons, each of which is only enabled when it can go somewhere.
#[derive(Script, ScriptHook, Widget)]
pub struct NavHistoryButtons {
    #[source] source: ScriptObjectRef,
    #[deref] view: View,
    #[rust] tooltip: RoomActionTooltip,
    #[rust] long_press: LongPressWatcher,
    #[rust] are_tooltips_hidden: bool,
}

impl Widget for NavHistoryButtons {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        // Long-pressing these buttons shows the nav history popup menu, not tooltips.
        if !self.are_tooltips_hidden && !matches!(event, Event::Actions(_)) {
            let buttons_iter = [(id!(go_back_button), "Go back"), (id!(go_forward_button), "Go forward")]
                .into_iter()
                .map(|(id, text)| (self.view.child(id), text));
            self.tooltip.handle_event(cx, event, buttons_iter, TooltipPosition::Bottom);
        }

        self.view.handle_event(cx, event, scope);

        let long_pressed_button = self.long_press.handle_event(cx, event, |cx, uid| {
            [ids!(go_back_button), ids!(go_forward_button)].into_iter()
                .any(|button_id| self.view.button(cx, button_id).widget_uid() == uid)
        });
        if let Some((uid, held_pointer)) = long_pressed_button {
            let back_button = self.view.button(cx, ids!(go_back_button));
            let (direction, button) = if back_button.widget_uid() == uid {
                (NavHistoryDirection::Back, back_button)
            } else {
                (NavHistoryDirection::Forward, self.view.button(cx, ids!(go_forward_button)))
            };
            request_nav_history_menu(cx, direction, &button, held_pointer);
        }
        if let Event::Actions(actions) = event {
            if self.view.button(cx, ids!(go_back_button)).clicked(actions) {
                cx.action(GoBackAction { steps: 1 });
            }
            if self.view.button(cx, ids!(go_forward_button)).clicked(actions) {
                cx.action(GoForwardAction { steps: 1 });
            }
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl NavHistoryButtonsRef {
    /// Hides (or shows) the buttons' tooltips and keeps them hidden (or shown).
    pub fn hide_tooltips(&self, cx: &mut Cx, hide: bool) {
        let Some(mut inner) = self.borrow_mut() else { return };
        if hide && !inner.are_tooltips_hidden {
            inner.tooltip.hide(cx);
        }
        inner.are_tooltips_hidden = hide;
    }
}
