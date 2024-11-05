use std::{borrow::Cow, time::SystemTime};

use chrono::{DateTime, Duration, Local, TimeZone};
use makepad_widgets::{error, image_cache::ImageError, Cx, ImageRef};
use matrix_sdk::{media::{MediaFormat, MediaThumbnailSettings, MediaThumbnailSize}, ruma::{api::client::media::get_content_thumbnail::v3::Method, MilliSecondsSinceUnixEpoch}};

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
pub fn relative_format(millis: &MilliSecondsSinceUnixEpoch) -> Option<String> {
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
        .filter(|&g| g != "@")
        .next()
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
    pub size: MediaThumbnailSizeConst,
    pub animated: bool,
}
impl From<MediaThumbnailSettingsConst> for MediaThumbnailSettings {
    fn from(constant: MediaThumbnailSettingsConst) -> Self {
        Self {
            size: constant.size.into(),
            animated: constant.animated,
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
pub const MEDIA_THUMBNAIL_FORMAT: MediaFormatConst = MediaFormatConst::Thumbnail(
    MediaThumbnailSettingsConst {
        size: MediaThumbnailSizeConst {
            method: Method::Scale,
            width: 40,
            height: 40,
        },
        animated: false,
    }
);


/// Looks for bare links in the given `text` and converts them into proper HTML links.
pub fn linkify<'s>(text: &'s str) -> Cow<'s, str> {
    use linkify::{LinkFinder, LinkKind};
    let mut links = LinkFinder::new()
        .links(text)
        .peekable();
    if links.peek().is_none() {
        return Cow::Borrowed(text);
    }

    let mut linkified_text = String::new();
    let mut last_end_index = 0;
    for link in links {
        let link_txt = link.as_str();
        // Only linkify the URL if it's not already part of an HTML href attribute.
        let is_link_within_href_attr = text.get(..link.start())
            .map_or(false, ends_with_href);
        let is_link_within_html_tag = text.get(link.end() ..)
            .map_or(false, |after| after.trim_end().starts_with("</a>"));

        if is_link_within_href_attr || is_link_within_html_tag {
            linkified_text = format!(
                "{linkified_text}{}",
                text.get(last_end_index..link.end()).unwrap_or_default(),
            );
        } else {
            match link.kind() {
                &LinkKind::Url => {
                    linkified_text = format!(
                        "{linkified_text}{}<a href=\"{link_txt}\">{}</a>",
                        text.get(last_end_index..link.start()).unwrap_or_default(),
                        htmlize::escape_attribute(link_txt),
                    );
                }
                &LinkKind::Email => {
                    linkified_text = format!(
                        "{linkified_text}{}<a href=\"mailto:{link_txt}\">{}</a>",
                        text.get(last_end_index..link.start()).unwrap_or_default(),
                        htmlize::escape_attribute(link_txt),
                    );
                }
                _ => return Cow::Borrowed(text), // unreachable
            }
        }
        last_end_index = link.end();
    }
    linkified_text.push_str(text.get(last_end_index..).unwrap_or_default());
    Cow::Owned(linkified_text)
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
        Some(b'\'') | Some(b'"') => {
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



#[cfg(test)]
mod tests_linkify {
    use super::*;

    #[test]
    fn test_linkify0() {
        let text = "Hello, world!";
        assert_eq!(linkify(text).as_ref(), text);
    }

    #[test]
    fn test_linkify1() {
        let text = "Check out this website: https://example.com";
        let expected = "Check out this website: <a href=\"https://example.com\">https://example.com</a>";
        let actual = linkify(text);
        println!("{:?}", actual.as_ref());
        assert_eq!(actual.as_ref(), expected);
    }

    #[test]
    fn test_linkify2() {
        let text = "Send an email to john@example.com";
        let expected = "Send an email to <a href=\"mailto:john@example.com\">john@example.com</a>";
        let actual = linkify(text);
        println!("{:?}", actual.as_ref());
        assert_eq!(actual.as_ref(), expected);
    }

    #[test]
    fn test_linkify3() {
        let text = "Visit our website at www.example.com";
        assert_eq!(linkify(text).as_ref(), text);
    }

    #[test]
    fn test_linkify4() {
        let text = "Link 1 http://google.com Link 2 https://example.com";
        let expected = "Link 1 <a href=\"http://google.com\">http://google.com</a> Link 2 <a href=\"https://example.com\">https://example.com</a>";
        let actual = linkify(text);
        println!("{:?}", actual.as_ref());
        assert_eq!(actual.as_ref(), expected);
    }


    #[test]
    fn test_linkify5() {
        let text = "html test <a href=http://google.com>Link title</a> Link 2 https://example.com";
        let expected = "html test <a href=http://google.com>Link title</a> Link 2 <a href=\"https://example.com\">https://example.com</a>";
        let actual = linkify(text);
        println!("{:?}", actual.as_ref());
        assert_eq!(actual.as_ref(), expected);
    }

    #[test]
    fn test_linkify6() {
        let text = "<a href=http://google.com>link title</a>";
        assert_eq!(linkify(text).as_ref(), text);
    }

    #[test]
    fn test_linkify7() {
        let text = "https://example.com";
        let expected = "<a href=\"https://example.com\">https://example.com</a>";
        assert_eq!(linkify(text).as_ref(), expected);
    }

    #[test]
    fn test_linkify8() {
        let text = "test test https://crates.io/crates/cargo-packager test test";
        let expected = "test test <a href=\"https://crates.io/crates/cargo-packager\">https://crates.io/crates/cargo-packager</a> test test";
        assert_eq!(linkify(text).as_ref(), expected);
    }

    #[test]
    fn test_linkify9() {
        let text = "<mx-reply><blockquote><a href=\"https://matrix.to/#/!ifW4td0it0scmZpEM6:computer.surgery/$GwDzIlPzNgxhJ2QCIsmcPMC-sHdoKNsb0g2MS1psyyM?via=matrix.org&via=mozilla.org&via=gitter.im\">In reply to</a> <a href=\"https://matrix.to/#/@spore:mozilla.org\">@spore:mozilla.org</a><br />So I asked if there's a crate for it (bc I don't have the time to test and debug it) or if there's simply a better way that involves less states and invariants</blockquote></mx-reply>https://docs.rs/aho-corasick/latest/aho_corasick/struct.AhoCorasick.html#method.stream_find_iter";

        let expected = "<mx-reply><blockquote><a href=\"https://matrix.to/#/!ifW4td0it0scmZpEM6:computer.surgery/$GwDzIlPzNgxhJ2QCIsmcPMC-sHdoKNsb0g2MS1psyyM?via=matrix.org&via=mozilla.org&via=gitter.im\">In reply to</a> <a href=\"https://matrix.to/#/@spore:mozilla.org\">@spore:mozilla.org</a><br />So I asked if there's a crate for it (bc I don't have the time to test and debug it) or if there's simply a better way that involves less states and invariants</blockquote></mx-reply><a href=\"https://docs.rs/aho-corasick/latest/aho_corasick/struct.AhoCorasick.html#method.stream_find_iter\">https://docs.rs/aho-corasick/latest/aho_corasick/struct.AhoCorasick.html#method.stream_find_iter</a>";
        assert_eq!(linkify(text).as_ref(), expected);
    }

    #[test]
    fn test_linkify10() {
        let text = "And then call <a href=\"https://doc.rust-lang.org/std/io/trait.BufRead.html#method.read_until\"><code>read_until</code></a> or other <code>BufRead</code> methods.";
        let expected = "And then call <a href=\"https://doc.rust-lang.org/std/io/trait.BufRead.html#method.read_until\"><code>read_until</code></a> or other <code>BufRead</code> methods.";
        assert_eq!(linkify(text).as_ref(), expected);
    }


    #[test]
    fn test_linkify11() {
        let text = "And then https://google.com call <a href=\"https://doc.rust-lang.org/std/io/trait.BufRead.html#method.read_until\"><code>read_until</code></a> or other <code>BufRead</code> methods.";
        let expected = "And then <a href=\"https://google.com\">https://google.com</a> call <a href=\"https://doc.rust-lang.org/std/io/trait.BufRead.html#method.read_until\"><code>read_until</code></a> or other <code>BufRead</code> methods.";
        assert_eq!(linkify(text).as_ref(), expected);
    }

    #[test]
    fn test_linkify12() {
        let text = "And then https://google.com call <a href=\"https://doc.rust-lang.org/std/io/trait.BufRead.html#method.read_until\"><code>read_until</code></a> or other <code>BufRead http://another-link.http.com </code> methods.";
        let expected = "And then <a href=\"https://google.com\">https://google.com</a> call <a href=\"https://doc.rust-lang.org/std/io/trait.BufRead.html#method.read_until\"><code>read_until</code></a> or other <code>BufRead <a href=\"http://another-link.http.com\">http://another-link.http.com</a> </code> methods.";
        assert_eq!(linkify(text).as_ref(), expected);
    }

    #[test]
    fn test_linkify13() {
        let text = "Check out this website: <a href=\"https://example.com\">https://example.com</a>";
        let expected = "Check out this website: <a href=\"https://example.com\">https://example.com</a>";
        assert_eq!(linkify(text).as_ref(), expected);
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
