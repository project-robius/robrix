//! A link preview widget that provides a method to populate link preview view for setting its' children.

use std::{
    collections::{btree_map::Entry, BTreeMap},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use makepad_widgets::*;
use crate::{LivePtr, utils, widget_ref_from_live_ptr};
use matrix_sdk::ruma::{events::room::{ImageInfo, MediaSource}, OwnedMxcUri, UInt};
use serde::Deserialize;
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

/// Specific error types for link preview failures
#[derive(Clone, Debug)]
pub enum LinkPreviewError {
    NetworkError(String),
    ParseError(String),
    Forbidden,
    NotFound,
    RateLimited,
    InvalidUrl,
}

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
    Failed(LinkPreviewError),
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

        item_template: RoundedView {
            cursor: MouseCursor.Hand,
            flow: Right,
            spacing: 4.0,
            width: Fill, height: Fit,
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
    item_template: Option<LivePtr>,
    #[rust]
    children: Vec<ViewRef>,
    #[layout]
    layout: Layout,
    #[rust]
    show_collapsible_buttons: bool,
    #[rust]
    is_expanded: bool,
    #[rust]
    hidden_links_count: usize,
    /// Tracks the URLs that were last used to populate this widget's children,
    /// so we can skip expensive widget recreation when the same links are shown.
    #[rust]
    last_populated_links: Vec<String>,
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
                    let mut view = view.clone();
                    script_apply_eval!(cx, view, {
                        draw_bg.color: mod.widgets.COLOR_BG_PREVIEW
                    });
                }
                Hit::FingerUp(fe) => {
                    // return to normal bg color
                    let mut view = view.clone();
                    script_apply_eval!(cx, view, {
                        draw_bg.color: mod.widgets.COLOR_BG_PREVIEW
                    });
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
        // Draw children (link preview items)
        let max_visible = if self.is_expanded { self.children.len() } else { 2 };
        for (index, view) in self.children.iter_mut().enumerate() {
            if index < max_visible {
                let _ = view.draw(cx, scope);
            }
        }
        // Draw the main view which includes the collapsible button
        let _ = self.view.draw_walk(cx, scope, walk);
        DrawStep::done()
    }
}

impl LinkPreview {
    fn item_template(&self) -> Option<LivePtr> {
        self.item_template
    }

    fn update_button_and_visibility(&mut self, cx: &mut Cx) {
        if self.show_collapsible_buttons {
            self.view.view(cx, ids!(collapsible_buttons)).set_visible(cx, true);
            let expand_btn = self.view.button(cx, ids!(collapsible_buttons.expand_button));
            let collapse_btn = self.view.button(cx, ids!(collapsible_buttons.collapse_button));
            if self.is_expanded {
                expand_btn.set_visible(cx, false);
                collapse_btn.set_visible(cx, true);
            } else {
                expand_btn.set_text(cx, &format!("Show {} more links", self.hidden_links_count));
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
    fn item_template(&self) -> Option<LivePtr> {
        if let Some(inner) = self.borrow() {
            return inner.item_template();
        }
        None
    }
    /// Sets the children of the LinkPreview widget.
    ///
    /// This function will replace all existing children of the LinkPreview widget with the provided views.
    ///
    /// # Parameters
    ///
    /// * `views`: A vector of ViewRef objects to be set as the children of the LinkPreview widget.
    fn set_children(&mut self, views: Vec<ViewRef>) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.children = views;
        }
    }

    /// Shows the collapsible button for the link preview.
    /// 
    /// This function is usually called when the link preview is updated.
    /// If the link preview is updated, and the collapsible button should be shown,
    /// this function should be called.
    fn show_collapsible_buttons(&mut self, cx: &mut Cx, hidden_count: usize) {
         if let Some(mut inner) = self.borrow_mut() {
            inner.show_collapsible_buttons = true;
            inner.hidden_links_count = hidden_count;
            let expand_btn = inner.view.button(cx, ids!(collapsible_buttons.expand_button));
            expand_btn.set_text(cx, &format!("Show {} more links", inner.hidden_links_count));
            expand_btn.set_visible(cx, true);
            inner.view.button(cx, ids!(collapsible_buttons.collapse_button)).set_visible(cx, false);
            inner.view.view(cx, ids!(collapsible_buttons)).set_visible(cx, true);
        }
    }

    /// Populates a link preview view with data and handles image population through a closure.
    /// Returns whether the link preview is fully drawn.
    fn populate_view<F>(
        &mut self,
        cx: &mut Cx,
        link_preview_cache_entry: LinkPreviewCacheEntry,
        link: &Url,
        media_cache: &mut MediaCache,
        image_populate_fn: F,
    ) -> (ViewRef, bool)
    where
        F: FnOnce(&mut Cx, &TextOrImageRef, Option<Box<ImageInfo>>, MediaSource, &str, &mut MediaCache) -> bool,
    {
        let view_ref = widget_ref_from_live_ptr(cx, self.item_template()).as_view();
        let mut fully_drawn = true;
        // Set title and URL
        let title_link = view_ref.link_label(cx, ids!(content_view.title_label));
        title_link.set_text(cx, link.as_str());
        if let Some(mut title_link) = title_link.borrow_mut() {
            title_link.url = link.to_string();
        }
        let text_or_image_ref = view_ref.text_or_image(cx, ids!(image));
        text_or_image_ref.show_default_image(cx);
        let link_preview_data = match link_preview_cache_entry {
            LinkPreviewCacheEntry::LoadedLinkPreview(link_preview_data) => link_preview_data,
            LinkPreviewCacheEntry::Failed(_) => return (view_ref, true),
            LinkPreviewCacheEntry::Requested => return (view_ref, false),
        };
        if let Some(url) = &link_preview_data.url {
            if let Some(mut title_link) = title_link.borrow_mut() {
                title_link.url = url.clone();
            }
        }
        if let Some(title) = &link_preview_data.title {
            title_link.set_text(cx, title);
        }

        // Set site name
        if let Some(site_name) = &link_preview_data.site_name {
            view_ref
                .label(cx, ids!(site_name_label))
                .set_text(cx, site_name);
        }

        // Set description; the description_label uses max_lines: 2 with
        // text_overflow: Ellipsis to wrap and truncate automatically.
        // Collapse all whitespace runs into single spaces so hard line breaks
        // don't burn one of the two available lines.
        if let Some(description_raw) = &link_preview_data.description {
            let description = utils::replace_linebreaks_separators(description_raw, false);
            view_ref
                .label(cx, ids!(description_label))
                .set_text(cx, &description);
        }

        // Handle image through closure
        if let Some(image) = &link_preview_data.image {
            let mut image_info = ImageInfo::default();
            image_info.mimetype = link_preview_data.image_type.clone();
            image_info.size = link_preview_data.image_size;
            let image_info_source = Some(Box::new(image_info));
            let owned_mxc_uri = OwnedMxcUri::from(image.clone());
            let text_or_image_ref = view_ref.text_or_image(cx, ids!(image));
            let original_source = MediaSource::Plain(owned_mxc_uri);
            // Calls the closure with the image populate function
            fully_drawn = image_populate_fn(
                cx,
                &text_or_image_ref,
                image_info_source,
                original_source,
                "",
                media_cache,
            );
        }

        (view_ref, fully_drawn)
    }

    /// Populates link previews below a message.
    ///
    /// The given `media_cache` is used to fetch the thumbnails from cache.
    ///
    /// The given `link_preview_cache` is used to fetch the link previews from cache.
    /// 
    /// Return true when the link preview is fully drawn
    pub fn populate_below_message<F>(
        &mut self,
        cx: &mut Cx,
        links: &Vec<url::Url>,
        media_cache: &mut MediaCache,
        link_preview_cache: &mut LinkPreviewCache,
        populate_image_fn: &F,
    ) -> bool 
    where
        F: Fn(&mut Cx, &TextOrImageRef, Option<Box<ImageInfo>>, MediaSource, &str, &mut MediaCache) -> bool,
    {
        const SKIPPED_DOMAINS: &[&str] = &["matrix.to", "matrix.io"];
        const MAX_LINK_PREVIEWS_BY_EXPAND: usize = 2;

        // Build the list of accepted URLs (after dedup + domain filtering)
        // to check if we can skip the expensive widget recreation.
        let mut accepted_urls: Vec<String> = Vec::new();
        let mut seen_urls = std::collections::HashSet::new();
        for link in links {
            let url_string = link.to_string();
            if seen_urls.contains(&url_string) {
                continue;
            }
            if let Some(domain) = link.host_str() {
                if SKIPPED_DOMAINS
                    .iter()
                    .any(|skip_domain| domain.ends_with(skip_domain))
                {
                    continue;
                }
            }
            seen_urls.insert(url_string.clone());
            accepted_urls.push(url_string);
        }

        // If the links haven't changed and children are already populated,
        // skip the expensive widget recreation entirely.
        if let Some(inner) = self.borrow() {
            if accepted_urls == inner.last_populated_links && !inner.children.is_empty() {
                return true;
            }
        }

        let mut fully_drawn_count = 0;
        let accepted_link_count = accepted_urls.len();
        let mut views = Vec::with_capacity(accepted_link_count);

        for (url_string, link) in accepted_urls.iter().zip(
            links.iter().filter(|l| accepted_urls.contains(&l.to_string()))
        ) {
            let (view_ref, was_image_drawn) = self.populate_view(
                cx,
                link_preview_cache.get_or_fetch_link_preview(url_string.clone()),
                link,
                media_cache,
                |cx, text_or_image_ref, image_info_source, original_source, body, media_cache| {
                    populate_image_fn(cx, text_or_image_ref, image_info_source, original_source, body, media_cache)
                },
            );
            fully_drawn_count += was_image_drawn as usize;
            views.push(view_ref);
        }
        if views.len() > MAX_LINK_PREVIEWS_BY_EXPAND {
            let hidden_count = views.len() - MAX_LINK_PREVIEWS_BY_EXPAND;
            self.show_collapsible_buttons(cx, hidden_count);
        }
        if let Some(mut inner) = self.borrow_mut() {
            inner.last_populated_links = accepted_urls;
        }
        self.set_children(views);
        fully_drawn_count == accepted_link_count
    }
}

/// The data structure from the link preview API, "/_matrix/client/v1/media/preview_url"
#[derive(Clone, Debug, Deserialize, Default)]
pub struct LinkPreviewData {
    #[serde(rename = "og:description")]
    pub description: Option<String>,
    /// The size of the image in bytes, if available
    #[serde(rename = "matrix:image:size")]
    pub image_size: Option<UInt>,
    /// The URL of the image
    #[serde(rename = "og:image")]
    pub image: Option<String>,
    /// The height of the image
    #[serde(rename = "og:image:height")]
    pub image_height: Option<UInt>,
    /// The width of the image
    #[serde(rename = "og:image:width")]
    pub image_width: Option<UInt>,
    /// The type of the image
    #[serde(rename = "og:image:type")]
    pub image_type: Option<String>,
    /// The locale of the link preview
    #[serde(rename = "og:locale")]
    pub locale: Option<String>,
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

/// The data structure from the link preview API whereby numeric values are strings, "/_matrix/client/v1/media/preview_url"
#[derive(Clone, Debug, Deserialize, Default)]
pub struct LinkPreviewDataNonNumeric {
    #[serde(rename = "og:description")]
    pub description: Option<String>,
    /// The size of the image in bytes, if available
    #[serde(rename = "matrix:image:size")]
    pub image_size: Option<String>,
    /// The URL of the image
    #[serde(rename = "og:image")]
    pub image: Option<String>,
    /// The height of the image
    #[serde(rename = "og:image:height")]
    pub image_height: Option<String>,
    /// The width of the image
    #[serde(rename = "og:image:width")]
    pub image_width: Option<String>,
    /// The type of the image
    #[serde(rename = "og:image:type")]
    pub image_type: Option<String>,
    /// The locale of the link preview
    #[serde(rename = "og:locale")]
    pub locale: Option<String>,
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

impl From<LinkPreviewDataNonNumeric> for LinkPreviewData {
    fn from(non_numeric: LinkPreviewDataNonNumeric) -> Self {
        Self {
            description: non_numeric.description,
            image_size: non_numeric.image_size.and_then(|s| s.parse().ok()),
            image: non_numeric.image,
            image_height: non_numeric.image_height.and_then(|s| s.parse().ok()),
            image_width: non_numeric.image_width.and_then(|s| s.parse().ok()),
            image_type: non_numeric.image_type,
            locale: non_numeric.locale,
            site_name: non_numeric.site_name,
            url: non_numeric.url,
            title: non_numeric.title,
        }
    }
}

/// The rate limit data structure from the 429 response code
#[derive(Clone, Debug, Deserialize, Default)]
pub struct LinkPreviewRateLimitResponse {
    /// The M_LIMIT_EXCEEDED error code
    pub errcode: String,
    /// A human-readable error message.
    pub error: Option<String>,
    /// The amount of time in milliseconds the client should wait before trying the request again.
    pub retry_after_ms: Option<UInt>,
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
    pub fn get_or_fetch_link_preview(&mut self, url: String) -> LinkPreviewCacheEntry {
        // Clean up old entries periodically
        if self.cache.len() > MAX_CACHE_ENTRIES_BEFORE_CLEANUP {
            self.cleanup_old_entries(Duration::from_secs(CACHE_ENTRY_MAX_AGE_SECS));
        }

        match self.cache.entry(url.clone()) {
            Entry::Vacant(vacant) => {
                let entry_ref = Arc::new(Mutex::new(TimestampedCacheEntry {
                    entry: LinkPreviewCacheEntry::Requested,
                    timestamp: Instant::now(),
                }));
                vacant.insert(entry_ref.clone());
                submit_async_request(MatrixRequest::GetUrlPreview {
                    url,
                    on_fetched: insert_into_cache,
                    destination: entry_ref,
                    update_sender: self.timeline_update_sender.clone(),
                });

                LinkPreviewCacheEntry::Requested
            }
            Entry::Occupied(occupied) => occupied.get().lock().unwrap().entry.clone(),
        }
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
    url: String,
    value_ref: Arc<Mutex<TimestampedCacheEntry>>,
    data: Result<LinkPreviewData, UrlPreviewError>,
    update_sender: Option<crossbeam_channel::Sender<TimelineUpdate>>,
) {
    let new_entry = match data {
        Ok(data) => LinkPreviewCacheEntry::LoadedLinkPreview(data),
        Err(e) => {
            let error_type = match e {
                UrlPreviewError::HttpStatus(403) => LinkPreviewError::Forbidden,
                UrlPreviewError::HttpStatus(404) => LinkPreviewError::NotFound,
                UrlPreviewError::HttpStatus(429) => LinkPreviewError::RateLimited,
                UrlPreviewError::Json(_) => LinkPreviewError::ParseError(e.to_string()),
                UrlPreviewError::Request(_) | 
                UrlPreviewError::ClientNotAvailable | 
                UrlPreviewError::AccessTokenNotAvailable |
                UrlPreviewError::UrlParse(_) |
                UrlPreviewError::HttpStatus(_) => LinkPreviewError::NetworkError(e.to_string()),
            };
            if let LinkPreviewError::RateLimited = error_type {
                LinkPreviewCacheEntry::Requested
            } else {
                warning!("Failed to fetch link preview data for {url}: {e:?}");
                LinkPreviewCacheEntry::Failed(error_type)
            }
        }
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
