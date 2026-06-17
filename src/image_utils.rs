//! Image processing utilities.

use std::io::Read;
use std::path::Path;

use makepad_widgets::image_cache::image_size_by_data;

/// Returns true if the given MIME type is an image format that robrix can display.
///
/// Delegates to [`crate::utils::is_supported_image_mimetype`] so display and
/// upload-preview gating share a single source of truth.
pub fn is_displayable_image(mime_type: &str) -> bool {
    crate::utils::is_supported_image_mimetype(mime_type)
}

/// Reads an image's pixel `(width, height)` from the file at `path`.
///
/// Tries a bounded header read first (cheap for large photos), and falls back to
/// the full file only if the prefix wasn't enough — e.g. an animated GIF/ICO, or
/// a JPEG with a large EXIF/ICC block before its SOF marker. Because of that
/// fallback, it never fails to parse dimensions that a full read would have found.
pub fn read_image_dimensions(path: &Path) -> Option<(usize, usize)> {
    // Generous prefix: covers every format's header plus typical EXIF/ICC blocks.
    const HEADER_PREFIX_LEN: u64 = 256 * 1024;
    let mut file = std::fs::File::open(path).ok()?;
    let mut buf = Vec::new();
    file.by_ref().take(HEADER_PREFIX_LEN).read_to_end(&mut buf).ok()?;
    if let Ok(dims) = image_size_by_data(&buf, path) {
        return Some(dims);
    }
    // The prefix wasn't enough; pull in the rest of the file and try once more.
    if file.read_to_end(&mut buf).ok()? > 0 {
        return image_size_by_data(&buf, path).ok();
    }
    None
}
