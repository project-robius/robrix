use std::time::SystemTime;

use chrono::NaiveDateTime;
use makepad_widgets::{ImageRef, ImageError, Cx};
use matrix_sdk::ruma::MilliSecondsSinceUnixEpoch;


/// Loads the given image `data` into the given `ImageRef` as either a
/// PNG or JPEG, using the `imghdr` library to determine which format it is.
///
/// Returns an error if either load fails or if the image format is unknown.
pub fn load_png_or_jpg(img: &ImageRef, cx: &mut Cx, data: &[u8]) -> Result<(), ImageError> {
   let res = match imghdr::from_bytes(data) {
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
    };
    if let Err(err) = res.as_ref() {
        // debugging: dump out the avatar image to disk
        let mut path = crate::temp_storage::get_temp_dir_path().clone();
        let filename = format!("img_{}",
            SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_millis(),
        );
        path.push(filename);
        path.set_extension("unknown");
        eprintln!("Failed to load PNG/JPG: {err}. Dumping bad image: {:?}", path);
        std::fs::write(path, &data)
            .expect("Failed to write user avatar image to disk");
    }
    res
}


pub fn unix_time_millis_to_datetime(millis: &MilliSecondsSinceUnixEpoch) -> Option<NaiveDateTime> {
    let millis: i64 = millis.get().into();
    NaiveDateTime::from_timestamp_millis(millis)
}
