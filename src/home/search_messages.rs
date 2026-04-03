
//! UI widgets for searching messages in one or more rooms.

use makepad_widgets::*;
use crate::{app::AppState, i18n::{AppLanguage, tr_key}};

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*


    mod.widgets.SearchMessagesButton = set_type_default() do #(SearchMessagesButton::register_widget(vm)) {
        ..mod.widgets.RoundedView

        width: Fit,
        height: 35,
        margin: 0
        enabled: false

        draw_bg +: {
            color: (COLOR_BG_DISABLED)
            // color: (COLOR_ROBRIX_PURPLE) // or `color: (COLOR_ACTIVE_PRIMARY)`
            // color_hover: (COLOR_PRIMARY_DARKER) // make it whiter (this value is mixed in with `color`)
            border_radius: 4.0
            border_color: (COLOR_SECONDARY)
            border_size: 1.0
        }
        draw_icon +: {
            svg: (ICON_SEARCH)
            color: (COLOR_FG_DISABLED)
            // color: (COLOR_PRIMARY),
            // color_hover: (COLOR_PRIMARY),
        }
        icon_walk: Walk{width: 16, height: 16, margin: Inset{left: -1, right: -2} }

        // text: "Search Messages"
        text: "Search (TODO)"
        draw_text +: {
            color: (COLOR_FG_DISABLED)
            // color: (COLOR_PRIMARY),
            // color_hover: (COLOR_PRIMARY),
        }
    }

    
}

#[derive(Script, ScriptHook, Widget)]
pub struct SearchMessagesButton {
    #[deref] button: Button,
    #[rust] app_language: AppLanguage,
}

impl Widget for SearchMessagesButton {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        let app_language = scope.data.get::<AppState>()
            .map(|app_state| app_state.app_language)
            .unwrap_or_default();
        if self.app_language != app_language {
            self.set_app_language(cx, app_language);
        }
        self.button.handle_event(cx, event, scope);

        if let Event::Actions(actions) = event {
            if self.button.clicked(actions) {

                // cx.action(AddRoomAction::SearchMessagesButtonClicked);
            }
        }
    }
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        let app_language = scope.data.get::<AppState>()
            .map(|app_state| app_state.app_language)
            .unwrap_or_default();
        if self.app_language != app_language {
            self.set_app_language(cx, app_language);
        }
        self.button.draw_walk(cx, scope, walk)
    }
}

impl SearchMessagesButton {
    fn set_app_language(&mut self, cx: &mut Cx, app_language: AppLanguage) {
        self.app_language = app_language;
        self.button.set_text(cx, tr_key(self.app_language, "search_messages.button.todo"));
    }
}

#[derive(Debug)]
pub enum AddRoomAction {
    SearchMessagesButtonClicked,
}
