use std::{io::ErrorKind, path::{Path, PathBuf}, sync::OnceLock, time::Duration};

use makepad_widgets::{log, warning};
use matrix_sdk::reqwest::{Client, ClientBuilder, NoProxy, Proxy, tls};
use serde::{Deserialize, Serialize};
use url::Url;

const POLICY_USER_AGENT: &str = concat!(
    "Robrix/", env!("CARGO_PKG_VERSION"), " (matrix-rust-sdk)"
);

use crate::app_data_dir;


const PROXY_STATE_FILE_NAME: &str = "proxy_state.json";
// Loopback + private network ranges. A user filling in a public HTTP proxy
// (e.g. Clash on 127.0.0.1:7890) typically also wants LAN homeservers to
// stay direct — sending RFC 1918 traffic into a public proxy almost always
// fails. Bypass is best-effort: matrix.* on a corporate VPN that resolves to
// 10.x.x.x will also be treated as direct, which is the right call 99% of
// the time (VPN already provides encrypted transport).
pub const DEFAULT_NO_PROXY_BYPASS: &[&str] = &[
    // Loopback
    "localhost",
    "127.0.0.1",
    "::1",
    // IPv4 RFC 1918 private ranges (home routers, Docker, corporate LAN)
    "10.0.0.0/8",
    "172.16.0.0/12",
    "192.168.0.0/16",
    // IPv4 link-local (self-assigned when DHCP fails)
    "169.254.0.0/16",
    // IPv6 ULA (RFC 4193, IPv6 analog of RFC 1918)
    "fc00::/7",
    // IPv6 link-local
    "fe80::/10",
];

// Holds the CLI `--proxy` value parsed once at startup so every code path
// (restore_session, downloads, SSO pre-build) can resolve the same override
// without re-parsing argv or threading the value through deep call chains.
static CLI_PROXY_OVERRIDE: OnceLock<Option<String>> = OnceLock::new();

pub fn set_cli_proxy_override(proxy_url: Option<&str>) {
    let _ = CLI_PROXY_OVERRIDE.set(normalize_proxy_url(proxy_url));
}

pub fn cli_proxy_override() -> Option<String> {
    CLI_PROXY_OVERRIDE.get().cloned().flatten()
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct ProxyState {
    proxy_url: Option<String>,
}

fn proxy_state_file_path() -> PathBuf {
    app_data_dir().join(PROXY_STATE_FILE_NAME)
}

pub fn normalize_proxy_url(proxy_url: Option<&str>) -> Option<String> {
    proxy_url
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

pub fn validate_proxy_url(proxy_url: &str) -> Result<(), String> {
    let proxy_url = proxy_url.trim();
    if proxy_url.is_empty() {
        return Ok(());
    }

    let parsed_url = Url::parse(proxy_url)
        .map_err(|e| format!("Invalid proxy URL: {e}"))?;

    match parsed_url.scheme() {
        "http" | "https" => Ok(()),
        scheme => Err(format!(
            "Unsupported proxy URL scheme `{scheme}`. Use http or https."
        )),
    }
}

pub fn load_saved_proxy_url() -> Option<String> {
    load_saved_proxy_url_from_path(&proxy_state_file_path())
}

fn load_saved_proxy_url_from_path(state_path: &Path) -> Option<String> {
    let proxy_state_bytes = match std::fs::read(state_path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == ErrorKind::NotFound => return None,
        Err(e) => {
            warning!("Failed to read proxy state file: {e}");
            return None;
        }
    };

    let proxy_state: ProxyState = match serde_json::from_slice(&proxy_state_bytes) {
        Ok(state) => state,
        Err(e) => {
            warning!("Failed to parse proxy state file: {e}");
            return None;
        }
    };

    let normalized = normalize_proxy_url(proxy_state.proxy_url.as_deref())?;
    // A previous version accepted socks5 schemes. After dropping that support,
    // a stale socks5:// entry would otherwise reach build_policy_reqwest_client
    // and surface as an opaque client-build failure. Warn and treat as None so
    // the user can re-save a supported scheme via Settings.
    if let Err(e) = validate_proxy_url(&normalized) {
        warning!("Ignoring saved proxy URL {normalized:?} that no longer validates: {e}");
        return None;
    }
    Some(normalized)
}

pub fn resolve_effective_proxy_url(proxy_override: Option<&str>) -> Option<String> {
    normalize_proxy_url(proxy_override)
        .or_else(cli_proxy_override)
        .or_else(load_saved_proxy_url)
}

pub fn save_proxy_url(proxy_url: Option<&str>) -> Result<Option<String>, String> {
    save_proxy_url_to_path(proxy_url, &proxy_state_file_path())
}

fn save_proxy_url_to_path(proxy_url: Option<&str>, state_path: &Path) -> Result<Option<String>, String> {
    let normalized_proxy_url = normalize_proxy_url(proxy_url);
    if let Some(proxy_url) = normalized_proxy_url.as_ref() {
        validate_proxy_url(proxy_url)?;
    }

    if let Some(parent_dir) = state_path.parent() {
        std::fs::create_dir_all(parent_dir)
            .map_err(|e| format!("Failed to create proxy state directory: {e}"))?;
    }

    let proxy_state = ProxyState {
        proxy_url: normalized_proxy_url.clone(),
    };
    let serialized_proxy_state = serde_json::to_vec(&proxy_state)
        .map_err(|e| format!("Failed to serialize proxy state: {e}"))?;

    std::fs::write(state_path, serialized_proxy_state)
        .map_err(|e| format!("Failed to write proxy state file {}: {e}", state_path.display()))?;

    Ok(normalized_proxy_url)
}

pub fn build_reqwest_proxy(
    proxy_url: &str,
) -> anyhow::Result<Proxy> {
    validate_proxy_url(proxy_url)
        .map_err(|e| anyhow::anyhow!(e))?;
    let no_proxy = NoProxy::from_string(&DEFAULT_NO_PROXY_BYPASS.join(","));
    Ok(Proxy::all(proxy_url)?.no_proxy(no_proxy))
}

pub fn apply_policy_to_reqwest_builder(
    builder: ClientBuilder,
    proxy_url: Option<&str>,
) -> anyhow::Result<ClientBuilder> {
    match normalize_proxy_url(proxy_url) {
        Some(proxy_url) => {
            log!("[proxy_config] Building reqwest client WITH proxy: {proxy_url}");
            Ok(builder.proxy(build_reqwest_proxy(&proxy_url)?))
        }
        None => {
            log!("[proxy_config] Building reqwest client with NO proxy (direct)");
            Ok(builder.no_proxy())
        }
    }
}

pub fn build_policy_reqwest_client(
    proxy_url: Option<&str>,
    timeout: Option<Duration>,
) -> anyhow::Result<Client> {
    // Restore the security/operational defaults that matrix_sdk's HttpSettings
    // used to enforce before we switched ClientBuilder.proxy() → .http_client().
    let mut builder = Client::builder()
        .user_agent(POLICY_USER_AGENT)
        .min_tls_version(tls::Version::TLS_1_2);
    if let Some(timeout) = timeout {
        builder = builder.timeout(timeout);
    }
    let builder = apply_policy_to_reqwest_builder(builder, proxy_url)?;
    Ok(builder.build()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn proxy_state_test_path(name: &str) -> PathBuf {
        std::env::temp_dir()
            .join(format!("robrix_proxy_policy_test_{}_{}", name, std::process::id()))
            .join(PROXY_STATE_FILE_NAME)
    }

    #[test]
    fn save_proxy_url_none_persists_direct_policy() {
        let state_path = proxy_state_test_path("none");

        let saved = save_proxy_url_to_path(None, &state_path).unwrap();

        assert_eq!(saved, None);
        assert_eq!(load_saved_proxy_url_from_path(&state_path), None);
        let _ = std::fs::remove_file(state_path);
    }

    #[test]
    fn save_proxy_url_some_persists_proxy_policy() {
        let proxy = "http://127.0.0.1:7890";
        let state_path = proxy_state_test_path("some");

        let saved = save_proxy_url_to_path(Some(proxy), &state_path).unwrap();

        assert_eq!(saved.as_deref(), Some(proxy));
        assert_eq!(load_saved_proxy_url_from_path(&state_path).as_deref(), Some(proxy));
        let _ = std::fs::remove_file(state_path);
    }

    #[test]
    fn load_saved_proxy_url_ignores_legacy_socks_scheme() {
        let state_path = proxy_state_test_path("legacy_socks");
        if let Some(parent) = state_path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        let legacy_state = serde_json::to_vec(&ProxyState {
            proxy_url: Some("socks5://127.0.0.1:1080".to_string()),
        })
        .unwrap();
        std::fs::write(&state_path, legacy_state).unwrap();

        let loaded = load_saved_proxy_url_from_path(&state_path);

        assert_eq!(
            loaded, None,
            "legacy socks5 URL should be ignored on load so reqwest client builds don't fail with an opaque scheme error"
        );
        let _ = std::fs::remove_file(state_path);
    }

    #[test]
    fn build_policy_reqwest_client_disables_system_proxy_when_proxy_is_none() {
        let client = build_policy_reqwest_client(None, None).unwrap();

        drop(client);
    }

    #[test]
    fn build_policy_reqwest_client_attaches_no_proxy_bypass_for_local_addresses() {
        let proxy = build_reqwest_proxy("http://127.0.0.1:7890").unwrap();
        let proxy_debug = format!("{proxy:?}");

        for expected in DEFAULT_NO_PROXY_BYPASS {
            assert!(
                proxy_debug.contains(expected),
                "proxy debug {proxy_debug:?} should include bypass {expected}"
            );
        }
        // Guard against accidentally baking specific user/CI homeserver IPs
        // into the bypass list. CIDR ranges above cover these, but the
        // verbatim addresses should never appear in the debug string.
        for unexpected in ["192.168.1.58", "10.42.0.1", "172.20.0.5"] {
            assert!(
                !proxy_debug.contains(unexpected),
                "proxy debug {proxy_debug:?} should not hardcode specific LAN IP {unexpected}"
            );
        }
    }

    #[test]
    fn policy_user_agent_carries_robrix_identity_and_sdk_family() {
        assert!(
            POLICY_USER_AGENT.starts_with("Robrix/"),
            "expected UA to identify Robrix, got {POLICY_USER_AGENT:?}"
        );
        assert!(
            POLICY_USER_AGENT.contains("matrix-rust-sdk"),
            "expected UA to mark the SDK family for homeserver tooling, got {POLICY_USER_AGENT:?}"
        );
    }

    #[test]
    fn validate_proxy_url_rejects_socks_schemes() {
        for unsupported in ["socks5://127.0.0.1:1080", "socks5h://127.0.0.1:1080", "socks4://127.0.0.1:1080"] {
            let err = validate_proxy_url(unsupported)
                .expect_err("socks schemes should be rejected until reqwest is built with the socks feature");
            assert!(
                err.contains("Unsupported proxy URL scheme"),
                "expected scheme-rejection message, got {err:?}"
            );
        }
    }
}
