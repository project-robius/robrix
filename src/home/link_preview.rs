//! A link preview widget that provides a method to populate link preview view for setting its' children.

use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use makepad_widgets::*;
use crate::{LivePtr, utils, widget_ref_from_live_ptr};
use matrix_sdk::ruma::{events::room::{ImageInfo, MediaSource}, OwnedMxcUri, UInt};
use serde::{Deserialize, Deserializer};
use url::Url;

use crate::{
    home::room_screen::TimelineUpdate,
    media_cache::MediaCache,
    shared::text_or_image::{TextOrImageRef, TextOrImageWidgetRefExt},
    sliding_sync::{submit_async_request, MatrixRequest, UrlPreviewError},
};

/// Maximum number of cache entries before cleanup is triggered
const MAX_CACHE_ENTRIES_BEFORE_CLEANUP: usize = 100;
/// Maximum age for cache entries in seconds (1 hour)
const CACHE_ENTRY_MAX_AGE_SECS: u64 = 3600;
/// How many previews we show before hiding the rest behind a "show more" button.
const MAX_DEFAULT_VISIBLE_PREVIEWS: usize = 2;

/// An entry in the Link Preview cache with timestamp for cleanup.
#[allow(clippy::large_enum_variant)]
#[derive(Clone)]
pub struct TimestampedCacheEntry {
    pub entry: LinkPreviewCacheEntry,
    pub timestamp: Instant,
}

/// An entry in the Link Preview cache.
#[allow(clippy::large_enum_variant)]
#[derive(Clone)]
pub enum LinkPreviewCacheEntry {
    Requested,
    LoadedLinkPreview(LinkPreviewData),
    Failed,
}

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.LINK_PREVIEW_MESSAGE_TEXT_STYLE = theme.font_regular {
        font_size: (16),
        line_spacing: (1.2),
    }

    mod.widgets.LinkPreview = #(LinkPreview::register_widget(vm)) {
        width: Fill, height: Fit,
        flow: Down,

        collapsible_buttons := View {
            width: Fill, height: Fit,
            flow: Right,
            align: Align{x: 0.5, y: 0.5},
            padding: Inset{top: 4},
            visible: false,

            expand_button := RobrixIconButton {
                width: Fit, height: Fit,
                spacing: 4,
                padding: Inset{top: 4, bottom: 4, left: 8, right: 8},
                draw_icon +: {
                    svg: (ICON_TRIANGLE_DOWN)
                    color: #666666
                }
                icon_walk: Walk{width: 10, height: 10}
                draw_text +: {
                    text_style: mod.widgets.LINK_PREVIEW_MESSAGE_TEXT_STYLE {
                        font_size: 10.0,
                    },
                    color: #666666,
                    color_hover: #666666,
                    color_down: #666666,
                }
                draw_bg +: {
                    color: (COLOR_BG_PREVIEW)
                    color_hover: (COLOR_BG_PREVIEW_HOVER)
                    color_down: #A8DBBF
                    border_size: 1.0
                    border_color: #CCCCCC
                    border_color_hover: #CCCCCC
                    border_color_down: #CCCCCC
                    border_radius: 4.0
                }
                text: "Show more links"
            }

            collapse_button := RobrixIconButton {
                visible: false,
                width: Fit, height: Fit,
                spacing: 4,
                padding: Inset{top: 4, bottom: 4, left: 8, right: 8},
                draw_icon +: {
                    svg: (ICON_TRIANGLE_UP)
                    color: #666666
                }
                icon_walk: Walk{width: 10, height: 10}
                draw_text +: {
                    text_style: mod.widgets.LINK_PREVIEW_MESSAGE_TEXT_STYLE {
                        font_size: 10.0,
                    },
                    color: #666666,
                    color_hover: #666666,
                    color_down: #666666,
                }
                draw_bg +: {
                    color: (COLOR_BG_PREVIEW)
                    color_hover: (COLOR_BG_PREVIEW_HOVER)
                    color_down: #A8DBBF
                    border_size: 1.0
                    border_color: #CCCCCC
                    border_color_hover: #CCCCCC
                    border_color_down: #CCCCCC
                    border_radius: 4.0
                }
                text: "Show fewer links"
            }
        }

        preview_template: RoundedView {
            cursor: MouseCursor.Hand,
            flow: Right,
            spacing: 4.0,
            width: Fill,
            height: 96,
            margin: Inset{ top: 7 }
            padding: Inset{ top: 8, bottom: 8, left: 12, right: 12 },
            spacing: 10
            show_bg: true,
            draw_bg +: {
                color: (COLOR_BG_PREVIEW)
                border_radius: 4.0
            }
            align: Align{ y: 0.5 }

            image_view := View {
                visible: true,
                width: Fit, height: 80,
                flow: Down
                image := TextOrImage {
                    width: 120, height: Fill,
                    align: Align{ y: 0.5 }
                    image_view +: {
                        height: Fill,
                        flow: Down,
                        align: Align{ x: 0.5, y: 0.5 }
                        image +: { height: Fill }
                    }
                }
            }

            content_view := View {
                width: Fill, height: Fill,
                flow: Down,

                inner_content_view := View {
                    width: Fit, height: Fit,
                    flow: Flow.Right{wrap: true},

                    title_label := LinkLabel {
                        width: Fit, height: Fit,
                        flow: Flow.Right{wrap: true},
                        draw_text +: {
                            text_style: mod.widgets.LINK_PREVIEW_MESSAGE_TEXT_STYLE {
                                font_size: 12.0,
                            },
                            color: #x0000EE,
                            color_hover: (COLOR_LINK_HOVER),
                        }
                    }

                    site_name_label := Label {
                        width: Fit, height: Fit,
                        flow: Flow.Right{wrap: true},
                        draw_text +: {
                            text_style: mod.widgets.LINK_PREVIEW_MESSAGE_TEXT_STYLE {
                                font_size: 12.0,
                            },
                            color: #666666,
                        }
                    }
                }


                description_label := Label {
                    width: Fill, height: Fit,
                    flow: Flow.Right{wrap: true},
                    padding: Inset{ left: 0.0 }
                    max_lines: 2
                    text_overflow: Ellipsis
                    draw_text +: {
                        text_style: mod.widgets.LINK_PREVIEW_MESSAGE_TEXT_STYLE {
                            font_size: 11.0,
                        },
                        color: #666666,
                    }
                }
            }
        }
    }
}

#[derive(Script, ScriptHook, Widget)]
pub struct LinkPreview {
    #[deref]
    view: View,
    #[live]
    preview_template: Option<LivePtr>,
    #[rust]
    children: Vec<ViewRef>,
    #[layout]
    layout: Layout,
    #[rust]
    is_expanded: bool,
    #[rust]
    num_hidden_links: usize,
    /// The links that were last populated in this widget, to avoid unnecessary repopulation.
    #[rust]
    last_populated_links: Vec<Url>,
}

impl Widget for LinkPreview {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        // Handle collapsible button clicks
        if let Event::Actions(actions) = event {
            let expand_btn = self.view.button(cx, ids!(collapsible_buttons.expand_button));
            let collapse_btn = self.view.button(cx, ids!(collapsible_buttons.collapse_button));
            if expand_btn.clicked(actions) || collapse_btn.clicked(actions) {
                self.is_expanded = !self.is_expanded;
                self.update_button_and_visibility(cx);
                cx.redraw_all();
            }
        }

        for view in self.children.iter() {
            match event.hits(cx, view.area()) {
                Hit::FingerHoverIn(_) | Hit::FingerDown(_) => {
                    let mut view = view.clone();
                    script_apply_eval!(cx, view, {
                        draw_bg.color: mod.widgets.COLOR_BG_PREVIEW_HOVER
                    });
                }
                Hit::FingerHoverOut(_) => {
                    reset_hover(cx, view);
                }
                Hit::FingerUp(fe) => {
                    // return to normal bg color
                    reset_hover(cx, view);
                    if fe.is_over && fe.is_primary_hit() && fe.was_tap() {
                        if let Some(html_link) = view.link_label(cx, ids!(content_view.title_label)).borrow() {
                            if !html_link.url.is_empty() {
                                cx.widget_action(
                                    html_link.widget_uid(), 
                                    HtmlLinkAction::Clicked {
                                        url: html_link.url.clone(),
                                        key_modifiers: fe.modifiers,
                                    },
                                );
                            }
                        }
                    }
                }
                _ => {}
            }
            view.handle_event(cx, event, scope);
        }
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        // First, draw as many children as should be visible.
        let num_visible = if self.is_expanded { self.children.len() } else { MAX_DEFAULT_VISIBLE_PREVIEWS };
        for child in self.children.iter_mut().take(num_visible) {
            let _ = child.draw(cx, scope);
        }
        // Then, draw the rest of the main view, e.g., the collapsible button.
        let _ = self.view.draw_walk(cx, scope, walk);
        DrawStep::done()
    }
}

impl LinkPreview {
    fn update_button_and_visibility(&mut self, cx: &mut Cx) {
        if self.num_hidden_links > 0 {
            self.view.view(cx, ids!(collapsible_buttons)).set_visible(cx, true);
            let expand_btn = self.view.button(cx, ids!(collapsible_buttons.expand_button));
            let collapse_btn = self.view.button(cx, ids!(collapsible_buttons.collapse_button));
            if self.is_expanded {
                expand_btn.set_visible(cx, false);
                collapse_btn.set_visible(cx, true);
            } else {
                expand_btn.set_text(cx, &format!("Show {} more links", self.num_hidden_links));
                expand_btn.set_visible(cx, true);
                collapse_btn.set_visible(cx, false);
            }
            expand_btn.reset_hover(cx);
            collapse_btn.reset_hover(cx);
        } else {
            self.view.view(cx, ids!(collapsible_buttons)).set_visible(cx, false);
        }
    }
}

impl LinkPreviewRef {
    /// Clears any displayed link preview(s), resetting this widget to its empty state.
    ///
    /// Needed for messages that never show link previews (e.g. redacted messages).
    pub fn clear(&mut self, cx: &mut Cx) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.children.clear();
            inner.last_populated_links.clear();
            inner.is_expanded = false;
            inner.num_hidden_links = 0;
            inner.update_button_and_visibility(cx);
            inner.redraw(cx);
        }
    }

    /// Populates the link previews below a message.
    ///
    /// Returns `true` if every preview was fully drawn.
    pub fn populate_below_message<F>(
        &mut self,
        cx: &mut Cx,
        links: &[url::Url],
        media_cache: &mut MediaCache,
        link_preview_cache: &mut LinkPreviewCache,
        populate_image_fn: &F,
    ) -> bool
    where
        F: Fn(&mut Cx, &TextOrImageRef, Option<&ImageInfo>, MediaSource, &str, &mut MediaCache) -> bool,
    {
        const SKIPPED_DOMAINS: &[&str] = &["matrix.to", "matrix.io"];

        // Deduplicate links and filter out matrix or invalid links.
        let mut accepted_links: Vec<url::Url> = Vec::new();
        for link in links {
            if link.host_str().is_some_and(|host|
                SKIPPED_DOMAINS.iter().any(|skip| host.ends_with(skip))
            ) {
                continue;
            }
            if !accepted_links.contains(link) {
                accepted_links.push(link.clone());
            }
        }

        let did_links_change = match self.borrow() {
            Some(inner) => inner.last_populated_links != accepted_links,
            None => return true,
        };
        if did_links_change {
            if let Some(mut inner) = self.borrow_mut() {
                let num_links = accepted_links.len();
                // Reuse as many old link preview child instances as we can.
                inner.children.truncate(num_links);
                while inner.children.len() < num_links {
                    let child = widget_ref_from_live_ptr(cx, inner.preview_template).as_view();
                    inner.children.push(child);
                }
                // Reset each preview's per-link visual state: a reused one keeps its old
                // hover color and image, and a new one shows TextOrImage's default view.
                for item in inner.children.iter() {
                    reset_hover(cx, item);
                    item.text_or_image(cx, ids!(image)).clear(cx);
                }
                inner.last_populated_links = accepted_links;
                inner.is_expanded = false;
                inner.num_hidden_links = num_links.saturating_sub(MAX_DEFAULT_VISIBLE_PREVIEWS);
                inner.update_button_and_visibility(cx);
            }
        }

        let Some(inner) = self.borrow() else { return true };
        let mut all_drawn = true;
        for (view, link) in inner.children.iter().zip(inner.last_populated_links.iter()) {
            let entry = link_preview_cache.get_or_fetch_link_preview(link.as_str());
            all_drawn &= populate_preview_item(cx, view, entry, link, media_cache, populate_image_fn);
        }
        all_drawn
    }
}

fn reset_hover(cx: &mut Cx, item: &ViewRef) {
    let mut item = item.clone();
    script_apply_eval!(cx, item, {
        draw_bg.color: mod.widgets.COLOR_BG_PREVIEW
    });
}

/// Populates a single link preview with whatever metadata has been fetched so far.
///
/// Returns `true` if the link preview was fully drawn.
fn populate_preview_item<F>(
    cx: &mut Cx,
    view: &ViewRef,
    entry: LinkPreviewCacheEntry,
    link: &Url,
    media_cache: &mut MediaCache,
    populate_image_fn: &F,
) -> bool
where
    F: Fn(&mut Cx, &TextOrImageRef, Option<&ImageInfo>, MediaSource, &str, &mut MediaCache) -> bool,
{
    let title_link = view.link_label(cx, ids!(content_view.title_label));
    // Always use the original link upon click (not the `og:url`).
    if let Some(mut title_link) = title_link.borrow_mut() {
        title_link.url = link.to_string();
    }
    let site_name_label = view.label(cx, ids!(site_name_label));
    let description_label = view.label(cx, ids!(description_label));
    let image_view = view.view(cx, ids!(image_view));
    let text_or_image = view.text_or_image(cx, ids!(image));

    site_name_label.set_text(cx, "");
    description_label.set_text(cx, "");

    let data = match entry {
        LinkPreviewCacheEntry::LoadedLinkPreview(data) => data,
        LinkPreviewCacheEntry::Requested | LinkPreviewCacheEntry::Failed => {
            title_link.set_text(cx, link.as_str());
            image_view.set_visible(cx, false);
            // We treat "Failed" as permanent, i.e., it's fully drawn.
            return matches!(entry, LinkPreviewCacheEntry::Failed);
        }
    };

    title_link.set_text(cx, data.title.as_deref().unwrap_or(link.as_str()));
    if let Some(site_name) = &data.site_name {
        site_name_label.set_text(cx, site_name);
    }
    // The description label is 2 lines max with ellipsis wrap, so we
    // ensure that hard link breaks are ignored by converting them to spaces.
    if let Some(description) = &data.description {
        let description = utils::replace_linebreaks_separators(description, false);
        description_label.set_text(cx, &description);
    }

    let Some(image) = &data.image else {
        image_view.set_visible(cx, false);
        return true;
    };
    let mut image_info = ImageInfo::default();
    image_info.mimetype = data.image_type.clone();
    image_info.size = data.image_size;
    let source = MediaSource::Plain(OwnedMxcUri::from(image.clone()));
    let fully_drawn = populate_image_fn(cx, &text_or_image, Some(&image_info), source, "", media_cache);
    // Only show the image area once there's an actual image in it. Anything else means
    // it's still loading or couldn't be fetched, and an error message doesn't belong here.
    image_view.set_visible(cx, text_or_image.status().is_image());
    fully_drawn
}

/// The data structure from the link preview API, "/_matrix/client/v1/media/preview_url"
#[derive(Clone, Debug, Deserialize, Default)]
pub struct LinkPreviewData {
    #[serde(rename = "og:description")]
    pub description: Option<String>,
    /// The size of the image in bytes, if available
    #[serde(rename = "matrix:image:size", default, deserialize_with = "deserialize_lenient_uint")]
    pub image_size: Option<UInt>,
    /// The URL of the image
    #[serde(rename = "og:image")]
    pub image: Option<String>,
    /// The type of the image
    #[serde(rename = "og:image:type")]
    pub image_type: Option<String>,
    /// The name of the site
    #[serde(rename = "og:site_name")]
    pub site_name: Option<String>,
    /// The URL of the site
    #[serde(rename = "og:url")]
    pub url: Option<String>,
    /// The title of the site
    #[serde(rename = "og:title")]
    pub title: Option<String>,
}

/// Deserializes an optional [`UInt`] that is either a JSON number or a JSON string.
///
/// Some homeservers encode the numeric preview fields as strings, so we handle both.
fn deserialize_lenient_uint<'de, D>(deserializer: D) -> Result<Option<UInt>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum NumberOrString {
        Number(UInt),
        String(String),
    }
    Ok(match Option::<NumberOrString>::deserialize(deserializer)? {
        Some(NumberOrString::Number(n)) => Some(n),
        Some(NumberOrString::String(s)) => s.parse::<u64>().ok().and_then(|n| UInt::try_from(n).ok()),
        None => None,
    })
}

/// The cache for link previews.
pub struct LinkPreviewCache {
    /// The actual cached data.
    cache: BTreeMap<String, Arc<Mutex<TimestampedCacheEntry>>>,
    /// A channel to send updates to a particular timeline when a link preview request has completed.
    timeline_update_sender: Option<crossbeam_channel::Sender<TimelineUpdate>>,
}

impl LinkPreviewCache {
    /// Creates a new link preview cache that will optionally send updates
    /// when a link preview request has completed.
    pub const fn new(
        timeline_update_sender: Option<crossbeam_channel::Sender<TimelineUpdate>>,
    ) -> Self {
        Self {
            cache: BTreeMap::new(),
            timeline_update_sender,
        }
    }

    /// Fetches the link preview for the specified URL.
    pub fn get_or_fetch_link_preview(&mut self, url: &str) -> LinkPreviewCacheEntry {
        // Clean up old entries periodically
        if self.cache.len() > MAX_CACHE_ENTRIES_BEFORE_CLEANUP {
            self.cleanup_old_entries(Duration::from_secs(CACHE_ENTRY_MAX_AGE_SECS));
        }

        if let Some(entry) = self.cache.get(url) {
            return entry.lock().unwrap().entry.clone();
        }
        let entry_ref = Arc::new(Mutex::new(TimestampedCacheEntry {
            entry: LinkPreviewCacheEntry::Requested,
            timestamp: Instant::now(),
        }));
        self.cache.insert(url.to_owned(), entry_ref.clone());
        submit_async_request(MatrixRequest::GetUrlPreview {
            url: url.to_owned(),
            on_fetched: insert_into_cache,
            destination: entry_ref,
            update_sender: self.timeline_update_sender.clone(),
        });
        LinkPreviewCacheEntry::Requested
    }

    /// Removes all `Requested` and `Failed` entries from the link preview cache,
    /// allowing them to be re-fetched.
    ///
    /// This should be called when the app transitions from offline back to online,
    /// because any in-flight requests that were submitted while offline have likely
    /// failed, leaving stale entries that permanently block re-fetching.
    pub fn clear_all_pending_and_failed_requests(&mut self) {
        self.cache.retain(|_, entry| {
            if let Ok(guard) = entry.lock() {
                matches!(guard.entry, LinkPreviewCacheEntry::LoadedLinkPreview(_))
            } else {
                true // Keep entries we can't lock
            }
        });
    }

    /// Removes cache entries older than the specified duration
    pub fn cleanup_old_entries(&mut self, max_age: Duration) {
        let now = Instant::now();
        self.cache.retain(|_url, entry| {
            if let Ok(timestamped_entry) = entry.lock() {
                now.duration_since(timestamped_entry.timestamp) < max_age
            } else {
                true // Keep entries we can't lock
            }
        });
    }
}

/// Insert data into a previously-requested media cache entry.
fn insert_into_cache(
    value_ref: Arc<Mutex<TimestampedCacheEntry>>,
    data: Result<LinkPreviewData, UrlPreviewError>,
    update_sender: Option<crossbeam_channel::Sender<TimelineUpdate>>,
) {
    let new_entry = match data {
        Ok(data) => LinkPreviewCacheEntry::LoadedLinkPreview(data),
        Err(_e) => LinkPreviewCacheEntry::Failed,
    };

    if let Ok(mut timestamped_entry) = value_ref.lock() {
        timestamped_entry.entry = new_entry;
        timestamped_entry.timestamp = Instant::now();
    }

    if let Some(sender) = update_sender {
        // Reuse TimelineUpdate MediaFetched to trigger redraw in the timeline.
        let _ = sender.send(TimelineUpdate::LinkPreviewFetched);
    }
    SignalToUI::set_ui_signal();
}
