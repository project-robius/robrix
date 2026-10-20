//! The go-back and go-forward buttons in the window's title/caption bar.

use makepad_widgets::*;

use crate::{
    app::AppState,
    home::{
        nav_history::{GoBackAction, GoForwardAction, can_navigate_history, can_show, current_place},
        nav_history_menu::{LongPressWatcher, NavHistoryDirection, NavHistoryMenuWidgetRefExt, request_nav_history_menu},
    },
    room::room_action_bar::RoomActionTooltip,
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
            color: (COLOR_TRANSPARENT)
            color_hover: (COLOR_NAV_HISTORY_BUTTON_BG_HOVER)
            color_down: (COLOR_NAV_HISTORY_BUTTON_BG_DOWN)
        }
        draw_icon.color: (COLOR_FG_DISABLED)
        icon_walk: Walk{width: 16, height: 16}
    }

    mod.widgets.NavHistoryButtons = #(NavHistoryButtons::register_widget(vm)) {
        width: Fit, height: Fit
        margin: Inset{left: 8}
        spacing: 2
        align: Align{y: 0.5}

        icon_color: (COLOR_TEXT)
        icon_color_disabled: (COLOR_FG_DISABLED)

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
    #[live] icon_color: Vec4f,
    #[live] icon_color_disabled: Vec4f,
    #[rust] tooltip: RoomActionTooltip,
    #[rust] long_press: LongPressWatcher,
    #[rust] are_tooltips_hidden: bool,
    #[rust] can_go_back: bool,
    #[rust] can_go_forward: bool,
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
        match event {
            Event::Actions(actions) => {
                if self.view.button(cx, ids!(go_back_button)).clicked(actions) {
                    cx.action(GoBackAction { steps: 1 });
                }
                if self.view.button(cx, ids!(go_forward_button)).clicked(actions) {
                    cx.action(GoForwardAction { steps: 1 });
                }
            }
            // A reapply resets the buttons to their DSL defaults, so we enable them again.
            Event::LiveEdit | Event::ScriptReapply => self.enable_buttons(cx, self.can_go_back, self.can_go_forward),
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
            let button = self.view.button(cx, button_id);
            if !enable {
                // A button that we disable while it's hovered shouldn't stay highlighted.
                button.reset_hover(cx);
            }
            button.set_enabled(cx, enable);
            if let Some(mut inner) = button.borrow_mut() {
                inner.draw_icon.color = if enable { self.icon_color } else { self.icon_color_disabled };
            }
        }
        self.view.redraw(cx);
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

    /// See [`NavHistoryButtons::enable_buttons()`].
    pub fn enable_buttons(&self, cx: &mut Cx, can_go_back: bool, can_go_forward: bool) {
        let Some(mut inner) = self.borrow_mut() else { return };
        if (inner.can_go_back, inner.can_go_forward) != (can_go_back, can_go_forward) {
            inner.enable_buttons(cx, can_go_back, can_go_forward);
        }
    }
}

/// Enables or disables the title bar's go-back and go-forward buttons,
/// depending on the nav history and where we can currently go right now.
pub fn enable_nav_history_buttons(cx: &mut Cx, ui: &WidgetRef, app_state: &AppState) {
    let buttons = ui.nav_history_buttons(cx, ids!(nav_history_buttons));
    // While one of the buttons shows its nav history menu, they keep their look,
    // but their tooltips would cover the menu.
    let is_menu_shown = ui.nav_history_menu(cx, ids!(nav_history_menu)).is_currently_shown();
    buttons.hide_tooltips(cx, is_menu_shown);
    if is_menu_shown {
        return;
    }
    let (can_go_back, can_go_forward) = if can_navigate_history(cx, ui, app_state) {
        let can_show = can_show(cx);
        let nav_history = &app_state.nav_history;
        let current = current_place(cx, ui, app_state);
        (
            nav_history.previous_place(current, 1, &can_show).is_some(),
            nav_history.next_place(current, 1, &can_show).is_some(),
        )
    } else {
        (false, false)
    };
    buttons.enable_buttons(cx, can_go_back, can_go_forward);
}
