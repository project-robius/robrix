use makepad_widgets::*;
use matrix_sdk::{media::MediaFormat, ruma::OwnedMxcUri};
use std::sync::{Arc, Mutex, OnceLock};

use crate::{
    media_cache::{get_media_cache, MediaCacheEntry},
    utils::{ImageFormat, load_png_or_jpg},
};

live_design! {
    use link::theme::*;
    use link::widgets::*;

    use crate::shared::styles::*;
    use crate::shared::icon_button::RobrixIconButton;

    pub ImageViewerModal = {{ImageViewerModal}} {
       
        image_modal = <Modal> {
            content: <RoundedView> {
                flow: Down
                width: 100
                height: 100
                padding: {top: 20, right: 20, bottom: 20, left: 20}
                spacing: 10

                show_bg: true
                draw_bg: {
                    color: #000
                    border_radius: 8.0
                }

                header = <View> {
                    width: Fill,
                    height: Fit,
                    flow: Right
                    padding: {bottom: 10}
                    align: {x: 1.0, y: 0.0}

                    // close_button = <RobrixIconButton> {
                    //     width: 30,
                    //     height: 30,
                    //     padding: 5,
                    //     draw_icon: {
                    //         svg_file: dep("crate://self/resources/icons/close.svg"),
                    //         color: #fff
                    //     }
                    //     icon_walk: {width: 16, height: 16}
                    //     draw_bg: {
                    //         color: #444,
                    //         border_radius: 4.0
                    //     }
                    // }
                }

                image_container = <View> {
                    width: Fill,
                    height: Fill,
                    align: {x: 0.5, y: 0.5}
                    
                    image = <Image> {
                        width: Fit,
                        height: Fit,
                        fit: Best,
                        source: EmptyTexture,
                    }
                    
                    loading_label = <Label> {
                        width: Fit,
                        height: Fit,
                        text: "Loading image...",
                        draw_text: {
                            text_style: <REGULAR_TEXT>{font_size: 14},
                            color: #fff
                        }
                    }
                    
                    error_label = <Label> {
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

#[derive(Live, Widget)]
pub struct ImageViewerModal {
    #[deref] view: View,
    #[rust] mxc_uri: Option<OwnedMxcUri>,
    #[rust] image_loaded: bool,
}

impl LiveHook for ImageViewerModal {
    fn after_new_from_doc(&mut self, _cx: &mut Cx) {
        self.mxc_uri = None;
        self.image_loaded = false;
    }
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
        //self.view.handle_event(cx, event, scope);
        
        if let Event::Actions(actions) = event {
            if self.view.button(id!(close_button)).clicked(actions) {
                cx.widget_action(
                    self.widget_uid(),
                    &scope.path,
                    ImageViewerModalAction::Close,
                );
            }
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        if let Some(mxc_uri) = &self.mxc_uri {
            if !self.image_loaded {
                self.try_load_image(cx, mxc_uri.clone());
            }
        }
        
        self.view.draw_walk(cx, scope, walk)
    }
}

impl ImageViewerModal {
    /// Set the image to display in the modal
    pub fn set_image(&mut self, cx: &mut Cx, mxc_uri: OwnedMxcUri) {
        self.mxc_uri = Some(mxc_uri);
        self.image_loaded = false;
        self.show_loading_state(cx);
    }

    /// Try to load the image from the media cache
    fn try_load_image(&mut self, cx: &mut Cx, mxc_uri: OwnedMxcUri) {
        let media_cache = get_media_cache();
        let mut cache = media_cache.lock().unwrap();
        
        // Try to get the full-size image first, fallback to thumbnail
        let (media_entry, _format) = cache.try_get_media_or_fetch(
            mxc_uri.clone(),
            MediaFormat::File,
        );
        
        match media_entry {
            MediaCacheEntry::Loaded(data) => {
                self.load_image_data(cx, &data);
                self.image_loaded = true;
            }
            MediaCacheEntry::Requested => {
                // Image is being fetched, keep showing loading state
                self.show_loading_state(cx);
            }
            MediaCacheEntry::Failed => {
                self.show_error_state(cx);
                self.image_loaded = true;
            }
        }
    }

    /// Load image data into the Image widget
    fn load_image_data(&mut self, cx: &mut Cx, data: &[u8]) {
        let image = self.view.image(id!(image));
        //let _format = ImageFormat::guess_format(data).unwrap_or(ImageFormat::Png);
        match load_png_or_jpg(&image, cx, data) {
            Ok(()) => {
                self.view.view(id!(loading_label)).set_visible(cx, false);
                self.view.view(id!(error_label)).set_visible(cx, false);
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
        self.view.view(id!(loading_label)).set_visible(cx, true);
        self.view.view(id!(error_label)).set_visible(cx, false);
        self.view.image(id!(image)).set_visible(cx, false);
    }

    /// Show error state
    fn show_error_state(&mut self, cx: &mut Cx) {
        self.view.view(id!(loading_label)).set_visible(cx, false);
        self.view.view(id!(error_label)).set_visible(cx, true);
        self.view.image(id!(image)).set_visible(cx, false);
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
    pub fn close(&self, _cx: &mut Cx) {
        // Reset the state
        if let Some(mut inner) = self.borrow_mut() {
            inner.mxc_uri = None;
            inner.image_loaded = false;
        }
    }
}
pub fn get_global_image_viewer_modal(cx: &mut Cx) -> ImageViewerModalRef {
    cx.get_global::<ImageViewerModalRef>().clone()
}

pub fn set_global_image_viewer_modal(cx: &mut Cx, modal: ImageViewerModalRef) {
    cx.set_global(modal);
}

#[derive(Clone, Debug, DefaultNone)]
pub enum ImageViewerAction {
    Clicked(OwnedMxcUri),
    Test,
    None
}