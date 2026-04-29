//! Account registration feature.
//!
//! Covers the dual-mode register flow (OIDC for MAS-delegated servers,
//! UIAA wizard for legacy servers). See `specs/task-register-flow.spec.md`.

use makepad_widgets::ScriptVm;

pub mod register_screen;
pub mod register_status_modal;
pub mod validation;

pub fn script_mod(vm: &mut ScriptVm) {
    register_status_modal::script_mod(vm);
    register_screen::script_mod(vm);
}

use matrix_sdk::ruma::api::client::uiaa::UiaaInfo;
use makepad_widgets::ActionDefaultRef;

/// Homeserver capabilities discovered before register branching.
#[derive(Clone, Debug)]
pub struct HsCapabilities {
    /// Normalized base URL the client will use.
    pub base_url: String,
    /// True iff the server advertises `m.authentication.issuer` in
    /// `.well-known/matrix/client` (MSC2965 / MAS delegation).
    pub is_mas_native_oidc: bool,
    /// True iff `POST /_matrix/client/v3/register` with empty body returns
    /// 401 with parseable UIAA flows (NOT 403 M_FORBIDDEN).
    pub registration_enabled: bool,
    /// Optional UIAA probe result (empty when server requires MAS).
    pub uiaa_probe: Option<UiaaInfo>,
    /// Identity providers harvested from `/_matrix/client/v3/login`.
    /// Phase 1 populates but does not render; Phase 4 renders buttons.
    pub sso_providers: Vec<IdentityProviderSummary>,

    /// URL to open in the system browser for MAS self-registration.
    /// Derived as `<issuer>/register` when a MAS issuer is discovered in
    /// `.well-known` `m.authentication` (stable) or
    /// `org.matrix.msc2965.authentication` (unstable). None for non-MAS
    /// servers. Intentionally does NOT use MSC2965's `account` field —
    /// that URL is for logged-in account management and loops when
    /// opened unauthenticated.
    pub mas_signup_url: Option<String>,
}

/// Minimal info per identity provider. Full matrix-sdk type is not
/// used because we only need name + id at this phase.
#[derive(Clone, Debug)]
pub struct IdentityProviderSummary {
    pub id: String,
    pub name: String,
    pub icon_url: Option<String>,
}

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
    /// Sliding-sync reports the result of capability discovery.
    CapabilitiesDiscovered(HsCapabilities),
    /// Capability discovery failed (network error, bad URL, 5xx).
    DiscoveryFailed(String),
    #[default]
    None,
}

impl ActionDefaultRef for RegisterAction {
    fn default_ref() -> &'static Self {
        static DEFAULT: RegisterAction = RegisterAction::None;
        &DEFAULT
    }
}
