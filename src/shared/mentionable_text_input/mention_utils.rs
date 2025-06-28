//! Utilities for handling Matrix mentions in different formats
//! 
//! This module provides conversion functions between HTML and Markdown formats
//! for Matrix user mentions, supporting multiple Matrix URI formats.

use makepad_widgets::log;

/// Constants for mention processing
pub const MAX_ITERATIONS: usize = 1000;
pub const MAX_URL_LENGTH: usize = 2048;
pub const MAX_USERNAME_LENGTH: usize = 255;
pub const MAX_DOMAIN_LENGTH: usize = 253;


/// Converts HTML format mentions to Markdown format.
/// 
/// Handles Matrix HTML mentions with any valid Matrix URI format.
/// Converts them to Markdown format while preserving the original URI.
/// 
/// # Arguments
/// * `html_text` - The HTML text containing potential mentions
/// 
/// # Returns
/// Markdown formatted text with HTML mentions converted to Markdown links
/// 
/// # Example
/// ```
/// let html = r#"Hello <a href="https://matrix.to/#/@alice:example.com">Alice</a>"#;
/// let markdown = convert_html_mentions_to_markdown(html);
/// assert_eq!(markdown, "Hello [Alice](https://matrix.to/#/@alice:example.com)");
/// ```
pub fn convert_html_mentions_to_markdown(html_text: &str) -> String {
    let mut markdown_text = html_text.to_string();
    let mut pos = 0;
    let mut iteration_count = 0;

    while let Some(start_pos) = markdown_text[pos..].find("<a href=\"") {
        iteration_count += 1;
        if iteration_count > MAX_ITERATIONS {
            log!("Warning: HTML to Markdown conversion stopped after {} iterations to prevent infinite loop", MAX_ITERATIONS);
            break;
        }

        let absolute_start = pos + start_pos;

        // Ensure we don't go out of bounds
        if absolute_start >= markdown_text.len() {
            break;
        }

        // Find the end of the href attribute with better bounds checking
        if let Some(href_end) = markdown_text[absolute_start..].find("\">") {
            let href_end_absolute = absolute_start + href_end;

            // Ensure indices are valid
            let href_start = absolute_start + "<a href=\"".len();
            if href_start > href_end_absolute || href_end_absolute > markdown_text.len() {
                pos = absolute_start + 1;
                continue;
            }

            // Extract the full URL from the href with bounds check
            let full_url = &markdown_text[href_start..href_end_absolute];

            // Validate URL is not empty and doesn't contain dangerous characters
            if full_url.is_empty() || full_url.contains('\n') || full_url.contains('\r') {
                pos = href_end_absolute + 2;
                continue;
            }

            // Check if this is a Matrix user mention
            if is_matrix_user_mention_url(full_url) {
                // Find the display name (text between > and </a>)
                let display_name_start = href_end_absolute + 2; // Skip ">

                // Bounds check for display name search
                if display_name_start >= markdown_text.len() {
                    pos = href_end_absolute + 2;
                    continue;
                }

                if let Some(link_end) = markdown_text[display_name_start..].find("</a>") {
                    let link_end_absolute = display_name_start + link_end;

                    // Ensure we don't exceed string bounds
                    if link_end_absolute > markdown_text.len() {
                        pos = href_end_absolute + 2;
                        continue;
                    }

                    let display_name = &markdown_text[display_name_start..link_end_absolute];

                    // Validate display name is reasonable (not empty, no newlines)
                    if display_name.is_empty() || display_name.contains('\n') || display_name.contains('\r') {
                        pos = href_end_absolute + 2;
                        continue;
                    }

                    // Create the Markdown mention, preserving the original URL
                    let markdown_mention = format!("[{}]({})", display_name.trim(), full_url);

                    // Calculate replacement range with bounds check
                    let full_link_end = link_end_absolute + 4; // Include "</a>"
                    if full_link_end > markdown_text.len() {
                        pos = href_end_absolute + 2;
                        continue;
                    }

                    // Perform the replacement
                    markdown_text.replace_range(absolute_start..full_link_end, &markdown_mention);

                    // Update position to continue searching after the replacement
                    pos = absolute_start + markdown_mention.len();
                } else {
                    // Malformed HTML (no closing </a>), skip this link
                    log!("Warning: Found malformed HTML link without closing </a> tag at position {}", absolute_start);
                    pos = href_end_absolute + 2;
                }
            } else {
                // Not a Matrix user mention, skip this link
                pos = href_end_absolute + 2;
            }
        } else {
            // Malformed HTML (no closing ">), skip
            log!("Warning: Found malformed HTML link without closing \"> at position {}", absolute_start);
            pos = absolute_start + 1;
        }
    }

    markdown_text
}

/// Checks if a URL is a Matrix user mention.
/// 
/// This method looks for @user:domain patterns in the URL, regardless of the URL format.
/// 
/// # Arguments
/// * `url` - The URL to check
/// 
/// # Returns
/// `true` if the URL contains a valid Matrix user mention pattern
/// 
/// # Example
/// ```
/// assert!(is_matrix_user_mention_url("https://matrix.to/#/@alice:example.com"));
/// assert!(is_matrix_user_mention_url("matrix:u/@bob:test.org"));
/// assert!(!is_matrix_user_mention_url("https://example.com"));
/// ```
pub fn is_matrix_user_mention_url(url: &str) -> bool {
    // Basic input validation
    if url.is_empty() || url.len() > MAX_URL_LENGTH {
        return false;
    }

    // Look for Matrix user ID pattern: @username:domain
    if let Some(at_pos) = url.find('@') {
        // Ensure @ is not at the very end
        if at_pos >= url.len() - 1 {
            return false;
        }

        // Find the colon after the @ within a reasonable range
        if let Some(colon_pos) = url[at_pos..].find(':') {
            let colon_abs_pos = at_pos + colon_pos;

            // Check that there's content after the colon (domain part)
            if colon_abs_pos + 1 >= url.len() {
                return false;
            }

            // Bounds checking
            if at_pos + 1 > colon_abs_pos || colon_abs_pos >= url.len() {
                return false;
            }

            // Extract potential username and domain
            let username_part = &url[at_pos + 1..colon_abs_pos];
            let remaining = &url[colon_abs_pos + 1..];

            // Basic length validation
            if username_part.is_empty() || username_part.len() > MAX_USERNAME_LENGTH || remaining.is_empty() {
                return false;
            }

            // Find where domain part ends (could be end of string, or next special char)
            let domain_end = remaining.find(|c: char| !c.is_alphanumeric() && c != '.' && c != '-')
                .unwrap_or(remaining.len());

            // Ensure domain_end is valid
            if domain_end == 0 || domain_end > remaining.len() {
                return false;
            }

            let domain_part = &remaining[..domain_end];

            // Enhanced validation with length limits
            let username_valid = !username_part.is_empty()
                && username_part.len() <= MAX_USERNAME_LENGTH
                && username_part.chars().all(|c| {
                    c.is_alphanumeric() || c == '.' || c == '_' || c == '=' || c == '-'
                })
                && !username_part.starts_with('-')
                && !username_part.ends_with('-');

            let domain_valid = !domain_part.is_empty()
                && domain_part.len() <= MAX_DOMAIN_LENGTH
                && domain_part.contains('.')
                && domain_part.chars().all(|c| c.is_alphanumeric() || c == '.' || c == '-')
                && !domain_part.starts_with('.')
                && !domain_part.ends_with('.')
                && !domain_part.starts_with('-')
                && !domain_part.ends_with('-')
                && !domain_part.contains("..");

            return username_valid && domain_valid;
        }
    }

    false
}


/// Checks if text contains Matrix user mention links in any supported format.
/// 
/// # Arguments
/// * `text` - The text to check
/// 
/// # Returns
/// `true` if the text contains Matrix user mentions
/// 
/// # Example
/// ```
/// let text = "Hello [Alice](https://matrix.to/#/@alice:example.com)";
/// assert!(contains_matrix_user_mentions(text));
/// ```
pub fn contains_matrix_user_mentions(text: &str) -> bool {
    // Check for traditional matrix.to format
    if text.contains("](https://matrix.to/#/@") {
        return true;
    }

    // Check for MSC1270 format: matrix:u/
    if text.contains("](matrix:u/") {
        return true;
    }

    // Check for custom Matrix server formats using pattern matching
    contains_custom_matrix_links(text)
}

/// Detects custom Matrix server mention links using pattern matching.
/// 
/// Looks for patterns like `](https://custom.server/path/#/@user:domain.com)`
/// or `](https://custom.server/#/@user:domain.com)`
/// 
/// # Arguments
/// * `text` - The text to search
/// 
/// # Returns
/// `true` if custom Matrix links are found
pub fn contains_custom_matrix_links(text: &str) -> bool {
    let mut pos = 0;
    while let Some(bracket_pos) = text[pos..].find("](https://") {
        let absolute_pos = pos + bracket_pos + 2; // Skip "]("

        // Find the closing parenthesis
        if let Some(close_paren) = text[absolute_pos..].find(')') {
            let url = &text[absolute_pos..absolute_pos + close_paren];

            // Check if this URL contains a Matrix user ID pattern (@user:domain)
            if is_matrix_user_mention_url(url) {
                return true;
            }
        }

        pos = absolute_pos + 1;
    }

    false
}




#[cfg(test)]
mod tests_html_to_markdown_conversion {
    use super::*;

    #[test]
    fn tests_convert_single_html_mention_to_markdown() {
        // Create a mock MentionableTextInput for testing
        // We can't easily create a full widget instance in tests,
        // so we'll test the conversion logic directly
        let html_input = r#"Hello <a href="https://matrix.to/#/@alice:example.com">Alice</a> how are you?"#;
        let expected_markdown = r#"Hello [Alice](https://matrix.to/#/@alice:example.com) how are you?"#;

        // Test the conversion logic
        let result = convert_html_mentions_to_markdown(html_input);
        assert_eq!(result, expected_markdown);
    }

    #[test]
    fn tests_convert_multiple_html_mentions_to_markdown() {
        let html_input = r#"@room @room  @room  <a href="https://matrix.to/#/@blackanger:matrix.org">Alex</a>  <a href="https://matrix.to/#/@feeds:integrations.ems.host">Feeds</a> @room  <a href="https://matrix.to/#/@blackanger:matrix.org">Alex</a>"#;
        let expected_markdown = r#"@room @room  @room  [Alex](https://matrix.to/#/@blackanger:matrix.org)  [Feeds](https://matrix.to/#/@feeds:integrations.ems.host) @room  [Alex](https://matrix.to/#/@blackanger:matrix.org)"#;

        let result = convert_html_mentions_to_markdown(html_input);
        assert_eq!(result, expected_markdown);
    }

    #[test]
    fn tests_convert_mixed_content_with_html_mentions() {
        let html_input = r#"Hi <a href="https://matrix.to/#/@user:server.com">User</a>, let's discuss this with <a href="https://matrix.to/#/@admin:server.com">Admin</a>."#;
        let expected_markdown = r#"Hi [User](https://matrix.to/#/@user:server.com), let's discuss this with [Admin](https://matrix.to/#/@admin:server.com)."#;

        let result = convert_html_mentions_to_markdown(html_input);
        assert_eq!(result, expected_markdown);
    }

    #[test]
    fn tests_no_conversion_when_no_html_mentions() {
        let plain_text = "Hello @room this is a test message";
        let result = convert_html_mentions_to_markdown(plain_text);
        assert_eq!(result, plain_text);
    }

    #[test]
    fn tests_convert_alternative_matrix_url_formats() {
        // Test with different Matrix URL formats
        let html_input = r#"Hi <a href="matrix:u/@user:example.com">User</a> and <a href="https://example.com/matrix/#/@admin:test.org">Admin</a>!"#;
        let expected_markdown = r#"Hi [User](matrix:u/@user:example.com) and [Admin](https://example.com/matrix/#/@admin:test.org)!"#;

        let result = convert_html_mentions_to_markdown(html_input);
        assert_eq!(result, expected_markdown);
    }

    #[test]
    fn tests_ignore_non_matrix_links() {
        // Test that non-Matrix links are left unchanged
        let html_input = r#"Check out <a href="https://example.com">this website</a> and <a href="mailto:test@example.com">send email</a>."#;
        let expected_markdown = html_input; // Should remain unchanged

        let result = convert_html_mentions_to_markdown(html_input);
        assert_eq!(result, expected_markdown);
    }

    #[test]
    fn tests_mixed_matrix_and_regular_links() {
        // Test mixing Matrix mentions with regular links
        let html_input = r#"Visit <a href="https://example.com">our site</a> or contact <a href="https://matrix.to/#/@support:example.com">Support</a>."#;
        let expected_markdown = r#"Visit <a href="https://example.com">our site</a> or contact [Support](https://matrix.to/#/@support:example.com)."#;

        let result = convert_html_mentions_to_markdown(html_input);
        assert_eq!(result, expected_markdown);
    }

    #[test]
    fn tests_extract_mentions_from_text_matrix_to_format() {
        // Test that extract_mentions_from_text can properly extract matrix.to format mentions
        let text_with_matrix_to = "Hello [Alice](https://matrix.to/#/@alice:example.com) and [Bob](https://matrix.to/#/@bob:test.org) how are you?";

        // We can't easily instantiate MentionableTextInput in tests, so let's test the logic directly
        // by checking if contains_matrix_user_mentions detects matrix.to format
        assert!(contains_matrix_user_mentions(text_with_matrix_to));

        // Test that mixed formats are both detected
        let text_mixed = "Hi [Alice](matrix:u/@alice:example.com) and [Bob](https://matrix.to/#/@bob:example.com)";
        assert!(contains_matrix_user_mentions(text_mixed));

        // Test matrix.to format specifically
        let text_matrix_to_only = "Check [User](https://matrix.to/#/@user:server.com)";
        assert!(contains_matrix_user_mentions(text_matrix_to_only));

        // Test that non-Matrix URLs are not detected
        let text_no_matrix = "Visit [our site](https://example.com) for more info";
        assert!(!contains_matrix_user_mentions(text_no_matrix));
    }

    #[test]
    fn tests_extract_mentions_matrix_to_edge_cases() {
        // Test edge cases for matrix.to format detection

        // Test with trailing content
        let text_with_trailing = "Hello [User Name](https://matrix.to/#/@user:server.com) goodbye!";
        assert!(contains_matrix_user_mentions(text_with_trailing));

        // Test with multiple matrix.to mentions
        let text_multiple = "[A](https://matrix.to/#/@a:test.com) [B](https://matrix.to/#/@b:test.com) [C](https://matrix.to/#/@c:test.com)";
        assert!(contains_matrix_user_mentions(text_multiple));

        // Test that incomplete matrix.to URLs are not detected
        let text_incomplete = "Hello [Invalid](https://matrix.to/#/) world";
        assert!(!contains_matrix_user_mentions(text_incomplete));

        // Test that matrix.to URLs without @ are not detected as user mentions
        let text_no_at = "Hello [Room](https://matrix.to/#/!room:server.com) world";
        assert!(!contains_matrix_user_mentions(text_no_at));
    }

    #[test]
    fn tests_matrix_to_vs_matrix_u_detection() {
        // Ensure both formats are detected correctly

        // MSC1270 format
        let text_msc1270 = "Hello [Alice](matrix:u/@alice:example.com)";
        assert!(contains_matrix_user_mentions(text_msc1270));

        // Traditional matrix.to format
        let text_matrix_to = "Hi [Bob](https://matrix.to/#/@bob:example.com)";
        assert!(contains_matrix_user_mentions(text_matrix_to));

        // Mixed formats in one message
        let text_mixed = "Hi [Alice](matrix:u/@alice:example.com) and [Bob](https://matrix.to/#/@bob:example.com)";
        assert!(contains_matrix_user_mentions(text_mixed));

        // Custom server format (should also be detected by contains_custom_matrix_links)
        let text_custom = "Hey [Charlie](https://custom.server/#/@charlie:example.com)";
        assert!(contains_matrix_user_mentions(text_custom));
    }

    #[test]
    fn tests_matrix_to_validation_patterns() {
        // Test various validation scenarios for matrix.to format detection

        // Valid patterns
        assert!(contains_matrix_user_mentions("[User](https://matrix.to/#/@user:server.com)"));
        assert!(contains_matrix_user_mentions("Hello [User](https://matrix.to/#/@user:server.com)"));
        assert!(contains_matrix_user_mentions("[Long User Name](https://matrix.to/#/@user:server.com)"));

        // Invalid patterns - these should NOT be detected as Matrix user mentions
        assert!(!contains_matrix_user_mentions("[Link](https://matrix.to/)"));
        assert!(!contains_matrix_user_mentions("[Room](https://matrix.to/#/!room:server.com)"));
        assert!(!contains_matrix_user_mentions("[Event](https://matrix.to/#/!room:server.com/$event)"));
        assert!(!contains_matrix_user_mentions("[Website](https://example.com)"));
        assert!(!contains_matrix_user_mentions("[Email](mailto:user@example.com)"));

        // Edge cases
        assert!(!contains_matrix_user_mentions(""));
        assert!(!contains_matrix_user_mentions("No links here"));
        assert!(!contains_matrix_user_mentions("@user:server.com without markdown"));
    }

    #[test]
    fn tests_debug_problematic_input() {
        // Test the exact problematic input to debug the issue
        let problematic_input = r#"@room [Feeds](https://matrix.to/#/@feeds:integrations.ems.host)"#;
        let result = convert_html_mentions_to_markdown(problematic_input);
        // This input is already markdown, should remain unchanged
        assert_eq!(result, problematic_input);

        // Test what the actual HTML input might look like
        let html_input = r#"@room <a href="https://matrix.to/#/@feeds:integrations.ems.host">Feeds</a>"#;
        let expected_markdown = r#"@room [Feeds](https://matrix.to/#/@feeds:integrations.ems.host)"#;
        let result = convert_html_mentions_to_markdown(html_input);
        assert_eq!(result, expected_markdown);
    }

    #[test]
    fn tests_matrix_user_mentions_detection() {
        // Create a mock instance for testing (we can't easily create a full widget in tests)

        // Test MSC1270 format
        let text_msc1270 = "Hello [Alice](matrix:u/@alice:example.com) how are you?";
        assert!(contains_matrix_user_mentions(text_msc1270));

        // Test traditional matrix.to format
        let text_matrix_to = "Hi [Bob](https://matrix.to/#/@bob:example.com) there!";
        assert!(contains_matrix_user_mentions(text_matrix_to));

        // Test custom server format
        let text_custom = "Hey [Charlie](https://custom.server/#/@charlie:example.com)!";
        assert!(contains_matrix_user_mentions(text_custom));

        // Test mixed formats
        let text_mixed = "Hi [Alice](matrix:u/@alice:example.com) and [Bob](https://matrix.to/#/@bob:example.com)";
        assert!(contains_matrix_user_mentions(text_mixed));

        // Test non-Matrix links (should return false)
        let text_no_matrix = "Check out [this link](https://example.com) and [email](mailto:test@example.com)";
        assert!(!contains_matrix_user_mentions(text_no_matrix));

        // Test plain text (should return false)
        let text_plain = "Hello @alice how are you?";
        assert!(!contains_matrix_user_mentions(text_plain));
    }

    #[test]
    fn test_convert_html_to_markdown() {
        let html = r#"Hello <a href="https://matrix.to/#/@alice:example.com">Alice</a> how are you?"#;
        let expected = r#"Hello [Alice](https://matrix.to/#/@alice:example.com) how are you?"#;
        assert_eq!(convert_html_mentions_to_markdown(html), expected);
    }

    #[test]
    fn test_multiple_html_mentions() {
        let html = r#"Hi <a href="https://matrix.to/#/@alice:example.com">Alice</a> and <a href="matrix:u/@bob:test.org">Bob</a>"#;
        let expected = r#"Hi [Alice](https://matrix.to/#/@alice:example.com) and [Bob](matrix:u/@bob:test.org)"#;
        assert_eq!(convert_html_mentions_to_markdown(html), expected);
    }

    #[test]
    fn test_ignore_non_matrix_links() {
        let html = r#"Visit <a href="https://example.com">website</a> and contact <a href="mailto:test@example.com">email</a>"#;
        assert_eq!(convert_html_mentions_to_markdown(html), html);
    }

    #[test]
    fn test_is_matrix_user_mention_url() {
        assert!(is_matrix_user_mention_url("https://matrix.to/#/@alice:example.com"));
        assert!(is_matrix_user_mention_url("matrix:u/@bob:test.org"));
        assert!(is_matrix_user_mention_url("https://custom.server/#/@charlie:domain.com"));
        assert!(!is_matrix_user_mention_url("https://example.com"));
        assert!(!is_matrix_user_mention_url("mailto:test@example.com"));
    }


    #[test]
    fn test_contains_matrix_user_mentions() {
        assert!(contains_matrix_user_mentions("Hello [Alice](https://matrix.to/#/@alice:example.com)"));
        assert!(contains_matrix_user_mentions("Hi [Bob](matrix:u/@bob:test.org)"));
        assert!(contains_matrix_user_mentions("Hey [Charlie](https://custom.server/#/@charlie:domain.com)"));
        assert!(!contains_matrix_user_mentions("Visit [website](https://example.com)"));
    }


    #[test]
    fn test_malformed_html_handling() {
        // Test malformed HTML doesn't crash
        let malformed = r#"<a href="invalid">broken</a href="#;
        let result = convert_html_mentions_to_markdown(malformed);
        // Should not crash and return some reasonable result
        assert!(!result.is_empty());
    }

    #[test]
    fn test_edge_cases() {
        // Empty input
        assert_eq!(convert_html_mentions_to_markdown(""), "");
        assert!(!contains_matrix_user_mentions(""));
        
        // Very long URLs (should be rejected)
        let long_url = format!("https://matrix.to/#/@{}", "a".repeat(3000));
        assert!(!is_matrix_user_mention_url(&long_url));
    }
}