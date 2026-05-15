//! Shared homeserver capability discovery state used by login and register.

use makepad_widgets::ActionDefaultRef;
use matrix_sdk::ruma::api::client::uiaa::UiaaInfo;

/// Homeserver capabilities discovered before branching into auth/register flows.
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
    pub sso_providers: Vec<IdentityProviderSummary>,
    /// URL to open in the system browser for MAS self-registration.
    pub mas_signup_url: Option<String>,
    /// OAuth issuer discovered from `.well-known`, used by MAS login.
    pub mas_issuer_url: Option<String>,
}

/// Minimal info per identity provider. Full matrix-sdk type is not
/// used because we only need name + id at this phase.
#[derive(Clone, Debug)]
pub struct IdentityProviderSummary {
    pub id: String,
    pub name: String,
    pub icon_url: Option<String>,
}

/// Shared capability probe result action.
#[derive(Clone, Debug, Default)]
pub enum CapabilityProbeAction {
    /// `requested_url` is echoed so screens can drop out-of-order responses.
    Discovered { requested_url: String, caps: Box<HsCapabilities> },
    /// `requested_url` is echoed so screens can drop out-of-order responses.
    Failed { requested_url: String, error: String },
    #[default]
    None,
}

impl ActionDefaultRef for CapabilityProbeAction {
    fn default_ref() -> &'static Self {
        static DEFAULT: CapabilityProbeAction = CapabilityProbeAction::None;
        &DEFAULT
    }
}

/// Outcome classification of capability discovery for login.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoginMode {
    MasOidc,
    Password,
}

pub fn login_mode(caps: &HsCapabilities) -> LoginMode {
    if caps.is_mas_native_oidc {
        LoginMode::MasOidc
    } else {
        LoginMode::Password
    }
}
