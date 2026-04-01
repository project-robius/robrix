//! A text input used to filter the rooms list
//! with a search icon and a button to clear the input.
//!
//! This is a dedicated widget instead of a general "SearchBar" so it can be
//! reused consistently across both Desktop and Mobile layouts.

use makepad_widgets::*;
use crate::{app::AppState, i18n::{AppLanguage, tr_key}};

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*


    mod.widgets.RoomFilterInputBar = set_type_default() do #(RoomFilterInputBar::register_widget(vm)) {
        ..mod.widgets.RoundedView

        width: Fill,
        height: 35,

        show_bg: true,
        draw_bg +: {
            color: (COLOR_PRIMARY)
            border_radius: 4.0
            border_color: (COLOR_SECONDARY)
            border_size: 1.0
        }

        padding: Inset{top: 3, bottom: 3, left: 10, right: 4.5}
        margin: 0
        spacing: 4,
        align: Align{x: 0.0, y: 0.5},

        Icon {
            draw_icon +: {
                svg: (ICON_SEARCH),
                color: (COLOR_TEXT_INPUT_IDLE),
            }
            icon_walk: Walk{width: 14, height: 14}
        }

        input := RobrixTextInput {
            width: Fill,
            height: Fit,
            flow: Right, // do not wrap
            padding: 5
            
            empty_text: "Filter rooms & spaces..."
            
            draw_bg.border_size: 0.0
            draw_text +: {
                text_style: theme.font_regular { font_size: 10 },
            }
        }

        clear_button := RobrixNeutralIconButton {
            visible: false,
            padding: Inset{top: 5, bottom: 5, left: 9, right: 9},
            spacing: 0,
            align: Align{x: 0.5, y: 0.5}
            draw_icon.svg: (ICON_CLOSE)
            icon_walk: Walk{width: Fit, height: 10, margin: 0}
        }
    }
}

/// A text input (with a search icon and cancel button) used to filter the rooms list.
///
/// See the module-level docs for more detail.
#[derive(Script, ScriptHook, Widget)]
pub struct RoomFilterInputBar {
    #[deref] view: View,
    #[rust] app_language: AppLanguage,
}

/// Actions emitted by the `RoomFilterInputBar` based on user interaction with it.
#[derive(Clone, Debug, Default)]
pub enum RoomFilterAction {
    /// The user has changed the text entered into the filter bar.
    Changed(String),
    #[default]
    None,
}

impl ActionDefaultRef for RoomFilterAction {
    fn default_ref() -> &'static Self {
        static DEFAULT: RoomFilterAction = RoomFilterAction::None;
        &DEFAULT
    }
}

impl Widget for RoomFilterInputBar {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        let app_language = scope.data.get::<AppState>()
            .map(|app_state| app_state.app_language)
            .unwrap_or_default();
        if self.app_language != app_language {
            self.set_app_language(cx, app_language);
        }
        self.view.handle_event(cx, event, scope);
        self.widget_match_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        let app_language = scope.data.get::<AppState>()
            .map(|app_state| app_state.app_language)
            .unwrap_or_default();
        if self.app_language != app_language {
            self.set_app_language(cx, app_language);
        }
        self.view.draw_walk(cx, scope, walk)
    }
}

impl WidgetMatchEvent for RoomFilterInputBar {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions, _scope: &mut Scope) {
        let input = self.text_input(cx, ids!(input));
        let clear_button = self.button(cx, ids!(clear_button));

        // Handle user changing the input text
        if let Some(keywords) = input.changed(actions) {
            // Trim whitespace, and only alloc a new string if it was trimmed.
            let keywords_trimmed = keywords.trim();
            let keywords = if keywords_trimmed.len() < keywords.len() {
                keywords_trimmed.to_string()
            } else {
                keywords
            };
            clear_button.set_visible(cx, !keywords.is_empty());
            clear_button.reset_hover(cx);
            cx.widget_action(
                self.widget_uid(), 
                RoomFilterAction::Changed(keywords)
            );
        }

        if clear_button.clicked(actions) {
            input.set_text(cx, "");
            clear_button.set_visible(cx, false);
            input.set_key_focus(cx);
            cx.widget_action(
                self.widget_uid(), 
                RoomFilterAction::Changed(String::new())
            );
        }
    }
}

impl RoomFilterInputBar {
    fn set_app_language(&mut self, cx: &mut Cx, app_language: AppLanguage) {
        self.app_language = app_language;
        self.view
            .text_input(cx, ids!(input))
            .set_empty_text(cx, tr_key(self.app_language, "room_filter_input.placeholder").to_string());
        self.view.redraw(cx);
    }
}
