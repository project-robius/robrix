//! Image processing utilities for thumbnail generation.

use std::fs::File;
use std::io::BufReader;

use image::{ImageFormat as ImageEncodingFormat, ImageReader};
use matrix_sdk::{attachment::Thumbnail, ruma::UInt};
use mime_guess::mime;

/// Image dimensions as (width, height)
pub type ImageDimensions = (u32, u32);

/// Result type for thumbnail generation
pub type ThumbnailResult = Result<(Option<Thumbnail>, Option<ImageDimensions>), Box<dyn std::error::Error>>;

/// Maximum dimensions for image thumbnails
const THUMBNAIL_MAX_WIDTH: u32 = 800;
const THUMBNAIL_MAX_HEIGHT: u32 = 600;

/// Generates a thumbnail and dimensions for an image file.
///
/// For images, this decodes the image and creates a thumbnail to reduce upload size.
/// For non-image files, returns None.
///
/// # Arguments
/// * `path` - Path to the file
/// * `mime_type` - MIME type of the file
///
/// # Returns
/// * `Ok((Some(Thumbnail), Some((width, height)))` - The thumbnail data for images
/// * `Ok((None, None))` - For non-image files
/// * `Err(Box<dyn std::error::Error>)` - Error if file cannot be read or processed
pub fn generate_thumbnail_dimension_if_image(
    path: &std::path::Path,
    mime_type: &mime::Mime,
) -> ThumbnailResult {
    // Only generate thumbnails for images
    if mime_type.type_() != mime::IMAGE {
        return Ok((None, None));
    }

    let file = File::open(path)?;
    let reader = BufReader::new(file);

    // Use ImageReader to decode only what's needed
    let img = ImageReader::new(reader).with_guessed_format()?.decode()?;

    // Create thumbnail if image is larger than max dimensions (preserves aspect ratio)
    let (orig_width, orig_height) = (img.width(), img.height());
    let needs_resize = orig_width > THUMBNAIL_MAX_WIDTH || orig_height > THUMBNAIL_MAX_HEIGHT;

    let final_img = if needs_resize {
        img.thumbnail(THUMBNAIL_MAX_WIDTH, THUMBNAIL_MAX_HEIGHT)
    } else {
        img
    };

    // Get the actual thumbnail dimensions (after aspect-ratio-preserving resize)
    let (thumb_width, thumb_height) = (final_img.width(), final_img.height());

    // Save to bytes as JPEG for efficient compression
    let mut bytes = Vec::new();

    final_img.write_to(
        &mut std::io::Cursor::new(&mut bytes),
        ImageEncodingFormat::Jpeg,
    )?;
    let bytes_size = bytes.len() as u32;
    Ok((
        Some(Thumbnail {
            data: bytes,
            content_type: mime::IMAGE_JPEG,
            height: UInt::from(thumb_height),
            width: UInt::from(thumb_width),
            size: UInt::from(bytes_size),
        }),
        Some((thumb_width, thumb_height)),
    ))
}
