use std::collections::BTreeMap;
use std::sync::Mutex;
use std::sync::Arc;

use makepad_widgets::*;

use matrix_sdk::media::{MediaFormat, MediaRequest};
use matrix_sdk::ruma::OwnedMxcUri;

use crate::home::room_screen::TimelineUpdate;
use crate::shared::text_or_image::TextOrImageAction;
use crate::{
    media_cache::{MediaCache, MediaCacheEntry},
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

#[derive(Live, LiveHook, Widget)]
pub struct ImageViewer {
    #[deref]
    view: View,
    /// Key is uid of `TextOrImage`, val is the corresponded image uri and its thumbnail data.
    #[rust]
    map: BTreeMap<WidgetUid, OriginalAndThumbnail>,
    /// We use a standalone `MediaCache` to store the original image data.
    #[rust]
    media_cache: MediaCache,
}
#[derive(Debug, Clone)]
pub struct OriginalAndThumbnail {
    original_uri: OwnedMxcUri,
    thumbnail_data: Arc<[u8]>,
}


impl OriginalAndThumbnail {
    pub const fn new(original_uri: OwnedMxcUri, thumbnail_data: Arc<[u8]>) -> Self {
        Self {original_uri, thumbnail_data}
    }
}

#[derive(Clone, Debug, DefaultNone)]
pub enum ImageViewerAction {
    SetData {
        text_or_image_uid: WidgetUid,
        original_thumbnail: OriginalAndThumbnail,
    },
    ///We post this action on fetching the image
    ///which is clicked by user first time (not in `media_cache` currently) in timeline.
    Fetched(Arc<[u8]>),
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
            if let Some(TextOrImageAction::ClickImage(text_or_image_uid)) = action.downcast_ref() {
                self.open(cx);
                let thumbnail_data = self.map.get(text_or_image_uid).unwrap().thumbnail_data.clone();
                self.load_with_data(cx, &thumbnail_data);

                let original_uri = self.map.get(text_or_image_uid).unwrap().original_uri.clone();
                match self.media_cache.try_get_media_or_fetch(original_uri, Some(MediaFormat::File), image_viewer_insert_into_cache) {
                    MediaCacheEntry::Loaded(data) => {
                        self.load_with_data(cx, &data);
                    },
                    MediaCacheEntry::Requested => { }
                    MediaCacheEntry::Failed => { }
                }
            }

            match action.downcast_ref() {
                Some(ImageViewerAction::SetData {
                    text_or_image_uid,
                    original_thumbnail,
                },) => {
                    self.set_data(text_or_image_uid, original_thumbnail)
                }
                Some(ImageViewerAction::Fetched(data)) => {
                    self.load_with_data(cx, data)
                }
                _ => {}
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
        original_thumbnail: &OriginalAndThumbnail
    ) {
        self.map.insert(*text_or_image_uid, original_thumbnail.clone());
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

fn image_viewer_insert_into_cache<D: Into<Arc<[u8]>>>(
    value_ref: &Mutex<MediaCacheEntry>,
    _request: MediaRequest,
    data: matrix_sdk::Result<D>,
    update_sender: Option<crossbeam_channel::Sender<TimelineUpdate>>,
) {
    let new_value = match data {
        Ok(data) => {
            let data: Arc<[u8]> = data.into();
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
