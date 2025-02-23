use std::ops::Deref;
use std::sync::Mutex;
use std::time::SystemTime;
use std::{
    collections::{btree_map::Entry, HashMap},
    sync::Arc,
};

use makepad_widgets::*;
use matrix_sdk::media::MediaRequest;
use matrix_sdk::ruma::events::room::MediaSource;
use matrix_sdk::ruma::OwnedMxcUri;

use crate::home::room_screen::TimelineUpdate;
use crate::shared::text_or_image::TextOrImageAction;
use crate::{
    media_cache::MediaCacheEntry,
    sliding_sync::{self, MatrixRequest},
    utils,
};

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::styles::*;
    use crate::shared::icon_button::RobrixIconButton;

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
                icon_walk: {width: 25, height: 25, margin: {left: -1, right: -1} }

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
    }
}

#[derive(Clone, Debug, DefaultNone)]
pub enum ImageViewerAction {
    SetData {
        text_or_image_uid: WidgetUid,
        thumbnail_and_original_image_uri: ThumbnailAndOriginalImageUri,
    },
    ///We post this action on fetching the image
    ///which is clicked by user first time (not in `media_cache` currently) in timeline.
    Fetched(Arc<[u8]>),
    None,
}

#[derive(Live, LiveHook, Widget)]
pub struct ImageViewer {
    #[deref]
    view: View,
    /// Key is uid of `TextOrImage`, val is the corresponded image uri and its thumbnail data.
    #[rust]
    image_uid_mxc_uri_map: HashMap<WidgetUid, ThumbnailAndOriginalImageUri>,
    #[rust]
    image_uid_thumbnail_data_map: HashMap<WidgetUid, Arc<[u8]>>,
}

#[derive(Clone, Debug)]
pub struct ThumbnailAndOriginalImageUri {
    original_uri: OwnedMxcUri,
    thumbnail_uri: Option<OwnedMxcUri>,
}

impl ThumbnailAndOriginalImageUri {
    pub const fn new(original_uri: OwnedMxcUri, thumbnail_uri: Option<OwnedMxcUri>) -> Self {
        Self {original_uri, thumbnail_uri }
    }
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
                Some(ImageViewerAction::SetData {
                    text_or_image_uid,
                    thumbnail_and_original_image_uri
                }) => {
                    self.set_data(text_or_image_uid, thumbnail_and_original_image_uri);
                }
                Some(ImageViewerAction::Fetched(mxc_uri)) => {
                    self.find_to_load(cx, mxc_uri);
                }
                _ => {}
            }

            if let Some(TextOrImageAction::ImageClicked(text_or_image_uid)) = action.downcast_ref() {
                self.open(cx);
                //Todo: show a spin loader before the image is loaded.
                match self.image_viewer_try_get_or_fetch(text_or_image_uid) {
                    MediaCacheEntry::Loaded(data) => {
                        self.load_with_data(cx, &data);
                    }
                    MediaCacheEntry::Requested => {
                        let image_uid_thumbnail_data_map =
                            self.image_uid_thumbnail_data_map.clone();

                        let Some(thumbnail_data) =
                            image_uid_thumbnail_data_map.get(text_or_image_uid) else { return };

                        self.load_with_data(cx, thumbnail_data);
                    }
                    MediaCacheEntry::Failed => {
                        // TODO
                    }
                }
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
    fn set_data(
        &mut self,
        text_or_image_uid: &WidgetUid,
        thumbnail_and_original_image_uri: &ThumbnailAndOriginalImageUri,
    ) {
        self.image_uid_mxc_uri_map
            .insert(*text_or_image_uid, thumbnail_and_original_image_uri.clone());
    }
    /// We find mx_uid via the given `text_or_image_uid`.
    fn image_viewer_try_get_or_fetch(
        &mut self,
        text_or_image_uid: &WidgetUid,
    ) -> MediaCacheEntry {
        let Some(mxc_uri) = self.image_uid_mxc_uri_map.get(text_or_image_uid) else {
            return MediaCacheEntry::Failed;
        };

        match self.media_cache.entry(mxc_uri.clone()) {
            Entry::Vacant(vacant) => {
                let destination = vacant.insert(Arc::new(Mutex::new(MediaCacheEntry::Requested)));
                sliding_sync::submit_async_request(MatrixRequest::FetchMedia {
                    media_request: (),
                    on_fetched: (),
                    destination: (),
                    update_sender: ()
                });

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
        if let Err(e) = utils::load_png_or_jpg(&image, cx, data) {
            log!("Error to load image: {e}");
        } else {
            self.view.redraw(cx);
        }
    }
}

/// Insert data into a previously-requested media cache entry.
pub fn f1<D: Into<Arc<[u8]>>>(
    value_ref: &Mutex<MediaCacheEntry>,
    _request: MediaRequest,
    data: matrix_sdk::Result<D>,
    update_sender: Option<crossbeam_channel::Sender<TimelineUpdate>>,
) {
    let new_value = match data {
        Ok(data) => {
            let data = data.into();

            // debugging: dump out the media image to disk
            if false {
                if let MediaSource::Plain(mxc_uri) = _request.source {
                    log!("Fetched media for {mxc_uri}");
                    let mut path = crate::temp_storage::get_temp_dir_path().clone();
                    let filename = format!("{}_{}_{}",
                        SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_millis(),
                        mxc_uri.server_name().unwrap(), mxc_uri.media_id().unwrap(),
                    );
                    path.push(filename);
                    path.set_extension("png");
                    log!("Writing user media image to disk: {:?}", path);
                    std::fs::write(path, &data)
                        .expect("Failed to write user media image to disk");
                }
            }
            Cx::post_action(ImageViewerAction::Fetched(data.clone()));
            MediaCacheEntry::Loaded(data)
        }
        Err(e) => {
            error!("Failed to fetch media for {:?}: {e:?}", _request.source);
            MediaCacheEntry::Failed
        }
    };
    *value_ref.lock().unwrap() = new_value;

    if let Some(sender) = update_sender {
        let _ = sender.send(TimelineUpdate::MediaFetched);
    }
    SignalToUI::set_ui_signal();
}
