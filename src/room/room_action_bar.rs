//! Room actions shared by the mobile stack header and desktop room tabs.

use makepad_widgets::*;

use crate::shared::popup_list::{enqueue_popup_notification, PopupKind};

// currently there's a fixed number of action buttons,
// but later we'll do that dynamically once we have more features implemented.
const ACTION_COUNT: usize = 5;
const HEADER_HEIGHT: f64 = 45.0;
const BUTTON_SIZE: f64 = 40.0;
const HEADER_BUTTON_GAP: f64 = 2.0;
const HEADER_BUTTON_INSET: f64 = (HEADER_HEIGHT - BUTTON_SIZE) * 0.5;
const BACK_BUTTON_WIDTH: f64 = 56.0;
const BACK_BUTTON_INSET: f64 = 3.0;
const GAP: f64 = 4.0;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    // A widget with a shadow effect shared by the mobile stack header and expanded desktop room actions.
    // This is shown right under the stack nav bar or the dock tabs, it should kinda blend it with them
    // on the top, but have a shadow beneath it to look like it's floating over the room timeline below it.
    mod.widgets.RobrixHeaderBackground = mod.draw.DrawQuad {
        color: instance((COLOR_PRIMARY_DARKER))
        color_dither: uniform(1.0)
        gradient_border_horizontal: uniform(0.0)
        gradient_fill_horizontal: uniform(0.0)
        color_2: instance(vec4(-1))

        border_radius: uniform(4.0)
        border_size: uniform(0.0)
        border_color: instance(#0000)
        border_color_2: instance(vec4(-1))

        shadow_color: instance(#0005)
        shadow_radius: uniform(12.0)
        shadow_offset: uniform(vec2(0.0, 0.0))

        rect_size2: varying(vec2(0))
        rect_size3: varying(vec2(0))
        rect_pos2: varying(vec2(0))
        rect_shift: varying(vec2(0))
        sdf_rect_pos: varying(vec2(0))
        sdf_rect_size: varying(vec2(0))

        vertex: fn() {
            let min_offset = min(self.shadow_offset vec2(0))
            self.rect_size2 = self.rect_size + 2.0*vec2(self.shadow_radius)
            self.rect_size3 = self.rect_size2 + abs(self.shadow_offset)
            self.rect_pos2 = self.rect_pos - vec2(self.shadow_radius) + min_offset
            self.sdf_rect_size = self.rect_size2 - vec2(self.shadow_radius * 2.0 + self.border_size * 2.0)
            self.sdf_rect_pos = -min_offset + vec2(self.border_size + self.shadow_radius)
            self.rect_shift = -min_offset

            return self.clip_and_transform_vertex(self.rect_pos2 self.rect_size3)
        }

        pixel: fn() {
            let sdf = Sdf2d.viewport(self.pos * self.rect_size3)

            let mut fill_color = self.color
            if self.color_2.x > -0.5 {
                let dither = Math.random_2d(self.pos.xy) * 0.04 * self.color_dither
                let dir = if self.gradient_fill_horizontal > 0.5 self.pos.x else self.pos.y
                fill_color = mix(self.color self.color_2 dir + dither)
            }

            let mut stroke_color = self.border_color
            if self.border_color_2.x > -0.5 {
                let dither = Math.random_2d(self.pos.xy) * 0.04 * self.color_dither
                let dir = if self.gradient_border_horizontal > 0.5 self.pos.x else self.pos.y
                stroke_color = mix(self.border_color self.border_color_2 dir + dither)
            }

            sdf.box(
                self.sdf_rect_pos.x
                self.sdf_rect_pos.y
                self.sdf_rect_size.x
                self.sdf_rect_size.y
                max(1.0 self.border_radius)
            )
            if sdf.shape > -1.0 {
                let m = self.shadow_radius
                let o = self.shadow_offset + self.rect_shift
                let v = GaussShadow.rounded_box_shadow(vec2(m) + o self.rect_size2+o self.pos * (self.rect_size3+vec2(m)) self.shadow_radius*0.5 self.border_radius*2.0)
                // Only draw shadow on the bottom half of the view
                let pixel_y = self.pos.y * self.rect_size3.y
                let mid_y = self.sdf_rect_pos.y + self.sdf_rect_size.y * 0.5
                let bottom_mask = smoothstep(mid_y - m * 0.3 mid_y + m * 0.3 pixel_y)
                sdf.clear(self.shadow_color * v * bottom_mask)
            }

            sdf.fill_keep(fill_color)

            if self.border_size > 0.0 {
                sdf.stroke(stroke_color self.border_size)
            }
            return sdf.result
        }
    }

    // An icon-only button that's shown in the room action bar when collapsed in mobile view mode.
    mod.widgets.RoomActionButton = RobrixNeutralIconButton {
        width: 40, height: 40
        padding: 8
        margin: 0
        spacing: 0
        text: ""
        align: Align{x: 0.5, y: 0.5}
        icon_walk: Walk{width: 18, height: 18}
        draw_icon.color: ROOM_NAME_TEXT_COLOR
        draw_bg +: {
            color: #0000
            color_hover: #0001
            color_down: #0002
        }
        draw_text +: {
            color: ROOM_NAME_TEXT_COLOR
            color_hover: ROOM_NAME_TEXT_COLOR
            color_down: ROOM_NAME_TEXT_COLOR
        }
    }

    // the same as above, but for the expanded room action bar, where text is shown next to the button icon.
    let RoomActionTextButton = mod.widgets.RoomActionButton {
        width: Fit
        spacing: 8
        margin: Inset{bottom: 4}
        draw_bg +: {
            color: #xEDE8FD
            color_hover: #xE8E1FA
            color_down: #xE1D7F7
        }
    }

    mod.widgets.RoomActionBar = #(RoomActionBar::register_widget(vm)) {
        width: Fill, height: Fit
        flow: Overlay
        show_bg: true
        draw_bg: mod.widgets.RobrixHeaderBackground {shadow_color: #0000}
        draw_shadow: mod.widgets.RobrixHeaderBackground {
            color: #0000
        }

        room_info_button            := mod.widgets.RoomActionButton {draw_icon.svg: ICON_INFO}
        room_threads_button         := mod.widgets.RoomActionButton {draw_icon.svg: ICON_REPLY_IN_THREAD}
        room_pinned_messages_button := mod.widgets.RoomActionButton {draw_icon.svg: ICON_PIN}
        room_members_button         := mod.widgets.RoomActionButton {draw_icon.svg: ICON_MEMBERS}
        room_settings_button        := mod.widgets.RoomActionButton {draw_icon.svg: ICON_SETTINGS}
        expand_room_actions_button  := mod.widgets.RoomActionButton {
            draw_icon.svg: ICON_CHEVRON_DOWN
            icon_walk: Walk{width: 17, height: 17}
        }
        collapse_room_actions_button := mod.widgets.RoomActionButton {
            draw_icon.svg: ICON_CHEVRON_UP
            icon_walk: Walk{width: 17, height: 17}
        }
        expanded_room_actions := View {
            width: Fill, height: Fit
            flow: Flow.Right{wrap: true}
            spacing: 4
            padding: Inset{left: 8, right: 8, top: 8, bottom: 4}
            room_info_button := RoomActionTextButton {draw_icon.svg: ICON_INFO}
            room_settings_button := RoomActionTextButton {draw_icon.svg: ICON_SETTINGS}
            room_threads_button := RoomActionTextButton {draw_icon.svg: ICON_REPLY_IN_THREAD}
            room_members_button := RoomActionTextButton {draw_icon.svg: ICON_MEMBERS}
            room_pinned_messages_button := RoomActionTextButton {draw_icon.svg: ICON_PIN}
        }
    }

    mod.widgets.RoomActionBarHeader = mod.widgets.RoomActionBar {
        is_desktop_mode: false
        // The stack nav header itself draws the background, not us.
        show_bg: false

        // the `button_container.left_button`` is what StackNavigation uses to find the button by path ID,
        // so we must maintain that exact path name here (we're just overriding its style).
        button_container := View {
            width: 56, height: 45
            align: Align{y: 0.5}
            left_button := mod.widgets.RoomActionButton {
                width: Fill
                enable_long_press: true
                draw_icon.svg: crate_resource("self://resources/icons/back.svg")
                icon_walk: Walk{width: 12, height: 18}
            }
        }
        title_container := View {
            width: Fill, height: 45
            align: Align{y: 0.5}
            title := Label {
                width: Fill, height: Fit
                margin: Inset{left: 4, right: 4}
                padding: 0
                align: Align{x: 0.0, y: 0.5}
                text: ""
                text_overflow: TextOverflow.Ellipsis
                max_lines: 1
                draw_text +: {
                    text_style: theme.font_bold {font_size: 12}
                    color: ROOM_NAME_TEXT_COLOR
                }
            }
        }
    }
}

/// This also defines the ordering of the buttons, from right to left
/// so that buttons don't move around when the bar's width changes.
const ACTIONS: [(LiveId, &str); ACTION_COUNT] = [
    (id!(room_info_button), "Room info"),
    (id!(room_settings_button), "Room settings"),
    (id!(room_threads_button), "Threads"),
    (id!(room_members_button), "Members"),
    (id!(room_pinned_messages_button), "Pinned messages"),
];

pub fn show_room_action_placeholder(label: &'static str) {
    // TODO: implement the features for these buttons
    enqueue_popup_notification(
        format!("{label}: not yet implemented."),
        PopupKind::Warning,
        Some(4.0),
    );
}

/// Actions emitted by [`RoomActionBar`].
#[derive(Clone, Debug, Default)]
pub enum RoomActionBarAction {
    /// Emitted after the header is first drawn and whenever its height changes.
    LayoutChanged {
        /// The action bar's height in logical pixels, including any expanded rows.
        new_height: f64,
    },
    #[default]
    None,
}

#[derive(Script, Widget)]
pub struct RoomActionBar {
    #[deref] view: View,
    #[live] draw_shadow: DrawQuad,
    /// Whether we're in desktop view mode (`true`) or mobile view mode (`false`).
    #[live(true)] is_desktop_mode: bool,
    /// Whether the full list of buttons is expanded.
    #[rust] is_expanded: bool,
    /// The layout we last drew, so we know when it needs to be redrawn.
    ///
    /// This is: `(width, is_desktop_mode, is_expanded, num buttons in header)`.
    #[rust] latest_layout: Option<(f64, bool, bool, usize)>,
    /// The header height we last broadcast.
    #[rust] latest_header_height: Option<f64>,
    #[rust] redraw_next_frame: NextFrame,
    #[rust] icon_tooltip: RoomActionTooltip,
}

impl ScriptHook for RoomActionBar {
    fn on_after_apply(&mut self, _vm: &mut ScriptVm, _apply: &Apply, _scope: &mut Scope, _value: ScriptValue) {
        self.latest_layout = None;
    }
}

impl Widget for RoomActionBar {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if self.redraw_next_frame.is_event(event).is_some() {
            self.redraw(cx);
        }
        if !self.is_desktop_mode {
            let mut buttons: Vec<_> = ACTIONS.iter().map(|(id, label)| {
                (self.view.widget(cx, &[*id]), *label)
            }).collect();
            buttons.push((self.view.widget(cx, ids!(button_container.left_button)), "Back"));
            self.icon_tooltip.handle_event(cx, event, buttons, TooltipPosition::Bottom);
        }
        self.view.handle_event(cx, event, scope);
        if let Event::Actions(actions) = event {
            if !self.is_desktop_mode && (self.view.button(cx, ids!(expand_room_actions_button)).clicked(actions)
                || self.view.button(cx, ids!(collapse_room_actions_button)).clicked(actions))
            {
                self.set_expanded(cx, !self.is_expanded);
            }
            for (id, label) in ACTIONS {
                if self.view.button(cx, &[id]).clicked(actions)
                    || self.view.button(cx, &[id!(expanded_room_actions), id]).clicked(actions)
                {
                    show_room_action_placeholder(label);
                }
            }
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, mut walk: Walk) -> DrawStep {
        let width = (cx.peek_walk_turtle(walk).size.x - self.view.layout.padding.left - self.view.layout.padding.right).max(0.0);
        let inline = if !self.is_desktop_mode {
            let title = self.view.label(cx, ids!(title_container.title));
            let title_width = title.borrow().map(|title| {
                title.draw_text.layout(cx, 0.0, 0.0, None, false, Align::default(), &title.text())
                    .size_in_lpxs.width as f64 * title.draw_text.font_scale as f64
                    + title.walk.margin.width()
            }).unwrap_or(width);
            inline_count(width, title_width)
        } else {
            // in desktop view mode, the dock tabs only show the expand/collapse button.
            0
        };
        let state = (width, self.is_desktop_mode, self.is_expanded, inline);
        if self.latest_layout != Some(state) {
            self.update_layout(cx, width, inline);
            self.latest_layout = Some(state);
        }
        walk.height = self.view.walk.height;
        let step = self.view.draw_walk(cx, scope, walk);
        if step.is_done() {
            let rect = self.view.area().rect(cx);
            if !self.is_desktop_mode && self.latest_header_height != Some(rect.size.y) {
                self.latest_header_height = Some(rect.size.y);
                cx.widget_action(
                    self.widget_uid(),
                    RoomActionBarAction::LayoutChanged { new_height: rect.size.y },
                );
                // Issue another redraw for the whole stack nav widget so the room content
                // (in the stack nav body) is properly drawn below the header.
                self.redraw_next_frame = cx.new_next_frame();
            }
        }
        step
    }
}

impl RoomActionBar {
    fn set_expanded(&mut self, cx: &mut Cx, expanded: bool) {
        self.is_expanded = expanded;
        self.latest_layout = None;
        self.redraw_next_frame = cx.new_next_frame();
        self.redraw(cx);
    }

    fn update_layout(&mut self, cx: &mut Cx, width: f64, inline: usize) {
        self.icon_tooltip.hide(cx);
        let show_overflow = self.is_expanded;
        self.view.show_bg = self.is_desktop_mode && show_overflow;
        self.view.walk.height = if show_overflow {
            Size::Fit { min: None, max: None }
        } else {
            Size::Fixed(if self.is_desktop_mode { 0.0 } else { HEADER_HEIGHT })
        };
        let mut expanded_room_actions = self.view.view(cx, ids!(expanded_room_actions));
        expanded_room_actions.set_visible(cx, show_overflow);
        let overflow_y = if self.is_desktop_mode { 0.0 } else { HEADER_HEIGHT };
        script_apply_eval!(cx, expanded_room_actions, {
            width: #(width)
            margin: mod.prelude.widgets.Inset{top: #(overflow_y)}
        });
        self.view.view(cx, ids!(button_container)).set_visible(cx, !self.is_desktop_mode);
        self.view.view(cx, ids!(title_container)).set_visible(cx, !self.is_desktop_mode);
        self.view.button(cx, ids!(expand_room_actions_button)).set_visible(cx, !self.is_desktop_mode && !self.is_expanded);
        self.view.button(cx, ids!(collapse_room_actions_button)).set_visible(cx, !self.is_desktop_mode && self.is_expanded);

        if !self.is_desktop_mode {
            // Inline action buttons can only use any space that's leftover after the room name.
            let title_x = BACK_BUTTON_INSET + BACK_BUTTON_WIDTH + GAP;
            let toggle_x = (width - HEADER_BUTTON_INSET - BUTTON_SIZE).max(0.0);
            let slots = inline + 1;
            let title_width = (width - HEADER_BUTTON_INSET - title_x - slots as f64 * (BUTTON_SIZE + HEADER_BUTTON_GAP)).max(0.0);
            place(cx, &self.view.widget(cx, ids!(button_container)), BACK_BUTTON_INSET, 0.0, BACK_BUTTON_WIDTH, HEADER_HEIGHT);
            place(cx, &self.view.widget(cx, ids!(title_container)), title_x, 0.0, title_width, HEADER_HEIGHT);
            for toggle in [id!(expand_room_actions_button), id!(collapse_room_actions_button)] {
                place(cx, &self.view.widget(cx, &[toggle]), toggle_x, (HEADER_HEIGHT - BUTTON_SIZE) * 0.5, BUTTON_SIZE, BUTTON_SIZE);
            }
        }

        for (index, (id, label)) in ACTIONS.iter().enumerate() {
            let button = self.view.widget(cx, &[*id]);
            let in_header_row = !self.is_desktop_mode && index < inline;
            button.set_visible(cx, in_header_row);
            let expanded_button = expanded_room_actions.widget(cx, &[*id]);
            expanded_button.set_visible(cx, show_overflow);
            expanded_button.set_text(cx, label);
            if in_header_row {
                let slots_after = index + 2;
                let x = width - HEADER_BUTTON_INSET - slots_after as f64 * (BUTTON_SIZE + HEADER_BUTTON_GAP) + HEADER_BUTTON_GAP;
                place(cx, &button, x, HEADER_BUTTON_INSET, BUTTON_SIZE, BUTTON_SIZE);
                let mut button = button;
                script_apply_eval!(cx, button, {enable_long_press: true});
            }
        }
    }
}

/// Reserve space for the full room name and the always-visible expand/collapse button.
fn inline_count(width: f64, title_width: f64) -> usize {
    let available = width - BACK_BUTTON_INSET - HEADER_BUTTON_INSET - BACK_BUTTON_WIDTH - GAP;
    let action_width = BUTTON_SIZE + HEADER_BUTTON_GAP;
    (((available - action_width - title_width.ceil()).max(0.0) / action_width) as usize).min(ACTION_COUNT)
}

fn place(cx: &mut Cx, widget: &WidgetRef, x: f64, y: f64, width: f64, height: f64) {
    let mut widget = widget.clone();
    script_apply_eval!(cx, widget, {
        width: #(width), height: #(height)
        margin: mod.prelude.widgets.Inset{left: #(x), top: #(y)}
    });
}

impl RoomActionBarRef {
    pub fn draw_shadow(&self, cx: &mut Cx2d, room_rect: Rect) {
        if let Some(mut inner) = self.borrow_mut()
            && inner.view.visible && inner.is_desktop_mode && inner.is_expanded
        {
            let rect = inner.view.area().rect(cx);
            cx.push_clip_rect(room_rect);
            inner.draw_shadow.draw_abs(cx, rect);
            cx.pop_clip_rect();
        }
    }

    pub fn set_expanded(&self, cx: &mut Cx, expanded: bool) {
        if let Some(mut inner) = self.borrow_mut() { inner.set_expanded(cx, expanded); }
    }

    pub fn is_expanded(&self) -> bool {
        self.borrow().is_some_and(|inner| inner.is_expanded)
    }
}

/// This uses the main top-level app's tooltip but doesn't consume hits.
#[derive(Default)]
pub(super) struct RoomActionTooltip {
    hovered: Option<WidgetUid>,
}

impl RoomActionTooltip {
    pub fn hide(&mut self, cx: &mut Cx) {
        if let Some(uid) = self.hovered.take() {
            cx.widget_action(uid, TooltipAction::HoverOut);
        }
    }

    pub fn handle_event(
        &mut self,
        cx: &mut Cx,
        event: &Event,
        buttons: impl IntoIterator<Item = (WidgetRef, &'static str)>,
        position: TooltipPosition,
    ) -> bool {
        // determine which tab is being hovered over, and get its target info:
        // `(tab widget uid, tab text, tab area's rect)`.
        let target = match event {
            Event::Actions(actions) => {
                let target = buttons.into_iter().find_map(|(button, text)| {
                    let rect = button.area().clipped_rect(cx);
                    (button.visible() && button.text().is_empty()
                        && button.as_button().long_pressed(actions)
                        && rect.size.x > 0.0 && rect.size.y > 0.0
                    )
                    .then_some((button.widget_uid(), text, rect))
                });
                if target.is_none() { return self.hovered.is_some(); }
                target
            }
            Event::MouseMove(mouse) if event.pointer_claimed_area().is_empty()
                && cx.fingers.first_mouse_button.is_none() => {
                buttons.into_iter().find_map(|(button, text)| {
                    let rect = button.area().clipped_rect(cx);
                    (button.visible() && button.text().is_empty()
                        && rect.size.x > 0.0 && rect.size.y > 0.0 && rect.contains(mouse.abs))
                        .then_some((button.widget_uid(), text, rect))
                })
            }
            Event::MouseMove(_) | Event::MouseLeave(_) | Event::MouseDown(_) | Event::MouseUp(_)
            | Event::Scroll(_) | Event::TouchUpdate(_) | Event::KeyDown(_) | Event::BackPressed { .. }
            | Event::WindowLostFocus(_) | Event::WindowGeomChange(_) | Event::ClearHover => None,
            _ => return self.hovered.is_some(),
        };
        if target.as_ref().map(|(uid, _, _)| *uid) != self.hovered {
            self.hide(cx);
            if let Some((uid, text, rect)) = target {
                self.hovered = Some(uid);
                cx.widget_action(uid, TooltipAction::HoverIn {
                    text: text.into(),
                    widget_rect: rect,
                    options: CalloutTooltipOptions { position, ..Default::default() },
                });
            }
        }
        self.hovered.is_some()
    }
}
