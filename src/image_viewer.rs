use std::{collections::{btree_map::Entry, HashMap}, sync::Arc, time::Instant};
use std::sync::Mutex;

use makepad_widgets::*;

use matrix_sdk::ruma::OwnedMxcUri;

use crate::{media_cache::{MediaCache, MediaCacheEntry}, sliding_sync::{self, MatrixRequest}, utils};

live_design! {
    use link::theme::*;
    use link::widgets::*;

    use crate::shared::styles::*;
    use crate::shared::icon_button::RobrixIconButton;

    SPIN_LOADER = dep("crate://self/resources/icons/loading1.svg")

    pub ImageViewer = {{ImageViewer}} {
        visible: false
        width: Fill, height: Fill
        align: {x: 0.5, y: 0.5}
        spacing: 12
        flow: Overlay
        show_bg: true
        draw_bg: {
            color: (COLOR_IMAGE_VIEWER_BG)
        }

        <View> {
            align: {x: 1.0, y: 0.0}
            width: Fill, height: Fill
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

        image_view = <View> {
            padding: {top: 40, bottom: 30, left: 20, right: 20}
            flow: Overlay
            align: {x: 0.5, y: 0.5}
            width: Fill, height: Fill,
            image = <Image> {
                width: Fill, height: Fill,
                fit: Smallest,
            }
        }

        spin_loader = <View> {
            visible: false
            align: {x: 0.5, y: 0.5}
            width: Fill, height: Fill,
            <Icon> {
                align: {x: 0.5, y: 0.5}
                width: Fill, height: Fill,
                draw_icon: {
                    svg_file: (SPIN_LOADER)
                }
                icon_walk: {width: 400, height: Fit}
            }
        }
    }
}

#[derive(Live, LiveHook, Widget)]
pub struct ImageViewer {
    #[deref] view: View,
    /// Key is uid of `TextOrImage`, val is the corresponded image uri and its thumbnail data.
    #[rust] image_uid_mxc_uri_map: HashMap<WidgetUid, OwnedMxcUri>,
    #[rust] image_uid_thumbnail_data_map: HashMap<WidgetUid, Arc<[u8]>>,
    /// We use a standalone `MediaCache` to store the original image data.
    #[rust] media_cache: MediaCache,
}

#[derive(Clone, Debug, DefaultNone)]
pub enum ImageViewerAction {
    SetData {text_or_image_uid: WidgetUid, mxc_uri: OwnedMxcUri, thumbnail_data: Arc<[u8]>},
    ImageClicked(WidgetUid),
    ///We post this action on fetching the image
    ///which is clicked by user first time (not in `media_cache` currently) in timeline.
    Fetched(OwnedMxcUri),
    None,
}

impl Widget for ImageViewer {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        let whole_area = self.view.area();
        let image_area = self.view.image(id!(image_view.image)).area();

        // click the blank area, close image viewer; click image area, nothing happen.
        event.hits(cx, image_area);
        if let Hit::FingerUp(fe) = event.hits(cx, whole_area) {
            if fe.was_tap() {
                // Once Clicking, we close image viewer.
                self.close(cx);
            }
        }

        self.match_event(cx, event);
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}
impl MatchEvent for ImageViewer {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        if self.view.button(id!(close_button)).clicked(actions) {
            // Clear the image cache once the modal is closed.
            self.close(cx);
        }

        for action in actions {
            match action.downcast_ref() {
                Some(ImageViewerAction::SetData{text_or_image_uid, mxc_uri, thumbnail_data}) => {
                    self.set_data(text_or_image_uid, mxc_uri, thumbnail_data);
                }
                Some(ImageViewerAction::ImageClicked(text_or_image_uid)) => {
                    self.open(cx);
                    //Todo: show a spin loader before the image is loaded.
                    match self.image_viewer_try_get_or_fetch(cx, text_or_image_uid) {
                        MediaCacheEntry::Loaded(data) => {
                            self.load_with_data(cx, &data);
                        }
                        MediaCacheEntry::Requested => {
                            log!("MediaCacheEntry::Requested");
                            let image_uid_thumbnail_data_map = self.image_uid_thumbnail_data_map.clone();
                            let Some(thumbnail_data) = image_uid_thumbnail_data_map.get(text_or_image_uid)
                                else { return };
                            self.view.view(id!(spin_loader)).set_visible(cx, true);
                            self.load_with_data(cx, thumbnail_data);
                        }
                        MediaCacheEntry::Failed => {
                            // TODO
                        }
                    }

                }
                Some(ImageViewerAction::Fetched(mxc_uri)) => {
                    self.view.view(id!(spin_loader)).set_visible(cx, false);
                    self.find_to_load(cx, mxc_uri);
                }
                 _ => { }
            }
        }
    }
}

impl ImageViewer {
    fn open(&mut self, cx: &mut Cx) {
        self.visible = true;
        self.redraw(cx);
    }
    fn close(&mut self, cx: &mut Cx) {
        self.visible = false;
        self.view.image(id!(image)).set_texture(cx, None);
        self.redraw(cx);
    }
    /// We restore image message uid and the image inside the message's mx_uri into HashMap
    /// when the message is being populated.
    fn set_data(&mut self, text_or_image_uid: &WidgetUid, mxc_uri: &OwnedMxcUri, thumbnail_data: &Arc<[u8]>) {
        self.image_uid_mxc_uri_map.insert(*text_or_image_uid, mxc_uri.clone());
        self.image_uid_thumbnail_data_map.insert(*text_or_image_uid, thumbnail_data.clone());
        log!("Inserted");
    }
    /// We find mx_uid via the given `text_or_image_uid`.
    fn image_viewer_try_get_or_fetch(
        &mut self,
        cx: &mut Cx,
        text_or_image_uid: &WidgetUid,
    ) -> MediaCacheEntry {
        let Some(mxc_uri) = self.image_uid_mxc_uri_map.get(text_or_image_uid) else {return MediaCacheEntry::Failed};

        match self.media_cache.entry(mxc_uri.clone()) {
            Entry::Vacant(vacant) => {
                self.view.view(id!(spin_loader)).set_visible(cx, true);

                let destination = vacant.insert(Arc::new(Mutex::new(MediaCacheEntry::Requested)));
                sliding_sync::submit_async_request(
                    MatrixRequest::FetchOriginalMedia {
                    destination: destination.clone(),
                    mxc_uri: mxc_uri.clone()
                    }
                );

                MediaCacheEntry::Requested
            }
            Entry::Occupied(occupied) => occupied.get().lock().unwrap().clone(),
        }
    }
    fn find_to_load(&mut self, cx: &mut Cx, mxc_uri: &OwnedMxcUri) {
            if let Some(MediaCacheEntry::Loaded(data)) = self.media_cache.try_get_media(mxc_uri) {
                self.load_with_data(cx, &data);
            }
        }
    fn load_with_data(&mut self, cx: &mut Cx, data: &[u8]) {
        let image = self.view.image(id!(image_view.image));

        let start = Instant::now();

        if let Err(e) = utils::load_png_or_jpg(&image, cx, data) {
            log!("Error to load image: {e}");
        } else {
            log!("Success loaded");
            self.view.redraw(cx);
        }
        let duration = start.elapsed();
        println!("time coust: {:?}", duration);
    }
}

pub fn image_viewer_insert_into_media_cache<D: Into<Arc<[u8]>>>(
    destination: &Mutex<MediaCacheEntry>,
    data: matrix_sdk::Result<D>,
    mxc_uri: OwnedMxcUri,
) {
    match data {
        Ok(data) => {
            let data = data.into();
            *destination.lock().unwrap() = MediaCacheEntry::Loaded(data);
            Cx::post_action(ImageViewerAction::Fetched(mxc_uri));
        }
        Err(e) => {
            error!("Failed to fetch media for {e:?}");
            *destination.lock().unwrap() = MediaCacheEntry::Failed
        }
    };
}
