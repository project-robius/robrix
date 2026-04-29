//! URL / localpart / password validators for registration.

use url::Url;

/// Errors for Matrix localpart validation. Permissive grammar — matches the
/// historical Matrix spec (lowercase alnum + `._=-/`), not the stricter
/// MSC3967 recommendation. Enough for Phase 3a's target servers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LocalpartError {
    Empty,
    TooLong,
    InvalidChars,
}

/// Validate a Matrix localpart (the part before `:` in `@alice:example.com`).
/// Returns the trimmed localpart on success.
pub fn validate_localpart(input: &str) -> Result<String, LocalpartError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(LocalpartError::Empty);
    }
    if trimmed.len() > 255 {
        return Err(LocalpartError::TooLong);
    }
    if !trimmed.chars().all(|c| matches!(c,
        'a'..='z' | '0'..='9' | '.' | '_' | '=' | '-' | '/'
    )) {
        return Err(LocalpartError::InvalidChars);
    }
    Ok(trimmed.to_owned())
}

/// Password validation errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PasswordError {
    Empty,
    Mismatch,
}

/// Validate password + confirm-password pair. Phase 3a uses emptiness +
/// equality checks; zxcvbn strength scoring is Phase 5 scope.
pub fn validate_passwords_match(password: &str, confirm: &str) -> Result<(), PasswordError> {
    if password.is_empty() || confirm.is_empty() {
        return Err(PasswordError::Empty);
    }
    if password != confirm {
        return Err(PasswordError::Mismatch);
    }
    Ok(())
}

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

    #[test]
    fn localpart_valid_simple() {
        assert!(validate_localpart("alice").is_ok());
        assert!(validate_localpart("bob.smith").is_ok());
        assert!(validate_localpart("user_123").is_ok());
    }

    #[test]
    fn localpart_empty_is_rejected() {
        assert_eq!(validate_localpart(""), Err(LocalpartError::Empty));
        assert_eq!(validate_localpart("   "), Err(LocalpartError::Empty));
    }

    #[test]
    fn localpart_uppercase_is_rejected() {
        assert!(matches!(
            validate_localpart("Alice"),
            Err(LocalpartError::InvalidChars)
        ));
    }

    #[test]
    fn localpart_too_long_is_rejected() {
        let long = "a".repeat(256);
        assert_eq!(validate_localpart(&long), Err(LocalpartError::TooLong));
    }

    #[test]
    fn passwords_match_accepts_equal() {
        assert!(validate_passwords_match("hunter2", "hunter2").is_ok());
    }

    #[test]
    fn passwords_match_rejects_empty() {
        assert_eq!(
            validate_passwords_match("", ""),
            Err(PasswordError::Empty),
        );
    }

    #[test]
    fn passwords_match_rejects_mismatch() {
        assert_eq!(
            validate_passwords_match("a", "b"),
            Err(PasswordError::Mismatch),
        );
    }
}
