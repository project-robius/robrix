//! URL / localpart / password validators for registration.

use url::Url;

/// Errors returned by homeserver URL normalization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HomeserverUrlError {
    /// Input was empty or whitespace only.
    Empty,
    /// Scheme is neither `http` nor `https`.
    UnsupportedScheme(String),
    /// URL could not be parsed.
    Invalid,
}

/// Normalize a user-entered homeserver URL.
///
/// - Bare hostname (e.g. `matrix.org`) becomes `https://matrix.org`.
/// - Explicit `http(s)://` schemes are kept as-is.
/// - Any non-`http(s)` scheme is rejected.
/// - Trailing `/` is stripped.
/// - Empty string is rejected.
pub fn normalize_homeserver_url(input: &str) -> Result<String, HomeserverUrlError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(HomeserverUrlError::Empty);
    }

    // Strip trailing slash from input before adding scheme
    let trimmed_slash = trimmed.trim_end_matches('/');

    let with_scheme = if trimmed_slash.contains("://") {
        trimmed_slash.to_string()
    } else {
        format!("https://{trimmed_slash}")
    };

    let url = Url::parse(&with_scheme).map_err(|_| HomeserverUrlError::Invalid)?;

    match url.scheme() {
        "http" | "https" => {}
        other => return Err(HomeserverUrlError::UnsupportedScheme(other.to_string())),
    }

    // Return the URL string without trailing slash (url crate always adds one for domain-only URLs)
    let canonical = url.as_str().trim_end_matches('/').to_string();
    Ok(canonical)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bare_hostname_gets_https() {
        let url = normalize_homeserver_url("matrix.org").unwrap();
        assert_eq!(url.as_str(), "https://matrix.org");
    }

    #[test]
    fn explicit_https_is_kept() {
        let url = normalize_homeserver_url("https://alvin.meldry.com").unwrap();
        assert_eq!(url.as_str(), "https://alvin.meldry.com");
    }

    #[test]
    fn explicit_http_is_kept() {
        let url = normalize_homeserver_url("http://127.0.0.1:8128").unwrap();
        assert_eq!(url.as_str(), "http://127.0.0.1:8128");
    }

    #[test]
    fn trailing_slash_is_stripped() {
        let url = normalize_homeserver_url("https://matrix.org/").unwrap();
        assert_eq!(url.as_str(), "https://matrix.org");
    }

    #[test]
    fn whitespace_is_trimmed() {
        let url = normalize_homeserver_url("  matrix.org  ").unwrap();
        assert_eq!(url.as_str(), "https://matrix.org");
    }

    #[test]
    fn empty_input_is_rejected() {
        assert_eq!(
            normalize_homeserver_url(""),
            Err(HomeserverUrlError::Empty),
        );
        assert_eq!(
            normalize_homeserver_url("   "),
            Err(HomeserverUrlError::Empty),
        );
    }

    #[test]
    fn non_http_scheme_is_rejected() {
        let result = normalize_homeserver_url("ftp://example.com");
        assert!(matches!(result, Err(HomeserverUrlError::UnsupportedScheme(ref s)) if s == "ftp"));
    }

    #[test]
    fn malformed_url_is_rejected() {
        let result = normalize_homeserver_url("http://  /not a url");
        assert!(matches!(result, Err(HomeserverUrlError::Invalid)));
    }
}
