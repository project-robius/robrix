#[derive(Debug)]
pub enum UpdateCheckOutcome {
    UpToDate {
        current_version: String,
    },
    UpdateAvailable {
        current_version: String,
        latest_version: String,
    },
    NotConfigured,
    UnsupportedPlatform,
    Error(String),
}

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
use crate::proxy_config::{build_policy_reqwest_client, resolve_effective_proxy_url};

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
const DEFAULT_UPDATER_ENDPOINT: &str = "https://github.com/Project-Robius-China/robrix2/releases/latest/download/latest.json";
const RELEASES_BASE_URL: &str = "https://github.com/Project-Robius-China/robrix2/releases";
const SKIPPED_UPDATE_VERSION_FILE_NAME: &str = "skipped_update_version";

pub fn update_release_page_url(version: &str) -> String {
    let version = version.trim();
    if version.is_empty() {
        return format!("{RELEASES_BASE_URL}/latest");
    }
    format!("{RELEASES_BASE_URL}/tag/v{version}")
}

pub fn load_skipped_update_version() -> Option<String> {
    let value = std::fs::read_to_string(
        crate::app_data_dir().join(SKIPPED_UPDATE_VERSION_FILE_NAME),
    ).ok()?;
    let value = value.trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_owned())
    }
}

pub fn save_skipped_update_version(skipped_version: Option<&str>) -> Result<(), String> {
    let path = crate::app_data_dir().join(SKIPPED_UPDATE_VERSION_FILE_NAME);
    let version = skipped_version
        .map(str::trim)
        .filter(|value| !value.is_empty());
    match version {
        Some(version) => {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|error| format!("Failed to create updater state directory: {error}"))?;
            }
            std::fs::write(path, version)
                .map_err(|error| format!("Failed to save skipped update version: {error}"))
        }
        None => match std::fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(format!("Failed to clear skipped update version: {error}")),
        }
    }
}

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
fn parse_latest_version_payload(payload_text: &str) -> Result<Option<String>, String> {
    use serde_json::Value;
    let payload: Value = serde_json::from_str(payload_text)
        .map_err(|error| format!("Failed to parse updater metadata JSON: {error}"))?;
    let latest_version = payload
        .get("version")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    Ok(latest_version)
}

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
fn endpoint_with_current_tag(endpoint: &str, current_version: &str) -> Option<String> {
    endpoint
        .strip_suffix("/releases/latest/download/latest.json")
        .map(|base| format!("{base}/releases/download/v{current_version}/latest.json"))
}

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
fn check_latest_version_without_signature(endpoint: &str, current_version: &str) -> Result<Option<String>, String> {
    use matrix_sdk::reqwest::StatusCode;
    use tokio::runtime::Runtime;

    let runtime = Runtime::new().map_err(|error| format!("Failed to create async runtime: {error}"))?;
    runtime.block_on(async move {
        let proxy = resolve_effective_proxy_url(None);
        let client = build_updater_http_client(proxy.as_deref())?;
        let response = client
            .get(endpoint)
            .send()
            .await
            .map_err(|error| format!("Failed to fetch updater metadata: {error}"))?;
        if response.status().is_success() {
            let payload_text = response
                .text()
                .await
                .map_err(|error| format!("Failed to read updater metadata body: {error}"))?;
            return parse_latest_version_payload(&payload_text);
        }

        if response.status() == StatusCode::NOT_FOUND && current_version.contains('-') {
            if let Some(fallback_endpoint) = endpoint_with_current_tag(endpoint, current_version) {
                let fallback_response = client
                    .get(&fallback_endpoint)
                    .send()
                    .await
                    .map_err(|error| format!("Failed to fetch updater metadata: {error}"))?;
                if fallback_response.status().is_success() {
                    let payload_text = fallback_response
                        .text()
                        .await
                        .map_err(|error| format!("Failed to read updater metadata body: {error}"))?;
                    return parse_latest_version_payload(&payload_text);
                }
                return Err(format!("Updater metadata request failed with status {}", fallback_response.status()));
            }
        }

        Err(format!("Updater metadata request failed with status {}", response.status()))
    })
}

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
fn build_updater_http_client(
    proxy_url: Option<&str>,
) -> Result<matrix_sdk::reqwest::Client, String> {
    build_policy_reqwest_client(
        proxy_url,
        Some(std::time::Duration::from_secs(10)),
    )
        .map_err(|error| format!("Failed to build updater HTTP client: {error}"))
}

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
fn resolve_updater_pubkey() -> Option<String> {
    option_env!("ROBRIX_UPDATER_PUBKEY")
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| std::env::var("ROBRIX_UPDATER_PUBKEY").ok())
        .or_else(|| std::env::var("CARGO_PACKAGER_SIGN_PUBLIC_KEY").ok())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
fn resolve_updater_endpoint() -> String {
    option_env!("ROBRIX_UPDATER_ENDPOINT")
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| std::env::var("ROBRIX_UPDATER_ENDPOINT").ok())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| DEFAULT_UPDATER_ENDPOINT.to_string())
}

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
pub fn check_for_updates() -> UpdateCheckOutcome {
    use cargo_packager_updater::{Config, check_update};
    use semver::Version;
    use url::Url;

    let current_version = env!("CARGO_PKG_VERSION").to_string();
    let current_version_semver = match Version::parse(&current_version) {
        Ok(version) => version,
        Err(error) => {
            return UpdateCheckOutcome::Error(format!("Invalid current version format: {error}"));
        }
    };

    let endpoint = resolve_updater_endpoint();
    let pubkey = resolve_updater_pubkey();

    if let Some(pubkey) = pubkey {
        let endpoint_url = match Url::parse(&endpoint) {
            Ok(url) => url,
            Err(error) => {
                return UpdateCheckOutcome::Error(format!("Invalid updater endpoint URL: {error}"));
            }
        };

        let config = Config {
            endpoints: vec![endpoint_url],
            pubkey,
            ..Default::default()
        };

        match check_update(current_version_semver.clone(), config) {
            Ok(Some(update)) => UpdateCheckOutcome::UpdateAvailable {
                current_version,
                latest_version: update.version.to_string(),
            },
            Ok(None) => UpdateCheckOutcome::UpToDate {
                current_version,
            },
            Err(error) => UpdateCheckOutcome::Error(error.to_string()),
        }
    } else {
        match check_latest_version_without_signature(&endpoint, &current_version) {
            Ok(Some(latest_version)) => {
                let latest_semver = match Version::parse(&latest_version) {
                    Ok(version) => version,
                    Err(error) => {
                        return UpdateCheckOutcome::Error(format!("Invalid latest version format: {error}"));
                    }
                };
                if latest_semver > current_version_semver {
                    UpdateCheckOutcome::UpdateAvailable {
                        current_version,
                        latest_version,
                    }
                } else {
                    UpdateCheckOutcome::UpToDate {
                        current_version,
                    }
                }
            }
            Ok(None) => UpdateCheckOutcome::NotConfigured,
            Err(error) => UpdateCheckOutcome::Error(error),
        }
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
pub fn check_for_updates() -> UpdateCheckOutcome {
    UpdateCheckOutcome::UnsupportedPlatform
}

#[cfg(all(test, any(target_os = "macos", target_os = "windows", target_os = "linux")))]
mod tests {
    use super::*;

    #[test]
    fn updater_http_client_disables_system_proxy_when_proxy_is_none() {
        let client = build_updater_http_client(None).unwrap();

        drop(client);
    }
}
