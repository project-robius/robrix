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

/// The timeout for hiding the UI overlays after no user mouse/tap activity.
const SHOW_UI_DURATION: f64 = 3.0;

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

/// Configuration for zoom and pan settings in the image viewer.
#[derive(Clone, Debug)]
pub struct ImageViewerZoomConfig {
    /// Minimum zoom level (default: 0.1)
    pub min_zoom: f64,
    /// Zoom scale factor for zoom in/out operations (default: 1.2)
    pub zoom_scale_factor: f64,
    /// Pan sensitivity multiplier for drag operations (default: 2.0)
    pub pan_sensitivity: f64,
}

impl Default for ImageViewerZoomConfig {
    fn default() -> Self {
        Self {
            min_zoom: 0.1,
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
    zoom_level: f64,
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

    UI_ANIMATION_DURATION_SECS = 0.5
    ROTATION_ANIMATION_DURATION_SECS = 0.2

    ImageViewerButton = <RobrixIconButton> {
        width: 44, height: 44
        align: {x: 0.5, y: 0.5},
        spacing: 0, 
        padding: 0,
        draw_bg: {
            color: (COLOR_SECONDARY * 0.9)
        }
        draw_icon: {
            svg_file: (ICON_ZOOM_OUT),
            fn get_color(self) -> vec4 {
                return #x0;
            }
        }
        icon_walk: {width: 27, height: 27}
    }

    pub ImageViewer = {{ImageViewer}} {
        width: Fill, height: Fill,
        flow: Overlay
        show_bg: true
        draw_bg: {
            color: (COLOR_IMAGE_VIEWER_BACKGROUND)
        }

        image_layer = <View> {
            width: Fill, height: Fill,
            align: {x: 0.5, y: 0.5}
            flow: Down

            rotated_image_container = <View> {
                width: Fill, height: Fill,
                flow: Down
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
            width: Fill, height: Fill,
            margin: 20,
            align: {x: 0.0, y: 1.0},
            metadata_rounded_view = <RoundedView> {
                width: Fill, height: Fit
                flow: Right
                align: {y: 0.5, x: 0.0}
                padding: 13
                spacing: 8,

                show_bg: true
                draw_bg: {
                    border_radius: 4.0
                    color: (COLOR_IMAGE_VIEWER_META_BACKGROUND)
                }

                // Display user profile view below the button group when the width is not enough.
                user_profile_view = <View> {
                    width: Fit { max: 200 }
                    height: Fit,
                    flow: Right,
                    spacing: 13,
                    align: { y: 0.5 }
                    
                    avatar = <Avatar> {
                        width: 45, height: 45,
                        text_view = { text = { draw_text: {
                            text_style: <TITLE_TEXT>{ font_size: 15.0 }
                        }}}
                    }

                    content = <View> {
                        width: Fit
                        height: Fit,
                        align: { y: 0.5 }
                        spacing: 3
                        flow: Down,

                        username = <Label> {
                            width: Fit
                            height: Fit,
                            padding: 0
                            margin: 0
                            flow: Right
                            draw_text: {
                                text_style: <REGULAR_TEXT>{font_size: 12},
                                color: (COLOR_TEXT)
                                wrap: Ellipsis
                            }
                        }

                        timestamp_view = <View> {
                            width: Fit
                            height: Fit

                            timestamp = <Timestamp> {
                                width: Fit,
                                height: Fit,
                                ts_label = {
                                    draw_text: {
                                        text_style: {font_size: 9.5},
                                        color: (COLOR_TEXT)
                                    }
                                }
                            }
                        }
                    }
                }

                // Display image name and size below the user_profile_view if the width is not enough.
                image_name_and_size_view = <View> {
                    width: Fill
                    height: Fit,
                    align: {x: 0.5, y: 0.5}
                    flow: Right
                    image_name_and_size = <Label> {
                        width: Fill,
                        height: Fit,
                        align: {x: 0.5, y: 0.5}
                        draw_text: {
                            text_style: <REGULAR_TEXT>{font_size: 13},
                            color: (COLOR_TEXT),
                            wrap: Word
                        }
                    }
                }
            }
        }

        button_group_view = <View> {
            width: Fill, height: Fit
            flow: Right
            margin: {top: 20, right: 20}
            align: {x: 1.0, y: 0.5},

            button_group_rounded_view = <RoundedView> {
                width: Fit, height: Fit
                spacing: 10
                show_bg: true
                draw_bg: {
                    color: (COLOR_IMAGE_VIEWER_META_BACKGROUND),
                    border_radius: 4.0
                }
                padding: { left: 7, top: 4, bottom: 4, right: 7}

                zoom_out_button = <ImageViewerButton> {
                    draw_icon: { svg_file: (ICON_ZOOM_OUT) }
                    icon_walk: {width: 27, height: 27, margin: {left: 2}}
                }

                zoom_in_button = <ImageViewerButton> {
                    draw_icon: { svg_file: (ICON_ZOOM_IN) }
                    icon_walk: {width: 27, height: 27, margin: {left: 2}}
                }

                rotation_button_anti_clockwise = <ImageViewerButton> {
                    draw_icon: { svg_file: (ICON_ROTATE_CCW) }
                }

                rotation_button_clockwise = <ImageViewerButton> {
                    draw_icon: { svg_file: (ICON_ROTATE_CW) }
                }

                reset_button = <ImageViewerButton> {
                    draw_icon: { svg_file: (ICON_JUMP) }
                    icon_walk: {width: 25, height: 25, margin: {bottom: 2}}
                }

                close_button = <ImageViewerButton> {
                    draw_icon: { svg_file: (ICON_CLOSE) }
                    icon_walk: {width: 21, height: 21 }
                }
            }
        }

        animator: {
            mode = {
                default: upright,
                degree_neg90 = {
                    redraw: false,
                    from: {all: Forward {duration: (ROTATION_ANIMATION_DURATION_SECS)}}
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
                    from: {all: Forward {duration: (ROTATION_ANIMATION_DURATION_SECS)}}
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
                    from: {all: Forward {duration: (ROTATION_ANIMATION_DURATION_SECS)}}
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
                    from: {all: Forward {duration: (ROTATION_ANIMATION_DURATION_SECS)}}
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
                    from: {all: Forward {duration: (ROTATION_ANIMATION_DURATION_SECS)}}
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
            ui_animator = {
                default: hide,
                show = {
                    redraw: false,
                    from: { all: Forward { duration: (UI_ANIMATION_DURATION_SECS) } }
                    apply: {
                        button_group_view = {
                            margin: { top: 20 }
                        }
                        metadata_view = {
                            margin: { bottom: 20 }
                        }
                    }
                }
                hide = {
                    redraw: false,
                    from: { all: Forward { duration: (UI_ANIMATION_DURATION_SECS) } }
                    apply: {
                        button_group_view = {
                            margin: { top: -200 }
                        }
                        metadata_view = {
                            margin: { bottom: -300 }
                        }
                    }
                }
            }
        }
    }
}

/// Actions emitted by the `ImageViewer` widget.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, DefaultNone)]
pub enum ImageViewerAction {
    /// No action.
    None,
    /// Display the ImageViewer widget based on the LoadState.
    Show(LoadState),
    /// Hide the ImageViewer widget.
    Hide,
}

#[derive(Live, LiveHook, Widget)]
struct ImageViewer {
    #[deref] view: View,
    #[rust] drag_state: DragState,
    /// The current rotation angle of the image. Max of 4, each step represents 90 degrees
    #[rust] rotation_step: i8,
    /// A lock to prevent multiple rotation animations from running at the same time
    #[rust] is_animating_rotation: bool,
    #[animator] animator: Animator,
    /// Zoom constraints for the image viewer
    #[rust] config: ImageViewerZoomConfig,
    /// Indicates if the mouse cursor is currently hovering over the image.
    /// If true, allows wheel scroll to zoom the image.
    #[rust] mouse_cursor_hover_over_image: bool,
    /// Distance between two touch points for pinch-to-zoom functionality
    #[rust] previous_pinch_distance: Option<f64>,
    /// The ID of the background task that is currently running
    #[rust] background_task_id: u32,
    /// The mpsc::Receiver used to receive the result of the background task
    #[rust] receiver: Option<(u32, Receiver<Result<ImageBuffer, ImageError>>)>,
    /// Whether the full image file has been loaded
    #[rust] is_loaded: bool,
    /// The size of the image container.
    ///
    /// Used to compute the necessary width and height for the full screen image.
    #[rust] image_container_size: DVec2,
    /// The texture containing the loaded image
    #[rust] texture: Option<Texture>,
    /// The event to trigger displaying with the loaded image after peek_walk_turtle of the widget.
    #[rust] next_frame: NextFrame,
    /// Whether to display the UI overlay, including buttons and metadata.
    #[rust] ui_visible_toggle: bool,
    /// Timer used to animate-out (hide) the UI view after the latest user input.
    #[rust] hide_ui_timer: Timer,
    #[rust] capped_dimension: DVec2,
}

impl Widget for ImageViewer {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        self.match_event(cx, event);

        // Handle the app window being resized.
        if matches!(event, Event::WindowGeomChange(_)) {
            let image_container_rect = self.view.area().rect(cx);
            self.image_container_size = image_container_rect.size;

            // Save current drag state to retain zoom and pan
            let saved_zoom_level = self.drag_state.zoom_level;
            let saved_pan_offset = self.drag_state.pan_offset;

            // Recalculate base dimensions for new container size
            self.display_using_texture(cx);

            // Restore drag state
            self.drag_state.zoom_level = saved_zoom_level;
            self.drag_state.pan_offset = saved_pan_offset;

            // Reapply zoom and pan if they differ from defaults
            if saved_zoom_level != 1.0 || saved_pan_offset.is_some() {
                let rotated_image = self.view.rotated_image(ids!(rotated_image));
                let width = self.capped_dimension.x * saved_zoom_level;
                let height = self.capped_dimension.y * saved_zoom_level;

                if let Some(offset) = saved_pan_offset {
                    rotated_image.apply_over(
                        cx,
                        live! {
                            margin: { top: (offset.y), left: (offset.x) },
                            width: (width),
                            height: (height),
                        },
                    );
                } else {
                    rotated_image.apply_over(
                        cx,
                        live! {
                            width: (width),
                            height: (height),
                        },
                    );
                }
            }
        }

        // Handle hover events for UI elements without consuming the main image events
        // We'll track hover state in the FingerMove event within the image handling
        let rotated_image = self.view.rotated_image(ids!(rotated_image));
        let button_group_rounded_view = self.view.view(ids!(button_group_rounded_view));
        match event.hits(cx, button_group_rounded_view.area()) {
            Hit::FingerHoverIn(_) if !self.ui_visible_toggle => {
                cx.stop_timer(self.hide_ui_timer);
                self.animator_cut(cx, ids!(ui_animator.show));
            }
            Hit::FingerHoverOut(fe) => {
                // FingerHoverOut is triggered when the cursor enters into the button.
                // Hence we need to check if the cursor is actually inside the button group.
                if !self.ui_visible_toggle
                    && !button_group_rounded_view.area().rect(cx).contains(fe.abs)
                {
                    self.hide_ui_timer = cx.start_timeout(SHOW_UI_DURATION);
                }
            }
            _ => {}
        }
        match event.hits(cx, self.view.view(ids!(metadata_rounded_view)).area()) {
            Hit::FingerHoverIn(_) if !self.ui_visible_toggle => {
                cx.stop_timer(self.hide_ui_timer);
                self.animator_cut(cx, ids!(ui_animator.show));
            }
            Hit::FingerHoverOut(_) if !self.ui_visible_toggle => {
                self.hide_ui_timer = cx.start_timeout(SHOW_UI_DURATION);
            }
            _ => {}
        }
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
                self.ui_visible_toggle = !self.ui_visible_toggle;
                if self.ui_visible_toggle {
                    self.animator_play(cx, ids!(ui_animator.show));
                } else {
                    self.animator_play(cx, ids!(ui_animator.hide));
                }
                cx.stop_timer(self.hide_ui_timer);
            }
            Hit::FingerHoverIn(_) => {
                self.mouse_cursor_hover_over_image = true;
                cx.set_cursor(MouseCursor::Hand);
            }
            Hit::FingerMove(fe) => {
                if let Some(current_offset) = self.drag_state.pan_offset {
                    let drag_delta = fe.abs - self.drag_state.drag_start;
                    let new_offset = current_offset + drag_delta * self.config.pan_sensitivity;
                    let rotated_image_container = self.view.rotated_image(ids!(rotated_image));
                    let size = rotated_image_container.area().rect(cx).size;
                    rotated_image_container.apply_over(
                        cx,
                        live! {
                            margin: { top: (new_offset.y), left: (new_offset.x) },
                            width: (size.x),
                            height: (size.y)
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
            Hit::FingerHoverOver(_) => {
                if !self.ui_visible_toggle
                    && !self.animator.animator_in_state(cx, ids!(ui_animator.show))
                {
                    self.animator_cut(cx, ids!(ui_animator.hide));
                    self.animator_play(cx, ids!(ui_animator.show));
                    cx.stop_timer(self.hide_ui_timer);
                    self.hide_ui_timer = cx.start_timeout(SHOW_UI_DURATION);
                }
            }
            _ => {}
        }
        if let Event::Scroll(scroll_event) = event {
            if self.mouse_cursor_hover_over_image {
                let scroll_delta = scroll_event.scroll.y;
                if scroll_delta > 0.0 {
                    // Scroll up = Zoom in
                    self.adjust_zoom(cx, self.config.zoom_scale_factor);
                } else if scroll_delta < 0.0 {
                    // Scroll down = Zoom out
                    self.adjust_zoom(cx, 1.0 / self.config.zoom_scale_factor);
                }
            }
        }
        if let Event::KeyDown(e) = event {
            match &e.key_code {
                KeyCode::Minus | KeyCode::NumpadSubtract => {
                    // Zoom out (make image smaller)
                    self.adjust_zoom(cx, 1.0 / self.config.zoom_scale_factor);
                }
                KeyCode::Equals | KeyCode::NumpadAdd => {
                    // Zoom in (make image larger)
                    self.adjust_zoom(cx, self.config.zoom_scale_factor);
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

        let animator_action = self.animator_handle_event(cx, event);
        if self.next_frame.is_event(event).is_some() {
            self.display_using_texture(cx);
        }
        else if let Event::NextFrame(_) = event {
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

        if event.back_pressed()
            || matches!(event, Event::KeyDown(KeyEvent { key_code: KeyCode::Escape, .. }))
        {
            self.reset(cx);
            cx.action(ImageViewerAction::Hide);
        }

        if self.hide_ui_timer.is_event(event).is_some() {
            self.animator_play(cx, ids!(ui_animator.hide));
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        if self.image_container_size.length() == 0.0 {
            let rect = cx.peek_walk_turtle(walk);
            self.image_container_size = rect.size;
            self.next_frame = cx.new_next_frame();
        }
        self.view.draw_walk(cx, scope, walk)
    }
}

impl MatchEvent for ImageViewer {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        if self.view.button(ids!(close_button)).clicked(actions) {
            self.reset(cx);
            cx.action(ImageViewerAction::Hide);
        }
        if self.view.button(ids!(reset_button)).clicked(actions) {
            self.reset(cx);
        }
        if self
            .view
            .button(ids!(zoom_out_button))
            .clicked(actions)
        {
            self.adjust_zoom(cx, 1.0 / self.config.zoom_scale_factor);
        }

        if self
            .view
            .button(ids!(zoom_in_button))
            .clicked(actions)
        {
            self.adjust_zoom(cx, self.config.zoom_scale_factor);
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

        for action in actions.iter() {
            if let Some(ImageViewerAction::Show(state)) = action.downcast_ref() {
                match state {
                    LoadState::Loading(texture, metadata) => {
                        self.texture = texture.clone();
                        self.next_frame = cx.new_next_frame();
                        if let Some(metadata) = metadata {
                            self.set_metadata(cx, metadata);
                        }
                        self.show_loading(cx);
                    }
                    LoadState::Loaded(image_bytes) => {
                        self.show_loaded(cx, image_bytes);
                    }
                    LoadState::FinishedBackgroundDecoding => {
                        self.is_loaded = true;
                        self.hide_footer(cx);
                    },
                    LoadState::Error(error) => {
                        self.show_error(cx, error);
                    }
                }
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
        self.is_loaded = false;
        self.image_container_size = DVec2::new();
        self.ui_visible_toggle = false;
        cx.stop_timer(self.hide_ui_timer);
        self.animator_cut(cx, ids!(ui_animator.show));
        self.hide_ui_timer = Timer::empty();
        self.reset_drag_state(cx);
        self.animator_cut(cx, ids!(mode.upright));
        let rotated_image_ref = self
            .view
            .rotated_image(ids!(rotated_image_container.rotated_image));
        rotated_image_ref.apply_over(cx, live! {
            draw_bg: { scale: 1.0 }
        });
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
        cx.stop_timer(self.hide_ui_timer);
        self.hide_ui_timer = cx.start_timeout(SHOW_UI_DURATION);
    }

    /// Displays an image in the image viewer widget using the provided texture.
    /// 
    /// `Texture` is an optional `Texture` that can be set to display an image. If `None`, the image is cleared.
    pub fn display_using_texture(&mut self, cx: &mut Cx) {
        if self.image_container_size.length() == 0.0 {
            return;
        }
        let texture = self.texture.clone();
        let rotated_image = self.rotated_image(ids!(rotated_image));
        let (texture_width, texture_height) = texture
            .as_ref()
            .and_then(|texture| texture.get_format(cx).vec_width_height())
            .unwrap_or_default();
        
        // Calculate scaling factors for both dimensions
        let scale_x = self.image_container_size.x / texture_width as f64;
        let scale_y = self.image_container_size.y / texture_height as f64;
        
        // Use the smaller scale factor to ensure image fits within container
        let scale = scale_x.min(scale_y);
        
        let capped_width = (texture_width as f64 * scale).floor();
        let capped_height = (texture_height as f64 * scale).floor();
        self.capped_dimension = DVec2{
            x: capped_width,
            y: capped_height
        };
        
        rotated_image.set_texture(cx, texture);
        rotated_image.apply_over(
            cx,
            live! {
                width: (capped_width),
                height: (capped_height),
            },
        );
    }

    /// Adjust the zoom level of the image viewer based on the provided zoom factor.
    fn adjust_zoom(&mut self, cx: &mut Cx, zoom_factor: f64) {
        let rotated_image = self.view.rotated_image(ids!(rotated_image));
        let size = rotated_image.area().rect(cx).size;
        let capped_dimension = self.capped_dimension;
        let target_zoom = self.drag_state.zoom_level * zoom_factor;
        let (width, height) = if target_zoom < self.config.min_zoom {
            (capped_dimension.x * self.config.min_zoom, capped_dimension.y * self.config.min_zoom)
        } else {
            let actual_zoom_factor = target_zoom / self.drag_state.zoom_level;
            self.drag_state.zoom_level = target_zoom;
            self.drag_state.zoom_level = (self.drag_state.zoom_level * 1000.0).round() / 1000.0;
            let width = (size.x * actual_zoom_factor * 1000.0).round() / 1000.0;
            let height = (size.y * actual_zoom_factor * 1000.0).round() / 1000.0;
            (width, height)
        };

        rotated_image.apply_over(cx, live! {
            width: (width),
            height: (height),
        });
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
                self.adjust_zoom(cx, scale);
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
        footer.set_visible(cx, true);
        self.ui_visible_toggle = true;
        cx.stop_timer(self.hide_ui_timer);
        self.hide_ui_timer = cx.start_timeout(SHOW_UI_DURATION);
    }

    /// Shows an error message in the footer.
    ///
    /// The loading spinner is hidden, the error icon is shown, and the
    /// status label is set to the error message provided.
    pub fn show_error(&mut self, cx: &mut Cx, error: &ImageViewerError) {
        if self.is_loaded {
            return;
        }
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
        footer.set_visible(cx, true);
    }

    /// Hides the footer of the image viewer.
    pub fn hide_footer(&mut self, cx: &mut Cx) {
        let footer = self.view.view(ids!(image_layer.footer));
        footer.set_visible(cx, false);
    }

    /// Sets the metadata view in the image viewer with the provided metadata.
    ///
    /// The metadata view is updated with the truncated image name and the human-readable size of the image.
    ///
    /// The image name is truncated to 24 characters and appended with "..." if it exceeds the limit.
    /// The human-readable size is calculated based on the image size in bytes.
    pub fn set_metadata(&mut self, cx: &mut Cx, metadata: &ImageViewerMetaData) {
        let meta_view = self.view.view(ids!(metadata_view));
        let truncated_name = truncate_image_name(&metadata.image_name);
        let human_readable_size = format_file_size(metadata.image_file_size);
        let display_text = format!("{} ({})", truncated_name, human_readable_size);
        meta_view
            .label(ids!(image_name_and_size))
            .set_text(cx, &display_text);
        if let Some(timestamp) = metadata.timestamp {
            meta_view
                .view(ids!(user_profile_view.content.timestamp_view))
                .set_visible(cx, true);
            meta_view
                .timestamp(ids!(user_profile_view.content.timestamp_view.timestamp))
                .set_date_time(cx, timestamp);
        }

        if let Some((room_id, event_timeline_item)) = &metadata.avatar_parameter {            
            let (sender, _) = self.view.avatar(ids!(user_profile_view.avatar)).set_avatar_and_get_username(
                cx,
                room_id,
                event_timeline_item.sender(),
                Some(event_timeline_item.sender_profile()),
                event_timeline_item.event_id(),
                false,
            );
            if sender.len() > MAX_USERNAME_LENGTH {
                meta_view
                    .label(ids!(user_profile_view.content.username))
                    .set_text(cx, &format!("{}...", &sender[..MAX_USERNAME_LENGTH - 3]));
            } else {
                meta_view
                    .label(ids!(user_profile_view.content.username))
                    .set_text(cx, &sender);
            };
        }
    }
}

impl ImageViewerRef {
    /// Configure zoom and pan settings for the image viewer
    pub fn configure_zoom(&mut self, config: ImageViewerZoomConfig) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.config = config;
    }

    /// See [`ImageViewer::show_loaded()`].
    pub fn show_loaded(&mut self, cx: &mut Cx, image_bytes: &[u8]) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.show_loaded(cx, image_bytes)
    }

    /// Display the image viewer widget with the provided texture, metadata and loading spinner.
    pub fn show_loading(
        &mut self,
        cx: &mut Cx,
        texture: Option<Texture>,
        metadata: &Option<ImageViewerMetaData>,
    ) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.texture = texture.clone();
        inner.next_frame = cx.new_next_frame();
        if let Some(metadata) = metadata {
            inner.set_metadata(cx, metadata);
        }
        inner.show_loading(cx);
    }

    /// See [`ImageViewer::show_error()`].
    pub fn show_error(&mut self, cx: &mut Cx, error: &ImageViewerError) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.show_error(cx, error);
    }

    /// See [`ImageViewer::hide_footer()`].
    pub fn hide_footer(&mut self, cx: &mut Cx) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.hide_footer(cx);
    }

    /// See [`ImageViewer::reset()`].
    pub fn reset(&mut self, cx: &mut Cx) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.reset(cx);
    }
}

/// Represents the possible states of an image load operation.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone)]
pub enum LoadState {
    /// The image is currently being loaded with its loading image texture.
    /// This texture is usually the image texture that's being selected.
    Loading(Option<Texture>, Option<ImageViewerMetaData>),
    /// The image has been successfully loaded given the data.
    Loaded(Arc<[u8]>),
    /// The image has been decoded from background thread.
    FinishedBackgroundDecoding,
    /// An error occurred while loading the image, with specific error type.
    Error(ImageViewerError),
}

#[derive(Debug, Clone)]
/// Metadata for an image.
pub struct ImageViewerMetaData {
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
