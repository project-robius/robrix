//! An `InlineImage` shows an `mxc://` image inline with text, like a custom emoji.
//!
//! It draws each `<img>` in a message's HTML, and the image of each custom emoji reaction.

use std::cell::RefCell;

use makepad_widgets::{makepad_draw::turtle::Baseline, *};
use matrix_sdk::{media::MediaFormat, ruma::{MxcUri, OwnedMxcUri, events::room::MediaSource}};

use crate::{media_cache::{MediaCache, MediaCacheEntry, get_image_cache_key}, utils};

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.InlineImage = #(InlineImage::register_widget(vm)) {
        width: 18, height: 18
        align: Align{x: 0.5, y: 0.5}

        image := Image {
            // Hidden until its image arrives.
            visible: false
            width: Fill, height: Fill
            fit: ImageFit.Smallest
            downscale_to_drawn_size: true
        }
    }
}

thread_local! {
    /// The images of every `InlineImage`, shared by all rooms, since the same custom emoji show up everywhere.
    ///
    /// We fetch each image in full, since most are small emoji that may be animated.
    static INLINE_IMAGE_CACHE: RefCell<MediaCache> = RefCell::new(MediaCache::new(None));
}

/// How far along an `InlineImage` is with showing its image.
#[derive(Clone, Copy, Default, PartialEq)]
enum FetchState {
    /// We haven't fetched it, or fetching it failed.
    #[default]
    NotFetched,
    /// We're waiting for it to arrive.
    Fetching,
    /// We've loaded it into our `Image`.
    Fetched,
}

/// An `mxc://` image drawn inline with text, like a custom emoji.
#[derive(Script, Widget)]
pub struct InlineImage {
    #[deref] view: View,

    /// Our image's URI, or `None` for an `<img>` from anywhere else, since we only fetch `mxc://` images.
    #[rust] mxc_uri: Option<OwnedMxcUri>,
    #[rust] fetch_state: FetchState,
    #[rust] alt_text: String,
    #[rust] tooltip: String,
    #[rust] is_custom_emoji: bool,
    /// The size that the `<img>` tag asks for, in pixels.
    #[rust] html_width: Option<f64>,
    #[rust] html_height: Option<f64>,
}

impl ScriptHook for InlineImage {
    fn on_after_new_scoped(&mut self, _vm: &mut ScriptVm, scope: &mut Scope) {
        let Some(doc) = scope.props.get::<makepad_html::HtmlDoc>() else { return };
        let parse_size = |attr: &str| attr.trim()
            .trim_end_matches("px")
            .parse::<f64>()
            .ok()
            .filter(|size| *size > 0.0);
        let mut walker = doc.new_walker_with_index(scope.index + 1);
        while let Some((lc, attr)) = walker.while_attr_lc() {
            match lc {
                id!(src) => self.mxc_uri = <&MxcUri>::from(attr).is_valid().then(|| attr.into()),
                id!(alt) => self.alt_text = attr.into(),
                id!(title) => self.tooltip = attr.into(),
                id!(width) => self.html_width = parse_size(attr),
                id!(height) => self.html_height = parse_size(attr),
                _ => ()
            }
        }
        // The SDK's sanitizer drops `data-mx-emoticon`, so we know a custom emoji by its `:shortcode:` name.
        self.is_custom_emoji = [&self.alt_text, &self.tooltip].into_iter().any(|name|
            name.len() > 2
                && name.starts_with(':')
                && name.ends_with(':')
                && !name.contains(char::is_whitespace)
        );
        if self.tooltip.is_empty() {
            self.tooltip = self.alt_text.clone();
        }
    }
}

impl Widget for InlineImage {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if let Event::Signal = event
            && self.fetch_state == FetchState::Fetching
        {
            self.load_image(cx);
            if self.fetch_state != FetchState::Fetching {
                self.redraw(cx);
            }
        }

        // We only hit-test mouse moves, so that pressing on this image still presses what it's in, e.g., a message.
        if !self.tooltip.is_empty() && matches!(event, Event::MouseMove(_) | Event::MouseLeave(_)) {
            let area = self.view.area();
            match event.hits(cx, area) {
                Hit::FingerHoverIn(_) => cx.widget_action(
                    self.widget_uid(),
                    TooltipAction::HoverIn {
                        text: self.tooltip.clone(),
                        widget_rect: area.rect(cx),
                        options: CalloutTooltipOptions {
                            position: TooltipPosition::Top,
                            ..Default::default()
                        },
                    },
                ),
                Hit::FingerHoverOut(_) => cx.widget_action(self.widget_uid(), TooltipAction::HoverOut),
                _ => ()
            }
        }

        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        if self.mxc_uri.is_none() {
            if let Some(tf) = scope.data.get_mut::<TextFlow>()
                && !self.alt_text.is_empty()
            {
                tf.draw_text(cx, &self.alt_text);
            }
            return DrawStep::done();
        }
        if self.fetch_state != FetchState::Fetched {
            self.load_image(cx);
        }

        // In HTML, we size ourselves to fit the text around us.
        let walk = match scope.data.get_mut::<TextFlow>() {
            Some(tf) => {
                let font_scale = tf.draw_text.font_scale as f64;
                let line_probe = tf.line_probe(cx);
                let Some(row) = line_probe.rows.first() else { return DrawStep::done() };
                let ascender = row.ascender_in_lpxs as f64 * font_scale;
                let line_height = ascender - row.descender_in_lpxs as f64 * font_scale;

                // A custom emoji is as tall as a line of text. So is any image in a short preview,
                // and any image of unknown size, since its height can't change once it arrives.
                let is_line_sized = self.is_custom_emoji
                    || tf.max_lines > 0
                    || (self.html_width.is_none() && self.html_height.is_none());
                if is_line_sized {
                    let aspect_ratio = self.html_width.zip(self.html_height)
                        .map_or(1.0, |(width, height)| width / height);
                    Walk {
                        width: Size::Fixed(line_height * aspect_ratio),
                        height: Size::Fixed(line_height),
                        // This puts our top and bottom at the text's ascender and descender.
                        baseline: Baseline::At(ascender as f32),
                        ..walk
                    }
                } else {
                    let width = self.html_width.or(self.html_height).unwrap_or(line_height);
                    let height = self.html_height.or(self.html_width).unwrap_or(line_height);
                    let scale = (cx.turtle().inner_width() / width).min(1.0);
                    Walk {
                        width: Size::Fixed(width * scale),
                        height: Size::Fixed(height * scale),
                        // Like in a browser, a bigger image sits on the text's baseline.
                        baseline: Baseline::None,
                        ..walk
                    }
                }
            }
            None => walk,
        };
        self.view.draw_walk(cx, scope, walk)
    }
}

impl InlineImage {
    /// Shows our image if it has arrived, or else fetches it.
    fn load_image(&mut self, cx: &mut Cx) {
        let Some(mxc_uri) = self.mxc_uri.as_ref() else { return };
        let entry = INLINE_IMAGE_CACHE.with_borrow_mut(|cache|
            cache.try_get_media_or_fetch(&MediaSource::Plain(mxc_uri.clone()), MediaFormat::File).0
        );
        self.fetch_state = match entry {
            MediaCacheEntry::Loaded(data) => {
                let image = self.view.image(cx, ids!(image));
                let cache_key = get_image_cache_key(mxc_uri, &MediaFormat::File);
                match utils::load_image_with_cache_key(&image, cx, &cache_key, data) {
                    Ok(()) => image.set_visible(cx, true),
                    Err(e) => error!("Failed to load inline image {mxc_uri}: {e:?}"),
                }
                FetchState::Fetched
            }
            MediaCacheEntry::Requested => FetchState::Fetching,
            MediaCacheEntry::Failed(_) => FetchState::NotFetched,
        };
    }
}

impl InlineImageRef {
    /// Shows the image at the given `mxc://` URI, fetching it first if needed.
    pub fn set_mxc_uri(&self, mxc_uri: OwnedMxcUri) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.mxc_uri = Some(mxc_uri);
        inner.fetch_state = FetchState::NotFetched;
    }
}

/// Clears every inline image we've fetched, e.g., upon logout.
pub fn clear_inline_image_cache(_cx: &mut Cx) {
    INLINE_IMAGE_CACHE.with_borrow_mut(|cache| cache.clear());
}

/// Removes all `Requested` and `Failed` entries from the inline image cache,
/// allowing them to be re-fetched, e.g., after we're back online.
pub fn clear_all_pending_and_failed_requests() {
    INLINE_IMAGE_CACHE.with_borrow_mut(|cache| cache.clear_all_pending_and_failed_requests());
}
