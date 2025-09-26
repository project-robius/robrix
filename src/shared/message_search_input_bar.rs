//! A text input used to search messages in a room
//! with a search icon and a button to clear the input.
//!
//! This is a dedicated widget instead of a general "SearchBar"
//! in order for us to be able to place it inside of a `CachedWidget`
//! and have a single instance be shared across the Mobile and Desktop app views.

use makepad_widgets::*;

/// Delay in seconds before sending a search request.
/// This is to avoid sending too many requests when the user is typing.
const SEARCH_INPUT_DELAY_SECS: f64 = 1.0;

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

/// Actions emitted by the `MessageSearchInputBar`.
#[derive(Clone, Debug, DefaultNone)]
pub enum MessageSearchAction {
    /// Emitted when the user has changed the text (after debounce delay or clear button).
    Changed(String),
    /// Emitted when the user has clicked the input bar.
    Clicked,
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
        let area = self.text_input(id!(input)).area();
        if let Hit::FingerDown(..) = event.hits(cx, area) {
            let widget_uid = self.widget_uid();
            cx.widget_action(
                widget_uid,
                &scope.path,
                MessageSearchAction::Clicked
            );
        }
        self.view.handle_event(cx, event, scope);
        self.widget_match_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.text_input(id!(input)).set_text(cx, &self.search_term);
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
            self.debounce_timer = cx.start_timeout(SEARCH_INPUT_DELAY_SECS);
            self.search_term = keywords;
        }

        if clear_button.clicked(actions) {
            clear_button.set_visible(cx, false);
            input.set_key_focus(cx);
            cx.widget_action(
                self.widget_uid(),
                &scope.path,
                MessageSearchAction::Changed(String::new())
            );
            self.search_term = String::new();
        }
    }
}

impl MessageSearchInputBarRef {

    /// Sets the text of the `MessageSearchInputBar`.
    ///
    /// `text` - The new text to set for the `MessageSearchInputBar`.
    ///
    /// This function sets the text of the `MessageSearchInputBar` to `text`.
    /// If the `MessageSearchInputBar` is not being viewed, it will not be updated.
    pub fn set_text(&self, text: &str) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.search_term = text.to_string();
        }
    }

    /// Returns the current search term entered by the user.
    ///
    /// Returns an empty string if the `MessageSearchInputBar` is not currently
    /// being viewed.
    pub fn get_text(&self) -> String {
        if let Some(inner) = self.borrow() {
            inner.search_term.clone()
        } else {
            String::new()
        }
    }
}
