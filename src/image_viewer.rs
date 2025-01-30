use std::collections::HashMap;
use makepad_widgets::*;
use matrix_sdk::ruma::OwnedMxcUri;

use crate::{media_cache::{MediaCache, MediaCacheEntry}, utils};

live_design! {
    use link::theme::*;
    use link::widgets::*;

    use crate::shared::styles::*;
    use crate::shared::icon_button::RobrixIconButton;

    pub ImageViewer = {{ImageViewer}} {
        width: 1600, height: 900
        align: {x: 0.5}
        spacing: 15
        flow: Down
        show_bg: true
        draw_bg: {
            color: #1F1F1F80
        }

        <View> {
            align: {x: 1.0, y: 0.0}
            width: Fill, height: Fit
            close_button = <RobrixIconButton> {
                padding: {left: 15, right: 15}
                draw_icon: {
                    svg_file: (ICON_CLOSE)
                    color: (COLOR_CLOSE),
                }
                icon_walk: {width: 20, height: 20, margin: {left: -1, right: -1} }

                draw_bg: {
                    border_color: (COLOR_CLOSE_BG),
                    color: (COLOR_CLOSE_BG) // light red
                }
            }
        }

        image_view = <Image> {
            width: Fill, height: Fill,
            fit: Smallest,
        }
    }
}

#[derive(Live, LiveHook, Widget)]
pub struct ImageViewer {
    #[deref] view: View,
    #[rust] widgetref_image_uri_map: HashMap<WidgetUid, OwnedMxcUri>,
    #[rust] media_cache: Option<MediaCache>,
}


#[derive(Clone, Debug, DefaultNone)]
pub enum ImageViewerAction {
    Insert(WidgetUid, OwnedMxcUri),
    SetMediaCache(MediaCache),
    Show(WidgetUid),
    None,
}

impl Widget for ImageViewer {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.match_event(cx, event);
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}
impl MatchEvent for ImageViewer {
    fn handle_actions(&mut self, _cx: &mut Cx, actions: &Actions) {
        for _action in actions {

        }
    }
}

impl ImageViewer {
    // We clone the media cache here, is unnecessary, but I can't find a way get its mut reference.
    fn set_media_cache(&mut self, media_cache: MediaCache) {
        self.media_cache = Some(media_cache);
        log!("Set media cache")
    }
    /// We restore image message uid and the image inside the message's mx_uri into HashMap
    /// when the message is being populated.
    fn insert_data(&mut self, text_or_image_uid: &WidgetUid, mx_uri: OwnedMxcUri) {
        self.widgetref_image_uri_map.insert(*text_or_image_uid, mx_uri);
        log!("Inserted");
    }
    /// We find mx_uid via the given `text_or_image_uid`.
    fn show_and_fill_image(&mut self, cx: &mut Cx, text_or_image_uid: &WidgetUid) {
        if let Some(mxc_uri) = self.widgetref_image_uri_map.get(text_or_image_uid) {
            match self.media_cache.as_mut().unwrap().try_get_media_or_fetch(mxc_uri.clone(), None) {
                MediaCacheEntry::Loaded(data) => {
                    let image_view = self.view.image(id!(image_view));

                    if let Err(e) = utils::load_png_or_jpg(&image_view, cx, &data) {
                        log!("Error to load image: {e}");
                    }
                    self.view.redraw(cx);
                }
                MediaCacheEntry::Requested => {

                }
                MediaCacheEntry::Failed => {

                }
            };
        }
    }
    pub fn clear_image(&mut self, cx: &mut Cx) {
        self.view.image(id!(image_view)).set_texture(cx, None);
    }
}

impl ImageViewerRef {
    pub fn set_media_cache(&self, media_cache: MediaCache) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_media_cache(media_cache)
        }
    }
    pub fn insert_data(&self, text_or_image_uid: &WidgetUid, mx_uri: OwnedMxcUri) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.insert_data(text_or_image_uid, mx_uri);
        }
    }
    pub fn show_and_fill_image(&self, cx: &mut Cx, text_or_image_uid: &WidgetUid) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.show_and_fill_image(cx, text_or_image_uid);
        }
    }
    pub fn clear_image(&self, cx: &mut Cx) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.clear_image(cx);
        }
    }
}
