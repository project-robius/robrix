//! A link preview widget that provides a template and.

use std::{
    collections::{btree_map::Entry, BTreeMap},
    sync::{Arc, Mutex},
};

use makepad_widgets::*;
use matrix_sdk::ruma::{events::room::{ImageInfo, MediaSource}, OwnedMxcUri, UInt};
use serde::Deserialize;

use crate::{
    home::room_screen::TimelineUpdate,
    media_cache::MediaCache,
    shared::text_or_image::{TextOrImageRef, TextOrImageWidgetRefExt},
    sliding_sync::{submit_async_request, MatrixRequest},
};

/// An entry in the Link Preview cache.
#[allow(clippy::large_enum_variant)]
#[derive(Clone)]
pub enum LinkPreviewCacheEntry {
    Requested,
    LoadedLinkPreview(LinkPreviewData),
    Failed,
}

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;
    use crate::shared::styles::*;
    use crate::shared::text_or_image::TextOrImage;

    pub MESSAGE_TEXT_STYLE = <THEME_FONT_REGULAR>{
        font_size: (16),
        line_spacing: (1.2),
    }

    pub LinkPreview = {{LinkPreview}} {
        width: Fill, height: Fit,
        flow: Down,
        item: <RoundedView> {
            flow: Right,
            spacing: 4.0,
            width: Fill, height: Fit,
            padding: {top: 8, bottom: 8, left: 12, right: 12},
            show_bg: true,
            draw_bg: {
                color: #f5f5f5,
            }
            align: { y: 0.0 }
            <View>{
                width: 2, height: 80,
                show_bg: true,
                draw_bg: {
                    color: #666666,
                }
            }
            image_view = <View> {
                visible: true,
                width: Fit, height: Fit,
                image = <TextOrImage> {
                    width: 80, height: 80,
                }
            }

            content_view = <View> {
                width: Fill, height: Fit,
                flow: Down,
                spacing: 0.0
                <View> {
                    width: Fit, height: Fit,
                    flow: RightWrap,
                    title_label = <LinkLabel> {
                        width: Fit, height: Fit,
                        draw_text: {
                            text_style: <MESSAGE_TEXT_STYLE> {
                                font_size: 12.0,
                            },
                            color: #x0000EE,
                            wrap: Word,
                        }
                    }
                    site_name_label = <Label> {
                        width: Fit, height: Fit,
                        draw_text: {
                            text_style: <MESSAGE_TEXT_STYLE> {
                                font_size: 12.0,
                            },
                            color: #666666,
                            wrap: Word,
                        }
                    }
                }
                <View> {
                    width: Fill, height: 48,
                    description_label = <Label> {
                        width: Fill, height: Fit,
                        draw_text: {
                            text_style: <MESSAGE_TEXT_STYLE> {
                                font_size: 11.0,
                            },
                            color: #666666,
                            wrap: Word,
                        }
                    }
                }
            }
        }
    }
}

#[derive(Live, LiveHook, Widget)]
pub struct LinkPreview {
    #[deref]
    view: View,
    #[live]
    item: Option<LivePtr>,
    #[rust]
    children: Vec<ViewRef>,
    #[layout]
    layout: Layout,
}

impl Widget for LinkPreview {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        for view in self.children.iter() {
            if let Some(html_link) = view.link_label(id!(content_view.title_label)).borrow() {
                if let Event::Actions(actions) = event {
                    if html_link.clicked(actions) && !html_link.url.is_empty() {
                        cx.widget_action(
                            html_link.widget_uid(),
                            &scope.path,
                            HtmlLinkAction::Clicked {
                                url: html_link.url.clone(),
                                key_modifiers: KeyModifiers::default(),
                            },
                        );
                    }
                }
            }
            view.handle_event(cx, event, scope);
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, _walk: Walk) -> DrawStep {
        for view in self.children.iter_mut() {
            let _ = view.draw(cx, scope);
        }
        DrawStep::done()
    }
}

impl LinkPreview {
    pub fn item_template(&self) -> Option<LivePtr> {
        self.item
    }
}

impl LinkPreviewRef {
    pub fn item_template(&self) -> Option<LivePtr> {
        if let Some(inner) = self.borrow() {
            return inner.item_template();
        }
        None
    }
    pub fn set_children(&mut self, views: Vec<ViewRef>) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.children = views;
        }
    }

    /// Populates a link preview view with data and handles image population through a closure.
    /// Returns whether the image was fully drawn.
    pub fn populate_link_preview_view<F>(
        &mut self,
        cx: &mut Cx2d,
        link_preview_data: &LinkPreviewData,
        media_cache: &mut MediaCache,
        image_populate_fn: F,
    ) -> (ViewRef, bool)
    where
        F: FnOnce(&mut Cx2d, &TextOrImageRef, Option<Box<ImageInfo>>, MediaSource, &str, &mut MediaCache) -> bool,
    {
        let view_ref = WidgetRef::new_from_ptr(cx, self.item_template()).as_view();
        let mut fully_drawn = true;

        // Set title and URL
        if let (Some(url), Some(title)) = (&link_preview_data.url, &link_preview_data.title) {
            let title_link = view_ref.link_label(id!(content_view.title_label));
            if let Some(mut title_link) = title_link.borrow_mut() {
                title_link.url = url.clone();
            }
            title_link.set_text(cx, title);
        }

        // Set site name
        if let Some(site_name) = &link_preview_data.site_name {
            view_ref
                .view(id!(content_view))
                .label(id!(site_name_label))
                .set_text(cx, site_name);
        }

        // Set description
        if let Some(description) = &link_preview_data.description {
            view_ref
                .view(id!(content_view))
                .label(id!(description_label))
                .set_text(cx, description);
        }

        // Handle image through closure
        if let Some(image) = &link_preview_data.image {
            let mut image_info = ImageInfo::default();
            image_info.height = link_preview_data.image_height;
            image_info.width = link_preview_data.image_width;
            image_info.mimetype = link_preview_data.image_type.clone();
            image_info.size = link_preview_data.image_size;
            let image_info_source = Some(Box::new(image_info));
            let owned_mxc_uri = OwnedMxcUri::from(image.clone());
            let text_or_image_ref = view_ref.text_or_image(id!(image));
            let original_source = MediaSource::Plain(owned_mxc_uri);
            
            // Call the closure with the image populate function
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
}

/// The data we get from the link preview API, "_matrix/media/v3/preview_url"
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

pub struct LinkPreviewCache {
    /// The actual cached data.
    cache: BTreeMap<String, Arc<Mutex<LinkPreviewCacheEntry>>>,
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

    pub fn get_or_fetch_link_preview(&mut self, url: String) -> LinkPreviewCacheEntry {
        match self.cache.entry(url.clone()) {
            Entry::Vacant(vacant) => {
                let entry_ref = Arc::new(Mutex::new(LinkPreviewCacheEntry::Requested));
                vacant.insert(entry_ref.clone());
                submit_async_request(MatrixRequest::GetUrlPreview {
                    url,
                    on_fetched: insert_into_cache,
                    destination: entry_ref,
                    update_sender: self.timeline_update_sender.clone(),
                });

                LinkPreviewCacheEntry::Requested
            }
            Entry::Occupied(occupied) => occupied.get().lock().unwrap().clone(),
        }
    }
}

/// Insert data into a previously-requested media cache entry.
fn insert_into_cache(
    url: String,
    value_ref: Arc<Mutex<LinkPreviewCacheEntry>>,
    data: anyhow::Result<LinkPreviewData>,
    update_sender: Option<crossbeam_channel::Sender<TimelineUpdate>>,
) {
    let new_value = match data {
        Ok(data) => LinkPreviewCacheEntry::LoadedLinkPreview(data),
        Err(e) => {
            error!("Failed to fetch link preview data for {url}: {e:?}");
            LinkPreviewCacheEntry::Failed
        }
    };
    *value_ref.lock().unwrap() = new_value;
    if let Some(sender) = update_sender {
        // Reuse TimelineUpdate MediaFetched to trigger redraw in the timeline.
        let _ = sender.send(TimelineUpdate::MediaFetched);
    }
    SignalToUI::set_ui_signal();
}
