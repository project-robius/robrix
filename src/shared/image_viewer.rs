//! Image viewer widget for displaying Image with zooming and panning.
//!
//! There are 2 types of ImageViewerActions handled by this widget. They are "Show" and "Hide".
//! ImageViewerRef has 2 public methods, `display_image` and `reset`.
use std::sync::{Arc, mpsc::Receiver};

use makepad_widgets::{event::TouchUpdateEvent, image_cache::{ImageBuffer, ImageError}, rotated_image::RotatedImageWidgetExt, *};

use crate::utils::get_png_or_jpg_image_buffer;

/// Duration for rotation animations in seconds.
/// This value should be consistent with the duration value in set in the animator.
const ROTATION_ANIMATION_DURATION: f64 = 1.0;

/// Configuration for zoom and pan settings in the image viewer
#[derive(Clone, Debug)]
pub struct Config {
    /// Minimum zoom level (default: 0.5)
    pub min_zoom: f32,
    /// Maximum zoom level (default: 4.0)
    pub max_zoom: f32,
    /// Zoom scale factor for zoom in/out operations (default: 1.2)
    pub zoom_scale_factor: f32,
    /// Pan sensitivity multiplier for drag operations (default: 2.0)
    pub pan_sensitivity: f64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            min_zoom: 0.5,
            max_zoom: 4.0,
            zoom_scale_factor: 1.2,
            pan_sensitivity: 2.0,
        }
    }
}

/// Error types for image loading operations
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ImageViewerError {
    /// Image not found
    NotFound,
    /// Access denied
    Unauthorized,
    /// Connection failed
    ConnectionFailed,
    /// Bad or corrupted data
    BadData,
    /// Server error
    ServerError,
    /// Unsupported format
    UnsupportedFormat,
    /// Unknown error
    Unknown,
    /// Image loading timed out
    Timeout,
}

/// The Drag state of the image viewer modal
struct DragState {
    /// The starting position of the drag.
    drag_start: DVec2,
    /// The zoom level of the image.
    /// The larger the value, the more zoomed in the image is.
    zoom_level: f32,
    /// The pan offset of the image.
    pan_offset: Option<DVec2>,
}

impl Default for DragState {
    /// Resets all the drag state to its default values. This is called when the image changes.
    fn default() -> Self {
        Self {
            drag_start: DVec2::default(),
            zoom_level: 1.0,
            pan_offset: None,
        }
    }
}

live_design! {
    use link::theme::*;
    use link::widgets::*;
    use crate::shared::styles::*;
    use crate::shared::icon_button::RobrixIconButton;

    pub MagnifyingGlass = <View> {
        width: Fit, height: Fit
        flow: Overlay
        visible: true

        magnifying_glass_button = <RobrixIconButton> {
            width: Fit, height: Fit,
            spacing: 0,
            margin: 8,
            padding: 3
            draw_bg: {
                color: (COLOR_PRIMARY)
            }
            draw_icon: {
                svg_file: (ICON_ZOOM),
                fn get_color(self) -> vec4 {
                    return #x0;
                }
            }
            icon_walk: {width: 30, height: 30}
        }
        sign_label = <View> {
            width: Fill, height: Fill,
            align: { x: 0.4, y: 0.35 }

            magnify_glass_sign = <Label> {
                text: "+",
                draw_text: {
                    text_style: <THEME_FONT_BOLD>{font_size: 15},
                    color: #000000
                }
            }
        }
    }
    pub Rotation_Button = <RobrixIconButton> {
        width: Fit, height: Fit,
        margin: 8,
        padding: 3
        align: {x: 0.5, y: 0.5}
        draw_bg: {
            color: (COLOR_PRIMARY)
        }
        draw_icon: {
            svg_file: (ICON_CLOCKWISE),
            fn get_color(self) -> vec4 {
                return #x0;
            }
        }
        icon_walk: {width: 30, height: 30, margin: {right: -10} }
    }
    pub ImageViewer = {{ImageViewer}} {
        width: Fill, height: Fill,
        align: {x: 0.5, y: 0.5}
        show_bg: true
        draw_bg: {
            color: (COLOR_PRIMARY)
        }
        flow: Down
        header = <View> {
            width: Fill, height: 80
            flow: Right
            spacing: 0
            align: {x: 1.0, y: 0.0},

            zoom_button_minus = <MagnifyingGlass> {
                sign_label = <View> {
                    width: Fill, height: Fill,
                    align: { x: 0.4, y: 0.35 }

                    magnify_glass_sign = <Label> {
                        text: "-",
                        draw_text: {
                            text_style: <THEME_FONT_BOLD>{font_size: 15},
                            color: #000000
                        }
                    }
                }
            }

            zoom_button_plus = <MagnifyingGlass> { }
            rotation_button_anti_clockwise = <Rotation_Button> {
                draw_icon: {
                    svg_file: (ICON_CLOCKWISE_ANTI),
                    fn get_color(self) -> vec4 {
                        return #x0;
                    }
                }
            }
            rotation_button_clockwise = <Rotation_Button> { }
            
            close_button = <RobrixIconButton> {
                width: Fit, height: Fit,
                spacing: 0,
                margin: 8,
                draw_bg: {
                    color: (COLOR_PRIMARY)
                }
                draw_icon: {
                    svg_file: (ICON_CLOSE),
                    fn get_color(self) -> vec4 {
                        return #x0;
                    }
                }
                icon_walk: { width: 14, height: 14 }
            }
        }
        rotated_image_container = <View> {
            width: Fill, height: Fill,
            flow: Overlay
            align: {x: 0.5, y: 0.5}

            rotated_image = <RotatedImage> {
                width: Fill, height: Fill,
                draw_bg: {
                    rotation: 0.0
                    opacity: 1.0
                }
            }
        }
        animator: {
            mode = {
                default: upright,
                degree_neg90 = {
                    redraw: false,
                    from: {all: Forward {duration: 1.0}}
                    apply: {
                        rotated_image_container = {
                            rotated_image = {
                                draw_bg: {rotation: -1.5708}
                            }
                        }
                    }
                }
                upright = {
                    redraw: false,
                    from: {all: Forward {duration: 1.0}}
                    apply: {
                        rotated_image_container = {
                            rotated_image = {
                                draw_bg: {rotation: 0.0}
                            }
                        }
                    }
                }
                degree_90 = {
                    redraw: false,
                    from: {all: Forward {duration: 1.0}}
                    apply: {
                        rotated_image_container = {
                            rotated_image = {
                                draw_bg: {rotation: 1.5708}
                            }
                        }
                    }
                }
                degree_180 = {
                    redraw: false,
                    from: {all: Forward {duration: 1.0}}
                    apply: {
                        rotated_image_container = {
                            rotated_image = {
                                draw_bg: {rotation: 3.14159}
                            }
                        }
                    }
                }
                degree_270 = {
                    redraw: false,
                    from: {all: Forward {duration: 1.0}}
                    apply: {
                        rotated_image_container = {
                            rotated_image = {
                                draw_bg: {rotation: 4.71239}
                            }
                        }
                    }
                }
                degree_360 = {
                    redraw: false,
                    from: {all: Forward {duration: 0.0}}
                    apply: {
                        rotated_image_container = {
                            rotated_image = {
                                draw_bg: {rotation: 6.28318}
                            }
                        }
                    }
                }
            }
            hover = {
                default: off
                off = {
                    apply: { }
                }
                on = {
                    apply: { }
                }
            }
        }
    }
}

/// Actions handled by the `ImageViewer` widget.
#[derive(Debug)]
pub enum ImageViewerAction {
    /// Display the ImageViewer widget based on the LoadState.
    Show(LoadState),
    /// Close the ImageViewer widget.
    Hide,
}

#[derive(Live, Widget)]
struct ImageViewer {
    #[deref]
    view: View,
    #[rust]
    image_loaded: bool,
    #[rust]
    drag_state: DragState,
    #[rust]
    rotation_step: i32,
    #[rust]
    is_animating_rotation: bool,
    #[animator]
    animator: Animator,
    /// Timer for rotation animation, prevents clicking the rotation buttons too often
    /// to start the animation
    #[rust]
    timer: Timer,
    /// Zoom constraints for the image viewer
    #[rust]
    min_zoom: f32,
    #[rust]
    max_zoom: f32,
    /// Zoom scale factor for zoom in/out operations
    #[rust]
    zoom_scale_factor: f32,
    /// Pan sensitivity multiplier for drag operations
    #[rust]
    pan_sensitivity: f64,
    /// Indicates if the mouse cursor is currently hovering over the image.
    /// If true, allows wheel scroll to zoom the image.
    #[rust]
    mouse_cursor_hover_over_image: bool,
    /// Distance between two touch points for pinch-to-zoom functionality
    #[rust]
    previous_pinch_distance: Option<f64>,
    #[rust]
    background_task_id: u32,
    #[rust]
    receiver: Option<(u32, Receiver<Result<ImageBuffer, ImageError>>)>,
}

impl LiveHook for ImageViewer {
    fn after_new_from_doc(&mut self, _cx: &mut Cx) {
        self.min_zoom = 0.5;
        self.max_zoom = 4.0;
        self.zoom_scale_factor = 1.2;
        self.pan_sensitivity = 2.0;
    }
}

impl Widget for ImageViewer {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if self.image_loaded {
            let rotated_image = self.view.rotated_image(id!(rotated_image));
            // Handle cursor changes on mouse hover
            match event.hits(cx, rotated_image.area()) {
                Hit::FingerDown(fe) => {
                    if fe.is_primary_hit() {
                        self.drag_state.drag_start = fe.abs;
                        // Initialize pan_offset with current position if not set
                        if self.drag_state.pan_offset.is_none() {
                            self.drag_state.pan_offset = Some(DVec2::default());
                        }
                    }
                }
                Hit::FingerUp(fe) if fe.is_over && fe.is_primary_hit() => {
                    // Only reset pan_offset on double-tap, not single tap
                    if fe.tap_count == 2 {
                        self.drag_state.pan_offset = Some(DVec2::default());
                        let rotated_image_container = self.view.rotated_image(id!(rotated_image));
                        rotated_image_container.apply_over(
                            cx,
                            live! {
                                margin: { top: 0.0, left: 0.0 },
                            }
                        );
                        rotated_image_container.redraw(cx);
                    }
                }
                Hit::FingerHoverIn(_) => {
                    self.mouse_cursor_hover_over_image = true;
                    cx.set_cursor(MouseCursor::Hand);
                }
                Hit::FingerMove(fe) => {
                    if let Some(current_offset) = self.drag_state.pan_offset {
                        let drag_delta = fe.abs - self.drag_state.drag_start;
                        let new_offset = current_offset + drag_delta * self.pan_sensitivity;
                        
                        let rotated_image_container = self.view.rotated_image(id!(rotated_image));
                        rotated_image_container.apply_over(
                            cx,
                            live! {
                                margin: { top: (new_offset.y), left: (new_offset.x) },
                            }
                        );
                        
                        // Update pan_offset with new position
                        self.drag_state.pan_offset = Some(new_offset);
                    }
                    self.drag_state.drag_start = fe.abs;
                }
                Hit::FingerHoverOut(_) => {
                    self.mouse_cursor_hover_over_image = false;
                    cx.set_cursor(MouseCursor::Default);
                }
                _ => {}
            }
            if let Event::Scroll(scroll_event) = event {
                if self.mouse_cursor_hover_over_image {
                    let scroll_delta = scroll_event.scroll.y;
                    
                    if scroll_delta > 0.0 {
                        // Scroll up = Zoom in
                        self.adjust_zoom(cx, self.zoom_scale_factor);
                    } else if scroll_delta < 0.0 {
                        // Scroll down = Zoom out
                        self.adjust_zoom(cx, 1.0 / self.zoom_scale_factor);
                    }
                }
            }
            if let Event::KeyDown(e) = event {
                match &e.key_code {
                    KeyCode::Minus | KeyCode::NumpadSubtract => {
                        // Zoom out (make image smaller)
                        self.adjust_zoom(cx, 1.0 / self.zoom_scale_factor);
                    }
                    KeyCode::Equals | KeyCode::NumpadAdd => {
                        // Zoom in (make image larger)
                        self.adjust_zoom(cx, self.zoom_scale_factor);
                    }
                    KeyCode::Key0 | KeyCode::Numpad0 => {
                        self.reset_drag_state(cx);
                    }
                    _ => {}
                }
            }
            if let Event::TouchUpdate(touch_event) = event {
                self.handle_touch_update(cx, touch_event);
            }  
        }
        if let Some(_timer) = self.timer.is_event(event) {
            self.is_animating_rotation = false;
        }
        if let Event::Signal = event {
            let mut to_remove = false;
            if let Some((_background_task_id, receiver)) = &mut self.receiver {
                while let Ok(image_buffer_res) = receiver.try_recv() {
                    match image_buffer_res {
                        Ok(image_buffer) => {
                            let rotated_image = self.view.rotated_image(id!(rotated_image));
                            let texture = image_buffer.into_new_texture(cx);
                            rotated_image.set_texture(cx, Some(texture));
                            self.image_loaded = true;
                            to_remove = true;
                            cx.action(ImageViewerAction::Show(LoadState::FinishedBackgroundDecoding));
                        
                        }
                        Err(error) => {
                            let error = match error {
                                ImageError::JpgDecode(_) | ImageError::PngDecode(_) => ImageViewerError::UnsupportedFormat,
                                ImageError::EmptyData => ImageViewerError::BadData,
                                ImageError::PathNotFound(_) => ImageViewerError::NotFound,
                                ImageError::UnsupportedFormat => ImageViewerError::UnsupportedFormat,
                                _ => ImageViewerError::BadData,
                            };
                            cx.action(ImageViewerAction::Show(LoadState::Error(error)));
                        }
                    }
                    break
                }
            }
            if to_remove {
                self.receiver = None;
            }
        }
        self.animator_handle_event(cx, event);
        self.view.handle_event(cx, event, scope);
        self.match_event(cx, event);
        
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl MatchEvent for ImageViewer {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        if self.view.button(id!(close_button)).clicked(actions) {
            self.reset(cx);
            cx.action(ImageViewerAction::Hide);
        }
        if self.view.button(id!(zoom_button_minus.magnifying_glass_button)).clicked(actions) {
            self.adjust_zoom(cx, 1.0 / self.zoom_scale_factor);
        }

        if self.view.button(id!(zoom_button_plus.magnifying_glass_button)).clicked(actions) {
            self.adjust_zoom(cx, self.zoom_scale_factor);
        }

        if self.view.button(id!(rotation_button_clockwise)).clicked(actions) {
            if !self.is_animating_rotation {
                self.timer = cx.start_timeout(ROTATION_ANIMATION_DURATION);
                self.is_animating_rotation = true;
                if self.rotation_step == 3 {
                    self.animator_cut(cx, id!(mode.degree_neg90));
                }
                self.rotation_step = (self.rotation_step + 1) % 4; // Rotate 90 degrees clockwise
                self.update_rotated_image_shader(cx);
            }
        }

        if self.view.button(id!(rotation_button_anti_clockwise)).clicked(actions) {
            if !self.is_animating_rotation {
                self.timer = cx.start_timeout(1.0);
                self.is_animating_rotation = true;
                if self.rotation_step == 0 {
                    self.rotation_step = 4;
                    self.animator_cut(cx, id!(mode.degree_360));
                } else if self.rotation_step == 1{

                }
                self.rotation_step = (self.rotation_step - 1) % 4; // Rotate 90 degrees clockwise
                self.update_rotated_image_shader(cx);
            }
        }
    }
}

impl ImageViewer {
    /// Reset state.
    pub fn reset(&mut self, cx: &mut Cx) {
        self.image_loaded = false;
        self.rotation_step = 0; // Reset to upright (0°)
        self.is_animating_rotation = false; // Reset animation state
        self.previous_pinch_distance = None; // Reset pinch tracking
        self.mouse_cursor_hover_over_image = false; // Reset hover state
        self.timer = Timer::default(); // Reset timer
        self.receiver = None;
        self.reset_drag_state(cx);
        // Clear the rotated image texture with a white background
        if let Ok(image_buffer) = ImageBuffer::new(&[255], 1, 1) {
            let texture = image_buffer.into_new_texture(cx);
            let _ = self.view.rotated_image(id!(rotated_image)).set_texture(cx, Some(texture));
            self.view.rotated_image(id!(rotated_image)).redraw(cx);
        }
        self.animator_cut(cx, id!(mode.upright));
        self.view.rotated_image(id!(rotated_image_container.rotated_image)).apply_over(cx, live!{
            draw_bg: { scale: 1.0 }
        });
    }

    /// Updates the shader uniforms of the rotated image widget with the current rotation,
    /// and requests a redraw.
    fn update_rotated_image_shader(&mut self, cx: &mut Cx) {
        // Map rotation step to animation state
        let animation_id = match self.rotation_step {
            0 => id!(mode.upright),     // 0°
            1 => id!(mode.degree_90),   // 90°
            2 => id!(mode.degree_180),  // 180°
            3 => id!(mode.degree_270),  // 270°
            _ => id!(mode.upright),
        };
        
        self.animator_play(cx, animation_id);
    }

    /// Resets the drag state of the modal to its initial state.
    ///
    /// This function can be used to reset drag state when the magnifying glass is toggled off.
    fn reset_drag_state(&mut self, cx: &mut Cx) {
        self.drag_state = DragState::default();
        
        // Reset image position and scale
        let rotated_image_container = self.view.rotated_image(id!(rotated_image));
        rotated_image_container.apply_over(
            cx,
            live! {
                margin: { top: 0.0, left: 0.0 },
                draw_bg: { scale: 1.0 }
            }
        );
        rotated_image_container.redraw(cx);
        
        self.update_rotated_image_shader(cx);
    }

    /// Displays an image in the image viewer widget.
    ///
    /// If `background_thread` is false, the image is decoded and displayed immediately.
    /// If `background_thread` is true, the image is decoded in a separate thread and displayed when the decoding is complete.
    ///
    /// The image is displayed in the center of the widget. If the image is larger than the widget, it is scaled down to fit the widget while retaining its aspect ratio.
    pub fn display_using_background_thread(&mut self, cx: &mut Cx, image_bytes: &[u8]) {
        self.image_loaded = true;
        if self.receiver.is_some() {
            return;
        }
        if let Some(new_value) = self.background_task_id.checked_add(1) {
            self.background_task_id = new_value;
        }
        let (sender, receiver) = std::sync::mpsc::channel();
        self.receiver = Some((self.background_task_id, receiver));
        let image_bytes_clone = image_bytes.to_vec();
        cx.spawn_thread(move || {
            let _ = sender.send(get_png_or_jpg_image_buffer(image_bytes_clone));
            SignalToUI::set_ui_signal();
        });
    }

    pub fn display_using_texture(&mut self, cx: &mut Cx, texture: Option<Texture>, size: &DVec2) {
        self.rotated_image(id!(rotated_image)).set_texture(cx, texture);
        self.rotated_image(id!(rotated_image)).apply_over(cx, live!{
            width: (size.x),
            height: (size.y),
        });
    }

    fn adjust_zoom(&mut self, cx: &mut Cx, zoom_factor: f32) {
        let rotated_image_container = self.view.rotated_image(id!(rotated_image));
        let size = rotated_image_container.area().rect(cx).size;
        
        // Calculate target zoom level and clamp it to bounds
        let target_zoom = self.drag_state.zoom_level * zoom_factor;
        let clamped_zoom = target_zoom.clamp(self.min_zoom, self.max_zoom);
        
        // If the clamped zoom is the same as current zoom, no change needed
        if (clamped_zoom - self.drag_state.zoom_level).abs() < 0.001 {
            return;
        }
        
        // Calculate the actual zoom factor based on clamped value
        let actual_zoom_factor = clamped_zoom / self.drag_state.zoom_level;
        
        self.drag_state.zoom_level = clamped_zoom;
        self.drag_state.zoom_level = (self.drag_state.zoom_level * 1000.0).round() / 1000.0;
        let width = (size.x as f32 * actual_zoom_factor * 1000.0).round() / 1000.0;
        let height = (size.y as f32 * actual_zoom_factor * 1000.0).round() / 1000.0;
        self.view.rotated_image(id!(rotated_image)).apply_over(cx, live!{
            width: (width),
            height: (height),
        });
    }

    fn handle_touch_update(&mut self, cx: &mut Cx, event: &TouchUpdateEvent) {
        if event.touches.len() == 2 {
            let touch1 = &event.touches[0];
            let touch2 = &event.touches[1];
            
            let current_distance = (touch1.abs - touch2.abs).length();
            
            if let Some(previous_distance) = self.previous_pinch_distance {
                let scale = current_distance / previous_distance;
                self.adjust_zoom(cx, scale as f32);
            }
            
            self.previous_pinch_distance = Some(current_distance);
        } else {
            self.previous_pinch_distance = None;
        }
    }
}

impl ImageViewerRef {
    /// Configure zoom and pan settings for the image viewer
    pub fn configure_zoom(&mut self, config: Config) {
        let Some(mut inner) = self.borrow_mut() else {
            return;
        };
        inner.min_zoom = config.min_zoom;
        inner.max_zoom = config.max_zoom;
        inner.zoom_scale_factor = config.zoom_scale_factor;
        inner.pan_sensitivity = config.pan_sensitivity;
    }

    /// See [`ImageViewer::display_using_background_thread()`].
    pub fn display_using_background_thread(&mut self, cx: &mut Cx, image_bytes: &[u8]) {
        let Some(mut inner) = self.borrow_mut() else {
            return;
        };
        inner.display_using_background_thread(cx, image_bytes)
    }

    /// See [`ImageViewer::display_using_texture()`].
    pub fn display_using_texture(&mut self, cx: &mut Cx, texture: Option<Texture>, size: &DVec2) {
        let Some(mut inner) = self.borrow_mut() else {
            return;
        };
        inner.display_using_texture(cx, texture, size)
    }
    /// See [`ImageViewer::reset()`].
    pub fn reset(&mut self, cx: &mut Cx) {
       let Some(mut inner) = self.borrow_mut() else {
            return;
        };
        inner.reset(cx);
    }
}

/// Represents the possible states of an image load operation.
#[derive(Debug,)]
pub enum LoadState {
    /// The image is currently being loaded with its thumbnail image texture and its image size.
    Loading(Arc<Option<Texture>>, DVec2),
    /// The image has been successfully loaded given the data.
    Loaded(Arc<[u8]>),
    /// The image has been loaded from background thread.
    FinishedBackgroundDecoding,
    /// An error occurred while loading the image, with specific error type.
    Error(ImageViewerError),
}
