use std::collections::HashMap;
use makepad_widgets::*;
use matrix_sdk::ruma::OwnedMxcUri;

use crate::{media_cache::{MediaCache, MediaCacheEntry}, utils};

live_design! {
    use link::theme::*;
    use link::widgets::*;

    use crate::shared::styles::*;
    use crate::shared::icon_button::RobrixIconButton;

    pub ImageViewerModal = {{ImageViewerModal}} {
        width: 700, height: 700
        flow: Overlay
        show_bg: true
        draw_bg: {
            color: #00000075
        }

        <View> {
            align: {x: 1.0, y: 0.0}
            width: Fill, height: Fill
            close_button = <RobrixIconButton> {
                debug = true
                padding: {left: 15, right: 15}
                draw_icon: {
                    svg_file: (ICON_CLOSE)
                    color: (COLOR_DANGER_RED),
                }
                icon_walk: {width: 16, height: 16, margin: {left: -2, right: -1} }

                draw_bg: {
                    border_color: (COLOR_DANGER_RED),
                    color: #fff0f0 // light red
                }
            }
        }


        image_view = <Image> {
            debug = true
            fit: Smallest,
            width: Fill, height: Fill,
            // draw_bg: {
            //     fn pixel(self) -> vec4 {
            //         let maxed = max(self.rect_size.x, self.rect_size.y);
            //         let sdf = Sdf2d::viewport(self.pos * vec2(maxed, maxed));
            //         let r = maxed * 0.5;
            //         sdf.circle(r, r, r);
            //         sdf.fill_keep(self.get_color());
            //         return sdf.result
            //     }
            // }
        }
    }
}

#[derive(Live, LiveHook, Widget)]
pub struct ImageViewerModal {
    #[deref] view: View,
    #[rust] widgetref_image_uri_map: HashMap<WidgetUid, OwnedMxcUri>,
    #[rust] media_cache: Option<MediaCache>,
}


#[derive(Clone, Debug, DefaultNone)]
pub enum ImageViewerAction {
    SetMediaCache(MediaCache),
    Insert(WidgetUid, OwnedMxcUri),
    Show(WidgetUid),
    None,
}

impl Widget for ImageViewerModal {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        // self.match_event(cx, event);
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}


impl ImageViewerModal {
    fn set_media_cache(&mut self, media_cache: MediaCache) {
        self.media_cache = Some(media_cache);
        log!("Set media cache")
    }
    fn insert_data(&mut self, text_or_image_uid: &WidgetUid, mx_uri: OwnedMxcUri) {
        self.widgetref_image_uri_map.insert(*text_or_image_uid, mx_uri);
        log!("Inserted");
    }
    fn show_and_fill_image(&mut self, cx: &mut Cx, text_or_image_uid: &WidgetUid) {
        if let Some(mxc_uri) = self.widgetref_image_uri_map.get(text_or_image_uid) {
            log!("Some!");
            match self.media_cache.as_mut().unwrap().try_get_media_or_fetch(mxc_uri.clone(), None) {
                MediaCacheEntry::Loaded(data) => {
                    log!("Receive origin image");
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
}

impl ImageViewerModalRef {
    pub fn set_media_cache(&mut self, media_cache: MediaCache) {
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
}


// impl MatchEvent for ImageViewerModal {
//     fn handle_actions(&mut self, _cx: &mut Cx, actions: &Actions) {
//         for action in actions {
//             if let Some(ImageViewerAction::SetMediaCache(media_cache)) = action.downcast_ref() {
//                 log!("Set Media Cache");
//                 self.media_cache = Some(media_cache.clone())
//             }
//         }
//     }
// }
