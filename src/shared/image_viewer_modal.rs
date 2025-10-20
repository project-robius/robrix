use std::sync::Arc;

use makepad_widgets::{image_cache::ImageError, *};
use matrix_sdk::ruma::{OwnedMxcUri, OwnedRoomId};

use crate::utils::load_png_or_jpg;
use crate::media_cache::MediaCacheEntry;
use matrix_sdk::media::MediaFormat;

// Image loading timeout in seconds (10 seconds)
pub const IMAGE_LOAD_TIMEOUT: f64 = 10.0;

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

                        magnify_button = <RobrixIconButton> {
                            width: Fit, height: Fit,
                            spacing: 0,
                            margin: 7,
                            draw_bg: {
                                color: (COLOR_SECONDARY)
                            }
                            draw_icon: {
                                svg_file: (ICON_SEARCH),
                                fn get_color(self) -> vec4 {
                                    return #x0;
                                }
                            }
                            icon_walk: {width: 14, height: 14}
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
                            icon_walk: {width: 14, height: 14}
                        }
                    }
                    <View> {
                        width: Fill, height: Fill,
                        align: {x: 0.5, y: 0.5}
                        flow: Down

                        loading_view = <View> {
                            width: Fill, height: Fit,
                            flow: Down,
                            align: {x: 0.5, y: 0.5},
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
                                width: Fit, height: Fit,
                                text: "Failed to load image",
                                draw_text: {
                                    text_style: <REGULAR_TEXT>{font_size: 14},
                                    color: #f44
                                }
                            }
                        }

                        timeout_label_view = <View> {
                            width: Fill, height: Fit,
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
                                width: Fit, height: Fit,
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

/// Actions handled by the `ImageViewer` widget.
#[derive(Clone, Debug, DefaultNone)]
pub enum ImageViewerModalAction {
    /// Initialize the ImageViewer widget with a source URI
    Initialize(String),
    /// Make the ImageViewer widget visible.
    Show(LoadState),
    /// Set the image being displayed by the ImageViewer. (given the image data)
    SetImage(Arc<[u8]>),
    DrawImage,
    None,
}

#[derive(Live, Widget, LiveHook)]
struct ImageViewerModal {
    #[deref]
    view: View,
    #[rust]
    mxc_uri: Option<OwnedMxcUri>,
    #[rust]
    image_loaded: bool,
    #[rust]
    timeout_timer: Timer,
    #[rust]
    drag_state: DragState,
    #[rust]
    room_id: Option<OwnedRoomId>,
    #[rust]
    unqiue_id: Option<String>,
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
                self.show_timeout_state(cx);
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
        if self.view.button(id!(magnify_button)).clicked(actions) {
            if self.drag_state.zoom_level == 1.0 {
                self.drag_state.zoom_level = 1.0 / 1.2;
                self.drag_state.is_panning = true;
                self.drag_state.pan_offset.x = (1.0 - self.drag_state.zoom_level) * 0.5;
                self.drag_state.pan_offset.y = (1.0 - self.drag_state.zoom_level) * 0.5;
            } else {
                self.reset_drag_state(cx);
            }
            self.update_image_shader(cx);
        }
        for action in actions {
            match action.downcast_ref::<ImageViewerModalAction>() {
                Some(ImageViewerModalAction::Initialize(source)) => {
                    self.image_loaded = false;
                    self.unqiue_id = Some(source.clone());
                }
                Some(ImageViewerModalAction::Show(load_state)) => {
                    cx.stop_timer(self.timeout_timer);
                    if load_state == &LoadState::Loading {
                        self.modal(id!(image_modal)).open(cx);
                        self.timeout_timer = cx.start_timeout(IMAGE_LOAD_TIMEOUT);
                    }
                    show_image_modal_view(cx, self.view_set(), *load_state);
                }
                Some(ImageViewerModalAction::SetImage(data)) => {
                    cx.stop_timer(self.timeout_timer);   
                    self.image_loaded = true;
                    let _ = load_image_data(cx, self.view.image(id!(zoomable_image)), self.view_set(), &data);
                }
                Some(ImageViewerModalAction::DrawImage) => {
                    self.image_loaded = true;
                }
                _ => {}
            }
        }
    }
}

impl ImageViewerModal {

    /// Show timeout state
    pub fn show_timeout_state(&mut self, cx: &mut Cx) {
        self.view
            .view(id!(timeout_label_view))
            .set_visible(cx, true);
        self.view
            .view_set(ids!(loading_view, error_label_view))
            .set_visible(cx, false);
        self.view.image(id!(zoomable_image)).set_visible(cx, false);
    }

    /// Close the modal
    fn close(&mut self, cx: &mut Cx) {
        self.mxc_uri = None;
        self.image_loaded = false;
        self.room_id = None;
        self.reset_drag_state(cx);
        self.update_image_shader(cx);
        self.view
            .view_set(ids!(loading_view, error_label_view, timeout_label_view))
            .set_visible(cx, false);
        self.view.image(id!(zoomable_image)).set_visible(cx, false);
        // Clear the image buffer. 
        let _ = self.view.image(id!(zoomable_image)).load_jpg_from_data(cx, &[]);
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
    fn view_set(&mut self) -> ViewSet {
        self.view.view_set(ids!(loading_view, error_label_view, timeout_label_view))
    }
}

impl ImageViewerModalRef {
    /// Returns whether to call mediacache's get_media_or_fetch function.
    pub fn get_media_or_fetch(&self, room_id: OwnedRoomId) -> bool {
        if let Some(inner) = self.borrow() {
            inner.room_id == Some(room_id) && !inner.image_loaded
        } else {
            false
        }
    }

    /// Returns a ViewSet that contains the loading, error and timeout views of the
    /// image viewer modal.
    pub fn get_view_set(&self) -> Option<ViewSet> {
        if let Some(mut inner) = self.borrow_mut() {
            Some(inner.view.view_set(ids!(loading_view, error_label_view, timeout_label_view)))
        } else {
            None
        }
    }

    /// Returns a reference to the zoomable image widget that is used to display the image
    /// in the image viewer modal
    pub fn get_zoomable_image(&self) -> Option<ImageRef> {
        if let Some(inner) = self.borrow() {
            Some(inner.view.image(id!(zoomable_image)))
        } else {
            None
        }
    }

    /// Returns a reference to the modal widget that is used to display the image viewer modal
    pub fn get_image_modal(&mut self) -> Option<ModalRef> {
        self.borrow().map(|inner| inner.modal(id!(image_modal)))
    }

    /// Returns the current media URI of the image viewer modal
    pub fn get_mxc_uri(&self) -> Option<OwnedMxcUri> {
        if let Some(inner) = self.borrow() {
            inner.mxc_uri.clone()
        } else {
            None
        }
    }

    /// Sets the `image_loaded` field of the inner state to true. This is used to
    /// indicate that the image has finished loading and can be displayed.
    pub fn set_image_loaded(&mut self) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.image_loaded = true;
        }
    }

    /// Initializes the image viewer modal's state with the given room ID and media
    /// URI. This function is called when the image viewer modal is opened or
    /// reopened with a new media URI. It is used to set the correct image URI,
    /// room ID, and timer for the modal. It also reset the image loaded flag to false.
    // pub fn initialized(&self, room_id: OwnedRoomId, mxc_uri: OwnedMxcUri, timer: Timer) {
    //     if let Some(mut inner) = self.borrow_mut() {
    //         inner.image_loaded = false;
    //         inner.mxc_uri = Some(mxc_uri);
    //         inner.room_id = Some(room_id);
    //         inner.timeout_timer = timer;
    //     }
    // }
    pub fn initialized(&self, unqiue_id: String) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.image_loaded = false;
            inner.unqiue_id = Some(unqiue_id);
        }
    }

    pub fn unique_key(&self) -> Option<String> {
        if let Some(inner) = self.borrow() {
            inner.unqiue_id.clone()
        } else {
            None
        }
    }
}

/// Retrieves a mutable reference to the global `ImageViewerModalRef`.
///
/// This function accesses the global context to obtain a reference to the
/// `ImageViewerModalRef`, which is used for managing and displaying the
/// image viewer modal within the application. It enables interaction with
/// the image viewer modal system from various parts of the application.
pub fn get_global_image_viewer_modal(cx: &mut Cx) -> &mut ImageViewerModalRef {
    cx.get_global::<ImageViewerModalRef>()
}

/// Sets the global image viewer modal reference.
///
/// This function sets the global context to point to the provided
/// `ImageViewerModalRef`, which is used for managing and displaying the
/// image viewer modal within the application. It enables interaction with
/// the image viewer modal system from various parts of the application.
pub fn set_global_image_viewer_modal(cx: &mut Cx, modal: ImageViewerModalRef) {
    cx.set_global(modal);
}

/// Rounds a given Dvec2 to 3 decimal places.
/// Used to prevent extremely small mouse movement from updating image shader.
fn round_to_3_decimal_places(dvec2: DVec2) -> DVec2 {
    DVec2 {
        x: (dvec2.x * 1000.0).round() / 1000.0,
        y: (dvec2.y * 1000.0).round() / 1000.0,
    }
}

/// Loads the image data into the given `image_ref` and displays it.
///
/// Shows the `image_ref` and hides all views in the given `view_set`.
///
/// If the image fails to load, an `ImageError` is returned.
fn load_image_data(cx: &mut Cx, image_ref: ImageRef, view_set: ViewSet, data: &[u8]) -> Result<(), ImageError> {
    load_png_or_jpg(&image_ref, cx, data)?;
    view_set
        .set_visible(cx, false);
    image_ref.set_visible(cx, true);
    Ok(())
}

/// Shows the view at the given load state in the provided view set,
/// and hides all other views in the set. The zoomable image is also
/// hidden.
/// 
/// The ViewSet is in this order: the loading, error and timeout views.
fn show_image_modal_view(cx: &mut Cx, view_set: ViewSet, load_state: LoadState) {
    for (i, view_ref) in view_set.iter().enumerate() {
        let should_show = match load_state {
            LoadState::Loading => i == 0,
            LoadState::Error => i == 1,
            LoadState::Timeout => i == 2,
            LoadState::Loaded => false, // Hide all views when loaded
        };
        view_ref.set_visible(cx, should_show);
    }
}

/// Represents the possible states of an image load operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoadState {
    Loading,
    Loaded,
    Error,
    Timeout,
}

/// Handles media cache entry states for the image modal
pub fn handle_media_cache_entry(
    cx: &mut Cx,
    mxc_uri: OwnedMxcUri,
    media_entry: (MediaCacheEntry, MediaFormat),
) -> LoadState {
    match media_entry {
        (MediaCacheEntry::Loaded(data), MediaFormat::File) => {
            cx.action(ImageViewerModalAction::SetImage(data));
            LoadState::Loaded
        }
        (MediaCacheEntry::Requested, _) | (MediaCacheEntry::Loaded(_), MediaFormat::Thumbnail(_)) => {
            LoadState::Loading
        }
        (MediaCacheEntry::Failed, _) => {
            cx.action(ImageViewerModalAction::Show(LoadState::Error));
            LoadState::Error
        }
    }
}