//! UIAA stage orchestration for Phase 3a (dummy-only).
//!
//! `UiaaStage` is the extensibility seam: future `TermsStage`,
//! `RegistrationTokenStage`, etc. implement this trait and get slotted into
//! the controller without touching the sliding_sync register handler.

use matrix_sdk::ruma::api::client::uiaa::{AuthData, Dummy};

/// Trait every UIAA stage implements. A stage decides whether it can
/// auto-advance (like `m.login.dummy`) or needs user input first.
pub trait UiaaStage: Send + Sync {
    /// The Matrix stage type string (e.g. `"m.login.dummy"`, `"m.login.terms"`).
    fn stage_type(&self) -> &'static str;

    /// If this stage requires no user input, return `Some(AuthData)` that
    /// can be submitted immediately. Otherwise return `None` — the caller
    /// must render UI and wait for user action.
    fn auto_submit(&self, session: &str) -> Option<AuthData>;
}

/// `m.login.dummy` — server wants to keep the UIAA protocol shape but has no
/// real challenge. We just echo back the session.
pub struct DummyStage;

impl UiaaStage for DummyStage {
    fn stage_type(&self) -> &'static str {
        "m.login.dummy"
    }

    fn auto_submit(&self, session: &str) -> Option<AuthData> {
        let mut dummy = Dummy::new();
        dummy.session = Some(session.to_owned());
        Some(AuthData::Dummy(dummy))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dummy_stage_auto_submits_with_session() {
        let stage = DummyStage;
        let auth = stage.auto_submit("test-session-123");
        match auth {
            Some(AuthData::Dummy(d)) => {
                assert_eq!(d.session.as_deref(), Some("test-session-123"))
            }
            other => panic!("expected Some(AuthData::Dummy), got {other:?}"),
        }
    }

    #[test]
    fn dummy_stage_type_string_matches_matrix_spec() {
        assert_eq!(DummyStage.stage_type(), "m.login.dummy");
    }
}
