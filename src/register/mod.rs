//! Account registration feature.
//!
//! Covers the dual-mode register flow (OIDC for MAS-delegated servers,
//! UIAA wizard for legacy servers). See `specs/task-register-flow.spec.md`.

use makepad_widgets::ScriptVm;

pub mod register_screen;
pub mod register_status_modal;
pub mod uiaa;
pub mod validation;

pub fn script_mod(vm: &mut ScriptVm) {
    register_status_modal::script_mod(vm);
    register_screen::script_mod(vm);
}

use crate::homeserver::HsCapabilities;
use makepad_widgets::ActionDefaultRef;

/// Outcome classification of capability discovery.
///
/// Derived from `HsCapabilities` by `mode()` below; used for UI display.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RegisterMode {
    /// Server advertises MAS OAuth; register goes through browser.
    MasWebOnly,
    /// Server supports direct UIAA register.
    Uiaa,
    /// Server explicitly disallows registration.
    Disabled,
}

impl HsCapabilities {
    /// Produce a human-routable mode. Follows element-web `Registration.tsx`:
    /// MAS presence wins over UIAA even when both are possible.
    pub fn mode(&self) -> RegisterMode {
        if self.is_mas_native_oidc {
            RegisterMode::MasWebOnly
        } else if self.registration_enabled {
            RegisterMode::Uiaa
        } else {
            RegisterMode::Disabled
        }
    }
}

/// Actions produced by or consumed by the register feature.
///
/// `Cx::post_action(RegisterAction::*)` from any widget; handled by `App` and
/// by `RegisterScreen`.
#[derive(Clone, Debug, Default)]
pub enum RegisterAction {
    /// User clicked the back button on RegisterScreen.
    NavigateToLogin,
    /// User submitted the RegistrationForm; first POST is in flight.
    RegistrationSubmitted,
    /// Registration completed and the client has been persisted via
    /// `finalize_authenticated_client`. App.rs only needs to switch screens.
    RegistrationSuccess,
    /// Registration failed at any stage (network, validation, UIAA error).
    /// The payload is a user-displayable message.
    RegistrationFailed(String),
    #[default]
    None,
}

impl ActionDefaultRef for RegisterAction {
    fn default_ref() -> &'static Self {
        static DEFAULT: RegisterAction = RegisterAction::None;
        &DEFAULT
    }
}

#[cfg(test)]
mod homeserver_login_mode_tests {
    use crate::homeserver::{HsCapabilities, IdentityProviderSummary, LoginMode, login_mode};

    #[test]
    fn login_mode_prefers_mas_oidc_when_mas_is_advertised() {
        let caps = HsCapabilities {
            base_url: "https://matrix.org".into(),
            is_mas_native_oidc: true,
            registration_enabled: false,
            uiaa_probe: None,
            sso_providers: Vec::<IdentityProviderSummary>::new(),
            mas_signup_url: Some("https://issuer/register".into()),
            mas_issuer_url: Some("https://issuer".into()),
        };

        assert_eq!(login_mode(&caps), LoginMode::MasOidc);
    }

    #[test]
    fn login_mode_falls_back_to_password_for_non_mas_servers() {
        let caps = HsCapabilities {
            base_url: "https://palpo.example".into(),
            is_mas_native_oidc: false,
            registration_enabled: true,
            uiaa_probe: None,
            sso_providers: Vec::<IdentityProviderSummary>::new(),
            mas_signup_url: None,
            mas_issuer_url: None,
        };

        assert_eq!(login_mode(&caps), LoginMode::Password);
    }
}
