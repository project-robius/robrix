use std::collections::HashMap;
use makepad_widgets::*;
use matrix_sdk::ruma::OwnedMxcUri;

use crate::{media_cache::{MediaCache, MediaCacheEntry}, utils};

live_design! {
    use link::theme::*;
    use link::widgets::*;
    use crate::shared::styles::*;
    use crate::shared::icon_button::RobrixIconButton;

        CloseButton = <RobrixIconButton> {
            padding: {left: 15, right: 15}
            draw_icon: {
                svg_file: (ICON_BLOCK_USER)
                color: (COLOR_DANGER_RED),
            }
            icon_walk: {width: 16, height: 16, margin: {left: -2, right: -1} }

            draw_bg: {
                border_color: (COLOR_DANGER_RED),
                color: #fff0f0 // light red
            }
            text: "Cancel"
            draw_text:{
                color: (COLOR_DANGER_RED),
            }
        }

    pub ImageViewer = {{ImageViewer}} {
        width: Fit, height: Fit
        flow: Overlay

        close_button = <CloseButton> {}

        image_view = <Image> { }
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
            match media_cache.try_get_media_or_fetch(mxc_uri.clone(), None) {
                MediaCacheEntry::Loaded(data) => {
                    let image_view = self.view.image(id!(image_view));

                    utils::load_png_or_jpg(&image_view, cx, &data).unwrap();
                }
                MediaCacheEntry::Requested => {

                }
                MediaCacheEntry::Failed => {

                }
            };
        };
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
