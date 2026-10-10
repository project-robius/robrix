//! The go-back and go-forward buttons in the window's title/caption bar.

use makepad_widgets::*;

use crate::{
    app::AppState,
    home::{home_screen::effective_is_desktop, nav_history::{GoBackAction, GoForwardAction, can_navigate_history, can_show}},
    room::room_action_bar::RoomActionTooltip,
    shared::styles::{COLOR_FG_DISABLED, COLOR_TEXT},
};

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.NavHistoryButton = RobrixIconButton {
        width: 28, height: 24
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
            color: #0000
            color_hover: #x00000014
            color_down: #x00000026
        }
        draw_icon.color: (COLOR_FG_DISABLED)
        icon_walk: Walk{width: 16, height: 16}
    }

    mod.widgets.NavHistoryButtons = #(NavHistoryButtons::register_widget(vm)) {
        width: Fit, height: Fit
        margin: Inset{left: 8}
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
    #[rust] can_go_back: bool,
    #[rust] can_go_forward: bool,
}

impl Widget for NavHistoryButtons {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        // This is lazy, as the tooltip only looks at the buttons for the few events that can show it.
        let buttons = [(id!(go_back_button), "Go back"), (id!(go_forward_button), "Go forward")]
            .into_iter()
            .map(|(id, text)| (self.view.child(id), text));
        self.tooltip.handle_event(cx, event, buttons, TooltipPosition::Bottom);
        self.view.handle_event(cx, event, scope);
        match event {
            Event::Actions(actions) => {
                if self.view.button(cx, ids!(go_back_button)).clicked(actions) {
                    cx.action(GoBackAction);
                }
                if self.view.button(cx, ids!(go_forward_button)).clicked(actions) {
                    cx.action(GoForwardAction);
                }
            }
            // A reapply resets the buttons to their DSL defaults, so we enable them again.
            Event::ScriptReapply => self.enable_buttons(cx, self.can_go_back, self.can_go_forward),
            _ => {}
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, mut walk: Walk) -> DrawStep {
        // Avoid drawing over the window chrome buttons, either on the left or right of the caption bar.
        if let Some(window_id) = cx.get_current_window_id() {
            let geom = &cx.windows[window_id].window_geom;
            let chrome = geom.window_chrome_buttons;
            if chrome.size.x > 0.0 && chrome.pos.x + chrome.size.x / 2.0 < geom.inner_size.x / 2.0 {
                walk.margin.left += chrome.pos.x + chrome.size.x;
            }
        }
        self.view.draw_walk(cx, scope, walk)
    }
}

impl NavHistoryButtons {
    /// Enables or disables (grays out) the go-back and go-forward buttons.
    fn enable_buttons(&mut self, cx: &mut Cx, can_go_back: bool, can_go_forward: bool) {
        self.can_go_back = can_go_back;
        self.can_go_forward = can_go_forward;
        for (button_id, enable) in [(ids!(go_back_button), can_go_back), (ids!(go_forward_button), can_go_forward)] {
            let mut button = self.view.button(cx, button_id);
            // A button that we disable while it's hovered shouldn't stay highlighted.
            if !enable {
                button.reset_hover(cx);
            }
            let icon_color = if enable { COLOR_TEXT } else { COLOR_FG_DISABLED };
            script_apply_eval!(cx, button, {
                enabled: #(enable),
                draw_icon.color: #(icon_color),
            });
        }
        self.view.redraw(cx);
    }
}

impl NavHistoryButtonsRef {
    /// See [`NavHistoryButtons::enable_buttons()`].
    pub fn enable_buttons(&self, cx: &mut Cx, can_go_back: bool, can_go_forward: bool) {
        let Some(mut inner) = self.borrow_mut() else { return };
        if (inner.can_go_back, inner.can_go_forward) != (can_go_back, can_go_forward) {
            inner.enable_buttons(cx, can_go_back, can_go_forward);
        }
    }
}

/// Enables or disables the title bar's go-back and go-forward buttons, depending on where we can go right now.
pub fn enable_nav_history_buttons(cx: &mut Cx, ui: &WidgetRef, app_state: &AppState) {
    let (can_go_back, can_go_forward) = if can_navigate_history(cx, ui, app_state) {
        let can_show = can_show(cx);
        let nav_history = &app_state.nav_history;
        let can_go_back = if effective_is_desktop(cx) {
            nav_history.previous_place(app_state.selected_room.as_ref(), &can_show).is_some()
        } else {
            // Mobile view mode goes back from any shown screen, or from the rooms list to a previous screen.
            ui.stack_navigation(cx, ids!(view_stack)).destination_view().is_some()
                || matches!(nav_history.previous_place(None, &can_show), Some(Some(_)))
        };
        (can_go_back, nav_history.next_place(&can_show).is_some())
    } else {
        (false, false)
    };
    ui.nav_history_buttons(cx, ids!(nav_history_buttons)).enable_buttons(cx, can_go_back, can_go_forward);
}
