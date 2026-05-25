//! Image processing utilities for thumbnail generation and image manipulation.

/// Returns true if the given MIME type represents an image format that can be displayed.
///
/// Note: Only JPEG and PNG are supported because those are the only formats
/// enabled in the `image` crate features. Other formats (gif, webp, bmp) would
/// fail to decode.
pub fn is_displayable_image(mime_type: &str) -> bool {
    matches!(
        mime_type,
        "image/png" | "image/jpeg" | "image/jpg"
    )
}
