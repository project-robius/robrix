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
mod tests {
    use super::*;

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