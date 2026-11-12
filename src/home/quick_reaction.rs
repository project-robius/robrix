//! `QuickReaction`, the reaction that double-tapping a message toggles on it.
//!
//! Users choose it in the App Settings, where they can also turn off double-tapping to react.

use makepad_widgets::*;
use serde::{Deserialize, Serialize};
use crate::{
    home::{event_reaction_list::{REACTING_NOT_ALLOWED, get_typed_reaction}, new_message_context_menu::{MessageAbilities, MessageDetails}, room_screen::MessageAction},
    settings::app_preferences::AppPreferencesGlobal,
    shared::{double_tap::Tap, popup_list::{PopupKind, enqueue_popup_notification}},
};

/// The reaction that double-tapping a message toggles on it, 👍 by default.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct QuickReaction(String);

impl Default for QuickReaction {
    fn default() -> Self {
        Self("👍".into())
    }
}

impl QuickReaction {
    /// See [`get_typed_reaction()`].
    pub fn from_typed_text(text: &str) -> Option<Self> {
        get_typed_reaction(text).map(Self)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The taps on a message, which toggle the user's quick reaction on it when they make a double tap.
#[derive(Default)]
pub struct QuickReactionTaps {
    last_tap: Option<Tap>,
}

impl QuickReactionTaps {
    /// Handles the release of a press on the given message,
    /// toggling the user's quick reaction on it if that was the second tap of a double tap.
    pub fn handle_finger_up(&mut self, cx: &mut Cx, fe: &FingerUpEvent, details: &MessageDetails) {
        let last_tap = self.last_tap.take();
        if !(fe.is_over && fe.is_primary_hit() && fe.was_tap()) {
            return;
        }
        if !last_tap.is_some_and(|tap| tap.is_followed_by(fe.abs_start, fe.capture_time, fe.device.is_mouse())) {
            self.last_tap = Some(Tap { down_abs: fe.abs_start, down_time: fe.capture_time, up_time: fe.time });
            return;
        }
        let prefs = &cx.global::<AppPreferencesGlobal>().0;
        if !prefs.double_tap_to_react {
            return;
        }
        if !details.abilities.contains(MessageAbilities::CanReact) {
            // A double tap has nothing to show for itself, so we always say why it didn't work.
            let reason = if !details.abilities.contains(MessageAbilities::IsMessage) {
                "You can only react to messages."
            } else if !details.abilities.contains(MessageAbilities::CanReactInRoom) {
                REACTING_NOT_ALLOWED
            } else if !details.abilities.contains(MessageAbilities::HasEventId) {
                "You can't react to a message that hasn't been sent yet."
            } else {
                "You can't react to a deleted message."
            };
            enqueue_popup_notification(reason, PopupKind::Warning, Some(4.0));
            return;
        }
        let reaction = prefs.quick_reaction.as_str().to_owned();
        cx.widget_action(
            details.room_screen_widget_uid,
            MessageAction::React { details: details.clone(), reaction },
        );
    }
}
