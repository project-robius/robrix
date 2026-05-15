# OIDC Login For MAS Servers Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add MAS/OIDC login to `robrix2` so an existing account on `matrix.org`, `alvin.meldry.com`, or any MSC2965-capable homeserver can sign in through the system browser and return to the app via a loopback callback.

**Architecture:** Reuse the existing homeserver capability probe and the existing login worker/persistence pipeline instead of building a parallel auth stack. The implementation has four seams: a shared homeserver capability model, auth-session persistence that supports both Matrix password sessions and OAuth sessions, an OIDC worker module under `src/login/`, and a LoginScreen state machine that branches between password login and MAS browser login. Logout and token-refresh persistence must be updated in the same patch so the new session type is survivable after restart.

**Tech Stack:** Rust 2024, Makepad 2.0 `script_mod!`, `matrix-sdk` OAuth API on `project-robius/matrix-rust-sdk@space_room_suggested`, `matrix_sdk::utils::local_server`, `robius_open`, existing `SyncService` / `finalize_authenticated_client()` pipeline.

---

## Constraints

- Do **not** run `cargo fmt` or `rustfmt`.
- Do **not** add dependencies.
- Do **not** commit before user testing.
- Keep the existing password login and legacy SSO flows working.
- Keep MAS detection aligned with the current register flow: accept both stable `m.authentication` and unstable `org.matrix.msc2965.authentication`.

## Task 1: Extract Shared Homeserver Capability State

**Files:**
- Create: `src/homeserver.rs`
- Modify: `src/lib.rs`
- Modify: `src/register/mod.rs`
- Modify: `src/register/register_screen.rs`
- Modify: `src/sliding_sync.rs`
- Test: `src/homeserver.rs`

**Step 1: Write the failing tests**

Add tests in `src/homeserver.rs` for the shared classifier surface before adding production code:

```rust
#[test]
fn login_mode_prefers_mas_oidc_when_mas_is_advertised() {
    let caps = HsCapabilities {
        base_url: "https://matrix.org".into(),
        is_mas_native_oidc: true,
        registration_enabled: false,
        uiaa_probe: None,
        sso_providers: Vec::new(),
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
        sso_providers: Vec::new(),
        mas_signup_url: None,
        mas_issuer_url: None,
    };
    assert_eq!(login_mode(&caps), LoginMode::Password);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test login_mode_ --lib`
Expected: FAIL because `src/homeserver.rs`, `LoginMode`, and `login_mode()` do not exist yet.

**Step 3: Write minimal implementation**

Create `src/homeserver.rs` and move the shared capability structs there:

```rust
#[derive(Clone, Debug)]
pub struct HsCapabilities {
    pub base_url: String,
    pub is_mas_native_oidc: bool,
    pub registration_enabled: bool,
    pub uiaa_probe: Option<UiaaInfo>,
    pub sso_providers: Vec<IdentityProviderSummary>,
    pub mas_signup_url: Option<String>,
    pub mas_issuer_url: Option<String>,
}

#[derive(Clone, Debug)]
pub enum CapabilityProbeAction {
    Discovered { requested_url: String, caps: Box<HsCapabilities> },
    Failed { requested_url: String, error: String },
}

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
```

Update `src/register/mod.rs` to import `HsCapabilities` from `crate::homeserver`, keep `RegisterMode`, and remove `CapabilitiesDiscovered` / `DiscoveryFailed` from `RegisterAction`.

Update `src/sliding_sync.rs` to:

```rust
Cx::post_action(crate::homeserver::CapabilityProbeAction::Discovered {
    requested_url,
    caps: Box::new(caps),
});
```

Also capture `mas_issuer_url` next to the existing `mas_signup_url` in `discover_homeserver_capabilities()`:

```rust
let (mas, mas_signup_url, mas_issuer_url) =
    ["m.authentication", "org.matrix.msc2965.authentication"]
        .iter()
        .find_map(|key| {
            let issuer = body.get(*key)?.get("issuer").and_then(|v| v.as_str())?;
            let issuer = issuer.trim_end_matches('/').to_string();
            Some((
                true,
                Some(format!("{issuer}/register")),
                Some(issuer),
            ))
        })
        .unwrap_or((false, None, None));
```

Update `src/register/register_screen.rs` to react to `CapabilityProbeAction` instead of `RegisterAction::CapabilitiesDiscovered` / `DiscoveryFailed`.

**Step 4: Run test to verify it passes**

Run: `cargo test login_mode_ --lib`
Expected: PASS

**Step 5: Run narrow compile verification**

Run: `cargo build`
Expected: PASS

## Task 2: Generalize Persisted Sessions For Matrix And OAuth

**Files:**
- Modify: `src/persistence/matrix_state.rs`
- Modify: `src/sliding_sync.rs`
- Modify: `src/logout/logout_state_machine.rs`
- Test: `src/persistence/matrix_state.rs`

**Step 1: Write the failing tests**

Add a round-trip serialization test in `src/persistence/matrix_state.rs`:

```rust
#[test]
fn persisted_auth_session_round_trips_oauth_variant() {
    let persisted = PersistedAuthSession::OAuth {
        client_id: "client-id".into(),
        user_session: matrix_sdk::authentication::oauth::UserSession {
            meta: SessionMeta {
                user_id: user_id!("@alice:example.org").to_owned(),
                device_id: device_id!("DEVICEID").to_owned(),
            },
            tokens: SessionTokens {
                access_token: "access".into(),
                refresh_token: Some("refresh".into()),
            },
        },
    };

    let json = serde_json::to_string(&persisted).unwrap();
    let restored: PersistedAuthSession = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.user_id().as_str(), "@alice:example.org");
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test persisted_auth_session_round_trips_oauth_variant --lib`
Expected: FAIL because `PersistedAuthSession` does not exist.

**Step 3: Write minimal implementation**

Replace `FullSessionPersisted.user_session: MatrixSession` with a new serializable enum:

```rust
#[derive(Debug, Serialize, Deserialize)]
pub enum PersistedAuthSession {
    Matrix(MatrixSession),
    OAuth {
        client_id: String,
        user_session: matrix_sdk::authentication::oauth::UserSession,
    },
}
```

Expose helpers:

```rust
impl PersistedAuthSession {
    fn user_id(&self) -> &UserId { ... }
    fn into_auth_session(self) -> matrix_sdk::authentication::AuthSession { ... }
}
```

Update `save_session()` to use `client.session()` instead of `client.matrix_auth().session()`:

```rust
let auth_session = match client.session().ok_or_else(|| anyhow!("..."))? {
    AuthSession::Matrix(session) => PersistedAuthSession::Matrix(session),
    AuthSession::OAuth(session) => PersistedAuthSession::OAuth {
        client_id: session.client_id.to_string(),
        user_session: session.user,
    },
};
```

Update `restore_session()` to match the persisted enum and call `client.restore_session(...)`.

Update `handle_session_changes()` in `src/sliding_sync.rs` so `SessionChange::TokensRefreshed` re-saves the session:

```rust
Ok(SessionChange::TokensRefreshed) => {
    if let Err(e) = persistence::save_session(&client, client_session.clone()).await {
        warning!("Failed to persist refreshed session tokens: {e}");
    }
}
```

Update `perform_server_logout()` in `src/logout/logout_state_machine.rs` to branch on `client.auth_api()`:

```rust
match client.auth_api() {
    Some(AuthApi::Matrix(_)) => client.matrix_auth().logout().await,
    Some(AuthApi::OAuth(_)) => client.oauth().logout().await,
    None => Ok(()),
}
```

**Step 4: Run test to verify it passes**

Run: `cargo test persisted_auth_session_round_trips_oauth_variant --lib`
Expected: PASS

**Step 5: Run focused regression checks**

Run: `cargo test login_failure_modal_is_suppressed_while_register_flow_is_active --lib`
Expected: PASS

Run: `cargo build`
Expected: PASS

## Task 3: Add OIDC Worker Flow And Cancellation

**Files:**
- Modify: `src/login/mod.rs`
- Create: `src/login/oidc_login.rs`
- Modify: `src/sliding_sync.rs`
- Test: `src/login/oidc_login.rs`

**Step 1: Write the failing tests**

Add pure error-mapping tests in `src/login/oidc_login.rs`:

```rust
#[test]
fn map_oidc_error_handles_cancelled_login() {
    let msg = map_oidc_error(&OidcLoginError::Cancelled);
    assert!(msg.contains("cancel"));
}

#[test]
fn map_oidc_error_handles_missing_dynamic_registration() {
    let msg = map_oidc_error(&OidcLoginError::DynamicRegistrationNotSupported);
    assert!(msg.contains("third-party sign-in apps"));
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test map_oidc_error_ --lib`
Expected: FAIL because `src/login/oidc_login.rs` and `OidcLoginError` do not exist.

**Step 3: Write minimal implementation**

Add `src/login/oidc_login.rs` with a worker-facing function:

```rust
pub async fn start_oidc_login(
    homeserver_url: String,
    proxy: Option<String>,
) -> anyhow::Result<(Client, ClientSessionPersisted, String)> { ... }
```

Implementation requirements:

- Build the client with the existing `build_client()` helper and the provided homeserver/proxy.
- Spawn `LocalServerBuilder::new().spawn().await?` and keep both the redirect URI and shutdown handle.
- Build OAuth authorization with:

```rust
let OAuthAuthorizationData { url, state } = client
    .oauth()
    .login(
        redirect_uri,
        None,
        Some(oidc_client_registration_data().into()),
        None,
    )
    .build()
    .await?;
```

- Open the returned URL with `robius_open::Uri::new(url.as_str()).open()`.
- Wait with `tokio::select!` across:
  - loopback callback
  - explicit cancel signal
  - timeout
- On cancel or timeout, shut down the local server and call `client.oauth().abort_login(&state).await`.
- On success, call `client.oauth().finish_login(query.into()).await?`.
- Return `(client, client_session, fallback_user_id)` where the fallback user ID comes from `client.user_id()` after `finish_login()`.

Add a new `MatrixRequest`:

```rust
StartOidcLogin {
    homeserver_url: String,
    proxy: Option<String>,
}
```

and a cancel request:

```rust
CancelOidcLogin
```

The worker should translate outcomes into `LoginAction`:

- `OidcLoginStarted`
- `OidcLoginCancelled`
- `OidcLoginFailed(String)`
- existing `LoginAction::Status`
- existing `LoginAction::LoginSuccess` through `finalize_authenticated_client()`

**Step 4: Run test to verify it passes**

Run: `cargo test map_oidc_error_ --lib`
Expected: PASS

**Step 5: Run compile verification**

Run: `cargo build`
Expected: PASS

## Task 4: Add LoginScreen MAS/OIDC Branching

**Files:**
- Modify: `src/login/login_screen.rs`
- Modify: `resources/i18n/en.json`
- Modify: `resources/i18n/zh-CN.json`
- Test: `src/login/login_screen.rs`

**Step 1: Write the failing tests**

Add pure helper tests near the existing login screen tests:

```rust
#[test]
fn capability_probe_is_required_when_login_mode_is_unknown() {
    assert!(should_probe_homeserver(None, false));
}

#[test]
fn capability_probe_is_not_required_while_oidc_login_is_in_flight() {
    assert!(!should_probe_homeserver(Some(LoginMode::MasOidc), true));
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test capability_probe_is_required_when_login_mode_is_unknown --lib`
Expected: FAIL because `should_probe_homeserver()` and `LoginMode` are not wired into the login screen.

**Step 3: Write minimal implementation**

Extend `LoginScreen` state with:

```rust
#[rust] login_mode: Option<LoginMode>,
#[rust] discovery_pending: bool,
#[rust] last_discovery_input_url: Option<String>,
#[rust] oidc_in_flight: bool,
```

Add a hidden MAS card to the DSL, for example:

```rust
oidc_card := View {
    visible: false
    width: 275, height: Fit,
    flow: Down,
    spacing: 8

    oidc_info_label := Label { text: "..." }
    oidc_continue_button := RobrixIconButton { text: "Continue in browser" }
}
```

Behavior:

- When homeserver input changes, clear `login_mode`, hide the OIDC card, reset CTA state.
- If mode is unknown and the user clicks the primary login button, run `MatrixRequest::DiscoverHomeserverCapabilities`.
- On `CapabilityProbeAction::Discovered`:
  - `LoginMode::MasOidc`: show the OIDC card, hide password submit affordance, keep SSO buttons hidden.
  - `LoginMode::Password`: keep existing username/password flow active.
- On `oidc_continue_button` click:
  - validate proxy settings
  - dispatch `MatrixRequest::StartOidcLogin { homeserver_url, proxy }`
  - set `oidc_in_flight = true`
- On cancel button while OIDC is active:
  - dispatch `MatrixRequest::CancelOidcLogin`
- On `LoginAction::OidcLoginCancelled` / `OidcLoginFailed`:
  - clear `oidc_in_flight`
  - keep the MAS card visible for retry

Add i18n strings for:

- `login.button.next`
- `login.button.continue_in_browser`
- `login.oidc.mas_info`
- `login.oidc.waiting`
- `login.status.checking_homeserver.title`
- `login.status.checking_homeserver.body`

**Step 4: Run test to verify it passes**

Run: `cargo test capability_probe_is_required_when_login_mode_is_unknown --lib`
Expected: PASS

**Step 5: Run runtime parse verification**

Run: `cargo run`
Expected: App starts without Makepad script errors; no `login_screen.rs` type/scope parse failures in the first screen.

## Task 5: End-To-End Verification

**Files:**
- Modify as needed from previous tasks only

**Step 1: Run unit tests**

Run: `cargo test login_mode_ persisted_auth_session_round_trips_oauth_variant map_oidc_error_ capability_probe_is_required_when_login_mode_is_unknown --lib`
Expected: PASS

**Step 2: Run full build**

Run: `cargo build`
Expected: PASS

**Step 3: Run manual smoke verification**

Run: `cargo run`

Manual checks:

1. Enter `http://127.0.0.1:8128` or another non-MAS homeserver, confirm password login still uses the current path.
2. Enter `https://matrix.org` or `https://alvin.meldry.com`, confirm the MAS card appears and `Continue in browser` opens the system browser.
3. Cancel the browser flow before callback, confirm the UI returns to retryable idle state.
4. Complete OIDC login on a MAS server, confirm the app transitions into the main UI.
5. Relaunch the app, confirm the restored session skips the login screen.
6. Logout from an OIDC session, confirm the app returns cleanly to the login screen.

**Step 4: Report for user testing**

Do not commit. Present:

- the plan file path
- the files changed
- build/test evidence
- the exact manual flows still needing user validation on target MAS servers
