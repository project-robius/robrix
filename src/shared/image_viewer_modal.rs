use makepad_widgets::*;
use matrix_sdk::{media::MediaFormat, ruma::OwnedMxcUri};

use crate::{
    media_cache::{get_media_cache, MediaCacheEntry},
    utils::load_png_or_jpg,
};

live_design! {
    use link::theme::*;
    use link::widgets::*;

    use crate::shared::styles::*;
    use crate::shared::icon_button::RobrixIconButton;
    IMG_DEFAULT_AVATAR = dep("crate://self/resources/img/default_avatar.png")
    pub ImageViewerModal = {{ImageViewerModal}} {
        width: Fill, height: Fill
        image_modal = <Modal> {
            content: {
                <RoundedView> {
                    flow: Down
                    width: 700
                    height: 700
                    padding: 10
                    spacing: 0

                    show_bg: true
                    draw_bg: {
                        //color: #000
                        color: #ffffff
                        border_radius: 8.0
                    }
                    header = <View> {
                        width: Fill,
                        height: Fit
                        flow: Right
                        align: {x: 1.0, y: 0.0},
                        close_button = <RobrixIconButton> {
                            width: Fit,
                            height: Fit,
                            spacing: 0,
                            margin: 7,
                            padding: 15,

                            draw_bg: {
                                color: (COLOR_SECONDARY)
                            }
                            draw_icon: {
                                svg_file: (ICON_CLOSE),
                                fn get_color(self) -> vec4 {
                                    return #x0;
                                }
                            }
                            icon_walk: {width: 14, height: 14}
                        }
                    }
                    image_container = <View> {
                        width: Fill,
                        height: Fill,
                        align: {x: 0.5}
                        image = <Image> {
                            height: Fill
                            width: Fill
                            fit: Smallest,
                            source: (IMG_DEFAULT_AVATAR)
                        }
                    }
                    loading_view = <View> {
                        width: Fill,
                        height: Fit,
                        flow: Down,
                        align: {x: 0.5, y: 0.5},
                        spacing: 10
                        
                        loading_spinner = <LoadingSpinner> {
                            width: 40,
                            height: 40,
                            draw_bg: {
                                color: (COLOR_ACTIVE_PRIMARY)
                                border_size: 3.0,
                            }
                        }
                        
                        <Label> {
                            width: Fit,
                            height: 30,
                            text: "Loading image...",
                            draw_text: {
                                text_style: <REGULAR_TEXT>{font_size: 14},
                                color: (COLOR_TEXT)
                            }
                        }
                    }
                    error_label_view = <View> {
                        width: Fill,
                        height: 30
                        visible: false
                        loading_label = <Label> {
                            width: Fit,
                            height: Fit,
                            text: "Failed to load image",
                            draw_text: {
                                text_style: <REGULAR_TEXT>{font_size: 14},
                                color: #f44
                            }
                        }
                    }
                }
            }
        }
    }
}

#[derive(Live, Widget, LiveHook)]
pub struct ImageViewerModal {
    #[deref] view: View,
    #[rust] mxc_uri: Option<OwnedMxcUri>,
    #[rust] image_loaded: bool,
}

#[derive(Clone, Debug, DefaultNone)]
pub enum ImageViewerModalAction {
    Open {
        mxc_uri: OwnedMxcUri,
    },
    Close,
    None,
}

impl Widget for ImageViewerModal {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        
        if !self.image_loaded && self.mxc_uri.is_some() {
            if let Event::Signal = event {
                self.populate_image_viewer(cx, self.mxc_uri.clone().unwrap());
            }
        }
        if let Event::Actions(actions) = event {
            if self.view.button(id!(close_button)).clicked(actions) {
                self.close(cx);
            }
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl ImageViewerModal {
    /// Set the image to display in the modal
    pub fn set_image(&mut self, cx: &mut Cx, mxc_uri: OwnedMxcUri) {
        self.mxc_uri = Some(mxc_uri.clone());
        self.image_loaded = false;
        self.show_loading_state(cx);
        self.populate_image_viewer(cx, mxc_uri);
    }

    /// Try to load the image from the media cache
    fn populate_image_viewer(&mut self, cx: &mut Cx, mxc_uri: OwnedMxcUri) {
        let media_cache = get_media_cache();
        let mut cache = media_cache.lock().unwrap();
        
        // Try to get the full-size image first, fallback to thumbnail
        let (media_entry, format) = cache.try_get_media_or_fetch(
            mxc_uri.clone(),
            MediaFormat::File,
        );
        drop(cache);
        match (media_entry, format) {
            (MediaCacheEntry::Loaded(data), MediaFormat::File) => {
                self.load_image_data(cx, &data);
                self.image_loaded = true;
            }
            (MediaCacheEntry::Requested, _) | (MediaCacheEntry::Loaded(_), MediaFormat::Thumbnail(_))=> {
                // Image is being fetched, keep showing loading state
                self.show_loading_state(cx);
            }
            (MediaCacheEntry::Failed, _) => {
                self.show_error_state(cx);
                self.image_loaded = true;
            }
        }
    }

    /// Load image data into the Image widget
    fn load_image_data(&mut self, cx: &mut Cx, data: &[u8]) {
        let image = self.view.image(id!(image));
        match load_png_or_jpg(&image, cx, data) {
            Ok(()) => {
                self.view.view(id!(loading_view)).set_visible(cx, false);
                self.view.view(id!(error_label_view)).set_visible(cx, false);
                self.view.image(id!(image)).set_visible(cx, true);
            }
            Err(e) => {
                error!("Failed to load image: {:?}", e);
                self.show_error_state(cx);
            }
        }
    }

    /// Show loading state
    fn show_loading_state(&mut self, cx: &mut Cx) {
        self.view.view(id!(loading_view)).set_visible(cx, true);
        self.view.view(id!(error_label_view)).set_visible(cx, false);
        self.view.image(id!(image)).set_visible(cx, false);
    }

    /// Show error state
    fn show_error_state(&mut self, cx: &mut Cx) {
        self.view.view(id!(loading_view)).set_visible(cx, false);
        self.view.view(id!(error_label_view)).set_visible(cx, true);
        self.view.image(id!(image)).set_visible(cx, false);
    }
    fn close(&mut self, cx: &mut Cx) {
        self.mxc_uri = None;
        self.image_loaded = false;
        self.modal(id!(image_modal)).close(cx);
    }
}

impl ImageViewerModalRef {
    /// Open the modal with the given image
    pub fn open(&self, cx: &mut Cx, mxc_uri: Option<OwnedMxcUri>) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.modal(id!(image_modal)).open(cx);
            if let Some(uri) = mxc_uri {
                inner.set_image(cx, uri);
            }
        }
    }

    /// Close the modal
    pub fn close(&self, cx: &mut Cx) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.close(cx);
        }
    }
}
pub fn get_global_image_viewer_modal(cx: &mut Cx) -> ImageViewerModalRef {
    cx.get_global::<ImageViewerModalRef>().clone()
}

pub fn set_global_image_viewer_modal(cx: &mut Cx, modal: ImageViewerModalRef) {
    cx.set_global(modal);
}
