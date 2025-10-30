//! Image viewer widget for displaying Image with zooming and panning.
//!
//! There are 2 types of ImageViewerActions handled by this widget. They are "Show" and "Hide".
//! ImageViewerRef has 2 public methods, `display_image` and `reset`.
use std::sync::Arc;

use makepad_widgets::{image_cache::ImageError, rotated_image::RotatedImageWidgetExt, *};

use crate::utils::{load_png_or_jpg, load_png_or_jpg_rotated_image};

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
    /// 
    /// 1.0 = 100%
    /// 0.5 = 200%
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
            color: (COLOR_SECONDARY)
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
            //color: #000
            color: #ffffff
        }
        flow: Down
        debug: false
        header = <View> {
            width: Fill, height: 50
            flow: Right
            spacing: 0
            align: {x: 1.0, y: 0.0},

            zoom_button_minus = <MagnifyingGlass> {
                sign_label = <View> {
                    width: Fill, height: 50,
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
        image_container = <View> {
            width: Fill, height: Fill,
            flow: Overlay,
            visible: false
            show_bg: true
                draw_bg: {
                    color: #FF0000
                    //color: #ffffff
                }
            // Overlay is required to center align the image.
            align: {x: 0.5, y: 0.5}
            zoomable_image = <Image> {
                width: Fill, height: Fill
                fit: Smallest,
            }
        }
        rotated_image_container = <View> {
            width: Fill, height: Fill,
            flow: Overlay
            show_bg: false
            draw_bg: {
                color: #FF0000
                //color: #ffffff
            }
            align: {x: 0.5, y: 0.5}
            debug: true
            rotated_image = <RotatedImage> {
                width: Fill, height: Fill,
                draw_bg: {
                    scale: 1.0,
                    rotation: 0.0
                    opacity: 1.0
                }
            }
        }
        animator: {
            mode = {
                default: upright,
                degree_neg90 = {
                    redraw: true,
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
                    redraw: true,
                    from: {all: Forward {duration: 1.0}}
                    apply: {
                        rotated_image_container = {
                            rotated_image = {
                                //draw_bg: {rotation: [{time: 0.0, value: 4.71239}, {time: 0.9999, value: 6.28318}, {time: 1.0, value: 0.0}]}
                                draw_bg: {rotation: 0.0}
                            }
                        }
                    }
                }
                degree_90 = {
                    redraw: true,
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
                    redraw: true,
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
                    redraw: true,
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
                    redraw: true,
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

#[derive(Live, Widget, LiveHook)]
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
                    cx.set_cursor(MouseCursor::Hand);
                }
                Hit::FingerMove(fe) => {
                    if let Some(current_offset) = self.drag_state.pan_offset {
                        let drag_delta = fe.abs - self.drag_state.drag_start;
                        let new_offset = current_offset + drag_delta * 2.0;
                        
                        let rotated_image_container = self.view.rotated_image(id!(rotated_image));
                        rotated_image_container.apply_over(
                            cx,
                            live! {
                                margin: { top: (new_offset.y), left: (new_offset.x) },
                            }
                        );
                        rotated_image_container.redraw(cx);
                        
                        // Update pan_offset with new position
                        self.drag_state.pan_offset = Some(new_offset);
                    }
                    self.drag_state.drag_start = fe.abs;
                }
                Hit::FingerHoverOut(_) => {
                    cx.set_cursor(MouseCursor::Default);
                }
                _ => {}
            }
            if let Event::KeyDown(e) = event {
                match &e.key_code {
                    KeyCode::Minus | KeyCode::NumpadSubtract => {
                        // Zoom out (make image smaller)
                        self.adjust_zoom(cx, 1.0 / 1.2);
                    }
                    KeyCode::Equals | KeyCode::NumpadAdd => {
                        // Zoom in (make image larger)
                        self.adjust_zoom(cx, 1.2);
                    }
                    KeyCode::Key0 | KeyCode::Numpad0 => {
                        self.reset_drag_state(cx);
                    }
                    _ => {}
                }
            }  
        }
        if let Some(_timer) = self.timer.is_event(event) {
            self.is_animating_rotation = false;
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
            self.adjust_zoom(cx, 1.0 / 1.2);
        }

        if self.view.button(id!(zoom_button_plus.magnifying_glass_button)).clicked(actions) {
            self.adjust_zoom(cx, 1.2);
        }

        if self.view.button(id!(rotation_button_clockwise)).clicked(actions) {
            if !self.is_animating_rotation {
                self.timer = cx.start_timeout(1.0);
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

        for action in actions {
            match action.downcast_ref::<ImageViewerAction>() {
                Some(ImageViewerAction::Hide) => {
                    self.reset(cx);
                }
                _ => {}
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
        self.reset_drag_state(cx);
        self.view.image(id!(zoomable_image)).set_visible(cx, false);
        // Clear the image buffer. 
        let _ = self.view.image(id!(zoomable_image)).load_jpg_from_data(cx, &[]);
        self.animator_cut(cx, id!(mode.upright));
        self.view.rotated_image(id!(rotated_image)).apply_over(cx, live!{
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

    /// Displays the given image bytes in the zoomable image widget.
    ///
    /// This will load the image bytes into the zoomable image widget and display it.
    /// If the image fails to load, an `ImageError` is returned.
    pub fn display_image(&mut self, cx: &mut Cx, image_bytes: &[u8]) -> Result<(), ImageError> {
        self.image_loaded = true;
        load_png_or_jpg(&self.view.image(id!(zoomable_image)), cx, image_bytes)?;
        self.view.image(id!(zoomable_image)).set_visible(cx, true);
        Ok(())
    }

    pub fn display_rotated_image(&mut self, cx: &mut Cx, image_bytes: &[u8]) -> Result<(), ImageError> {
        self.image_loaded = true;
        load_png_or_jpg_rotated_image(&self.view.rotated_image(id!(rotated_image)), cx, image_bytes)
    }

    fn adjust_zoom(&mut self, cx: &mut Cx, zoom_factor: f32) {
        const MIN_ZOOM: f32 = 0.5;
        const MAX_ZOOM: f32 = 2.0;
        if (self.drag_state.zoom_level >= MAX_ZOOM && zoom_factor > 1.0) || self.drag_state.zoom_level <= MIN_ZOOM && zoom_factor < 1.0 {
            return;
        }
        let rotated_image_container = self.view.rotated_image(id!(rotated_image));
        let size = rotated_image_container.area().rect(cx).size;
        self.drag_state.zoom_level *= zoom_factor;
        self.drag_state.zoom_level = (self.drag_state.zoom_level * 1000.0).round() / 1000.0;
        let width = (size.x as f32 * zoom_factor *1000.0).round() / 1000.0;
        let height = (size.y as f32 * zoom_factor *1000.0).round() / 1000.0;
        self.view.rotated_image(id!(rotated_image)).apply_over(cx, live!{
            width: (width),
            height: (height),
        });
    }
}

impl ImageViewerRef {
    /// See [`ImageViewer::display_image()`].
    pub fn display_image(&mut self, cx: &mut Cx, image_bytes: &[u8]) -> Result<(), ImageError> {
        let Some(mut inner) = self.borrow_mut() else {
            return Ok(());
        };
        inner.display_image(cx, image_bytes)
    }

    /// See [`ImageViewer::display_rotated_image()`].
    pub fn display_rotated_image(&mut self, cx: &mut Cx, image_bytes: &[u8]) -> Result<(), ImageError> {
        let Some(mut inner) = self.borrow_mut() else {
            return Ok(());
        };
        inner.display_rotated_image(cx, image_bytes)
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
#[derive(Debug, PartialEq, Eq)]
pub enum LoadState {
    /// The image is currently being loaded with its thumbnail image.
    Loading(Arc<[u8]>),
    /// The image has been successfully loaded given the data.
    Loaded(Arc<[u8]>),
    /// An error occurred while loading the image, with specific error type.
    Error(ImageViewerError),
}
