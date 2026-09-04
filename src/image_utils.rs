//! Image processing utilities.

/// Returns true if the given MIME type is an image format that robrix can display.
///
/// Delegates to [`crate::utils::is_supported_image_mimetype`] so display and
/// upload-preview gating share a single source of truth.
pub fn is_displayable_image(mime_type: &str) -> bool {
    crate::utils::is_supported_image_mimetype(mime_type)
}
