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
    IMG_DEFAULT = dep("crate://self/resources/img/default_image.png")
    
    ZoomableImage = {{ZoomableImage}} {
        width: Fill,
        height: Fill,
        flow: Overlay,
        
        // Transform state
        scale: 1.0,
        offset: (0.0, 0.0),
        
        // Interaction state
        dragging: false,
        last_mouse_pos: (0.0, 0.0),
        
        // Internal image widget to hold the actual texture
        inner_image = <Image> {
            width: Fill,
            height: Fill,
            fit: Smallest,
            source: (IMG_DEFAULT)
        }
    }
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
                    border_radius: 8.0
                }
                image_container = <View> {
                    width: Fill,
                    height: Fill,
                    flow: Overlay,
                    align: {x: 0.5, y: 0.5}
                    zoomable_image = <ZoomableImage> {
                        width: Fill,
                        height: Fill,
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

#[derive(Live, LiveHook, Widget)]
pub struct ZoomableImage {
    #[deref] 
    view: View,
    
    // Transform state
    #[live] 
    scale: f64,
    #[live] 
    offset: DVec2,
    
    // Interaction state
    #[live] 
    dragging: bool,
    #[live] 
    last_mouse_pos: DVec2,
}

impl Widget for ZoomableImage {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        
        match event {
            Event::MouseDown(e) => {
                if e.button.is_primary() {
                    self.dragging = true;
                    self.last_mouse_pos = e.abs;
                    cx.set_key_focus(self.view.area());
                    self.redraw(cx);
                }
            }
            Event::MouseUp(e) => {
                if e.button.is_primary() {
                    self.dragging = false;
                    self.redraw(cx);
                }
            }
            Event::MouseMove(e) => {
                if self.dragging {
                    let delta = e.abs - self.last_mouse_pos;
                    
                    // Scale delta by inverse of zoom to maintain consistent pan speed  
                    let scaled_delta = delta / self.scale;
                    self.offset += scaled_delta;
                    
                    self.last_mouse_pos = e.abs;
                    self.redraw(cx);
                }
            }
            Event::Scroll(e) => {
                // Zoom in/out with mouse wheel
                let zoom_factor = if e.scroll.y > 0.0 { 1.1 } else { 0.9 };
                let old_scale = self.scale;
                self.scale *= zoom_factor;
                self.scale = self.scale.clamp(0.1, 10.0); // Clamp zoom level
                
                if self.scale != old_scale {
                    // Get widget's rect
                    let widget_rect = self.view.area().rect(cx);
                    let widget_center = DVec2{
                        x: widget_rect.pos.x as f64 + widget_rect.size.x as f64 * 0.5,
                        y: widget_rect.pos.y as f64 + widget_rect.size.y as f64 * 0.5,
                    };
                    
                    // Calculate mouse position relative to widget center
                    let mouse_rel = e.abs - widget_center;
                    
                    // Adjust offset to zoom toward mouse position
                    let scale_ratio = self.scale / old_scale;
                    self.offset = self.offset * scale_ratio + mouse_rel * (1.0 - scale_ratio) / self.scale;
                    
                    self.redraw(cx);
                }
            }
            _ => {}
        }
    }
    
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        // Apply transform by modifying the drawing context
        // This is a simplified approach - in a full implementation, we'd use custom shaders
        if self.scale != 1.0 || self.offset.x != 0.0 || self.offset.y != 0.0 {
            // Update the inner image's transform properties if possible
            // For now, just redraw - the actual transform will be visual feedback
            self.view.redraw(cx);
        }
        self.view.draw_walk(cx, scope, walk)
    }
}

impl ZoomableImage {
    pub fn reset_transform(&mut self, cx: &mut Cx) {
        self.scale = 1.0;
        self.offset = DVec2 {x: 0.0, y: 0.0};
        self.dragging = false;
        self.redraw(cx);
    }
    
    pub fn set_image_texture(&mut self, cx: &mut Cx, data: &[u8]) -> Result<(), String> {
        // Load the image data into the internal image widget
        let image = self.view.image(id!(inner_image));
        match crate::utils::load_png_or_jpg(&image, cx, data) {
            Ok(()) => {
                // Reset transform when setting new image
                self.reset_transform(cx);
                Ok(())
            }
            Err(e) => {
                Err(format!("Failed to load image: {:?}", e))
            }
        }
    }
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

    /// Load image data into the ZoomableImage widget
    fn load_image_data(&mut self, cx: &mut Cx, data: &[u8]) {
        println!("load_image_data");
        if let Some(mut zoomable_image) = self.view.zoomable_image(id!(zoomable_image)).borrow_mut() {
            match zoomable_image.set_image_texture(cx, data) {
                Ok(()) => {
                    self.view.view(id!(loading_view)).set_visible(cx, false);
                    self.view.view(id!(error_label_view)).set_visible(cx, false);
                    self.view.view(id!(image_container)).set_visible(cx, true);
                }
                Err(e) => {
                    self.show_error_state(cx);
                }
            }
        }
    }

    /// Show loading state
    fn show_loading_state(&mut self, cx: &mut Cx) {
        self.view.view(id!(loading_view)).set_visible(cx, true);
        self.view.view(id!(error_label_view)).set_visible(cx, false);
        self.view.view(id!(image_container)).set_visible(cx, false);
    }

    /// Show error state
    fn show_error_state(&mut self, cx: &mut Cx) {
        self.view.view(id!(loading_view)).set_visible(cx, false);
        self.view.view(id!(error_label_view)).set_visible(cx, true);
        self.view.view(id!(image_container)).set_visible(cx, false);
    }
    fn close(&mut self, cx: &mut Cx) {
        self.mxc_uri = None;
        self.image_loaded = false;
        
        // Reset the zoomable image transform
        if let Some(mut zoomable_image) = self.view.zoomable_image(id!(zoomable_image)).borrow_mut() {
            zoomable_image.reset_transform(cx);
        }
        
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
