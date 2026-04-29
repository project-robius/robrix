//! OIDC (MAS) login orchestration for existing Matrix accounts.
//!
//! This module owns the server-side OAuth 2.0 authorization-code flow that
//! lets robrix2 sign users into MAS-delegated homeservers (matrix.org,
//! alvin.meldry.com, any MSC2965 server). The UI-facing entry point is
//! `MatrixRequest::StartOidcLogin` in the Matrix worker; on the worker
//! side, the spawned task calls `start_oidc_login()` here and funnels the
//! outcome back into the normal `LoginRequest::LoginByOidcSuccess` pipeline
//! (shared with password + SSO paths).
//!
//! Error variants are only added when an actual construction site lands —
//! the project disallows `#[allow(dead_code)]` (see `feedback_no_allow_dead_code`).

use std::time::Duration;

use makepad_widgets::Cx;
use matrix_sdk::{
    Client,
    authentication::oauth::{
        ClientRegistrationData, OAuthError,
        error::{OAuthAuthorizationCodeError, OAuthClientRegistrationError},
        registration::{ApplicationType, ClientMetadata, Localized, OAuthGrantType},
    },
    ruma::{OwnedUserId, serde::Raw},
    utils::local_server::LocalServerBuilder,
};
use robius_open::Uri;
use tokio::sync::oneshot;
use url::Url;

use crate::{
    login::login_screen::LoginAction,
    persistence::ClientSessionPersisted,
    sliding_sync::build_client_for_oidc,
};

/// The total time we're willing to wait for the user to finish the browser
/// auth flow before giving up and releasing the loopback server.
const OIDC_FLOW_TIMEOUT: Duration = Duration::from_secs(180);

/// Every terminal state an OIDC login can reach.
#[derive(Debug)]
pub enum OidcLoginError {
    /// The flow was aborted before the homeserver issued tokens. Possible
    /// triggers: in-app Cancel, browser `error=access_denied`, loopback
    /// server closing without a redirect. All collapse to one message
    /// because the corrective action is identical — click Continue again.
    Cancelled,

    /// 3-minute total timeout elapsed without a browser callback.
    Timeout,

    /// Dynamic client registration (MSC2966) is unsupported by this server.
    /// The message tells the user the failure is server-side.
    DynamicRegistrationNotSupported,

    /// `build_client()` (sqlite setup, homeserver discovery) failed before
    /// the OAuth flow even started.
    ClientBuild(String),

    /// OAuth server metadata discovery (`.well-known/openid-configuration`)
    /// failed. Distinct from "not MAS" because capability probe already
    /// classified this homeserver as MAS — so this is network/server sick.
    ServerMetadata(String),

    /// Loopback HTTP server failed to bind (ports exhausted, firewall block).
    LoopbackServer(String),

    /// `OAuth::login().build().await` failed for a reason other than one of
    /// the specific cases above (malformed metadata, scope mismatch, etc).
    AuthorizeBuild(String),

    /// `robius_open` couldn't launch the system browser.
    BrowserOpen(String),

    /// Token exchange (`finish_login`) or session load failed — the user
    /// completed the browser step but robrix2 couldn't finalize the session.
    FinishLogin(String),

    /// Catch-all for invariant-violating conditions that should never happen
    /// (e.g., `client.user_id()` returning None after a successful login).
    Other(String),
}

/// Translate an `OidcLoginError` into a sentence safe to show in the UI.
///
/// Pattern mirrors `map_register_error()` in sliding_sync.rs: technical
/// details go to `log!` / `error!` at the construction site; this function
/// is deliberately terse and friendly.
pub fn map_oidc_error(err: &OidcLoginError) -> String {
    match err {
        OidcLoginError::Cancelled => {
            "Sign-in was cancelled.".to_string()
        }
        OidcLoginError::Timeout => {
            "Sign-in timed out. Please try again.".to_string()
        }
        OidcLoginError::DynamicRegistrationNotSupported => {
            "This server doesn't support third-party sign-in apps yet.".to_string()
        }
        OidcLoginError::ClientBuild(e) => {
            format!("Couldn't prepare the sign-in client: {e}")
        }
        OidcLoginError::ServerMetadata(e) => {
            format!("Couldn't discover sign-in server: {e}")
        }
        OidcLoginError::LoopbackServer(e) => {
            format!("Couldn't start local sign-in server: {e}")
        }
        OidcLoginError::AuthorizeBuild(e) => {
            format!("Couldn't build sign-in URL: {e}")
        }
        OidcLoginError::BrowserOpen(e) => {
            format!("Couldn't open your browser: {e}")
        }
        OidcLoginError::FinishLogin(e) => {
            format!("Sign-in failed at the last step: {e}")
        }
        OidcLoginError::Other(e) => {
            format!("Sign-in failed: {e}")
        }
    }
}

/// Classify an `OAuthError` returned by `OAuth::login().build()`.
///
/// MSC2966 "not supported" gets its own user message; everything else is
/// bucketed into ServerMetadata (discovery path) or AuthorizeBuild (authorize
/// path) so the ops class is visible in logs.
fn classify_auth_build_error(err: OAuthError) -> OidcLoginError {
    match &err {
        OAuthError::ClientRegistration(OAuthClientRegistrationError::NotSupported) => {
            OidcLoginError::DynamicRegistrationNotSupported
        }
        OAuthError::Discovery(_) => {
            OidcLoginError::ServerMetadata(err.to_string())
        }
        _ => OidcLoginError::AuthorizeBuild(err.to_string()),
    }
}

/// Classify a `matrix_sdk::Error` returned by `OAuth::finish_login()`.
///
/// Browser-side cancellation (MAS redirects with `error=access_denied`) lands
/// here as `OAuthError::AuthorizationCode(_::Cancelled)`; collapse it to our
/// shared Cancelled variant so the UI state machine stays simple.
fn classify_finish_error(err: matrix_sdk::Error) -> OidcLoginError {
    if let matrix_sdk::Error::OAuth(oauth_err) = &err {
        if let OAuthError::AuthorizationCode(ref ac) = **oauth_err {
            if matches!(ac, OAuthAuthorizationCodeError::Cancelled) {
                return OidcLoginError::Cancelled;
            }
        }
    }
    OidcLoginError::FinishLogin(err.to_string())
}

/// Build the `ClientRegistrationData` describing Robrix to the MAS server.
///
/// Dynamic registration happens per-flow (no client_id caching yet) because
/// the redirect URI carries a fresh loopback port each run. Caching is an
/// Open Question to be answered in Phase 3c once the basic flow is stable.
fn client_registration_data(redirect_uri: &Url) -> Result<ClientRegistrationData, OidcLoginError> {
    let client_uri = Url::parse("https://github.com/project-robius/robrix")
        .map_err(|e| OidcLoginError::Other(format!("client URI parse: {e}")))?;
    let mut metadata = ClientMetadata::new(
        ApplicationType::Native,
        vec![OAuthGrantType::AuthorizationCode {
            redirect_uris: vec![redirect_uri.clone()],
        }],
        Localized::new(client_uri, None),
    );
    metadata.client_name = Some(Localized::new("Robrix".to_owned(), None));
    Raw::new(&metadata)
        .map(ClientRegistrationData::new)
        .map_err(|e| OidcLoginError::Other(format!("metadata serialize: {e}")))
}

/// End-to-end MAS OIDC login.
///
/// Flow:
///   1. Build a fresh `Client` keyed to `homeserver_url` + `proxy`.
///   2. Spawn a loopback HTTP server on 127.0.0.1 to catch the redirect.
///   3. Ask MAS for an authorization URL (triggers dynamic client registration).
///   4. Open the URL in the system browser and post `OidcLoginStarted`.
///   5. Race loopback callback vs in-app Cancel vs 3-minute timeout.
///   6. Exchange the authorization code for tokens via `finish_login`.
///
/// On success the function returns `(client, client_session, user_id)` —
/// the caller pipes those into `LoginRequest::LoginByOidcSuccess` so the
/// normal post-login pipeline (save_session, sync service startup, main
/// UI navigation) runs unchanged.
pub async fn start_oidc_login(
    homeserver_url: String,
    proxy: Option<String>,
    cancel_rx: oneshot::Receiver<()>,
) -> Result<(Client, ClientSessionPersisted, OwnedUserId), OidcLoginError> {
    // 1. Build client. Trim to None when the probe handed us an empty URL so
    //    build_client falls back to its usual homeserver inference.
    let homeserver = {
        let trimmed = homeserver_url.trim();
        if trimmed.is_empty() { None } else { Some(trimmed.to_owned()) }
    };
    let (client, client_session) = build_client_for_oidc(homeserver, proxy).await
        .map_err(|e| OidcLoginError::ClientBuild(e.to_string()))?;

    // 2. Spawn loopback server. Dropping `redirect_handle` later in this
    //    function tears the server down; all early-return paths also release
    //    it because `redirect_handle` goes out of scope.
    let (redirect_uri, redirect_handle) = LocalServerBuilder::new().spawn().await
        .map_err(|e| OidcLoginError::LoopbackServer(e.to_string()))?;

    // 3. Kick off the authorization flow. Dynamic registration happens here
    //    (client not yet registered), and it's where MSC2966-unsupported
    //    servers fail fast.
    let registration_data = client_registration_data(&redirect_uri)?;
    let auth_data = client.oauth()
        .login(redirect_uri.clone(), None, Some(registration_data), None)
        .build()
        .await
        .map_err(classify_auth_build_error)?;

    // 4. Launch the browser. On failure, abort_login to release the CSRF
    //    state held inside matrix-sdk so the next retry starts fresh.
    if let Err(e) = Uri::new(auth_data.url.as_str()).open() {
        client.oauth().abort_login(&auth_data.state).await;
        return Err(OidcLoginError::BrowserOpen(format!("{e:?}")));
    }

    // 5. Tell the UI the loopback is live + browser has launched.
    Cx::post_action(LoginAction::OidcLoginStarted);

    // 6. Race: loopback callback | explicit cancel | 3-min timeout.
    //    Every non-success path calls abort_login + returns an error; the
    //    redirect_handle drop then tears the loopback down.
    let query = tokio::select! {
        q = redirect_handle => match q {
            Some(query) => query,
            None => {
                client.oauth().abort_login(&auth_data.state).await;
                return Err(OidcLoginError::Cancelled);
            }
        },
        _ = cancel_rx => {
            client.oauth().abort_login(&auth_data.state).await;
            return Err(OidcLoginError::Cancelled);
        }
        _ = tokio::time::sleep(OIDC_FLOW_TIMEOUT) => {
            client.oauth().abort_login(&auth_data.state).await;
            return Err(OidcLoginError::Timeout);
        }
    };

    // 7. Token exchange. `query.into()` converts `QueryString` → `UrlOrQuery`
    //    via the From impl that ships under the `local-server` feature.
    client.oauth().finish_login(query.into()).await
        .map_err(classify_finish_error)?;

    // 8. Resolve user_id for the caller's account-manager bookkeeping.
    let user_id = client.user_id()
        .ok_or_else(|| OidcLoginError::Other(
            "no user_id after finish_login — matrix-sdk invariant violated".to_string(),
        ))?
        .to_owned();

    Ok((client, client_session, user_id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_oidc_error_handles_cancelled_login() {
        let msg = map_oidc_error(&OidcLoginError::Cancelled);
        assert!(msg.to_lowercase().contains("cancel"), "got: {msg}");
    }

    #[test]
    fn map_oidc_error_handles_missing_dynamic_registration() {
        let msg = map_oidc_error(&OidcLoginError::DynamicRegistrationNotSupported);
        assert!(msg.contains("third-party sign-in apps"), "got: {msg}");
    }

    #[test]
    fn map_oidc_error_handles_timeout_is_retriable() {
        let msg = map_oidc_error(&OidcLoginError::Timeout);
        assert!(msg.to_lowercase().contains("try again"), "got: {msg}");
    }

    #[test]
    fn map_oidc_error_surfaces_inner_finish_login_text() {
        let msg = map_oidc_error(&OidcLoginError::FinishLogin(
            "token endpoint 502".to_string(),
        ));
        assert!(msg.contains("token endpoint 502"), "got: {msg}");
    }
}
