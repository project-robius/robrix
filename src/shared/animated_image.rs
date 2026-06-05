//! A standalone widget for animated image messages.

use std::{path::PathBuf, sync::Arc};

use makepad_widgets::{image_cache::ImageError, *};
use matrix_sdk::{
    media::MediaFormat,
    ruma::{events::room::MediaSource, OwnedMxcUri},
};

use crate::media_cache::{MediaCache, MediaCacheEntry};

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.AnimatedImage = #(AnimatedImage::register_widget(vm)) {
        width: Fill, height: Fit,
        flow: Overlay,

        text_view := SolidView {
            visible: true,
            width: Fill, height: Fit,
            show_bg: true,
            draw_bg.color: #dddddd

            label := Label {
                width: Fill, height: Fit,
                flow: Flow.Right{wrap: true},
                draw_text +: {
                    text_style: MESSAGE_TEXT_STYLE { }
                    color: (MESSAGE_TEXT_COLOR),
                }
            }
        }
        image_view := View {
            visible: false,
            cursor: MouseCursor.Default,
            width: Fill, height: Fit,
            image := Image {
                width: Fill, height: Fit,
                fit: ImageFit.Smallest,
            }
        }
    }
}

#[derive(Script, Widget, ScriptHook)]
pub struct AnimatedImage {
    #[deref]
    view: View,
    #[rust]
    status: AnimatedImageStatus,
    #[rust]
    size_in_pixels: (usize, usize),
}

impl Widget for AnimatedImage {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl AnimatedImage {
    pub fn show_text<T: AsRef<str>>(&mut self, cx: &mut Cx, text: T) {
        self.view(cx, ids!(image_view)).set_visible(cx, false);
        self.view(cx, ids!(text_view)).set_visible(cx, true);
        self.view
            .label(cx, ids!(text_view.label))
            .set_text(cx, text.as_ref());
        self.status = AnimatedImageStatus::Text;
    }

    pub fn show_image_data(
        &mut self,
        cx: &mut Cx,
        source_url: Option<MediaSource>,
        cache_key: PathBuf,
        data: &[u8],
    ) -> Result<(), ImageError> {
        let image_ref = self.view.image(cx, ids!(image_view.image));
        image_ref.load_image_from_data_async(cx, &cache_key, Arc::new(data.to_vec()))?;
        self.status = AnimatedImageStatus::Image(source_url);
        self.size_in_pixels = image_ref.size_in_pixels(cx).unwrap_or_default();
        self.view(cx, ids!(image_view)).set_visible(cx, true);
        self.view(cx, ids!(text_view)).set_visible(cx, false);
        Ok(())
    }

    pub fn populate_from_mxc(
        &mut self,
        cx: &mut Cx,
        mxc_uri: OwnedMxcUri,
        body: &str,
        media_cache: &mut MediaCache,
    ) -> bool {
        // `try_get_media_or_fetch` now takes a `&MediaSource` rather than
        // a `&OwnedMxcUri`, so wrap the plain URI in a `MediaSource::Plain`
        // for the cache lookup. The same wrapped value is reused below as
        // the source we record on the widget.
        let source = MediaSource::Plain(mxc_uri.clone());
        match media_cache.try_get_media_or_fetch(&source, MediaFormat::File) {
            (MediaCacheEntry::Loaded(data), MediaFormat::File) => {
                let cache_key = animated_image_cache_key(&mxc_uri, body);
                if let Err(e) =
                    self.show_image_data(cx, Some(source.clone()), cache_key, &data)
                {
                    let err_str = format!("{body}\n\nFailed to display animated image: {e:?}");
                    error!("{err_str}");
                    self.show_text(cx, &err_str);
                }
                true
            }
            (MediaCacheEntry::Loaded(_), _) | (MediaCacheEntry::Requested, _) => {
                self.show_text(cx, "Loading animated image...");
                false
            }
            (MediaCacheEntry::Failed(_), _) => {
                self.show_text(
                    cx,
                    format!(
                        "{body}\n\nFailed to fetch animated image from {:?}",
                        mxc_uri
                    ),
                );
                true
            }
        }
    }

    pub fn populate_from_media_source(
        &mut self,
        cx: &mut Cx,
        media_source: MediaSource,
        body: &str,
        media_cache: &mut MediaCache,
    ) -> bool {
        match media_source {
            MediaSource::Encrypted(encrypted) => {
                self.show_text(
                    cx,
                    format!(
                        "{body}\n\n[TODO] fetch encrypted animated image at {:?}",
                        encrypted.url
                    ),
                );
                true
            }
            MediaSource::Plain(mxc_uri) => self.populate_from_mxc(cx, mxc_uri, body, media_cache),
        }
    }
}

impl AnimatedImageRef {
    pub fn show_text<T: AsRef<str>>(&self, cx: &mut Cx, text: T) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.show_text(cx, text);
        }
    }

    pub fn populate_from_media_source(
        &self,
        cx: &mut Cx,
        media_source: MediaSource,
        body: &str,
        media_cache: &mut MediaCache,
    ) -> bool {
        self.borrow_mut()
            .map(|mut inner| inner.populate_from_media_source(cx, media_source, body, media_cache))
            .unwrap_or(true)
    }
}

#[derive(Debug, Default, Clone)]
pub enum AnimatedImageStatus {
    #[default]
    Text,
    Image(Option<MediaSource>),
}

fn animated_image_cache_key(mxc_uri: &OwnedMxcUri, body: &str) -> PathBuf {
    let extension = body
        .rsplit_once('.')
        .map(|(_, extension)| extension.to_ascii_lowercase())
        .filter(|extension| matches!(extension.as_str(), "gif" | "apng" | "webp"))
        .unwrap_or_else(|| "img".to_string());
    let sanitized_uri: String = mxc_uri
        .as_str()
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
        .collect();

    PathBuf::from(format!("robrix_animated_image_{sanitized_uri}.{extension}"))
}
