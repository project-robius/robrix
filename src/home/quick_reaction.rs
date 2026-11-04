//! `QuickReaction`, the reaction that double-tapping a message toggles on it.
//!
//! Users choose it in the App Settings, where they can also turn off double-tapping to react.

use makepad_widgets::*;
use serde::{Deserialize, Serialize};
use crate::{
    home::{event_reaction_list::get_typed_reaction, new_message_context_menu::{MessageAbilities, MessageDetails}, room_screen::MessageAction},
    settings::app_preferences::AppPreferencesGlobal,
    shared::double_tap::Tap,
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
        if !prefs.double_tap_to_react || !details.abilities.contains(MessageAbilities::CanReact) {
            return;
        }
        let reaction = prefs.quick_reaction.as_str().to_owned();
        cx.widget_action(
            details.room_screen_widget_uid,
            MessageAction::React { details: details.clone(), reaction },
        );
    }
}
