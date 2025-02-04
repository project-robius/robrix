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
        visible: false
        width: Fill, height: Fill
        align: {x: 0.5, y: 0.5}
        spacing: 12
        flow: Down
        show_bg: true
        draw_bg: {
            color: #00000080
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

        // Empty space to let `image_view` far from the bottom, which euqals to `close_button`'s height.
        <View> {
            height: 20
        }
    }
}

#[derive(Live, LiveHook, Widget)]
pub struct ImageViewer {
    #[deref] view: View,
    /// Key is `TextOrImage`'s uid.
    #[rust] widgetref_image_uri_map: HashMap<WidgetUid, OwnedMxcUri>,
    /// We use a standalone `MediaCache` to store the image data.
    #[rust] media_cache: MediaCache,
}


#[derive(Clone, Debug, DefaultNone)]
pub enum ImageViewerAction {
    SetData {text_or_image_uid: WidgetUid, mxc_uri: OwnedMxcUri},
    ImageClicked(WidgetUid),
    ///We post this action on fetching the image
    ///which is clicked by user first time (not in `media_cache` currently) in timeline.
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
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        if self.view.button(id!(close_button)).clicked(actions) {
            // Clear the image cache once the modal is closed.
            self.close(cx);
            self.clear_image(cx);
        }

        for action in actions {
            match action.downcast_ref() {
                Some(ImageViewerAction::SetData{text_or_image_uid, mxc_uri}) => {
                //We restore image message id and the image inside the message's mx_uri into HashMap.
                    self.insert_data(text_or_image_uid, mxc_uri.clone());
                }
                // We open the image viewer modal and show the image once the status of `text_or_image` is image and it was clicked.
                Some(ImageViewerAction::ImageClicked(text_or_image_uid)) => {
                    self.clear_image(cx);
                    self.open(cx);
                    if let MediaCacheEntry::Loaded(data) = self.image_viewer_try_get_or_fetch(text_or_image_uid) {
                        self.load_and_redraw(cx, &data);
                    }
                }
                Some(ImageViewerAction::Fetched(mxc_uri)) => {
                    self.find_and_load(cx, mxc_uri)
                }
                 _ => { }
            }
        }
    }
}

impl ImageViewer {
    fn open(&mut self, cx: &mut Cx) {
        self.visible = true;
        self.view.redraw(cx);
    }
    fn close(&mut self, cx: &mut Cx) {
        self.visible = false;
        self.view.redraw(cx);
    }
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
