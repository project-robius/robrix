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
        width: Fill, height: Fill
        flow: Overlay
        show_bg: true
        draw_bg: {
            color: #00000075
        }

        close_button = <RobrixIconButton> {
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

        image_view = <Image> {
            fit: Stretch,
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
    #[rust] widgetref_image_uri_map: HashMap<WidgetUid, OwnedMxcUri>
}


#[derive(Clone, Debug, DefaultNone)]
pub enum ImageViewerAction {
    Insert(WidgetUid, OwnedMxcUri),
    Show(WidgetUid),
    None,
}

impl Widget for ImageViewerModal {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.match_event(cx, event);
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}


impl ImageViewerModal {
    fn insert_data(&mut self, text_or_image_uid: &WidgetUid, mx_uri: OwnedMxcUri) {
        self.widgetref_image_uri_map.insert(*text_or_image_uid, mx_uri);
        log!("Inserted");
    }
    fn show_and_fill_image(&mut self, cx: &mut Cx, text_or_image_uid: &WidgetUid, media_cache: &mut MediaCache) {
        if let Some(mxc_uri) = self.widgetref_image_uri_map.get(text_or_image_uid) {
            log!("Some!");
            match media_cache.try_get_media_or_fetch(mxc_uri.clone(), None) {
                MediaCacheEntry::Loaded(data) => {
                    log!("Receive origin image");
                    self.view.visible = true;
                    self.view.redraw(cx);

                    let image_view = self.view.image(id!(image_view));

                    if let Err(e) = utils::load_png_or_jpg(&image_view, cx, &data) {
                        log!("Error to load image: {e}");
                    } else {
                        log!("Success");
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
    pub fn insert_data(&self, text_or_image_uid: &WidgetUid, mx_uri: OwnedMxcUri) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.insert_data(text_or_image_uid, mx_uri);
        }
    }
    pub fn show_and_fill_image(&self, cx: &mut Cx, text_or_image_uid: &WidgetUid, media_cache: &mut MediaCache) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.show_and_fill_image(cx, text_or_image_uid, media_cache);
        }
    }
}



impl MatchEvent for ImageViewerModal {
    fn handle_actions(&mut self, _cx: &mut Cx, actions: &Actions) {
        for action in actions {
            if let Some(ImageViewerAction::Insert(uid, mx_uri)) = action.downcast_ref() {
                self.widgetref_image_uri_map.insert(*uid, mx_uri.clone());
            }
        }
    }
}
