// Ignore clippy warnings in `DeRon` macro derive bodies.
#![allow(clippy::question_mark)]

use std::{borrow::Cow, fmt::Display, ops::{Deref, DerefMut}, str::{Chars, FromStr}, time::SystemTime};
use url::Url;
use makepad_widgets::*;
use unicode_segmentation::UnicodeSegmentation;
use chrono::{DateTime, Duration, Local, TimeZone};
use makepad_widgets::{error, image_cache::{ImageBuffer, ImageError}, live, makepad_micro_serde::{DeRon, DeRonErr, DeRonState, SerRon, SerRonState}, rotated_image::RotatedImageRef, Cx, Event, ImageRef, Texture};
use matrix_sdk::{media::{MediaFormat, MediaThumbnailSettings}, ruma::{api::client::media::get_content_thumbnail::v3::Method, MilliSecondsSinceUnixEpoch, OwnedRoomId, RoomId}};
use matrix_sdk_ui::timeline::{EventTimelineItem, PaginationError, TimelineDetails};

use crate::{room::FetchedRoomAvatar, shared::image_viewer::ImageViewerError, sliding_sync::{MatrixRequest, submit_async_request}};

/// The scheme for GEO links, used for location messages in Matrix.
pub const GEO_URI_SCHEME: &str = "geo:";

/// The max size (width or height) of a rotated image to display.
const ROTATED_IMAGE_MAX_SIZE: u32 = 500;

/// A wrapper type that implements the `Debug` trait for non-`Debug` types.
pub struct DebugWrapper<T>(T);
impl<T> std::fmt::Debug for DebugWrapper<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "({})", std::any::type_name::<T>())
    }
}
impl<T> Deref for DebugWrapper<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl<T> DerefMut for DebugWrapper<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
impl<T> From<T> for DebugWrapper<T> {
    fn from(value: T) -> Self {
        DebugWrapper(value)
    }
}
impl<T: Default> Default for DebugWrapper<T> {
    fn default() -> Self {
        DebugWrapper(T::default())
    }
}
impl<T> DebugWrapper<T> {
    /// Consumes the wrapper and returns the inner value.
    pub fn into_inner(self) -> T {
        self.0
    }
}

/// Returns true if the given event is an interactive hit-related event
/// that should require a view/widget to be visible in order to handle/receive it.
pub fn is_interactive_hit_event(event: &Event) -> bool {
    matches!(
        event,
        Event::MouseDown(..)
        | Event::MouseUp(..)
        | Event::MouseMove(..)
        | Event::MouseLeave(..)
        | Event::TouchUpdate(..)
        | Event::Scroll(..)
        | Event::KeyDown(..)
        | Event::KeyUp(..)
        | Event::TextInput(..)
        | Event::TextCopy(..)
        | Event::TextCut(..)
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ImageFormat {
    Png,
    Jpeg,
    XIcon,
}

impl ImageFormat {
    pub fn from_mimetype(mimetype: &str) -> Option<Self> {
        match mimetype {
            "image/png" => Some(Self::Png),
            "image/jpeg" => Some(Self::Jpeg),
            "image/x-icon" => Some(Self::XIcon),
            _ => None,
        }
    }
}

/// Loads the given image `data` into the given `ImageRef` as either a
/// PNG or JPEG, using the `imghdr` library to determine which format it is.
///
/// Returns an error if either load fails or if the image format is unknown.
pub fn load_png_or_jpg(img: &ImageRef, cx: &mut Cx, data: &[u8]) -> Result<(), ImageError> {

    fn attempt_both(img: &ImageRef, cx: &mut Cx, data: &[u8]) -> Result<(), ImageError> {
        img.load_png_from_data(cx, data)
            .or_else(|_| img.load_jpg_from_data(cx, data))
    }

    let res = match imghdr::from_bytes(data) {
        Some(imghdr::Type::Png) => img.load_png_from_data(cx, data),
        Some(imghdr::Type::Jpeg) => img.load_jpg_from_data(cx, data),
        Some(unsupported) => {
            // Attempt to load it as a PNG or JPEG anyway, since imghdr isn't perfect.
            attempt_both(img, cx, data).map_err(|_| {
                error!("load_png_or_jpg(): The {unsupported:?} image format is unsupported");
                ImageError::UnsupportedFormat
            })
        }
        None => {
            // Attempt to load it as a PNG or JPEG anyway, since imghdr isn't perfect.
            attempt_both(img, cx, data).map_err(|_| {
                error!("load_png_or_jpg(): Unknown image format");
                ImageError::UnsupportedFormat
            })
        }
    };
    if let Err(err) = res.as_ref() {
        // debugging: dump out the bad image to disk
        let mut path = crate::temp_storage::get_temp_dir_path().clone();
        let filename = format!(
            "img_{}",
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or_else(|_| rand::random::<u128>()),
        );
        path.push(filename);
        path.set_extension("unknown");
        error!("Failed to load PNG/JPG: {err}. Dumping bad image: {:?}", path);
        let _ = std::fs::write(path, data)
            .inspect_err(|e| error!("Failed to write bad image to disk: {e}"));
    }
    res
}

pub fn load_png_or_jpg_rotated_image(img: &RotatedImageRef, cx: &mut Cx, data: &[u8]) -> Result<(), ImageError> {
    fn retain_aspect_ratio(width: u32, height: u32) -> (f32, f32) {
        let aspect_ratio: f32 = width as f32 / height as f32;
        let (mut capped_width, mut capped_height) = (width, height);
        if capped_height > ROTATED_IMAGE_MAX_SIZE {
            capped_height = ROTATED_IMAGE_MAX_SIZE;
            capped_width = (capped_height as f32 * aspect_ratio).floor() as u32;
        }
        if capped_width > ROTATED_IMAGE_MAX_SIZE {
            capped_width = ROTATED_IMAGE_MAX_SIZE;
            capped_height = (capped_width as f32 / aspect_ratio).floor() as u32;
        }
        (capped_width as f32, capped_height as f32)
    }
    match imghdr::from_bytes(data) {
        Some(imghdr::Type::Png) => {
            let buffer = ImageBuffer::from_png(data)?;
            let (width, height) = (buffer.width, buffer.height);
            let texture = buffer.into_new_texture(cx);
            img.set_texture(cx, Some(texture));
            let (capped_width, capped_height) = retain_aspect_ratio(width as u32, height as u32);
            img.apply_over(cx, live!{
                width: (capped_width as f32), height: (capped_height as f32)
            });
        },
        Some(imghdr::Type::Jpeg) => {
            let buffer = ImageBuffer::from_jpg(data)?;
            let (width, height) = (buffer.width, buffer.height);
             let texture = buffer.into_new_texture(cx);
            img.set_texture(cx, Some(texture));
            let (capped_width, capped_height) = retain_aspect_ratio(width as u32, height as u32);
            img.apply_over(cx, live!{
                width: (capped_width as f32), height: (capped_height as f32)
            });
        },
        Some(_unsupported) => {
            // Attempt to load it as a PNG or JPEG anyway, since imghdr isn't perfect.
            return Err(ImageError::UnsupportedFormat);
        }
        None => {
            // Attempt to load it as a PNG or JPEG anyway, since imghdr isn't perfect.
            return Err(ImageError::UnsupportedFormat);
        }
    };
    Ok(())
}

/// Returns a width and height pair that has been capped to the maximum allowed size, while retaining the aspect ratio of the original image.
///
/// The maximum allowed size is defined by the `ROTATED_IMAGE_MAX_SIZE` constant.
///
/// This function is used to ensure that images displayed in the image viewer do not exceed the maximum allowed size, while still maintaining their aspect ratio.
pub fn retain_aspect_ratio(width: u32, height: u32) -> (f64, f64) {
    let aspect_ratio: f64 = width as f64 / height as f64;
    let (mut capped_width, mut capped_height) = (width, height);
    
    if width <= ROTATED_IMAGE_MAX_SIZE && height <= ROTATED_IMAGE_MAX_SIZE {
        capped_height = ROTATED_IMAGE_MAX_SIZE;
        capped_width = (capped_height as f64 * aspect_ratio).floor() as u32;
    } else {
        if capped_height > ROTATED_IMAGE_MAX_SIZE {
            capped_height = ROTATED_IMAGE_MAX_SIZE;
            capped_width = (capped_height as f64 * aspect_ratio).floor() as u32;
        }
        if capped_width > ROTATED_IMAGE_MAX_SIZE {
            capped_width = ROTATED_IMAGE_MAX_SIZE;
            capped_height = (capped_width as f64 / aspect_ratio).floor() as u32;
        }
    }
    (capped_width as f64, capped_height as f64)
}

pub fn get_png_or_jpg_image_buffer(data: Vec<u8>) -> Result<ImageBuffer, ImageError> {
    match imghdr::from_bytes(&data) {
        Some(imghdr::Type::Png) => {
            ImageBuffer::from_png(&data)
        },
        Some(imghdr::Type::Jpeg) => {
            ImageBuffer::from_jpg(&data)
        },
        Some(_unsupported) => {
            // Attempt to load it as a PNG or JPEG anyway, since imghdr isn't perfect.
            Err(ImageError::UnsupportedFormat)
        }
        None => {
            // Attempt to load it as a PNG or JPEG anyway, since imghdr isn't perfect.
            Err(ImageError::UnsupportedFormat)
        }
    }
}

/// Creates a texture from JPEG image data, similar to `load_jpg_from_data` but returns a Texture.
///
/// Returns an error if the image fails to load or if the format is unsupported.
pub fn create_texture_from_jpg_data(cx: &mut Cx, data: &[u8]) -> Result<Texture, ImageError> {
    match ImageBuffer::from_jpg(data) {
        Ok(image_buffer) => {
            Ok(image_buffer.into_new_texture(cx))
        }
        Err(err) => {
            Err(err)
        }
    }
}

/// Creates a texture from PNG image data, similar to `load_png_from_data` but returns a Texture.
///
/// Returns an error if the image fails to load or if the format is unsupported.
pub fn create_texture_from_png_data(cx: &mut Cx, data: &[u8]) -> Result<Texture, ImageError> {
    match ImageBuffer::from_png(data) {
        Ok(image_buffer) => {
            Ok(image_buffer.into_new_texture(cx))
        }
        Err(err) => {
            Err(err)
        }
    }
}

/// Creates a texture from either PNG or JPEG image data, automatically detecting the format.
///
/// Uses the `imghdr` library to determine the image format and creates the appropriate texture.
/// Returns an error if either load fails or if the image format is unknown.
pub fn create_texture_from_png_or_jpg_data(cx: &mut Cx, data: &[u8]) -> Result<Texture, ImageError> {
    fn attempt_both(cx: &mut Cx, data: &[u8]) -> Result<Texture, ImageError> {
        create_texture_from_png_data(cx, data)
            .or_else(|_| create_texture_from_jpg_data(cx, data))
    }

    let res = match imghdr::from_bytes(data) {
        Some(imghdr::Type::Png) => create_texture_from_png_data(cx, data),
        Some(imghdr::Type::Jpeg) => create_texture_from_jpg_data(cx, data),
        Some(unsupported) => {
            // Attempt to load it as a PNG or JPEG anyway, since imghdr isn't perfect.
            attempt_both(cx, data).map_err(|_| {
                error!("create_texture_from_png_or_jpg_data(): The {unsupported:?} image format is unsupported");
                ImageError::UnsupportedFormat
            })
        }
        None => {
            // Attempt to load it as a PNG or JPEG anyway, since imghdr isn't perfect.
            attempt_both(cx, data).map_err(|_| {
                error!("create_texture_from_png_or_jpg_data(): Unknown image format");
                ImageError::UnsupportedFormat
            })
        }
    };
    if let Err(err) = res.as_ref() {
        // debugging: dump out the bad image to disk
        let mut path = crate::temp_storage::get_temp_dir_path().clone();
        let filename = format!(
            "img_{}",
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or_else(|_| rand::random::<u128>()),
        );
        path.push(filename);
        path.set_extension("unknown");
        error!("Failed to create texture from PNG/JPG: {err}. Dumping bad image: {:?}", path);
        let _ = std::fs::write(path, data)
            .inspect_err(|e| error!("Failed to write bad image to disk: {e}"));
    }
    res
}


pub fn unix_time_millis_to_datetime(millis: MilliSecondsSinceUnixEpoch) -> Option<DateTime<Local>> {
    let millis: i64 = millis.get().into();
    Local.timestamp_millis_opt(millis).single()
}

/// Returns a string error message, handling special cases related to joining/leaving rooms.
pub fn stringify_join_leave_error(
    error: &matrix_sdk::Error,
    room_name: Option<&str>,
    was_join: bool,
    was_invite: bool,
) -> String {
    let room_str = room_name.map_or_else(
        || String::from("room"),
        |r| format!("\"{r}\""),
    );
    let msg_opt = match error {
        // The below is a stupid hack to workaround `WrongRoomState` being private.
        // We get the string representation of the error and then search for the "got" state.
        matrix_sdk::Error::WrongRoomState(wrs) => {
            if was_join && wrs.to_string().contains(", got: Joined") {
                Some(format!("Failed to join {room_str}: it has already been joined."))
            } else if !was_join && wrs.to_string().contains(", got: Left") {
                Some(format!("Failed to leave {room_str}: it has already been left."))
            } else {
                None
            }
        }
        // Special case for 404 errors, which indicate the room no longer exists.
        // This avoids the weird "no known servers" error, which is misleading and incorrect.
        // See: <https://github.com/element-hq/element-web/issues/25627>.
        matrix_sdk::Error::Http(error)
            if error.as_client_api_error().is_some_and(|e| e.status_code.as_u16() == 404) =>
        {
            Some(format!(
                "Failed to {} {room_str}: the room no longer exists on the server.{}",
                if was_join { "join" } else { "leave" },
                if was_join && was_invite { "\n\nYou may safely reject this invite." } else { "" },
            ))
        }
        _ => None,
    };
    msg_opt.unwrap_or_else(|| format!(
        "Failed to {} {}: {}",
        match (was_join, was_invite) {
            (true, true) => "accept invite to",
            (true, false) => "join",
            (false, true) => "reject invite to",
            (false, false) => "leave",
        },
        room_str,
        error
    ))
}

/// Returns a string error message for pagination errors,
/// handling special cases related to common pagination errors, e.g., timeouts.
pub fn stringify_pagination_error(
    error: &matrix_sdk_ui::timeline::Error,
    room_name: &str,
) -> String {
    use matrix_sdk::{paginators::PaginatorError, event_cache::EventCacheError};
    use matrix_sdk_ui::timeline::Error as TimelineError;

    #[allow(clippy::single_match)]
    let match_paginator_error = |paginator_error: &PaginatorError| {
        match paginator_error {
            PaginatorError::SdkError(sdk_error) => match sdk_error.deref() {
                matrix_sdk::Error::Http(http_error) => match http_error.deref() {
                    matrix_sdk::HttpError::Reqwest(reqwest_error) if reqwest_error.is_timeout() => {
                        return Some(format!("Failed to load earlier messages in \"{room_name}\": request timed out."));
                    }
                    _ => {}
                }
                _ => {}
            }
            _ => {}
        }
        None
    };

    match error {
        TimelineError::PaginationError(PaginationError::NotSupported) => {
            return format!("Failed to load earlier messages in \"{room_name}\": \
                pagination is not supported in this timeline focus mode.");
        }
        TimelineError::PaginationError(PaginationError::Paginator(paginator_error)) => {
            if let Some(message) = match_paginator_error(paginator_error) {
                return message;
            }
        }
        TimelineError::EventCacheError(EventCacheError::BackpaginationError(error)) => {
            return format!("Failed to load earlier messages in \"{room_name}\": \
                Back-pagination error in the event cache: {error}.");
        }
        _ => {}
    }
    format!("Failed to load earlier messages in \"{room_name}\": {error}")
}


/// Returns a string representation of the room name or ID.
pub fn room_name_or_id(
    room_name: Option<impl Into<String>>,
    room_id: impl AsRef<RoomId>,
) -> String {
    room_name.map_or_else(
        || format!("Room ID {}", room_id.as_ref()),
        |name| name.into(),
    )
}

/// Formats a given Unix timestamp in milliseconds into a relative human-readable date.
///
/// # Cases:
/// - **Less than 60 seconds ago**: Returns `"Just now"`.
/// - **Less than 60 minutes ago**: Returns `"X minutes ago"`, where X is the number of minutes.
/// - **Same day**: Returns `"HH:MM"` (current time format for today).
/// - **Yesterday**: Returns `"Yesterday at HH:MM"` for messages from the previous day.
/// - **Within the past week**: Returns the name of the day (e.g., "Tuesday").
/// - **Older than a week**: Returns `"DD/MM/YY"` as the absolute date.
///
/// # Arguments:
/// - `millis`: The Unix timestamp in milliseconds to format.
///
/// # Returns:
/// - `Option<String>` representing the human-readable time or `None` if formatting fails.
pub fn relative_format(millis: MilliSecondsSinceUnixEpoch) -> Option<String> {
    let datetime = unix_time_millis_to_datetime(millis)?;

    // Calculate the time difference between now and the given timestamp
    let now = Local::now();
    let duration = now - datetime;

    // Handle different time ranges and format accordingly
    if duration < Duration::seconds(60) {
        Some("Now".to_string())
    } else if duration < Duration::minutes(60) {
        let minutes_text = if duration.num_minutes() == 1 { "min" } else { "mins" };
        Some(format!("{} {} ago", duration.num_minutes(), minutes_text))
    } else if duration < Duration::hours(24) && now.date_naive() == datetime.date_naive() {
        Some(format!("{}", datetime.format("%H:%M"))) // "HH:MM" format for today
    } else if duration < Duration::hours(48) {
        if let Some(yesterday) = now.date_naive().succ_opt() {
            if yesterday == datetime.date_naive() {
                return Some(format!("Yesterday at {}", datetime.format("%H:%M")));
            }
        }
        Some(format!("{}", datetime.format("%A"))) // Fallback to day of the week if not yesterday
    } else if duration < Duration::weeks(1) {
        Some(format!("{}", datetime.format("%A"))) // Day of the week (e.g., "Tuesday")
    } else {
        Some(format!("{}", datetime.format("%F"))) // "YYYY-MM-DD" format for older messages
    }
}

/// Returns the first "letter" (Unicode grapheme) of given user name,
/// skipping any leading "@" characters.
pub fn user_name_first_letter(user_name: &str) -> Option<&str> {
    use unicode_segmentation::UnicodeSegmentation;
    user_name
        .graphemes(true)
        .find(|&g| g != "@")
}


/// A const-compatible version of [`MediaFormat`].
#[derive(Clone, Debug)]
pub enum MediaFormatConst {
    /// The file that was uploaded.
    File,
    /// A thumbnail of the file that was uploaded.
    Thumbnail(MediaThumbnailSettingsConst),
}
impl From<MediaFormatConst> for MediaFormat {
    fn from(constant: MediaFormatConst) -> Self {
        match constant {
            MediaFormatConst::File => Self::File,
            MediaFormatConst::Thumbnail(size) => Self::Thumbnail(size.into()),
        }
    }
}

/// A const-compatible version of [`MediaThumbnailSettings`].
#[derive(Clone, Debug)]
pub struct MediaThumbnailSettingsConst {
    /// The desired resizing method.
    pub method: Method,
    /// The desired width of the thumbnail. The actual thumbnail may not match
    /// the size specified.
    pub width: u32,
    /// The desired height of the thumbnail. The actual thumbnail may not match
    /// the size specified.
    pub height: u32,
    /// If we want to request an animated thumbnail from the homeserver.
    ///
    /// If it is `true`, the server should return an animated thumbnail if
    /// the media supports it.
    ///
    /// Defaults to `false`.
    pub animated: bool,
}
impl From<MediaThumbnailSettingsConst> for MediaThumbnailSettings {
    fn from(constant: MediaThumbnailSettingsConst) -> Self {
        Self {
            method: constant.method,
            width: constant.width.into(),
            height: constant.height.into(),
            animated: constant.animated,
        }
    }
}


/// The thumbnail format to use for user and room avatars.
pub const AVATAR_THUMBNAIL_FORMAT: MediaFormatConst = MediaFormatConst::Thumbnail(
    MediaThumbnailSettingsConst {
        method: Method::Scale,
        width: 40,
        height: 40,
        animated: false,
    }
);

/// The thumbnail format to use for regular media images.
pub const MEDIA_THUMBNAIL_FORMAT: MediaFormatConst = MediaFormatConst::Thumbnail(
    MediaThumbnailSettingsConst {
        method: Method::Scale,
        width: 400,
        height: 400,
        animated: false,
    }
);

/// Removes leading whitespace and HTML whitespace tags (`<p>` and `<br>`) from the given `text`.
pub fn trim_start_html_whitespace(mut text: &str) -> &str {
    let mut prev_text_len = text.len();
    loop {
        text = text
            .trim_start_matches("<p>")
            .trim_start_matches("<br>")
            .trim_start_matches("<br/>")
            .trim_start_matches("<br />")
            .trim_start();

        if text.len() == prev_text_len {
            break;
        }
        prev_text_len = text.len();
    }
    text
}

/// Looks for bare links in the given `text` and converts them into proper HTML links.
///
/// If `links_found` is provided, it will be populated with the list of URLs found in the text.
pub fn linkify_get_urls<'t>(
    text: &'t str,
    is_html: bool,
    mut links_found: Option<&mut Vec<Url>>,
) -> Cow<'t, str> {
    const MAILTO: &str = "mailto:";

    use linkify::{Link, LinkFinder, LinkKind};
    let mut links = LinkFinder::new()
        .links(text)
        .peekable();
    if links.peek().is_none() {
        return Cow::Borrowed(text);
    }

    // A closure to escape text if it's not HTML.
    let escaped = |text| {
        if is_html {
            Cow::from(text)
        } else {
            htmlize::escape_text(text)
        }
    };

    let mut linkified_text = String::new();
    let mut last_end_index = 0;
    for link in links {
        let link_txt = link.as_str();

        // Only linkify the URL if it's not already part of an HTML or mailto href attribute.
        let is_link_within_href_attr = text.get(.. link.start())
            .is_some_and(ends_with_href);
        let is_link_within_html_tag = |link: &Link| {
            text.get(link.end() ..)
                .is_some_and(|after| after.trim_end().starts_with("</a>"))
        };
        let is_mailto_link_within_href_attr = |link: &Link| {
            if !matches!(link.kind(), LinkKind::Email) { return false; }
            let mailto_start = link.start().saturating_sub(MAILTO.len());
            text.get(mailto_start .. link.start())
                .is_some_and(|t| t == MAILTO)
                .then(|| text.get(.. mailto_start))
                .flatten()
                .is_some_and(ends_with_href)
        };

        if is_link_within_href_attr
            || is_link_within_html_tag(&link)
            || is_mailto_link_within_href_attr(&link)
        {
            linkified_text = format!(
                "{linkified_text}{}",
                text.get(last_end_index..link.end()).unwrap_or_default(),
            );
            if let Some(links_found) = links_found.as_mut() {
                if let Ok(url) = Url::parse(link_txt) {
                    links_found.push(url);
                }
            }
        } else {
            match link.kind() {
                LinkKind::Url => {
                    linkified_text = format!(
                        "{linkified_text}{}<a href=\"{}\">{}</a>",
                        escaped(text.get(last_end_index..link.start()).unwrap_or_default()),
                        htmlize::escape_attribute(link_txt),
                        htmlize::escape_text(link_txt),
                    );
                    if let Some(links_found) = links_found.as_mut() {
                        if let Ok(url) = Url::parse(link_txt) {
                            links_found.push(url);
                        }
                    }
                }
                LinkKind::Email => {
                    linkified_text = format!(
                        "{linkified_text}{}<a href=\"mailto:{}\">{}</a>",
                        escaped(text.get(last_end_index..link.start()).unwrap_or_default()),
                        htmlize::escape_attribute(link_txt),
                        htmlize::escape_text(link_txt),
                    );
                }
                _ => return Cow::Borrowed(text), // unreachable
            }
        }
        last_end_index = link.end();
    }
    linkified_text.push_str(
        &escaped(text.get(last_end_index..).unwrap_or_default())
    );
    Cow::Owned(linkified_text)
}

/// Looks for bare links in the given `text` and converts them into proper HTML links.
///
/// To obtain the list of found URLs, use [`linkify_get_urls()`] instead.
pub fn linkify(text: &str, is_html: bool) -> Cow<'_, str> {
    linkify_get_urls(text, is_html, None)
}

/// Returns true if the given `text` string ends with a valid href attribute opener.
///
/// An href attribute looks like this: `href="http://example.com"`,.
/// so we look for `href="` at the end of the given string.
///
/// Spaces are allowed to exist in between the `href`, `=`, and `"`.
/// In addition, the quotation mark is optional, and can be either a single or double quote,
/// so this function takes those into account as well.
pub fn ends_with_href(text: &str) -> bool {
    // let mut idx = text.len().saturating_sub(1);
    let mut substr = text.trim_end();
    // Search backwards for a single quote, double quote, or an equals sign.
    match substr.as_bytes().last() {
        Some(b'\'' | b'"') => {
            if substr
                .get(.. substr.len().saturating_sub(1))
                .map(|s| {
                    substr = s.trim_end();
                    substr.as_bytes().last() == Some(&b'=')
                })
                .unwrap_or(false)
            {
                substr = &substr[..substr.len().saturating_sub(1)];
            } else {
                return false;
            }
        }
        Some(b'=') => {
            substr = &substr[..substr.len().saturating_sub(1)];
        }
        _ => return false,
    }

    // Now we have found the equals sign, so search backwards for the `href` attribute.
    substr.trim_end().ends_with("href")
}

/// Converts a list of names into a human-readable string with a limit parameter.
///
/// # Examples
/// ```
/// assert_eq!(human_readable_list(&vec!["Alice"], 3), String::from("Alice"));
/// assert_eq!(human_readable_list(&vec![String::from("Alice"), String::from("Bob")], 3), String::from("Alice and Bob"));
/// assert_eq!(human_readable_list(&vec!["Alice", "Bob", "Charlie"], 3), String::from("Alice, Bob and Charlie"));
/// assert_eq!(human_readable_list(&vec!["Alice", "Bob", "Charlie", "Dennis", "Eudora", "Fanny"], 3), String::from("Alice, Bob, Charlie, and 3 others"));
/// ```
pub fn human_readable_list<S>(names: &[S], limit: usize) -> String
where
    S: AsRef<str>
{
    let mut result = String::new();
    match names.len() {
        0 => return result, // early return if no names provided
        1 => {
            result.push_str(names[0].as_ref());
        },
        2 => {
            result.push_str(names[0].as_ref());
            result.push_str(" and ");
            result.push_str(names[1].as_ref());
        },
        _ => {
            let display_count = names.len().min(limit);
            for (i, name) in names.iter().take(display_count - 1).enumerate() {
                if i > 0 {
                    result.push_str(", ");
                }
                result.push_str(name.as_ref());
            }
            if names.len() > limit {
                let remaining = names.len() - limit;
                result.push_str(", ");
                result.push_str(names[display_count - 1].as_ref());
                result.push_str(", and ");
                if remaining == 1 {
                    result.push_str("1 other");
                } else {
                    result.push_str(&format!("{} others", remaining));
                }
            } else {
                result.push_str(" and ");
                result.push_str(names[display_count - 1].as_ref());
            }
        }
    };
    result
}


/// Returns the sender's display name if available.
///
/// If not available, and if the `room_id` is provided, this function will
/// submit an async request to fetch the event details.
/// In this case, this will return the event sender's user ID as a string.
pub fn get_or_fetch_event_sender(
    event_tl_item: &EventTimelineItem,
    room_id: Option<&OwnedRoomId>,
) -> String {
    let sender_username = match event_tl_item.sender_profile() {
        TimelineDetails::Ready(profile) => profile.display_name.as_deref(),
        TimelineDetails::Unavailable => {
            if let Some(room_id) = room_id {
                if let Some(event_id) = event_tl_item.event_id() {
                    submit_async_request(MatrixRequest::FetchDetailsForEvent {
                        room_id: room_id.clone(),
                        event_id: event_id.to_owned(),
                    });
                }
            }
            None
        }
        _ => None,
    }
    .unwrap_or_else(|| event_tl_item.sender().as_str());
    sender_username.to_owned()
}

/// Converts a byte index in a string to the corresponding grapheme index
pub fn byte_index_to_grapheme_index(text: &str, byte_idx: usize) -> usize {
    let mut current_byte_pos = 0;
    for (i, g) in text.graphemes(true).enumerate() {
        if current_byte_pos <= byte_idx && current_byte_pos + g.len() > byte_idx {
            return i;
        }
        current_byte_pos += g.len();
    }
    // If byte_idx is at end of string or past it, return grapheme count
    text.graphemes(true).count()
}

/// Safely extracts a substring between two byte indices, ensuring proper
/// grapheme boundaries are respected
pub fn safe_substring_by_byte_indices(text: &str, start_byte: usize, end_byte: usize) -> String {
    if start_byte >= end_byte || start_byte >= text.len() {
        return String::new();
    }

    let start_grapheme_idx = byte_index_to_grapheme_index(text, start_byte);
    let end_grapheme_idx = byte_index_to_grapheme_index(text, end_byte);

    text.graphemes(true)
        .enumerate()
        .filter(|(i, _)| *i >= start_grapheme_idx && *i < end_grapheme_idx)
        .map(|(_, g)| g)
        .collect()
}

/// Safely replaces text between byte indices with a new string,
/// ensuring proper grapheme boundaries are respected
pub fn safe_replace_by_byte_indices(text: &str, start_byte: usize, end_byte: usize, replacement: &str) -> String {
    let text_graphemes: Vec<&str> = text.graphemes(true).collect();

    let start_grapheme_idx = byte_index_to_grapheme_index(text, start_byte);
    let end_grapheme_idx = byte_index_to_grapheme_index(text, end_byte);

    let before = text_graphemes[..start_grapheme_idx].join("");
    let after = text_graphemes[end_grapheme_idx..].join("");

    format!("{before}{replacement}{after}")
}

/// Builds a mapping array from graphemes to byte positions in the string
pub fn build_grapheme_byte_positions(text: &str) -> Vec<usize> {
    let mut positions = Vec::with_capacity(text.graphemes(true).count() + 1);
    let mut byte_pos = 0;

    positions.push(0);

    for g in text.graphemes(true) {
        byte_pos += g.len();
        positions.push(byte_pos);
    }

    positions
}

/// A RON-(de)serializable wrapper around [`OwnedRoomId`].
#[derive(Clone, Debug)]
pub struct OwnedRoomIdRon(pub OwnedRoomId);
impl SerRon for OwnedRoomIdRon {
    /// Serialize a `OwnedRoomId` to its string form, using ron.
    fn ser_ron(&self, d: usize, s: &mut SerRonState) {
        self.0.to_string().ser_ron(d, s);
    }
}
impl DeRon for OwnedRoomIdRon {
    fn de_ron(s: &mut DeRonState, i: &mut Chars) -> Result<Self, DeRonErr> {
        OwnedRoomId::from_str(&String::de_ron(s, i)?)
            .map(OwnedRoomIdRon)
            .map_err(|e| DeRonErr {
                msg: e.to_string(),
                line: s.line,
                col: s.col,
            })
    }
}
impl From<OwnedRoomId> for OwnedRoomIdRon {
    fn from(room_id: OwnedRoomId) -> Self {
        OwnedRoomIdRon(room_id)
    }
}
impl<'a> From<&'a OwnedRoomIdRon> for &'a OwnedRoomId {
    fn from(room_id: &'a OwnedRoomIdRon) -> Self {
        &room_id.0
    }
}
impl From<OwnedRoomIdRon> for OwnedRoomId {
    fn from(room_id: OwnedRoomIdRon) -> Self {
        room_id.0
    }
}
impl<'a> From<&'a OwnedRoomIdRon> for &'a RoomId {
    fn from(room_id: &'a OwnedRoomIdRon) -> Self {
        &room_id.0
    }
}
impl AsRef<RoomId> for OwnedRoomIdRon {
    fn as_ref(&self) -> &RoomId {
        &self.0
    }
}
impl Deref for OwnedRoomIdRon {
    type Target = OwnedRoomId;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl Display for OwnedRoomIdRon {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Returns a text avatar string containing the first character of the room name.
///
/// Skips the first character if it is a `#` or `!`, the sigils used for Room aliases and Room IDs.
pub fn avatar_from_room_name(room_name: Option<&str>) -> FetchedRoomAvatar {
    let first = room_name.and_then(|rn| rn
        .graphemes(true)
        .find(|&g| g != "#" && g != "!")
        .map(ToString::to_string)
    ).unwrap_or_else(|| String::from("?"));
    FetchedRoomAvatar::Text(first)
}

/// Returns a human-readable string describing the given ImageViewerError.
///
/// This can be used to update the label text of an error display.
///
/// The error type is matched against a string which describes the error in a way that is visible to the user.
///
/// The strings returned by this function will be appropriate for display in a label or similar widget.
pub fn image_viewer_error_to_string(error: &ImageViewerError) -> &str {
    match error {
        ImageViewerError::NotFound => "Full image is not found",
        ImageViewerError::BadData => "Image appears to be empty or corrupted",
        ImageViewerError::UnsupportedFormat => "This image format isn't supported",
        ImageViewerError::ConnectionFailed => "Check your internet connection",
        ImageViewerError::Unauthorized => "You don't have permission to view this image",
        ImageViewerError::ServerError => "Server temporarily unavailable",
        ImageViewerError::Unknown => "Unable to load image",
        ImageViewerError::Timeout => "Timed out loading this image",
    }
}

#[cfg(test)]
mod tests_human_readable_list {
    use super::*;
    #[test]
    fn test_human_readable_list_empty() {
        let names: Vec<&str> = Vec::new();
        let result = human_readable_list(&names, 3);
        assert_eq!(result, "");
    }

    #[test]
    fn test_human_readable_list_single() {
        let names: Vec<&str> = vec!["Alice"];
        let result = human_readable_list(&names, 3);
        assert_eq!(result, "Alice");
    }

    #[test]
    fn test_human_readable_list_two() {
        let names: Vec<&str> = vec!["Alice", "Bob"];
        let result = human_readable_list(&names, 3);
        assert_eq!(result, "Alice and Bob");
    }

    #[test]
    fn test_human_readable_list_many() {
        let names: Vec<&str> = vec!["Alice", "Bob", "Charlie", "David"];
        let result = human_readable_list(&names, 3);
        assert_eq!(result, "Alice, Bob, Charlie, and 1 other");
    }

    #[test]
    fn test_human_readable_list_long() {
        let names: Vec<&str> = vec!["Alice", "Bob", "Charlie", "Dennis", "Eudora", "Fanny", "Gina", "Hiroshi", "Ivan", "James", "Karen", "Lisa", "Michael", "Nathan", "Oliver", "Peter", "Quentin", "Rachel", "Sally", "Tanya", "Ulysses", "Victor", "William", "Xenia", "Yuval", "Zachariah"];
        let result = human_readable_list(&names, 3);
        assert_eq!(result, "Alice, Bob, Charlie, and 23 others");
    }
}

#[cfg(test)]
mod tests_linkify {
    use super::*;

    #[test]
    fn test_linkify0() {
        let text = "Hello, world!";
        assert_eq!(linkify(text, false).as_ref(), text);
    }

    #[test]
    fn test_linkify1() {
        let text = "Check out this website: https://example.com";
        let expected = "Check out this website: <a href=\"https://example.com\">https://example.com</a>";
        let actual = linkify(text, false);
        println!("{:?}", actual.as_ref());
        assert_eq!(actual.as_ref(), expected);
    }

    #[test]
    fn test_linkify2() {
        let text = "Send an email to john@example.com";
        let expected = "Send an email to <a href=\"mailto:john@example.com\">john@example.com</a>";
        let actual = linkify(text, false);
        println!("{:?}", actual.as_ref());
        assert_eq!(actual.as_ref(), expected);
    }

    #[test]
    fn test_linkify3() {
        let text = "Visit our website at www.example.com";
        assert_eq!(linkify(text, false).as_ref(), text);
    }

    #[test]
    fn test_linkify4() {
        let text = "Link 1 http://google.com Link 2 https://example.com";
        let expected = "Link 1 <a href=\"http://google.com\">http://google.com</a> Link 2 <a href=\"https://example.com\">https://example.com</a>";
        let actual = linkify(text, false);
        println!("{:?}", actual.as_ref());
        assert_eq!(actual.as_ref(), expected);
    }


    #[test]
    fn test_linkify5() {
        let text = "html test <a href=http://google.com>Link title</a> Link 2 https://example.com";
        let expected = "html test <a href=http://google.com>Link title</a> Link 2 <a href=\"https://example.com\">https://example.com</a>";
        let actual = linkify(text, true);
        println!("{:?}", actual.as_ref());
        assert_eq!(actual.as_ref(), expected);
    }

    #[test]
    fn test_linkify6() {
        let text = "<a href=http://google.com>link title</a>";
        assert_eq!(linkify(text, true).as_ref(), text);
    }

    #[test]
    fn test_linkify7() {
        let text = "https://example.com";
        let expected = "<a href=\"https://example.com\">https://example.com</a>";
        assert_eq!(linkify(text, false).as_ref(), expected);
    }

    #[test]
    fn test_linkify8() {
        let text = "test test https://crates.io/crates/cargo-packager test test";
        let expected = "test test <a href=\"https://crates.io/crates/cargo-packager\">https://crates.io/crates/cargo-packager</a> test test";
        assert_eq!(linkify(text, false).as_ref(), expected);
    }

    #[test]
    fn test_linkify9() {
        let text = "<mx-reply><blockquote><a href=\"https://matrix.to/#/!ifW4td0it0scmZpEM6:computer.surgery/$GwDzIlPzNgxhJ2QCIsmcPMC-sHdoKNsb0g2MS1psyyM?via=matrix.org&via=mozilla.org&via=gitter.im\">In reply to</a> <a href=\"https://matrix.to/#/@spore:mozilla.org\">@spore:mozilla.org</a><br />So I asked if there's a crate for it (bc I don't have the time to test and debug it) or if there's simply a better way that involves less states and invariants</blockquote></mx-reply>https://docs.rs/aho-corasick/latest/aho_corasick/struct.AhoCorasick.html#method.stream_find_iter";

        let expected = "<mx-reply><blockquote><a href=\"https://matrix.to/#/!ifW4td0it0scmZpEM6:computer.surgery/$GwDzIlPzNgxhJ2QCIsmcPMC-sHdoKNsb0g2MS1psyyM?via=matrix.org&via=mozilla.org&via=gitter.im\">In reply to</a> <a href=\"https://matrix.to/#/@spore:mozilla.org\">@spore:mozilla.org</a><br />So I asked if there's a crate for it (bc I don't have the time to test and debug it) or if there's simply a better way that involves less states and invariants</blockquote></mx-reply><a href=\"https://docs.rs/aho-corasick/latest/aho_corasick/struct.AhoCorasick.html#method.stream_find_iter\">https://docs.rs/aho-corasick/latest/aho_corasick/struct.AhoCorasick.html#method.stream_find_iter</a>";
        assert_eq!(linkify(text, true).as_ref(), expected);
    }

    #[test]
    fn test_linkify10() {
        let text = "And then call <a href=\"https://doc.rust-lang.org/std/io/trait.BufRead.html#method.read_until\"><code>read_until</code></a> or other <code>BufRead</code> methods.";
        let expected = "And then call <a href=\"https://doc.rust-lang.org/std/io/trait.BufRead.html#method.read_until\"><code>read_until</code></a> or other <code>BufRead</code> methods.";
        assert_eq!(linkify(text, true).as_ref(), expected);
    }


    #[test]
    fn test_linkify11() {
        let text = "And then https://google.com call <a href=\"https://doc.rust-lang.org/std/io/trait.BufRead.html#method.read_until\"><code>read_until</code></a> or other <code>BufRead</code> methods.";
        let expected = "And then <a href=\"https://google.com\">https://google.com</a> call <a href=\"https://doc.rust-lang.org/std/io/trait.BufRead.html#method.read_until\"><code>read_until</code></a> or other <code>BufRead</code> methods.";
        assert_eq!(linkify(text, true).as_ref(), expected);
    }

    #[test]
    fn test_linkify12() {
        let text = "And then https://google.com call <a href=\"https://doc.rust-lang.org/std/io/trait.BufRead.html#method.read_until\"><code>read_until</code></a> or other <code>BufRead http://another-link.http.com </code> methods.";
        let expected = "And then <a href=\"https://google.com\">https://google.com</a> call <a href=\"https://doc.rust-lang.org/std/io/trait.BufRead.html#method.read_until\"><code>read_until</code></a> or other <code>BufRead <a href=\"http://another-link.http.com\">http://another-link.http.com</a> </code> methods.";
        assert_eq!(linkify(text, true).as_ref(), expected);
    }

    #[test]
    fn test_linkify13() {
        let text = "Check out this website: <a href=\"https://example.com\">https://example.com</a>";
        let expected = "Check out this website: <a href=\"https://example.com\">https://example.com</a>";
        assert_eq!(linkify(text, true).as_ref(), expected);
    }

    #[test]
    fn test_linkify14() {
        let text = "<p>If you have any questions please drop us an email to <a href=\"mailto:legal@matrix.org\">legal@matrix.org</a></p>";
        let expected = text;
        assert_eq!(linkify(text, true).as_ref(), expected);
    }

    #[test]
    fn test_linkify15() {
        let text = "If you have any questions please drop us an email to:legal@matrix.org";
        let expected = "If you have any questions please drop us an email to:<a href=\"mailto:legal@matrix.org\">legal@matrix.org</a>";
        assert_eq!(linkify(text, false).as_ref(), expected);
    }
}

#[cfg(test)]
mod tests_ends_with_href {
    use super::*;

    #[test]
    fn test_ends_with_href0() {
        assert!(ends_with_href("href=\""));
    }

    #[test]
    fn test_ends_with_href1() {
        assert!(ends_with_href("href = \""));
    }

    #[test]
    fn test_ends_with_href2() {
        assert!(ends_with_href("href  =  \""));
    }

    #[test]
    fn test_ends_with_href3() {
        assert!(ends_with_href("href='"));
    }

    #[test]
    fn test_ends_with_href4() {
        assert!(ends_with_href("href = '"));
    }

    #[test]
    fn test_ends_with_href5() {
        assert!(ends_with_href("href  =  '"));
    }

    #[test]
    fn test_ends_with_href6() {
        assert!(ends_with_href("href="));
    }

    #[test]
    fn test_ends_with_href7() {
        assert!(ends_with_href("href ="));
    }

    #[test]
    fn test_ends_with_href8() {
        assert!(ends_with_href("href  =  "));
    }

    #[test]
    fn test_ends_with_href9() {
        assert!(!ends_with_href("href"));
    }

    #[test]
    fn test_ends_with_href10() {
        assert!(ends_with_href("href ="));
    }

    #[test]
    fn test_ends_with_href11() {
        assert!(!ends_with_href("href  ==  "));
    }

    #[test]
    fn test_ends_with_href12() {
        assert!(ends_with_href("href =\""));
    }

    #[test]
    fn test_ends_with_href13() {
        assert!(ends_with_href("href = '"));
    }

    #[test]
    fn test_ends_with_href14() {
        assert!(ends_with_href("href ="));
    }

    #[test]
    fn test_ends_with_href15() {
        assert!(!ends_with_href("href =a"));
    }

    #[test]
    fn test_ends_with_href16() {
        assert!(!ends_with_href("href '="));
    }

    #[test]
    fn test_ends_with_href17() {
        assert!(!ends_with_href("href =''"));
    }

    #[test]
    fn test_ends_with_href18() {
        assert!(!ends_with_href("href =\"\""));
    }

    #[test]
    fn test_ends_with_href19() {
        assert!(!ends_with_href("hrf="));
    }

    #[test]
    fn test_ends_with_href20() {
        assert!(ends_with_href(" href = \""));
    }

    #[test]
    fn test_ends_with_href21() {
        assert!(ends_with_href("href = \" "));
    }

    #[test]
    fn test_ends_with_href22() {
        assert!(ends_with_href(" href = \" "));
    }

    #[test]
    fn test_ends_with_href23() {
        assert!(ends_with_href("href = ' "));
    }

    #[test]
    fn test_ends_with_href24() {
        assert!(ends_with_href(" href = ' "));
    }

    #[test]
    fn test_ends_with_href25() {
        assert!(ends_with_href("href = "));
    }

    #[test]
    fn test_ends_with_href26() {
        assert!(ends_with_href(" href = "));
    }

    #[test]
    fn test_ends_with_href27() {
        assert!(ends_with_href("href =\" "));
    }

    #[test]
    fn test_ends_with_href28() {
        assert!(ends_with_href(" href =\" "));
    }

    #[test]
    fn test_ends_with_href29() {
        assert!(ends_with_href("href = ' "));
    }

    #[test]
    fn test_ends_with_href30() {
        assert!(ends_with_href(" href = ' "));
    }

    #[test]
    fn test_ends_with_href31() {
        assert!(!ends_with_href("href =\"\" "));
    }

    #[test]
    fn test_ends_with_href32() {
        assert!(!ends_with_href(" href =\"\" "));
    }

    #[test]
    fn test_ends_with_href33() {
        assert!(!ends_with_href("href ='' "));
    }

    #[test]
    fn test_ends_with_href34() {
        assert!(!ends_with_href(" href ='' "));
    }

    #[test]
    fn test_ends_with_href35() {
        assert!(ends_with_href("href = "));
    }

    #[test]
    fn test_ends_with_href36() {
        assert!(ends_with_href(" href = "));
    }

    #[test]
    fn test_ends_with_href37() {
        assert!(!ends_with_href("hrf= "));
    }

    #[test]
    fn test_ends_with_href38() {
        assert!(!ends_with_href(" hrf= "));
    }
}
