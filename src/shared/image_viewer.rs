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
        debug: true
        visible: false
        width: Fill, height: Fill
        flow: Overlay
        show_bg: true
        draw_bg: {
            color: #00000075
        }

        image_view = <Image> {
            fit: Stretch,
            width: Fill, height: Fill,
            source: (IMG_DEFAULT_AVATAR),
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
pub struct ImageViewer {
    #[deref] view: View,
    #[rust] widgetref_image_uri_map: HashMap<WidgetUid, OwnedMxcUri>
}


#[derive(Clone, Debug, DefaultNone)]
pub enum ImageViewerAction {
    Open(WidgetUid),
    None,
}

impl Widget for ImageViewer {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        self.widget_match_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl WidgetMatchEvent for ImageViewer {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions, scope: &mut Scope) {
        if self.view.button(id!(close_button)).clicked(actions) {
            self.view.visible = false;
            self.view.redraw(cx);
        }
    }
}

impl ImageViewer {
    fn insert_date(&mut self, text_or_image_uid: WidgetUid, mx_uri: OwnedMxcUri) {
        self.widgetref_image_uri_map.insert(text_or_image_uid, mx_uri);
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
impl ImageViewerRef {
    pub fn insert_date(&self, text_or_image_uid: WidgetUid, mx_uri: OwnedMxcUri) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.insert_date(text_or_image_uid, mx_uri);
        }
    }

    pub fn show_and_fill_image(&self, cx: &mut Cx, text_or_image_uid: &WidgetUid, media_cache: &mut MediaCache) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.show_and_fill_image(cx, text_or_image_uid, media_cache);
        }
    }
}
