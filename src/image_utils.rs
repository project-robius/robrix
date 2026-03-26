//! Image processing utilities for thumbnail generation and image manipulation.

use std::io::Cursor;

/// The maximum dimension (width or height) for generated thumbnails.
pub const THUMBNAIL_MAX_DIMENSION: u32 = 800;

/// Generates a thumbnail from the given image data.
///
/// The thumbnail is scaled to fit within `THUMBNAIL_MAX_DIMENSION` while preserving aspect ratio.
/// Returns the thumbnail as JPEG-encoded bytes, along with the thumbnail's dimensions.
///
/// # Arguments
/// * `image_data` - The raw bytes of the source image (PNG, JPEG, etc.)
///
/// # Returns
/// * `Ok((jpeg_bytes, width, height))` - The thumbnail data and dimensions
/// * `Err(String)` - Error message if thumbnail generation fails
pub fn generate_thumbnail(image_data: &[u8]) -> Result<(Vec<u8>, u32, u32), String> {
    use image::{ImageFormat, ImageReader};

    // Load the image from bytes
    let img = ImageReader::new(Cursor::new(image_data))
        .with_guessed_format()
        .map_err(|e| format!("Failed to guess image format: {e}"))?
        .decode()
        .map_err(|e| format!("Failed to decode image: {e}"))?;

    let (orig_width, orig_height) = (img.width(), img.height());

    // Calculate thumbnail dimensions while preserving aspect ratio
    let (thumb_width, thumb_height) = if orig_width > THUMBNAIL_MAX_DIMENSION
        || orig_height > THUMBNAIL_MAX_DIMENSION
    {
        let ratio = f64::from(orig_width) / f64::from(orig_height);
        if orig_width > orig_height {
            let new_width = THUMBNAIL_MAX_DIMENSION;
            let new_height = (f64::from(new_width) / ratio) as u32;
            (new_width, new_height)
        } else {
            let new_height = THUMBNAIL_MAX_DIMENSION;
            let new_width = (f64::from(new_height) * ratio) as u32;
            (new_width, new_height)
        }
    } else {
        (orig_width, orig_height)
    };

    // Resize the image using a high-quality filter
    let thumbnail = img.resize(
        thumb_width,
        thumb_height,
        image::imageops::FilterType::Lanczos3,
    );

    // Encode as JPEG
    let mut jpeg_bytes = Vec::new();
    thumbnail
        .write_to(&mut Cursor::new(&mut jpeg_bytes), ImageFormat::Jpeg)
        .map_err(|e| format!("Failed to encode thumbnail as JPEG: {e}"))?;

    Ok((jpeg_bytes, thumb_width, thumb_height))
}

/// Returns the MIME type string for the given image data by inspecting its header bytes.
///
/// Returns `None` if the image format cannot be determined.
pub fn detect_mime_type(data: &[u8]) -> Option<&'static str> {
    match imghdr::from_bytes(data) {
        Some(imghdr::Type::Png) => Some("image/png"),
        Some(imghdr::Type::Jpeg) => Some("image/jpeg"),
        Some(imghdr::Type::Gif) => Some("image/gif"),
        Some(imghdr::Type::Webp) => Some("image/webp"),
        Some(imghdr::Type::Bmp) => Some("image/bmp"),
        Some(imghdr::Type::Tiff) => Some("image/tiff"),
        _ => None,
    }
}

/// Returns true if the given MIME type represents an image format that can be displayed.
pub fn is_displayable_image(mime_type: &str) -> bool {
    matches!(
        mime_type,
        "image/png"
            | "image/jpeg"
            | "image/jpg"
            | "image/gif"
            | "image/webp"
            | "image/bmp"
    )
}

/// Gets the dimensions of an image from its raw bytes.
///
/// Returns `None` if the image cannot be decoded.
pub fn get_image_dimensions(data: &[u8]) -> Option<(u32, u32)> {
    use image::ImageReader;

    ImageReader::new(Cursor::new(data))
        .with_guessed_format()
        .ok()?
        .into_dimensions()
        .ok()
}
