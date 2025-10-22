//! Image viewer modal widget for displaying Matrix media content.
//!
//! It supports zooming, panning, loading states, error handling, and timeout management.
//! The modal integrates with the media cache to efficiently load and display images.
//! There are 3 types of ImageViewerModalActions handled by this widget. They are "OpenModal", "Show" and "SetImage".
//! 
use std::sync::Arc;

use makepad_widgets::{image_cache::ImageError, *};

use crate::utils::load_png_or_jpg;

// Image loading timeout in seconds (10 seconds)
pub const IMAGE_LOAD_TIMEOUT: f64 = 10.0;

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
    /// Whether the user is currently dragging the image
    is_dragging: bool,
    /// The starting position of the drag.
    drag_start: DVec2,
    /// The zoom level of the image.
    /// 
    /// 1.0 = 100%
    /// 0.5 = 200%
    zoom_level: f64,
    /// The pan offset of the image.
    pan_offset: DVec2,
    /// Whether the user has clicked the magnifying glass to pan the image.
    is_panning: bool,
}

impl Default for DragState {
    /// Resets all the drag state to its default values. This is called when the image changes.
    fn default() -> Self {
        Self {
            is_dragging: false,
            drag_start: DVec2::default(),
            zoom_level: 1.0,
            pan_offset: DVec2::default(),
            is_panning: false,
        }
    }
}

live_design! {
    use link::theme::*;
    use link::widgets::*;
    use crate::shared::styles::*;
    use crate::shared::icon_button::RobrixIconButton;

    pub ImageViewerModal = {{ImageViewerModal}} {
        width: Fill, height: Fill
    
        image_modal = <Modal> {
            content: <View> {
                width: Fill, height: Fill,
                flow: Overlay
                padding: 0
                spacing: 0
                show_bg: true
                draw_bg: {
                    color: #000
                }

                image_container = <View> {
                    width: Fill, height: Fill,
                    // Overlay is required to center align the image.
                    flow: Overlay
                    padding: {top: 50}
                    align: {x: 0.5, y: 0.5}

                    zoomable_image = <Image> {
                        width: Fill, height: Fill
                        fit: Smallest,
                    }
                }

                <View> {
                    width: Fill, height: Fill,
                    flow: Down,
                    align: {x: 0.5}

                    header = <View> {
                        width: Fill, height: Fit
                        flow: Right
                        align: {x: 1.0, y: 0.0},

                        <View> {
                            width: Fit, height: Fit
                            flow: Overlay

                            magnifying_glass_button = <RobrixIconButton> {
                                width: Fit, height: Fit,
                                spacing: 0,
                                margin: 8,
                                padding: 3
                                draw_bg: {
                                    color: (COLOR_SECONDARY)
                                }
                                draw_icon: {
                                    svg_file: (ICON_ZOOM),
                                    fn get_color(self) -> vec4 {
                                        return #x0;
                                    }
                                }
                                icon_walk: {width: 30, height: 30}
                            }
                            <View> {
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
                        
                        close_button = <RobrixIconButton> {
                            width: Fit, height: Fit,
                            spacing: 0,
                            margin: 8,
                            draw_bg: {
                                color: (COLOR_SECONDARY)
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

                    <View> {
                        width: Fill, height: Fill,
                        align: { x: 0.5, y: 0.5 }
                        flow: Down

                        loading_view = <View> {
                            width: Fill, height: Fit,
                            flow: Down,
                            align: { x: 0.5, y: 0.5 },
                            spacing: 10

                            loading_spinner = <LoadingSpinner> {
                                width: 40, height: 40,
                                draw_bg: {
                                    color: (COLOR_PRIMARY)
                                    border_size: 3.0,
                                }
                            }

                            <Label> {
                                width: Fit, height: 30,
                                text: "Loading image...",
                                draw_text: {
                                    text_style: <REGULAR_TEXT>{font_size: 14},
                                    color: (COLOR_PRIMARY)
                                }
                            }
                        }

                        error_label_view = <View> {
                            width: Fill, height: Fit,
                            flow: Down,
                            align: { x: 0.5, y: 0.5 },
                            spacing: 10
                            visible: false
                            <Icon> {
                                draw_icon: {
                                    svg_file: (ICON_FORBIDDEN),
                                    color: #ffffff,
                                }
                                icon_walk: { width: 50, height: 50 }
                            }

                            error_label = <Label> {
                                width: Fill, height: Fit,
                                flow: RightWrap,
                                align: { x: 0.5, y: 0.5 }
                                draw_text: {
                                    text_style: <REGULAR_TEXT>{ font_size: 14 },
                                    color: #f44,
                                    wrap: Word
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Actions handled by the `ImageViewer` widget.
#[derive(Clone, Debug, DefaultNone)]
pub enum ImageViewerModalAction {
    /// OpenModal the ImageViewerModal widget with a source id.
    /// The source is will be in-flight mode to avoid handling actions with different source id.
    /// This will open the ImageViewerModal widget with loading state.
    OpenModal(String),
    /// Display the ImageViewerModal widget based on the given source id and LoadState.
    Show(String, LoadState),
    /// Set the image being displayed by the ImageViewer based on the given source id andthe image data.
    SetImage(String, Arc<[u8]>),
    None,
}

#[derive(Live, Widget, LiveHook)]
struct ImageViewerModal {
    #[deref]
    view: View,
    #[rust]
    image_loaded: bool,
    /// The timer for displaying the timeout error message after waiting for IMAGE_LOAD_TIMEOUT.
    #[rust]
    timeout_timer: Timer,
    #[rust]
    drag_state: DragState,
    /// The source id of the image being displayed.
    /// This is used to avoid handling actions with different source id.
    /// This is set during the `OpenModal` action.
    #[rust]
    source_id: Option<String>,
}

impl Widget for ImageViewerModal {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if self.image_loaded && self.drag_state.is_panning {
            let zoomable_image = self.view.image(id!(zoomable_image));

            // Handle cursor changes on mouse hover
            match event.hits(cx, zoomable_image.area()) {
                Hit::FingerDown(fe) => {
                    if fe.is_primary_hit() {
                        self.drag_state.drag_start = fe.abs;
                        self.drag_state.is_dragging = true;
                    }
                }
                Hit::FingerUp(fe) if fe.is_over && fe.is_primary_hit() && fe.was_tap() => {
                    if fe.is_primary_hit() {
                        self.drag_state.is_dragging = false;
                    }
                }
                Hit::FingerHoverIn(_) => {
                    if self.drag_state.is_panning {
                        cx.set_cursor(MouseCursor::Hand);
                    }
                }
                Hit::FingerMove(fe) => {
                    if self.drag_state.is_dragging && self.drag_state.zoom_level != 0.0 {
                        let delta = fe.abs - self.drag_state.drag_start;
                        let prev_pan_offset = round_to_3_decimal_places(self.drag_state.pan_offset);
                        self.drag_state.pan_offset += delta * -0.001 * self.drag_state.zoom_level; // Scale movement by zoom level

                        // Restrict pan_offset to keep image within bounds
                        // When zoom_level < 1.0, the image is enlarged, so we need to limit panning
                        if self.drag_state.zoom_level < 1.0 {
                            // Calculate the maximum pan offset based on how much the image is enlarged
                            let max_pan = (1.0 - self.drag_state.zoom_level) * 0.5;
                            self.drag_state.pan_offset.x =
                                self.drag_state.pan_offset.x.clamp(0.0, max_pan * 2.0);
                            self.drag_state.pan_offset.y =
                                self.drag_state.pan_offset.y.clamp(0.0, max_pan * 2.0);
                            if round_to_3_decimal_places(self.drag_state.pan_offset)
                                != prev_pan_offset
                            {
                                self.update_image_shader(cx);
                            }
                        }
                        self.drag_state.drag_start = fe.abs;
                    }
                }
                Hit::FingerHoverOut(_) => {
                    cx.set_cursor(MouseCursor::Arrow);
                }
                _ => {}
            }
            if let Event::KeyDown(e) = event {
                match &e.key_code {
                    KeyCode::Minus | KeyCode::NumpadSubtract => {
                        // Zoom out (make image smaller)
                        self.adjust_zoom(cx, 1.2, 0.2, 1.0);
                    }
                    KeyCode::Equals | KeyCode::NumpadAdd => {
                        // Zoom in (make image larger)
                        self.adjust_zoom(cx, 1.0 / 1.2, 0.2, 1.0);
                    }
                    KeyCode::Key0 | KeyCode::Numpad0 => {
                        self.reset_drag_state(cx);
                    }
                    _ => {}
                }
            }
        }

        self.view.handle_event(cx, event, scope);

        if self.timeout_timer.is_event(event).is_some() {
            cx.stop_timer(self.timeout_timer);
            // Only show timeout if the image hasn't already loaded
            if !self.image_loaded {
                self.show_image_modal_view(cx, LoadState::Error(ImageViewerError::Timeout));
            }
        }

        self.match_event(cx, event);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl MatchEvent for ImageViewerModal {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        if self.view.button(id!(close_button)).clicked(actions) {
            self.close(cx);
        }
        if self.view.button(id!(magnifying_glass_button)).clicked(actions) {
            if self.drag_state.zoom_level == 1.0 {
                self.view.label(id!(magnify_glass_sign)).set_text(cx, "-");
                self.drag_state.zoom_level = 1.0 / 1.2;
                self.drag_state.is_panning = true;
                self.drag_state.pan_offset.x = (1.0 - self.drag_state.zoom_level) * 0.5;
                self.drag_state.pan_offset.y = (1.0 - self.drag_state.zoom_level) * 0.5;
            } else {
                self.view.label(id!(magnify_glass_sign)).set_text(cx, "+");
                self.reset_drag_state(cx);
            }
            self.update_image_shader(cx);
        }
        for action in actions {
            match action.downcast_ref::<ImageViewerModalAction>() {
                Some(ImageViewerModalAction::OpenModal(source)) => {
                    self.image_loaded = false;
                    self.source_id = Some(source.clone());
                    self.modal(id!(image_modal)).open(cx);
                    self.timeout_timer = cx.start_timeout(IMAGE_LOAD_TIMEOUT);
                    self.show_image_modal_view(cx, LoadState::Loading);
                }
                Some(ImageViewerModalAction::Show(source, load_state)) => {
                    // Ignore action if the source doesn't match
                    if Some(source) != self.source_id.as_ref() {
                        continue;
                    }
                    if load_state == &LoadState::Loading {
                        self.modal(id!(image_modal)).open(cx);
                        self.timeout_timer = cx.start_timeout(IMAGE_LOAD_TIMEOUT);
                    } else {
                        cx.stop_timer(self.timeout_timer);
                    }
                    self.show_image_modal_view(cx, load_state.clone());
                }
                Some(ImageViewerModalAction::SetImage(source, data)) => {
                    // Ignore action if the source doesn't match
                    if Some(source) != self.source_id.as_ref() {
                        continue;
                    }
                    cx.stop_timer(self.timeout_timer);   
                    self.image_loaded = true;
                    if let Err(e) = self.load_image_data(cx, self.view.image(id!(zoomable_image)), data) {
                        // Determine error type based on the image error
                        let error_type = match e {
                            ImageError::JpgDecode(_) | ImageError::PngDecode(_) => ImageViewerError::UnsupportedFormat,
                            ImageError::EmptyData => ImageViewerError::BadData,
                            ImageError::PathNotFound(_) => ImageViewerError::NotFound,
                            ImageError::UnsupportedFormat => ImageViewerError::UnsupportedFormat,
                            _ => ImageViewerError::BadData,
                        };
                        self.show_image_modal_view(cx, LoadState::Error(error_type));
                    }
                }
                _ => {}
            }
        }
    }
}

impl ImageViewerModal {

    /// Close the modal and reset its state.
    fn close(&mut self, cx: &mut Cx) {
        self.image_loaded = false;
        self.source_id = None;
        self.reset_drag_state(cx);
        self.update_image_shader(cx);
        self.view
            .view_set(ids!(loading_view, error_label_view))
            .set_visible(cx, false);
        self.view.image(id!(zoomable_image)).set_visible(cx, false);
        // Clear the image buffer. 
        let _ = self.view.image(id!(zoomable_image)).load_jpg_from_data(cx, &[]);
        self.view.label(id!(magnify_glass_sign)).set_text(cx, "+");
        self.modal(id!(image_modal)).close(cx);
        cx.stop_timer(self.timeout_timer);
    }

    /// Updates the shader uniforms of the zoomable image widget with the current zoom level and pan offset,
    /// and requests a redraw.
    fn update_image_shader(&mut self, cx: &mut Cx) {
        // Get the zoomable image widget and update its shader uniforms
        let zoomable_image = self.view.image(id!(zoomable_image));
        zoomable_image.apply_over(
            cx,
            live! {
                draw_bg: {
                    image_scale: (self.drag_state.zoom_level),
                    image_pan: (self.drag_state.pan_offset)
                }
            },
        );
        // Request a redraw
        zoomable_image.redraw(cx);
    }

    /// Adjusts the zoom level while maintaining the relative pan position
    fn adjust_zoom(&mut self, cx: &mut Cx, zoom_factor: f64, min_zoom: f64, max_zoom: f64) {
        let old_zoom = self.drag_state.zoom_level;
        let old_max_pan = (1.0 - old_zoom) * 0.5;
        
        // Calculate the relative position (0.0 to 1.0) in the current pan range
        let relative_x = if old_max_pan > 0.0 {
            self.drag_state.pan_offset.x / (old_max_pan * 2.0)
        } else {
            0.5 // Center if no panning range
        };
        let relative_y = if old_max_pan > 0.0 {
            self.drag_state.pan_offset.y / (old_max_pan * 2.0)
        } else {
            0.5 // Center if no panning range
        };
        
        // Update zoom level
        self.drag_state.zoom_level = (self.drag_state.zoom_level * zoom_factor).clamp(min_zoom, max_zoom);
        
        // Calculate new max pan range and maintain relative position
        let new_max_pan = (1.0 - self.drag_state.zoom_level) * 0.5;
        self.drag_state.pan_offset.x = relative_x * new_max_pan * 2.0;
        self.drag_state.pan_offset.y = relative_y * new_max_pan * 2.0;
        
        // Clamp to ensure we stay within bounds
        self.drag_state.pan_offset.x = self.drag_state.pan_offset.x.clamp(0.0, new_max_pan * 2.0);
        self.drag_state.pan_offset.y = self.drag_state.pan_offset.y.clamp(0.0, new_max_pan * 2.0);
        
        self.update_image_shader(cx);
    }

    /// Resets the drag state of the modal to its initial state.
    ///
    /// This function can be used to reset drag state when the magnifying glass is toggled off.
    fn reset_drag_state(&mut self, cx: &mut Cx) {
        self.drag_state = DragState::default();
        self.update_image_shader(cx);
    }

    /// Shows the view at the given load state in the view set,
    /// and hides all other views in the set. The zoomable image is also
    /// hidden.
    /// 
    /// The ViewSet is in this order: the loading, error views.
    fn show_image_modal_view(&mut self, cx: &mut Cx, load_state: LoadState) {
        match load_state {
            LoadState::Loading => {
                self.view(id!(loading_view)).set_visible(cx, true);
                self.view(id!(error_label_view)).set_visible(cx, false);
            }
            LoadState::Loaded => {
                self.view(id!(loading_view)).set_visible(cx, false);
                self.view(id!(error_label_view)).set_visible(cx, false);
            }
            LoadState::Error(_) => {
                self.view(id!(loading_view)).set_visible(cx, false);
                self.view(id!(error_label_view)).set_visible(cx, true);
            }
        }
        // If it's an error state, update the error message and icon
        if let LoadState::Error(e) = &load_state {
            update_error_display(cx, &self.view(id!(error_label_view)), e);
        }
    }

    /// Loads the image data into the given `image_ref` and displays it.
    ///
    /// Shows the `image_ref` and hides loading and error label views.
    ///
    /// If the image fails to load, an `ImageError` is returned.
    fn load_image_data(&mut self, cx: &mut Cx, image_ref: ImageRef, data: &[u8]) -> Result<(), ImageError> {
        load_png_or_jpg(&image_ref, cx, data)?;
        self.view.view_set(ids!(loading_view, error_label_view)).set_visible(cx, false);
        image_ref.set_visible(cx, true);
        Ok(())
    }
}

/// Rounds a given Dvec2 to 3 decimal places.
/// Used to prevent extremely small mouse movement from updating image shader.
fn round_to_3_decimal_places(dvec2: DVec2) -> DVec2 {
    DVec2 {
        x: (dvec2.x * 1000.0).round() / 1000.0,
        y: (dvec2.y * 1000.0).round() / 1000.0,
    }
}




/// Updates the error display with specific message and icon based on error type
fn update_error_display(cx: &mut Cx, error_view: &ViewRef, error: &ImageViewerError) {
    let message = match error {
        ImageViewerError::NotFound => "Image not found",
        ImageViewerError::BadData => "Image appears to be empty or corrupted",
        ImageViewerError::UnsupportedFormat => "This image format isn't supported",
        ImageViewerError::ConnectionFailed => "Check your internet connection",
        ImageViewerError::Unauthorized => "You don't have permission to view this image",
        ImageViewerError::ServerError => "Server temporarily unavailable",
        ImageViewerError::Unknown => "Unable to load image",
        ImageViewerError::Timeout => "Image load timed out",
    };

    // Update the label text
    error_view.label(id!(error_label)).set_text(cx, message);
}

/// Represents the possible states of an image load operation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LoadState {
    /// The image is currently being loaded.
    Loading,
    /// The image has been successfully loaded.
    Loaded,
    /// An error occurred while loading the image, with specific error type.
    Error(ImageViewerError),
}
