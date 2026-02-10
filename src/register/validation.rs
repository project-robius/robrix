use url::Url;

pub(super) const MIN_PASSWORD_CHARS: usize = 8;

pub(super) fn password_has_min_chars(password: &str, min: usize) -> bool {
    password.chars().count() >= min
}

pub(super) fn validate_username_localpart(raw: &str) -> Result<&str, &'static str> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("Username is required");
    }

    if trimmed.starts_with('@') || trimmed.contains(':') {
        return Err("Use a username like alice (not a full Matrix ID)");
    }

    if trimmed
        .chars()
        .all(|c| matches!(c, 'a'..='z' | '0'..='9' | '.' | '_' | '=' | '/' | '-'))
    {
        Ok(trimmed)
    } else {
        Err("Username can only use lowercase letters, numbers, and . _ = / -")
    }
}

pub(super) fn normalize_username(raw: &str) -> Result<String, &'static str> {
    validate_username_localpart(raw).map(ToOwned::to_owned)
}

pub(super) fn normalize_custom_homeserver(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    let normalized = if trimmed.contains("://") {
        trimmed.to_string()
    } else {
        format!("https://{trimmed}")
    };

    Url::parse(&normalized).ok().map(|_| normalized)
}

pub(super) fn needs_custom_homeserver_input(
    pending_custom_homeserver: bool,
    selected_homeserver: &str,
) -> bool {
    pending_custom_homeserver || selected_homeserver.trim().is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn username_validation_rejects_matrix_id_uppercase_and_spaces() {
        assert!(validate_username_localpart("@Alice:matrix.org").is_err());
        assert!(validate_username_localpart("Alice").is_err());
        assert!(validate_username_localpart("alice user").is_err());
    }

    #[test]
    fn username_validation_accepts_matrix_localpart_charset() {
        assert!(validate_username_localpart("alice_123.-=/").is_ok());
    }

    #[test]
    fn password_min_length_counts_unicode_chars_not_bytes() {
        assert!(password_has_min_chars("密码安全1234", 8));
        assert!(!password_has_min_chars("密码12", 8));
    }

    #[test]
    fn normalize_username_trims_and_rejects_empty() {
        assert!(normalize_username("   ").is_err());
        assert_eq!(normalize_username(" alice ").unwrap(), "alice");
    }

    #[test]
    fn normalize_custom_homeserver_adds_https_for_bare_domain() {
        assert_eq!(
            normalize_custom_homeserver("my-server.example").as_deref(),
            Some("https://my-server.example"),
        );
    }

    #[test]
    fn choosing_other_requires_explicit_custom_value_before_submit() {
        assert!(needs_custom_homeserver_input(true, "matrix.org"));
        assert!(needs_custom_homeserver_input(true, ""));
        assert!(!needs_custom_homeserver_input(false, "https://my-server.example"));
    }
}
