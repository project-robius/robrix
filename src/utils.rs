use chrono::NaiveDateTime;
use makepad_widgets::{ImageRef, ImageError, Cx};
use matrix_sdk::ruma::MilliSecondsSinceUnixEpoch;


/// Loads the given image `data` into the given `ImageRef` as either a
/// PNG or JPEG, using the `imghdr` library to determine which format it is.
///
/// Returns an error if either load fails or if the image format is unknown.
pub fn load_png_or_jpg(img: &ImageRef, cx: &mut Cx, data: &[u8]) -> Result<(), ImageError> {
    match imghdr::from_bytes(data) {
        Some(imghdr::Type::Png) => img.load_png_from_data(cx, data),
        Some(imghdr::Type::Jpeg) => img.load_jpg_from_data(cx, data),
        Some(unsupported) => {
            eprintln!("load_png_or_jpg(): The {unsupported:?} image format is unsupported");
            Err(ImageError::UnsupportedFormat)
        }
        None => {
            eprintln!("load_png_or_jpg(): Unknown image format");
            Err(ImageError::UnsupportedFormat)
        }
    }
}


pub fn unix_time_millis_to_datetime(millis: &MilliSecondsSinceUnixEpoch) -> Option<NaiveDateTime> {
    let millis: i64 = millis.get().into();
    NaiveDateTime::from_timestamp_millis(millis)
}
