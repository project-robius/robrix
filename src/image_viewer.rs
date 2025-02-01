use std::{collections::{btree_map::Entry, HashMap}, sync::Arc};
use std::sync::Mutex;
use makepad_widgets::*;
use matrix_sdk::ruma::OwnedMxcUri;

use crate::{media_cache::{MediaCache, MediaCacheEntry}, sliding_sync::{self, MatrixRequest}, utils};

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
    #[rust] media_cache: MediaCache,
}


#[derive(Clone, Debug, DefaultNone)]
pub enum ImageViewerAction {
    SetData {text_or_image_uid: WidgetUid, mxc_uri: OwnedMxcUri},
    Clicked(WidgetUid),
    Fetched(OwnedMxcUri),
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
    /// We restore image message uid and the image inside the message's mx_uri into HashMap
    /// when the message is being populated.
    fn insert_data(&mut self, text_or_image_uid: &WidgetUid, mxc_uri: OwnedMxcUri) {
        self.widgetref_image_uri_map.insert(*text_or_image_uid, mxc_uri);
        log!("Inserted");
    }
    /// We find mx_uid via the given `text_or_image_uid`.
    fn image_viewer_try_get_or_fetch(
        &mut self,
        text_or_image_uid: &WidgetUid,
    ) -> MediaCacheEntry {
        if let Some(mxc_uri) = self.widgetref_image_uri_map.get(text_or_image_uid) {

            let destination = match self.media_cache.entry(mxc_uri.clone()) {
                Entry::Vacant(vacant) => {
                    vacant.insert(Arc::new(Mutex::new(MediaCacheEntry::Requested)))
                }
                Entry::Occupied(occupied) => return occupied.get().lock().unwrap().clone(),
            };

            let destination = destination.clone();

            sliding_sync::submit_async_request(
                MatrixRequest::FetchOriginalMedia {
                    destination,
                    mxc_uri: mxc_uri.clone()
                }
            );
        }
        MediaCacheEntry::Requested
    }
    fn find_and_load(&mut self, cx: &mut Cx, mxc_uri: &OwnedMxcUri) {
        if let Some(MediaCacheEntry::Loaded(image_data)) = self.media_cache.try_get_media(mxc_uri) {
            let image_view = self.view.image(id!(image_view));

            if let Err(e) = utils::load_png_or_jpg(&image_view, cx, &image_data) {
                log!("Error to load image: {e}");
            } else {
                self.view.redraw(cx);
            }
        }
    }
    fn load_and_redraw(&mut self, cx: &mut Cx, data: &[u8]) {
            let image_view = self.view.image(id!(image_view));
            if let Err(e) = utils::load_png_or_jpg(&image_view, cx, data) {
                log!("Error to load image: {e}");
            } else {
                self.view.redraw(cx);
            }
    }
    fn clear_image(&mut self, cx: &mut Cx) {
        self.view.image(id!(image_view)).set_texture(cx, None);
    }
}

impl ImageViewerRef {
    pub fn insert_data(&self, text_or_image_uid: &WidgetUid, mxc_uri: OwnedMxcUri) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.insert_data(text_or_image_uid, mxc_uri);
        }
    }
    pub fn image_viewer_try_get_or_fetch(&self, text_or_image_uid: &WidgetUid) -> MediaCacheEntry {
        let mut inner = self.borrow_mut().unwrap();
        inner.image_viewer_try_get_or_fetch(text_or_image_uid)
    }
    pub fn find_and_load(&self, cx: &mut Cx, mxc_uri: &OwnedMxcUri) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.find_and_load(cx, mxc_uri);
        }
    }
    pub fn clear_image(&self, cx: &mut Cx) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.clear_image(cx);
        }
    }
    pub fn load_and_redraw(&self, cx: &mut Cx, data: &[u8]) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.load_and_redraw(cx, data);
        }
    }
}


pub fn image_viewer_insert_into_media_cache<D: Into<Arc<[u8]>>>(
    destination: &Mutex<MediaCacheEntry>,
    data: matrix_sdk::Result<D>,
    mxc_uri: OwnedMxcUri,
) {
    let mut finished = false;

    let new_value = match data {
        Ok(data) => {
            let data = data.into();
            finished = true;
            MediaCacheEntry::Loaded(data)
        }
        Err(e) => {
            error!("Failed to fetch media for {e:?}");
            MediaCacheEntry::Failed
        }
    };

    *destination.lock().unwrap() = new_value;

    if finished {
        Cx::post_action(ImageViewerAction::Fetched(mxc_uri));
    }
}
