//! Image viewer widget for displaying Image with zooming and panning.
//!
//! There are 2 types of ImageViewerAction handled by this widget. They are "Show" and "Hide".
//! ImageViewerRef has 4 public methods, `configure_zoom`, `show_loading`, `show_loaded` and `reset`.
use std::sync::{mpsc::Receiver, Arc};

use chrono::{DateTime, Local};
use makepad_widgets::{
    event::TouchUpdateEvent,
    image_cache::{ImageBuffer, ImageError},
    rotated_image::RotatedImageWidgetExt,
    *,
};
use matrix_sdk::ruma::OwnedRoomId;
use matrix_sdk_ui::timeline::EventTimelineItem;
use thiserror::Error;
use crate::shared::{avatar::AvatarWidgetExt, timestamp::TimestampWidgetRefExt};

/// Loads the given image `data` into an `ImageBuffer` as either a PNG or JPEG, using the `imghdr` library to determine which format it is.
///
/// Returns an error if either load fails or if the image format is unknown.
pub fn get_png_or_jpg_image_buffer(data: Vec<u8>) -> Result<ImageBuffer, ImageError> {
    match imghdr::from_bytes(&data) {
        Some(imghdr::Type::Png) => {
            ImageBuffer::from_png(&data)
        },
        Some(imghdr::Type::Jpeg) => {
            ImageBuffer::from_jpg(&data)
        },
        Some(_unsupported) => {
            Err(ImageError::UnsupportedFormat)
        }
        None => {
            Err(ImageError::UnsupportedFormat)
        }
    }
}

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
#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum ImageViewerError {
    #[error("Image appears to be empty or corrupted")]
    BadData,
    #[error("Full image was not found")]
    NotFound,
    #[error("Check your internet connection")]
    ConnectionFailed,
    #[error("You don't have permission to view this image")]
    Unauthorized,
    #[error("Server temporarily unavailable")]
    ServerError,
    #[error("This image format isn't supported")]
    UnsupportedFormat,
    #[error("Unable to load image")]
    Unknown,
    #[error("Please reconnect your internet to load the image")]
    Offline,
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
    use crate::shared::avatar::Avatar;
    use crate::shared::timestamp::Timestamp;

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

            magnifying_glass_sign = <Label> {
                text: "+",
                draw_text: {
                    text_style: <THEME_FONT_BOLD>{font_size: 15},
                    color: #000000
                }
            }
        }
    }
    pub RotationButton = <RobrixIconButton> {
        width: Fit, height: Fit,
        margin: 8,
        padding: 3
        align: {x: 0.5, y: 0.5}
        draw_bg: {
            color: (COLOR_PRIMARY)
        }
        draw_icon: {
            svg_file: (ICON_ROTATE_CW),
            fn get_color(self) -> vec4 {
                return #x0;
            }
        }
        icon_walk: {width: 30, height: 30, margin: {right: -10} }
    }
    pub ImageViewer = {{ImageViewer}} {
        width: Fill, height: Fill,
        flow: Overlay
        show_bg: true
        draw_bg: {
            color: (COLOR_PRIMARY)
        }

        image_layer = <View> {
            width: Fill, height: Fill,
            align: {x: 0.5, y: 0.5}
            show_bg: true
            draw_bg: {
                color: (COLOR_PRIMARY)
            }
            flow: Down
            
            header = <View> {
                width: Fill, height: 50
                flow: Right
                align: {x: 1.0, y: 0.5},
    
                zoom_button_minus = <MagnifyingGlass> {
                    sign_label = <View> {
                        width: Fill, height: Fill,
                        align: { x: 0.4, y: 0.35 }

                        magnifying_glass_sign = <Label> {
                            text: "-",
                            draw_text: {
                                text_style: <THEME_FONT_BOLD>{font_size: 15},
                                color: #000000
                            }
                        }
                    }
                }

                zoom_button_plus = <MagnifyingGlass> { }

                rotation_button_anti_clockwise = <RotationButton> {
                    draw_icon: {
                        svg_file: (ICON_ROTATE_CCW),
                        fn get_color(self) -> vec4 {
                            return #x0;
                        }
                    }
                }

                rotation_button_clockwise = <RotationButton> { }

                close_button = <RobrixIconButton> {
                    width: Fit, height: Fit,
                    spacing: 0,
                    padding: 5
                    draw_bg: {
                        color: (COLOR_PRIMARY)
                    }
                    draw_icon: {
                        svg_file: (ICON_CLOSE),
                        fn get_color(self) -> vec4 {
                            return #x0;
                        }
                    }
                    icon_walk: { width: 25, height: 25 }
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

            footer = <View> {
                width: Fill, height: 50,
                flow: Right
                padding: 10
                align: {x: 0.5, y: 0.8}
                spacing: 10

                image_viewer_loading_spinner_view = <View> {
                    width: Fit, height: Fit

                    loading_spinner = <LoadingSpinner> {
                        width: 40, height: 40,
                        draw_bg: {
                            color: (COLOR_TEXT)
                            border_size: 3.0,
                        }
                    }
                }

                image_viewer_forbidden_view = <View> {
                    width: Fit, height: Fit
                    visible: false
                    <Icon> {
                        draw_icon: {
                            svg_file: (ICON_FORBIDDEN),
                            color: (COLOR_TEXT),
                        }
                        icon_walk: { width: 30, height: 30 }
                    }
                }

                image_viewer_status_label = <Label> {
                    width: Fit, height: 30,
                    text: "Loading image...",
                    draw_text: {
                        text_style: <REGULAR_TEXT>{font_size: 14},
                        color: (COLOR_TEXT)
                    }
                }
            }
        }

        metadata_view = <View> {
            width: Fill, height: Fill
            flow: RightWrap

            top_left_container = <View> {
                width: Fit, height: Fit,
                flow: Right,
                spacing: 10,
                margin: {left: 20, top: 40}
                align: { y: 0.5 }

                avatar = <Avatar> {
                    width: 40, height: 40,
                }

                content = <View> {
                    width: Fit, height: Fit,
                    flow: Down,
                    spacing: 4,

                    username = <Label> {
                        width: Fit, height: Fit,
                        draw_text: {
                            text_style: <REGULAR_TEXT>{font_size: 10},
                            color: (COLOR_TEXT)
                        }
                    }

                    timestamp_view = <View> {
                        width: Fit, height: Fit

                        timestamp = <Timestamp> {
                            width: Fit, height: Fit,
                            margin: { left: 5 }
                        }
                    }
                }
            }
            image_name_and_size_view = <View> {
                width: 200, height: Fit,
                image_name_and_size = <Label> {
                    width: Fill, height: Fit,
                    margin: {top: 40}
                    align: { x: 0.5, }
                    draw_text: {
                        text_style: <REGULAR_TEXT>{font_size: 12},
                        color: (COLOR_TEXT),
                        wrap: Word
                    }
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
                        image_layer = {
                            rotated_image_container = {
                                rotated_image = {
                                    draw_bg: {rotation: -1.5708}
                                }
                            }
                        }
                    }
                }
                upright = {
                    redraw: false,
                    from: {all: Forward {duration: 1.0}}
                    apply: {
                        image_layer = {
                            rotated_image_container = {
                                rotated_image = {
                                    draw_bg: {rotation: 0.0}
                                }
                            }
                        }
                    }
                }
                degree_90 = {
                    redraw: false,
                    from: {all: Forward {duration: 1.0}}
                    apply: {
                        image_layer = {
                            rotated_image_container = {
                                rotated_image = {
                                    draw_bg: {rotation: 1.5708}
                                }
                            }
                        }
                    }
                }
                degree_180 = {
                    redraw: false,
                    from: {all: Forward {duration: 1.0}}
                    apply: {
                        image_layer = {
                            rotated_image_container = {
                                rotated_image = {
                                    draw_bg: {rotation: 3.14159}
                                }
                            }
                        }
                    }
                }
                degree_270 = {
                    redraw: false,
                    from: {all: Forward {duration: 1.0}}
                    apply: {
                        image_layer = {
                            rotated_image_container = {
                                rotated_image = {
                                    draw_bg: {rotation: 4.71239}
                                }
                            }
                        }
                    }
                }
                degree_360 = {
                    redraw: false,
                    from: {all: Forward {duration: 0.0}}
                    apply: {
                        image_layer = {
                            rotated_image_container = {
                                rotated_image = {
                                    draw_bg: {rotation: 6.28318}
                                }
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

/// Actions emitted by the `ImageViewer` widget.
#[derive(Clone, Debug, DefaultNone)]
pub enum ImageViewerAction {
    /// No action.
    None,
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
    drag_state: DragState,
    /// The current rotation angle of the image. Max of 4, each step represents 90 degrees
    #[rust]
    rotation_step: i32,
    /// A lock to prevent multiple rotation animations from running at the same time
    #[rust]
    is_animating_rotation: bool,
    #[animator]
    animator: Animator,
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
    /// The ID of the background task that is currently running
    #[rust]
    background_task_id: u32,
    /// The mpsc::Receiver used to receive the result of the background task
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
        let rotated_image = self.view.rotated_image(ids!(rotated_image));
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
                    let rotated_image_container = self.view.rotated_image(ids!(rotated_image));
                    rotated_image_container.apply_over(
                        cx,
                        live! {
                            margin: { top: 0.0, left: 0.0 },
                        },
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

                    let rotated_image_container = self.view.rotated_image(ids!(rotated_image));
                    rotated_image_container.apply_over(
                        cx,
                        live! {
                            margin: { top: (new_offset.y), left: (new_offset.x) },
                        },
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
            self.handle_pinch_to_zoom(cx, touch_event);
        }

        if let (Event::Signal, Some((_background_task_id, receiver))) = (event, &mut self.receiver) {
            let mut remove_receiver = false;
            match receiver.try_recv() {
                Ok(Ok(image_buffer)) => {
                    let rotated_image = self.view.rotated_image(ids!(rotated_image));
                    let texture = image_buffer.into_new_texture(cx);
                    rotated_image.set_texture(cx, Some(texture));
                    remove_receiver = true;
                    cx.action(ImageViewerAction::Show(
                        LoadState::FinishedBackgroundDecoding,
                    ));
                }
                Ok(Err(error)) => {
                    let error = match error {
                        ImageError::JpgDecode(_) | ImageError::PngDecode(_) => {
                            ImageViewerError::UnsupportedFormat
                        }
                        ImageError::EmptyData => ImageViewerError::BadData,
                        ImageError::PathNotFound(_) => ImageViewerError::NotFound,
                        ImageError::UnsupportedFormat => ImageViewerError::UnsupportedFormat,
                        _ => ImageViewerError::BadData,
                    };
                    cx.action(ImageViewerAction::Show(LoadState::Error(error)));
                }
                Err(_) => {}
            }
            if remove_receiver {
                self.receiver = None;
            }
        }
        if let Event::NextFrame(_) = event {
            let animator_action = self.animator_handle_event(cx, event);
            let animation_id = match self.rotation_step {
                0 => ids!(mode.upright),    // 0°
                1 => ids!(mode.degree_90),  // 90°
                2 => ids!(mode.degree_180), // 180°
                3 => ids!(mode.degree_270), // 270°
                _ => ids!(mode.upright),
            };
            if self.animator.animator_in_state(cx, animation_id) {
                self.is_animating_rotation = animator_action.is_animating();
            }
        }
        
        self.view.handle_event(cx, event, scope);
        self.match_event(cx, event);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl MatchEvent for ImageViewer {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        if self.view.button(ids!(close_button)).clicked(actions) {
            self.reset(cx);
            cx.action(ImageViewerAction::Hide);
        }
        if self
            .view
            .button(ids!(zoom_button_minus.magnifying_glass_button))
            .clicked(actions)
        {
            self.adjust_zoom(cx, 1.0 / self.zoom_scale_factor);
        }

        if self
            .view
            .button(ids!(zoom_button_plus.magnifying_glass_button))
            .clicked(actions)
        {
            self.adjust_zoom(cx, self.zoom_scale_factor);
        }

        if self
            .view
            .button(ids!(rotation_button_clockwise))
            .clicked(actions)
        {
            if !self.is_animating_rotation {
                self.is_animating_rotation = true;
                if self.rotation_step == 3 {
                    self.animator_cut(cx, ids!(mode.degree_neg90));
                }
                self.rotation_step = (self.rotation_step + 1) % 4; // Rotate 90 degrees clockwise
                self.update_rotation_animation(cx);
            }
        }

        if self
            .view
            .button(ids!(rotation_button_anti_clockwise))
            .clicked(actions)
        {
            if !self.is_animating_rotation {
                self.is_animating_rotation = true;
                if self.rotation_step == 0 {
                    self.rotation_step = 4;
                    self.animator_cut(cx, ids!(mode.degree_360));
                }
                self.rotation_step = (self.rotation_step - 1) % 4; // Rotate 90 degrees clockwise
                self.update_rotation_animation(cx);
            }
        }
    }
}

impl ImageViewer {
    /// Reset state.
    pub fn reset(&mut self, cx: &mut Cx) {
        self.rotation_step = 0; // Reset to upright (0°)
        self.is_animating_rotation = false; // Reset animation state
        self.previous_pinch_distance = None; // Reset pinch tracking
        self.mouse_cursor_hover_over_image = false; // Reset hover state
        self.receiver = None;
        self.reset_drag_state(cx);
        self.animator_cut(cx, ids!(mode.upright));
        let rotated_image_ref = self
            .view
            .rotated_image(ids!(rotated_image_container.rotated_image));
        rotated_image_ref.apply_over(
            cx,
            live! {
                draw_bg: { scale: 1.0 }
            },
        );
        rotated_image_ref.set_texture(cx, None);
    }

    /// Updates the shader uniforms of the rotated image widget with the current rotation,
    /// and requests a redraw.
    fn update_rotation_animation(&mut self, cx: &mut Cx) {
        // Map rotation step to animation state
        let animation_id = match self.rotation_step {
            0 => ids!(mode.upright),    // 0°
            1 => ids!(mode.degree_90),  // 90°
            2 => ids!(mode.degree_180), // 180°
            3 => ids!(mode.degree_270), // 270°
            _ => ids!(mode.upright),
        };
        self.animator_play(cx, animation_id);
    }

    /// Resets the drag state of the modal to its initial state.
    ///
    /// This function can be used to reset drag state when the magnifying glass is toggled off.
    fn reset_drag_state(&mut self, cx: &mut Cx) {
        self.drag_state = DragState::default();

        // Reset image position and scale
        let rotated_image_container = self.view.rotated_image(ids!(rotated_image));
        rotated_image_container.apply_over(
            cx,
            live! {
                margin: { top: 0.0, left: 0.0 },
                draw_bg: { scale: 1.0 }
            },
        );
        rotated_image_container.redraw(cx);

        self.update_rotation_animation(cx);
    }

    /// Displays an image in the image viewer widget.
    ///
    /// The image is displayed in the center of the widget. If the image is larger than the widget, it is scaled down to fit the widget while retaining its aspect ratio.
    pub fn show_loaded(&mut self, cx: &mut Cx, image_bytes: &[u8]) {
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

    /// Displays an image in the image viewer widget using the provided texture.
    /// 
    /// `Texture` is an optional `Texture` that can be set to display an image. If `None`, the image is cleared.
    pub fn display_using_texture(&mut self, cx: &mut Cx, texture: Option<Texture>) {
        let rotated_image = self.rotated_image(ids!(rotated_image));
        let (width, height) = texture
            .as_ref()
            .and_then(|texture| texture.get_format(cx).vec_width_height())
            .unwrap_or_default();
        rotated_image.set_texture(cx, texture);
        rotated_image.apply_over(
            cx,
            live! {
                width: (width as f64),
                height: (height as f64),
            },
        );
    }

    /// Adjust the zoom level of the image viewer based on the provided zoom factor.
    ///
    /// The zoom factor is a value greater than 0.0, where 1.0 means no zoom change.
    /// A value greater than 1.0 means zoom in, and a value less than 1.0 means zoom out.
    /// The target zoom level is calculated by multiplying the current zoom level by the provided zoom factor.
    /// The target zoom level is then clamped to the minimum and maximum zoom levels.
    /// If the clamped zoom level is the same as the current zoom level, no change is made.
    /// Otherwise, the actual zoom factor is calculated based on the clamped value, and the image is resized accordingly.
    fn adjust_zoom(&mut self, cx: &mut Cx, zoom_factor: f32) {
        let rotated_image_container = self.view.rotated_image(ids!(rotated_image));
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
        // Zoom level may not reach back to 1.0 when zooming in after zooming out because of floating point multiplication.
        // In this case, zoom_level will be clamped to 1.0
        if self.drag_state.zoom_level < 1.0 * zoom_factor
            && self.drag_state.zoom_level > 1.0 / zoom_factor
        {
            self.drag_state.zoom_level = 1.0;
        }
        let width = (size.x as f32 * actual_zoom_factor * 1000.0).round() / 1000.0;
        let height = (size.y as f32 * actual_zoom_factor * 1000.0).round() / 1000.0;
        self.view.rotated_image(ids!(rotated_image)).apply_over(
            cx,
            live! {
                width: (width),
                height: (height),
            },
        );
    }

    /// Handle touch update events, specifically the pinch gesture to zoom in/out.
    ///
    /// This method implements pinch-to-zoom functionality by:
    /// 1. Detecting when exactly two touch points are present
    /// 2. Calculating the current distance between the two touch points
    /// 3. Comparing it to the previous distance to determine the scale factor
    /// 4. Applying the scale factor to adjust the zoom level
    /// 5. Resetting the pinch tracking when fewer than two touches are detected
    ///
    /// When the event contains two touches, the distance between the two touches is used
    /// to calculate a scale factor. The scale factor is then passed to `adjust_zoom` to
    /// adjust the zoom level of the image viewer. When the event contains less than two
    /// touches, the previous pinch distance is reset to `None`.
    fn handle_pinch_to_zoom(&mut self, cx: &mut Cx, event: &TouchUpdateEvent) {
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

    /// Shows a loading message in the footer.
    ///
    /// The loading spinner is shown, the error icon is hidden, and the
    /// status label is set to "Loading...".
    pub fn show_loading(&mut self, cx: &mut Cx) {
        let footer = self.view.view(ids!(image_layer.footer));
        footer
            .view(ids!(image_viewer_loading_spinner_view))
            .set_visible(cx, true);
        footer
            .label(ids!(image_viewer_status_label))
            .set_text(cx, "Loading...");
        footer
            .view(ids!(image_viewer_forbidden_view))
            .set_visible(cx, false);
        footer.apply_over(
            cx,
            live! {
                height: 50
            },
        );
    }

    /// Shows an error message in the footer.
    ///
    /// The loading spinner is hidden, the error icon is shown, and the
    /// status label is set to the error message provided.
    pub fn show_error(&mut self, cx: &mut Cx, error: &ImageViewerError) {
        let footer = self.view.view(ids!(image_layer.footer));
        footer
            .view(ids!(image_viewer_loading_spinner_view))
            .set_visible(cx, false);
        footer
            .view(ids!(image_viewer_forbidden_view))
            .set_visible(cx, true);
        footer
            .label(ids!(image_viewer_status_label))
            .set_text(cx, &error.to_string());
        footer.apply_over(
            cx,
            live! {
                height: 50
            },
        );
    }

    /// Hides the footer of the image viewer.
    ///
    /// This method is used to hide the footer of the image viewer, which contains the status label and the loading spinner.
    ///
    /// The footer is hidden by setting its height to 0.
    pub fn hide_loading(&mut self, cx: &mut Cx) {
        let footer = self.view.view(ids!(image_layer.footer));
        footer.apply_over(
            cx,
            live! {
                height: 0
            },
        );
    }

    /// Sets the metadata view in the image viewer with the provided metadata.
    ///
    /// The metadata view is updated with the truncated image name and the human-readable size of the image.
    ///
    /// The image name is truncated to 24 characters and appended with "..." if it exceeds the limit.
    /// The human-readable size is calculated based on the image size in bytes.
    pub fn set_metadata(&mut self, cx: &mut Cx, metadata: &MetaData) {
        let meta_view = self.view.view(ids!(metadata_view));
        let truncated_name = truncate_image_name(&metadata.image_name);
        let human_readable_size = format_file_size(metadata.image_file_size);
        let display_text = format!("{} ({})", truncated_name, human_readable_size);
        meta_view
            .label(ids!(image_name_and_size))
            .set_text(cx, &display_text);
        if let Some(timestamp) = metadata.timestamp {
            meta_view
                .view(ids!(top_left_container.content.timestamp_view))
                .set_visible(cx, true);
            meta_view
                .timestamp(ids!(top_left_container.content.timestamp_view.timestamp))
                .set_date_time(cx, timestamp);
        }

        if let Some((room_id, event_timeline_item)) = &metadata.avatar_parameter {            
            let (sender, _) =self.view.avatar(ids!(top_left_container.avatar))
                .set_avatar_and_get_username(
                    cx,
                    room_id,
                    event_timeline_item.sender(),
                    Some(event_timeline_item.sender_profile()),
                    event_timeline_item.event_id(),
                );
            if sender.len() > MAX_USERNAME_LENGTH {
                meta_view
                    .label(ids!(top_left_container.content.username))
                    .set_text(cx, &format!("{}...", &sender[..MAX_USERNAME_LENGTH - 3]));
            } else {
                meta_view
                    .label(ids!(top_left_container.content.username))
                    .set_text(cx, &sender);
            };
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

    /// See [`ImageViewer::show_loaded()`].
    pub fn show_loaded(&mut self, cx: &mut Cx, image_bytes: &[u8]) {
        let Some(mut inner) = self.borrow_mut() else {
            return;
        };
        inner.show_loaded(cx, image_bytes)
    }

    /// Display the image viewer widget with the provided texture, metadata and loading spinner.
    pub fn show_loading(
        &mut self,
        cx: &mut Cx,
        texture: Option<Texture>,
        metadata: &Option<MetaData>,
    ) {
        let Some(mut inner) = self.borrow_mut() else {
            return;
        };
        inner.display_using_texture(cx, texture);
        if let Some(metadata) = metadata {
            inner.set_metadata(cx, metadata);
        }
        inner.show_loading(cx);
    }

    /// See [`ImageViewer::show_error()`].
    pub fn show_error(&mut self, cx: &mut Cx, error: &ImageViewerError) {
        let Some(mut inner) = self.borrow_mut() else {
            return;
        };
        inner.show_error(cx, error);
    }

    /// See [`ImageViewer::hide_loading()`].
    pub fn hide_loading(&mut self, cx: &mut Cx) {
        let Some(mut inner) = self.borrow_mut() else {
            return;
        };
        inner.hide_loading(cx);
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
#[derive(Debug, Clone)]
pub enum LoadState {
    /// The image is currently being loaded with its loading image texture.
    /// This texture is usually the image texture that's being selected.
    Loading(std::rc::Rc<Option<Texture>>, Option<MetaData>),
    /// The image has been successfully loaded given the data.
    Loaded(Arc<[u8]>),
    /// The image has been decoded from background thread.
    FinishedBackgroundDecoding,
    /// An error occurred while loading the image, with specific error type.
    Error(ImageViewerError),
}

#[derive(Debug, Clone)]
/// Metadata for an image.
pub struct MetaData {
    // Optional avatar parameter containing room ID and event timeline item
    // to be used for the avatar.
    pub avatar_parameter: Option<(OwnedRoomId, EventTimelineItem)>,
    pub timestamp: Option<DateTime<Local>>,
    pub image_name: String,
    // Image size in bytes
    pub image_file_size: u64,
}

/// Maximum image name length to be displayed
const MAX_IMAGE_NAME_LENGTH: usize = 50;
/// Maximum username length to be displayed
const MAX_USERNAME_LENGTH: usize = 50;

/// Truncate image name while preserving file extension
fn truncate_image_name(image_name: &str) -> String {
    let max_length = MAX_IMAGE_NAME_LENGTH;

    if image_name.len() <= max_length {
        return image_name.to_string();
    }

    // Find the last dot to separate name and extension
    if let Some(dot_pos) = image_name.rfind('.') {
        let name_part = &image_name[..dot_pos];
        let extension = &image_name[dot_pos..];

        // Reserve space for "..." and the extension
        let available_length = max_length.saturating_sub(3 + extension.len());

        if available_length > 0 && name_part.len() > available_length {
            format!("{}...{}", &name_part[..available_length], extension)
        } else {
            image_name.to_string()
        }
    } else {
        // No extension found, just truncate the name
        format!("{}...", &image_name[..max_length.saturating_sub(3)])
    }
}

/// Convert bytes to human-readable file size format
fn format_file_size(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];

    if bytes == 0 {
        return "0 B".to_string();
    }

    let mut size = bytes as f64;
    let mut unit_index = 0;

    while size >= 1024.0 && unit_index < UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }

    if unit_index == 0 {
        format!("{} {}", bytes, UNITS[unit_index])
    } else {
        format!("{:.1} {}", size, UNITS[unit_index])
    }
}
