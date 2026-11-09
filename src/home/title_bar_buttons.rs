//! The rooms list toggle, go-back and go-forward buttons, which stay together in the window's title/caption bar,
//! or next to the rooms filter bar if there's no caption bar.

use makepad_widgets::*;

use crate::{
    app::AppState,
    home::{
        home_screen::effective_is_desktop,
        nav_history::{can_navigate_history, can_show, current_place},
        nav_history_buttons::NavHistoryButtonsWidgetRefExt,
        nav_history_menu::{NavHistoryDirection, NavHistoryMenuWidgetRefExt},
        navigation_tab_bar::SelectedTab,
    },
};

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.TitleBarButtons = #(TitleBarButtons::register_widget(vm)) {
        width: Fit, height: Fit
        margin: Inset{left: 8}
        spacing: 2
        align: Align{y: 0.5}

        icon_color: (COLOR_TEXT)
        icon_color_disabled: (COLOR_FG_DISABLED)

        toggle_rooms_sidebar_button := ToggleRoomsSideBarButton {}
        nav_history_buttons := NavHistoryButtons {}
    }
}

/// The title bar's buttons, each of which is only enabled when it can do something.
#[derive(Script, ScriptHook, Widget)]
pub struct TitleBarButtons {
    #[source] source: ScriptObjectRef,
    #[deref] view: View,
    #[live] icon_color: Vec4f,
    #[live] icon_color_disabled: Vec4f,
    #[live] is_in_caption_bar: bool,
    /// Whether the rooms list toggle, go-back and go-forward buttons are enabled, in that order.
    #[rust] are_enabled: [bool; 3],
}

impl Widget for TitleBarButtons {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        // A reapply resets the buttons to their DSL defaults, so we enable them again.
        if let Event::LiveEdit | Event::ScriptReapply = event {
            self.enable_buttons(cx, self.are_enabled);
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, mut walk: Walk) -> DrawStep {
        // Avoid drawing over the window chrome buttons, either on the left or right of the caption bar.
        if self.is_in_caption_bar && let Some(window_id) = cx.get_current_window_id() {
            let geom = &cx.windows[window_id].window_geom;
            let chrome = geom.window_chrome_buttons;
            if chrome.size.x > 0.0 && chrome.pos.x + chrome.size.x / 2.0 < geom.inner_size.x / 2.0 {
                walk.margin.left += chrome.pos.x + chrome.size.x;
            }
        }
        self.view.draw_walk(cx, scope, walk)
    }
}

impl TitleBarButtons {
    /// Enables or disables (grays out) each of our buttons.
    fn enable_buttons(&mut self, cx: &mut Cx, are_enabled: [bool; 3]) {
        self.are_enabled = are_enabled;
        let button_ids: [&[LiveId]; 3] = [ids!(toggle_rooms_sidebar_button.button), ids!(go_back_button), ids!(go_forward_button)];
        for (button_id, enable) in button_ids.into_iter().zip(are_enabled) {
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

/// Enables or disables the title bar's buttons, depending on the nav history and what's shown right now.
///
/// Without a caption bar (like on Android), this shows and updates the desktop home screen's buttons instead.
pub fn enable_title_bar_buttons(cx: &mut Cx, ui: &WidgetRef, app_state: &AppState) {
    let buttons = if ui.view(cx, ids!(caption_bar)).visible() {
        ui.title_bar_buttons(cx, ids!(caption_bar.title_bar_buttons))
    } else {
        let buttons = ui.title_bar_buttons(cx, ids!(home_screen.title_bar_buttons));
        buttons.set_visible(cx, true);
        buttons
    };
    // The home screen has no buttons of its own in mobile view mode.
    if buttons.is_empty() { return; }
    // There's no rooms sidebar to hide in mobile view mode.
    let is_desktop = effective_is_desktop(cx);
    buttons.widget(cx, ids!(toggle_rooms_sidebar_button)).set_visible(cx, is_desktop);
    // While one of the buttons shows its nav history menu, they keep their look,
    // but their tooltips would cover the menu.
    let is_menu_shown = ui.nav_history_menu(cx, ids!(nav_history_menu)).is_currently_shown();
    buttons.nav_history_buttons(cx, ids!(nav_history_buttons)).hide_tooltips(cx, is_menu_shown);
    if is_menu_shown {
        return;
    }
    let are_enabled = if can_navigate_history(cx, ui, app_state) {
        let can_show = can_show(cx);
        let current = current_place(cx, ui, app_state);
        [
            // The rooms sidebar isn't shown behind the AddRoom or Settings screens.
            matches!(app_state.selected_tab, SelectedTab::Home | SelectedTab::Space { .. }),
            NavHistoryDirection::Back.places(app_state, current, &can_show).next().is_some(),
            NavHistoryDirection::Forward.places(app_state, current, &can_show).next().is_some(),
        ]
    } else {
        [false; 3]
    };
    if let Some(mut inner) = buttons.borrow_mut() && inner.are_enabled != are_enabled {
        inner.enable_buttons(cx, are_enabled);
    }
}
