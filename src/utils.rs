use std::time::SystemTime;

use chrono::{DateTime, Local, TimeZone};
use makepad_widgets::{error, image_cache::ImageError, Cx, ImageRef};
use matrix_sdk::{ruma::{MilliSecondsSinceUnixEpoch, api::client::media::get_content_thumbnail::v3::Method}, media::{MediaThumbnailSize, MediaFormat}};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ImageFormat {
    Png,
    Jpeg,
}
impl ImageFormat {
    pub fn from_mimetype(mimetype: &str) -> Option<Self> {
        match mimetype {
            "image/png" => Some(Self::Png),
            "image/jpeg" => Some(Self::Jpeg),
            _ => None,
        }
    }
}

/// Loads the given image `data` into the given `ImageRef` as either a
/// PNG or JPEG, using the `imghdr` library to determine which format it is.
///
/// Returns an error if either load fails or if the image format is unknown.
pub fn load_png_or_jpg(img: &ImageRef, cx: &mut Cx, data: &[u8]) -> Result<(), ImageError> {
   let res = match imghdr::from_bytes(data) {
        Some(imghdr::Type::Png) => img.load_png_from_data(cx, data),
        Some(imghdr::Type::Jpeg) => img.load_jpg_from_data(cx, data),
        Some(unsupported) => {
            error!("load_png_or_jpg(): The {unsupported:?} image format is unsupported");
            Err(ImageError::UnsupportedFormat)
        }
        None => {
            error!("load_png_or_jpg(): Unknown image format");
            Err(ImageError::UnsupportedFormat)
        }
    };
    if let Err(err) = res.as_ref() {
        // debugging: dump out the avatar image to disk
        let mut path = crate::temp_storage::get_temp_dir_path().clone();
        let filename = format!("img_{}",
            SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_millis(),
        );
        path.push(filename);
        path.set_extension("unknown");
        error!("Failed to load PNG/JPG: {err}. Dumping bad image: {:?}", path);
        std::fs::write(path, &data)
            .expect("Failed to write user avatar image to disk");
    }
    res
}


pub fn unix_time_millis_to_datetime(millis: &MilliSecondsSinceUnixEpoch) -> Option<DateTime<Local>> {
    let millis: i64 = millis.get().into();
    Local.timestamp_millis_opt(millis).single()
}


/// A const-compatible version of [`MediaFormat`].
#[derive(Clone, Debug)]
pub enum MediaFormatConst {
    /// The file that was uploaded.
    File,
    /// A thumbnail of the file that was uploaded.
    Thumbnail(MediaThumbnailSizeConst),
}
impl From<MediaFormatConst> for MediaFormat {
    fn from(constant: MediaFormatConst) -> Self {
        match constant {
            MediaFormatConst::File => Self::File,
            MediaFormatConst::Thumbnail(size) => Self::Thumbnail(size.into()),
        }
    }
}

/// A const-compatible version of [`MediaThumbnailSize`].
#[derive(Clone, Debug)]
pub struct MediaThumbnailSizeConst {
    /// The desired resizing method.
    pub method: Method,

    /// The desired width of the thumbnail. The actual thumbnail may not match
    /// the size specified.
    pub width: u32,

    /// The desired height of the thumbnail. The actual thumbnail may not match
    /// the size specified.
    pub height: u32,
}
impl From<MediaThumbnailSizeConst> for MediaThumbnailSize {
    fn from(constant: MediaThumbnailSizeConst) -> Self {
        Self {
            method: constant.method,
            width: constant.width.into(),
            height: constant.height.into(),
        }
    }
}

/// The default media format to use for thumbnail requests.
pub const MEDIA_THUMBNAIL_FORMAT: MediaFormatConst = MediaFormatConst::Thumbnail(MediaThumbnailSizeConst {
    method: Method::Scale,
    width: 40,
    height: 40,
});

