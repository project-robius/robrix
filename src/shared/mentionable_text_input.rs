//! A temporary mock/placeholder for MentionableTextInput that uses a simple TextInput
//! instead of the full @mention popup system (CommandTextInput).
//!
//! This preserves the same external-facing API so that the real MentionableTextInput
//! can be slotted back in later without changing the code that depends on it.

use makepad_widgets::*;
use matrix_sdk::ruma::{
    events::room::message::RoomMessageEventContent,
    OwnedRoomId,
};

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.MentionableTextInput = #(MentionableTextInput::register_widget(vm)) {
        width: Fill,
        height: Fit

        // Keep the same nested structure so that external DSL overrides
        // (e.g., `persistent.center.text_input.empty_text`) still work.
        persistent := RoundedView {
            width: Fill,
            height: Fit,
            flow: Down,
            top := View { height: 0 }
            center := RoundedView {
                width: Fill,
                height: Fit,
                text_input := RobrixTextInput {
                    empty_text: "Start typing..."
                }
            }
            bottom := View { height: 0 }
        }
    }
}

#[derive(Debug)]
pub enum MentionableTextInputAction {
    /// Notifies the MentionableTextInput about updated power levels for the room.
    PowerLevelsUpdated {
        room_id: OwnedRoomId,
        can_notify_room: bool,
    }
}

/// Temporary mock widget that wraps a simple TextInput (RobrixTextInput)
/// while preserving the same external API as the real MentionableTextInput.
#[derive(Script, ScriptHook, Widget)]
pub struct MentionableTextInput {
    #[source] source: ScriptObjectRef,
    #[deref] view: View,
    /// Whether the current user can notify everyone in the room (@room mention).
    /// Stored but not used in this mock; kept for API compatibility.
    #[rust] can_notify_room: bool,
}

impl Widget for MentionableTextInput {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);

        // Handle MentionableTextInputAction for API compatibility.
        if let Event::Actions(actions) = event {
            for action in actions {
                if let Some(MentionableTextInputAction::PowerLevelsUpdated {
                    can_notify_room, ..
                }) = action.downcast_ref()
                {
                    self.can_notify_room = *can_notify_room;
                }
            }
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }

    fn text(&self) -> String {
        self.child_by_path(ids!(text_input)).as_text_input().text()
    }

    fn set_text(&mut self, cx: &mut Cx, text: &str) {
        self.text_input(cx, ids!(persistent.center.text_input)).set_text(cx, text);
        self.redraw(cx);
    }

    fn set_key_focus(&self, cx: &mut Cx) {
        self.text_input(cx, ids!(persistent.center.text_input)).set_key_focus(cx);
    }
}

impl MentionableTextInput {

    /// Sets whether the current user can notify the entire room (@room mention).
    pub fn set_can_notify_room(&mut self, can_notify: bool) {
        self.can_notify_room = can_notify;
    }

    /// Gets whether the current user can notify the entire room (@room mention).
    pub fn can_notify_room(&self) -> bool {
        self.can_notify_room
    }
}

impl MentionableTextInputRef {
    /// Returns a reference to the inner `TextInput` widget.
    pub fn text_input_ref(&self) -> TextInputRef {
        self.child_by_path(ids!(persistent.center.text_input)).as_text_input()
    }

    /// Sets whether the current user can notify the entire room (@room mention).
    pub fn set_can_notify_room(&self, can_notify: bool) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_can_notify_room(can_notify);
        }
    }

    /// Gets whether the current user can notify the entire room (@room mention).
    pub fn can_notify_room(&self) -> bool {
        self.borrow().is_some_and(|inner| inner.can_notify_room())
    }

    /// Creates a message from the entered text.
    ///
    /// This mock version handles `/html` and `/plain` prefixes
    /// but does not track or extract @mentions (since the mention popup is disabled).
    pub fn create_message_with_mentions(&self, entered_text: &str) -> RoomMessageEventContent {
        if let Some(html_text) = entered_text.strip_prefix("/html") {
            RoomMessageEventContent::text_html(html_text, html_text)
        } else if let Some(plain_text) = entered_text.strip_prefix("/plain") {
            RoomMessageEventContent::text_plain(plain_text)
        } else {
            RoomMessageEventContent::text_markdown(entered_text)
        }
    }
}
