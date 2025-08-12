//! A text input used to search messages in a room
//! with a search icon and a button to clear the input.
//!
//! This is a dedicated widget instead of a general "SearchBar"
//! in order for us to be able to place it inside of a `CachedWidget`
//! and have a single instance be shared across the Mobile and Desktop app views.

use makepad_widgets::*;

use crate::shared::popup_list::{PopupItem, PopupKind};

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::styles::*;
    use crate::shared::icon_button::RobrixIconButton;

    ICON_SEARCH = dep("crate://self/resources/icons/search.svg")

    pub MessageSearchInputBar = {{MessageSearchInputBar}}<RoundedView> {
        width: Fill, height: 35,
        show_bg: true,
        draw_bg: {
            color: (COLOR_PRIMARY),
            border_radius: 4.0,
            border_color: (COLOR_SECONDARY),
            border_size: 0.0,
        }
        padding: {top: 3, bottom: 3, left: 10, right: 10}
        margin: {top: 0, bottom: 3, left: 0, right: 0}
        spacing: 4,
        align: {x: 0.0, y: 0.5},

        <Icon> {
            draw_icon: {
                svg_file: (ICON_SEARCH),
                fn get_color(self) -> vec4 {
                    return (COLOR_TEXT_INPUT_IDLE);
                }
            }
            icon_walk: {width: 14, height: Fit}
        }
        input = <RobrixTextInput> {
            width: Fill,
            height: Fit,
            flow: Right, // do not wrap
            empty_text: "Search Messages..."
            draw_text: {
                text_style: { font_size: 10 },
            }
        }
        clear_button = <RobrixIconButton> {
            visible: false,
            padding: {top: 7, bottom: 7, left: 10, right: 10},
            spacing: 0,
            align: {x: 0.5, y: 0.5}
            draw_icon: {
                svg_file: (ICON_CLOSE),
                color: (COLOR_TEXT_INPUT_IDLE)
            }
            icon_walk: {width: Fit, height: 10, margin: 0}
        }
    }
}

/// Widget for searching messages across rooms
#[derive(Live, LiveHook, Widget)]
pub struct MessageSearchInputBar {
    #[deref] view: View,
    /// Debounce timer to prevent excessive search requests when typing
    #[rust] debounce_timer: Timer,
    /// The current search term entered by the user
    #[rust] search_term: String,
}

/// Actions emitted by the `MessageSearchInputBar` based on user interaction with it.
#[derive(Clone, Debug, DefaultNone)]
pub enum MessageSearchAction {
    /// The user has changed the text entered into the filter bar.
    Changed(String),
    /// The user has clicked the input bar.
    Clicked(String),
    /// Clear the text entered into the input bar.
    Clear,
    /// Set the text entered into the input bar.
    SetText(String),
    None,
}

impl Widget for MessageSearchInputBar {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if let Event::Timer(te) = event {
            if self.debounce_timer.is_timer(te).is_some() {
                cx.widget_action(
                    self.widget_uid(),
                    &scope.path,
                    MessageSearchAction::Changed(self.search_term.clone()),
                );
            }
        }
        self.view.handle_event(cx, event, scope);
        let area = self.text_input(id!(input)).area();
        if let Hit::FingerDown(..) = event.hits(cx, area) {
            let widget_uid = self.widget_uid();
            cx.widget_action(
                widget_uid,
                &scope.path,
                MessageSearchAction::Clicked(self.view.text_input(id!(input)).text())
            );
        }
        self.widget_match_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl WidgetMatchEvent for MessageSearchInputBar {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions, scope: &mut Scope) {
        let input = self.text_input(id!(input));
        let clear_button = self.button(id!(clear_button));

        // Handle user changing the input text
        if let Some(keywords) = input.changed(actions) {
            clear_button.set_visible(cx, !keywords.is_empty());
            self.debounce_timer = cx.start_timeout(1.0);
            self.search_term = keywords.clone();
        }

        if clear_button.clicked(actions) {
            input.set_text(cx, "");
            clear_button.set_visible(cx, false);
            input.set_key_focus(cx);
            cx.widget_action(
                self.widget_uid(),
                &scope.path,
                MessageSearchAction::Changed(String::new())
            );
        }
        for action in actions {
            if let MessageSearchAction::Clear = action.as_widget_action().cast() {
                cx.stop_timer(self.debounce_timer);
                self.text_input(id!(input)).set_text(cx, "");
                cx.widget_action(
                    self.widget_uid(),
                    &scope.path,
                    MessageSearchAction::Changed(String::new()));
            }
            if let Some(MessageSearchAction::SetText(text)) = action.downcast_ref() {
                self.text_input(id!(input)).set_text(cx, text);
            }
        }
    }
}
