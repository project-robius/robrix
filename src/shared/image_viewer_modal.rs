use makepad_widgets::*;
use matrix_sdk::{media::MediaFormat, ruma::OwnedMxcUri};

use crate::{
    media_cache::{get_media_cache, MediaCacheEntry},
    utils::load_png_or_jpg,
};

// Image loading timeout in seconds (10 seconds)
const IMAGE_LOAD_TIMEOUT: f64 = 10.0;

live_design! {
    use link::theme::*;
    use link::widgets::*;
    IMAGE_DEFAULT = dep("crate://self/resources/img/default_image.png");
    use crate::shared::styles::*;
    use crate::shared::icon_button::RobrixIconButton;
    pub ImageViewerModal = {{ImageViewerModal}} {
        width: Fill, height: Fill
        image_modal = <Modal> {
            content: <View> {
                flow: Overlay
                width: Fill
                height: Fill
                padding: 5
                spacing: 0

                show_bg: true
                draw_bg: {
                    color: #000
                }
                image_container = <View> {
                    width: Fill,
                    height: Fill,
                    flow: Down,
                    align: {x: 0.5}
                    image = <Image> {
                        height: Fill
                        width: Fill
                        fit: Smallest,
                        source: (IMAGE_DEFAULT)
                    }
                }
                <View> {
                    width: Fill,
                    height: Fill,
                    flow: Down,
                    align: {x: 0.5}
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
                    <View> {
                        width: Fill,
                        height: Fill,
                        align: {x: 0.5, y: 0.5}
                        flow: Down
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
                                    color: (COLOR_PRIMARY)
                                    border_size: 3.0,
                                }
                            }
                            
                            <Label> {
                                width: Fit,
                                height: 30,
                                text: "Loading image...",
                                draw_text: {
                                    text_style: <REGULAR_TEXT>{font_size: 14},
                                    color: (COLOR_PRIMARY)
                                }
                            }
                        }
                        error_label_view = <View> {
                            width: Fill,
                            height: Fit,
                            flow: Down,
                            align: {x: 0.5, y: 0.5},
                            spacing: 10
                            <Icon> {
                                draw_icon: {
                                    svg_file: (ICON_FORBIDDEN),
                                    color: #ffffff,
                                }
                                icon_walk: { width: 50, height: 50 }
                            }
    
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
                        timeout_label_view = <View> {
                            width: Fill,
                            height: Fit,
                            flow: Down,
                            align: {x: 0.5, y: 0.5},
                            spacing: 10
                            <Icon> {
                                draw_icon: {
                                    svg_file: (ICON_WARNING),
                                    color: #ffffff,
                                }
                                icon_walk: { width: 50, height: 50 }
                            }
    
                            loading_label = <Label> {
                                width: Fit,
                                height: Fit,
                                text: "Timeout loading image",
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
}

#[derive(Live, Widget, LiveHook)]
pub struct ImageViewerModal {
    #[deref] view: View,
    #[rust] mxc_uri: Option<OwnedMxcUri>,
    #[rust] image_loaded: bool,
    #[rust] timeout_timer: Timer,
    #[rust] is_timed_out: bool,
    #[live(false)] visible: bool
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
        if !self.visible {
            return;
        }
        self.view.handle_event(cx, event, scope);
        
        if self.timeout_timer.is_event(event).is_some() {
            cx.stop_timer(self.timeout_timer);
            self.show_timeout_state(cx);
        }
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
        if !self.visible {
            return DrawStep::done();
        }
        self.view.draw_walk(cx, scope, walk)
    }
}

impl ImageViewerModal {
    /// Set the image to display in the modal
    fn set_image(&mut self, cx: &mut Cx, mxc_uri: OwnedMxcUri) {
        self.mxc_uri = Some(mxc_uri.clone());
        self.image_loaded = false;
        self.is_timed_out = false;
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
                cx.stop_timer(self.timeout_timer);
            }
            (MediaCacheEntry::Requested, _) | (MediaCacheEntry::Loaded(_), MediaFormat::Thumbnail(_))=> {
                self.show_loading_state(cx);
                self.timeout_timer = cx.start_timeout(IMAGE_LOAD_TIMEOUT);
            }
            (MediaCacheEntry::Failed, _) => {
                self.show_error_state(cx);
                self.image_loaded = true;
                cx.stop_timer(self.timeout_timer);
            }
        }
    }

    /// Load image data into the Image widget
    fn load_image_data(&mut self, cx: &mut Cx, data: &[u8]) {
        let image_ref = self.view.image(id!(image));
        match load_png_or_jpg(&image_ref, cx, data) {
            Ok(()) => {
                self.view.view_set(ids!(loading_view, error_label_view, timeout_label_view)).set_visible(cx, false);
                image_ref.set_visible(cx, true);
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
        self.view.view_set(ids!(error_label_view, timeout_label_view)).set_visible(cx, false);
        self.view.image(id!(image)).set_visible(cx, false);
    }

    /// Show error state
    fn show_error_state(&mut self, cx: &mut Cx) {
        self.view.view(id!(error_label_view)).set_visible(cx, true);
        self.view.view_set(ids!(loading_view, timeout_label_view)).set_visible(cx, false);
        self.view.image(id!(image)).set_visible(cx, false);
    }

    /// Show timeout state
    fn show_timeout_state(&mut self, cx: &mut Cx) {
        self.view.view(id!(timeout_label_view)).set_visible(cx, true);
        self.view.view_set(ids!(loading_view, error_label_view)).set_visible(cx, false);
        self.view.image(id!(image)).set_visible(cx, false);
    }

    /// Close the modal
    fn close(&mut self, cx: &mut Cx) {
        self.mxc_uri = None;
        self.image_loaded = false;
        self.is_timed_out = false;
        self.modal(id!(image_modal)).close(cx);
        cx.stop_timer(self.timeout_timer);
    }
}

impl ImageViewerModalRef {
    /// Open the modal with the given image URI.
    pub fn open(&self, cx: &mut Cx, mxc_uri: Option<OwnedMxcUri>) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.visible = true;
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
            inner.visible = false;
        }
    }
}
pub fn get_global_image_viewer_modal(cx: &mut Cx) -> ImageViewerModalRef {
    cx.get_global::<ImageViewerModalRef>().clone()
}

pub fn set_global_image_viewer_modal(cx: &mut Cx, modal: ImageViewerModalRef) {
    cx.set_global(modal);
}
