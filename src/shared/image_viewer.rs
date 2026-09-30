//! Image viewer widget for displaying Image with zooming and panning.
//!
//! There are 2 types of ImageViewerAction handled by this widget. They are "Show" and "Hide".
use std::sync::{mpsc::Receiver, Arc};

use chrono::{DateTime, Local};
use makepad_widgets::{
    event::TouchState,
    image_cache::{decode_image_from_data, looks_like_svg, ImageBuffer, ImageError},
    *,
};
use matrix_sdk_ui::timeline::EventTimelineItem;

use crate::home::room_image_viewer::ImageViewerFetchAction;

use crate::utils::format_decimal_file_size;
use thiserror::Error;
use crate::{
    shared::{attachment_download::{DownloadableAttachment, save_loaded_attachment, share_loaded_attachment, start_attachment_download, start_attachment_share}, avatar::AvatarWidgetExt, pinch_zoom::{PinchZoom, ReleasedPress}, timestamp::TimestampWidgetRefExt},
    sliding_sync::TimelineKind,
};

/// The timeout for hiding the UI overlays after no user mouse/tap activity.
const SHOW_UI_DURATION: f64 = 3.0;

/// Duration of one 90° rotation spin, in seconds (matches the DSL const).
const ROTATION_ANIMATION_DURATION_SECS: f64 = 0.2;

/// How much each press of a zoom button or a zoom key zooms in or out by.
const ZOOM_STEP: f64 = 1.2;

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

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.UI_ANIMATION_DURATION_SECS = 0.4

    mod.widgets.ImageViewerButton = RobrixNeutralIconButton {

        width: 44, height: 44
        align: Align{x: 0.5, y: 0.5},
        spacing: 0,
        padding: 0,
        draw_bg +: {
            color: (COLOR_SECONDARY * 0.925)
            color_hover: (COLOR_SECONDARY * 0.825)
            color_down: (COLOR_SECONDARY * 0.7)
        }
        draw_icon +: {
            svg: (ICON_ZOOM_OUT),
            color: #000
        }
        icon_walk: Walk{width: 27, height: 27}
    }

    mod.widgets.ImageViewer = set_type_default() do #(ImageViewer::register_widget(vm)) {
        ..mod.widgets.SolidView

        width: Fill, height: Fill,
        flow: Overlay
        show_bg: true
        draw_bg +: {
            color: (COLOR_IMAGE_VIEWER_BACKGROUND)
        }

        image_layer := View {
            width: Fill, height: Fill,
            align: Align{x: 0.5, y: 0.5}
            flow: Down

            rotated_image_container := View {
                width: Fill, height: Fill,
                flow: Down
                rotated_image := Image {
                    width: Fill, height: Fill,
                    // The viewer computes the exact frame size itself (and the
                    // shader handles fit + rotation), so don't let the widget
                    // re-fit the texture's own aspect over our rotated frame.
                    fit: ImageFit.Stretch
                }
            }

            footer := View {
                width: Fill, height: Fit,
                flow: Right
                padding: 10
                align: Align{x: 0.5, y: 0.5}
                spacing: 10

                image_viewer_loading_spinner_view := View {
                    width: Fit, height: Fit

                    loading_spinner := LoadingSpinner {
                        width: 40, height: 40,
                        draw_bg +: {
                            color: (COLOR_TEXT)
                            border_size: 3.0
                        }
                    }
                }

                image_viewer_forbidden_view := View {
                    width: Fit, height: Fit
                    visible: false
                    Icon {
                        draw_icon +: {
                            svg: (ICON_FORBIDDEN),
                            color: (COLOR_TEXT),
                        }
                        icon_walk: Walk{ width: 30, height: 30 }
                    }
                }

                image_viewer_status_label := Label {
                    width: Fit{max: FitBound.Rel{base: Base.Line, factor: 1.0}}, height: Fit,
                    text_overflow: Ellipsis,
                    text: "Loading image...",
                    draw_text +: {
                        text_style: REGULAR_TEXT {font_size: 14},
                        color: (COLOR_TEXT)
                    }
                }
            }
        }

        metadata_view := View {
            width: Fill, height: Fill,
            // placeholder values: real values are set in Rust code, see `draw_walk`.
            margin: Inset{top: 20, left: 20, right: 20, bottom: 20}
            align: Align{x: 0.0, y: 1.0},
            metadata_rounded_view := RoundedView {
                width: Fill, height: Fit
                flow: Right
                align: Align{y: 0.5, x: 0.0}
                padding: Inset{top: 13, bottom: 8, left: 13, right: 13}
                spacing: 8,

                show_bg: true
                draw_bg +: {
                    border_radius: 4.0
                    color: (COLOR_IMAGE_VIEWER_META_BACKGROUND)
                }

                avatar_timestamp_view := View {
                    width: Fit
                    height: Fit
                    flow: Down
                    spacing: 2
                    align: Align{x: 0.5, y: 0.0}

                    avatar := Avatar {
                        width: 45, height: 45,
                        text_view +: {
                            text +: {
                                draw_text +: {
                                    text_style: TITLE_TEXT { font_size: 15.0 }
                                }
                            }
                        }
                    }
                    timestamp := Timestamp {
                        width: Fit,
                        height: Fit,
                        ts_label := Label {
                            draw_text +: {
                                text_style: theme.font_regular {font_size: 9.5},
                                color: (COLOR_TEXT)
                            }
                        }
                    }
                }

                username_label_view := View {
                    width: Fill{weight: 0.35},
                    // width: Fill,
                    height: Fit,
                    flow: Right,
                    align: Align{ y: 0.5 }

                    username := Label {
                        width: Fill,
                        height: Fit,
                        padding: 0
                        margin: 0
                        flow: Flow.Right{wrap: true}
                        max_lines: 2
                        text_overflow: Ellipsis
                        draw_text +: {
                            text_style: REGULAR_TEXT {font_size: 12},
                            color: (COLOR_TEXT)
                        }
                    }
                }

                // Display image name and size below the username when the width is not enough.
                image_name_and_size_view := View {
                    width: Fill{weight: 0.65},
                    // width: Fill
                    height: Fit,
                    align: Align{x: 0, y: 0.5}
                    flow: Right
                    image_name_and_size := Label {
                        width: Fill,
                        height: Fit,
                        align: Align{x: 0, y: 0.5}
                        flow: Flow.Right{wrap: true}
                        max_lines: 2
                        text_overflow: Ellipsis
                        draw_text +: {
                            text_style: REGULAR_TEXT {font_size: 13},
                            color: (COLOR_TEXT),
                        }
                    }
                }
            }
        }

        button_group_view := View {
            width: Fill, height: Fit
            flow: Right
            // Placeholder, see `metadata_view` above.
            margin: Inset{top: 20, right: 20}
            align: Align{x: 1.0, y: 0.5},

            button_group_rounded_view := RoundedView {
                width: Fit, height: Fit
                spacing: 10
                show_bg: true
                draw_bg +: {
                    color: (COLOR_IMAGE_VIEWER_META_BACKGROUND),
                    border_radius: 4.0
                }
                padding: Inset{ left: 7, top: 4, bottom: 4, right: 7}

                zoom_out_button := mod.widgets.ImageViewerButton {
                    draw_icon +: { svg: (ICON_ZOOM_OUT) }
                    icon_walk: Walk{width: 27, height: 27, margin: Inset{left: 2}}
                }

                zoom_in_button := mod.widgets.ImageViewerButton {
                    draw_icon +: { svg: (ICON_ZOOM_IN) }
                    icon_walk: Walk{width: 27, height: 27, margin: Inset{left: 2}}
                }

                rotate_cw_button := mod.widgets.ImageViewerButton {
                    draw_icon +: { svg: (ICON_ROTATE_CW) }
                    icon_walk: Walk{width: 30, height: 30, margin: Inset{left: 2}}
                }

                zoom_to_fit_button := mod.widgets.ImageViewerButton {
                    draw_icon +: { svg: (ICON_ZOOM_TO_FIT) }
                    icon_walk: Walk{width: 25, height: 25}
                }

                download_button := mod.widgets.ImageViewerButton {
                    draw_icon +: { svg: (ICON_DOWNLOAD) }
                    icon_walk: Walk{width: 24, height: 24}
                }

                share_button := mod.widgets.ImageViewerButton {
                    draw_icon +: { svg: (ICON_SHARE) }
                    icon_walk: Walk{width: 24, height: 24}
                }

                close_button := mod.widgets.ImageViewerButton {
                    draw_icon +: { svg: (ICON_CLOSE) }
                    icon_walk: Walk{width: 21, height: 21 }
                }
            }
        }

        animator: Animator{
            hover: {
                default: @off
                off: AnimatorState{
                    apply: { }
                }
                on: AnimatorState{
                    apply: { }
                }
            }
            ui_animator: {
                default: @hide
                show: AnimatorState{
                    redraw: true,
                    from: { all: Forward { duration: (mod.widgets.UI_ANIMATION_DURATION_SECS) } }
                    apply: {
                        ui_overlay_slide: 0.0
                    }
                }
                hide: AnimatorState{
                    redraw: true,
                    from: { all: Forward { duration: (mod.widgets.UI_ANIMATION_DURATION_SECS) } }
                    apply: {
                        ui_overlay_slide: 1.0
                    }
                }
            }
        }
    }
}

/// Actions emitted by the `ImageViewer` widget.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, Default)]
pub enum ImageViewerAction {
    /// No action.
    #[default]
    None,
    /// Display the ImageViewer widget based on the LoadState.
    Show(LoadState),
    /// Hide the ImageViewer widget.
    Hide,
}

#[derive(Script, Widget, Animator)]
struct ImageViewer {
    #[source] source: ScriptObjectRef,
    #[deref] view: View,
    /// Tracks how far the image is zoomed in and where it has been panned to.
    #[rust] pinch_zoom: PinchZoom,
    /// Drives the animations of `pinch_zoom`.
    #[rust] zoom_next_frame: NextFrame,
    /// Toggles the UI overlay once a tap can no longer become a double tap.
    #[rust] single_tap_timer: Timer,
    /// The current rotation angle of the image. Max of 4, each step represents 90 degrees
    #[rust] rotation_step: i8,
    /// A lock to prevent multiple rotation animations from running at the same time
    #[rust] is_animating_rotation: bool,
    #[apply_default] animator: Animator,
    /// Indicates if the mouse cursor is currently hovering over the image.
    #[rust] mouse_cursor_hover_over_image: bool,
    /// The ID of the background task that is currently running
    #[rust] background_task_id: u32,
    /// The mpsc::Receiver used to receive the result of the background task
    #[rust] receiver: Option<(u32, Receiver<Result<ImageBuffer, ImageError>>)>,
    /// Whether the full image file has been loaded
    #[rust] is_loaded: bool,
    /// The size of the image container.
    #[rust] image_container_size: DVec2,
    /// Set when a `WindowAction::WindowGeomChange` arrives (resize/rotation),
    /// which gets handled on the next `draw_walk`.
    #[rust] needs_refit: bool,
    /// Used to trigger a NextFrame event to re-fit the image instead of doing it mid-draw.
    #[rust] refit_next_frame: NextFrame,
    /// The texture containing the loaded image
    #[rust] texture: Option<Texture>,
    /// The event to trigger displaying with the loaded image after peek_walk_turtle of the widget.
    #[rust] next_frame: NextFrame,
    /// Whether the UI overlay (buttons + metadata) is currently visible or animating to visible.
    #[rust] ui_overlay_visible: bool,
    /// Whether the mouse is hovering over the overlay UI (buttons or metadata).
    /// When true, the auto-hide timer should not run.
    #[rust] mouse_over_overlay_ui: bool,
    /// Whether the hide animation is currently playing. When it finishes,
    /// the overlay views are set to invisible.
    #[rust] is_hiding_overlay: bool,
    /// Animated slide value for the UI overlay: 0.0 = fully visible, 1.0 = fully hidden.
    /// The animator interpolates this value; `draw_walk` uses it to position the views.
    #[live] ui_overlay_slide: f32,
    /// Timer used to animate-out (hide) the UI overlay after no user mouse/tap activity.
    #[rust] hide_ui_timer: Timer,
    /// Last known mouse position, used to distinguish actual mouse movement
    /// from the continuous `FingerHoverOver` events that fire every frame.
    #[rust] last_mouse_pos: DVec2,
    /// The image's intrinsic (unrotated) pixel size, kept so we can re-fit the
    /// rotated bounding box at every angle of the spin.
    #[rust] natural_dimension: DVec2,
    /// The currently-displayed rotation angle in degrees (continuous during the
    /// animated spin; settles on a multiple of 90°).
    #[rust] current_angle: f64,
    /// Animation endpoints for the in-progress rigid rotation.
    #[rust] rotation_anim_from: f64,
    #[rust] rotation_target_angle: f64,
    /// Wall-clock start of the rotation animation (set on its first frame).
    #[rust] rotation_anim_start_time: Option<f64>,
    /// Drives the per-frame rotation animation.
    #[rust] rotation_next_frame: NextFrame,
    /// Info about how to download the image being shown.
    #[rust] downloadable: Option<DownloadableAttachment>,
    /// A reference to the image being shown so we can easily save it to storage.
    #[rust] loaded_bytes: Option<Arc<[u8]>>,
}

impl ScriptHook for ImageViewer {
    fn on_after_apply(&mut self, vm: &mut ScriptVm, apply: &Apply, _scope: &mut Scope, _value: ScriptValue) {
        // A reapply (like rotating the device) resets the image's walk and shader values.
        if apply.is_reload() {
            self.apply_image_transform(vm.cx_mut());
        }
    }
}

impl Widget for ImageViewer {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        let content_rect = self.pinch_zoom.get_content_rect();
        let had_pointers_down = self.pinch_zoom.has_pointers_down();
        // Block all scrolling, as the image viewer modal is full-screen.
        cx.block_scrolling_except_within(Area::Empty);
        self.view.handle_event(cx, event, scope);
        self.match_event(cx, event);

        // Handle hover events for UI overlay elements.
        // Only hit-test these when the overlay is visible; when hidden, their areas
        // persist from the last draw and would consume events before rotated_image.
        // All hit events (hover + finger) must use self.view.area() because the inner
        // View's handle_event captures events on its own area first (due to its animator),
        // preventing rotated_image.area() from receiving them.
        // Position checks distinguish image vs. background interactions.
        match event.hits(cx, self.view.area()) {
            Hit::FingerHoverIn(he) if content_rect.contains(he.abs) => {
                self.mouse_cursor_hover_over_image = true;
                cx.set_cursor(MouseCursor::Hand);
            }
            Hit::FingerHoverOut(_) => {
                self.mouse_cursor_hover_over_image = false;
                cx.set_cursor(MouseCursor::Default);
            }
            Hit::FingerHoverOver(he) => {
                // Update cursor based on position over image.
                let on_image = content_rect.contains(he.abs);
                if on_image != self.mouse_cursor_hover_over_image {
                    self.mouse_cursor_hover_over_image = on_image;
                    cx.set_cursor(if on_image { MouseCursor::Hand } else { MouseCursor::Default });
                }
                // Track whether cursor is over the overlay UI elements.
                let on_overlay = self.is_over_overlay_ui(cx, he.abs);
                if on_overlay != self.mouse_over_overlay_ui {
                    self.mouse_over_overlay_ui = on_overlay;
                    if on_overlay {
                        cx.stop_timer(self.hide_ui_timer);
                    } else {
                        self.hide_ui_timer = cx.start_timeout(SHOW_UI_DURATION);
                    }
                }
                // FingerHoverOver fires every frame the cursor is over the area,
                // even without actual movement. Only react to real mouse movement.
                let dist = (he.abs - self.last_mouse_pos).length();
                let mouse_moved = dist > 0.5;
                self.last_mouse_pos = he.abs;
                if mouse_moved {
                    self.show_overlay_ui(cx, true);
                }
            }
            // Touches are handled below, since this only tells us about one of them at a time.
            Hit::FingerDown(fe) if fe.is_mouse() && fe.is_primary_hit() => {
                self.stop_single_tap_timer(cx);
                let can_accept = !self.is_over_overlay_ui(cx, fe.abs);
                self.pinch_zoom.handle_mouse(TouchState::Start, fe.abs, fe.time, can_accept);
            }
            Hit::FingerMove(fe) if fe.is_mouse() && fe.is_primary_hit() => {
                self.pinch_zoom.handle_mouse(TouchState::Move, fe.abs, fe.time, true);
            }
            Hit::FingerUp(fe) if fe.is_mouse() && fe.is_primary_hit() => {
                let released_press = self.pinch_zoom.handle_mouse(TouchState::Stop, fe.abs, fe.time, true);
                self.handle_released_press(cx, released_press);
            }
            _ => {}
        }
        if let Event::TouchUpdate(e) = event {
            if e.touches.iter().any(|touch| touch.state == TouchState::Start) {
                self.stop_single_tap_timer(cx);
            }
            let view_area = self.view.area();
            let view_rect = view_area.rect(cx);
            let overlay_rects = self.get_overlay_ui_rects(cx);
            let released_press = self.pinch_zoom.handle_touch_update(e.time, &e.touches, |touch| {
                // A touch is ours if it's on us and not on a button or a panel of the UI overlay.
                let claimed_by = touch.handled.get();
                let is_ours = (claimed_by.is_empty() || claimed_by == view_area)
                    && view_rect.contains(touch.abs)
                    && !overlay_rects.iter().flatten().any(|rect| rect.contains(touch.abs));
                if is_ours && touch.state == TouchState::Start {
                    touch.handled.set(view_area);
                }
                is_ours
            });
            self.handle_released_press(cx, released_press);
        }
        // The overlay doesn't auto-hide while pointers are down on the image.
        let has_pointers_down = self.pinch_zoom.has_pointers_down();
        if has_pointers_down != had_pointers_down {
            cx.stop_timer(self.hide_ui_timer);
            self.hide_ui_timer = Timer::empty();
            if !has_pointers_down && self.ui_overlay_visible && !self.mouse_over_overlay_ui {
                self.hide_ui_timer = cx.start_timeout(SHOW_UI_DURATION);
            }
        }
        if let Event::Scroll(scroll_event) = event {
            if content_rect.contains(scroll_event.abs) {
                let scroll_delta = scroll_event.scroll.y;
                // Scale the zoom factor proportionally to the scroll magnitude,
                // clamped so each scroll tick produces a gentle zoom step.
                let normalized = (scroll_delta.abs() / 200.0).clamp(0.005, 0.06);
                if scroll_delta > 0.0 {
                    self.pinch_zoom.zoom_by(1.0 + normalized, scroll_event.abs);
                } else if scroll_delta < 0.0 {
                    self.pinch_zoom.zoom_by(1.0 / (1.0 + normalized), scroll_event.abs);
                }
            }
        }
        if let Event::Pinch(pinch) = event {
            self.pinch_zoom.handle_trackpad_pinch(pinch.phase, pinch.abs, pinch.scale);
        }
        if let Event::KeyDown(e) = event {
            match &e.key_code {
                KeyCode::Minus | KeyCode::NumpadSubtract => {
                    // Zoom out (make image smaller)
                    self.pinch_zoom.animate_zoom_by(1.0 / ZOOM_STEP);
                }
                KeyCode::Equals | KeyCode::NumpadAdd => {
                    // Zoom in (make image larger)
                    self.pinch_zoom.animate_zoom_by(ZOOM_STEP);
                }
                KeyCode::Key0 | KeyCode::Numpad0 => {
                    self.pinch_zoom.animate_zoom_to_fit();
                }
                _ => {}
            }
        }
        if let Some(ne) = self.zoom_next_frame.is_event(event) {
            self.zoom_next_frame = NextFrame::default();
            self.pinch_zoom.advance_animation(ne.time);
        }
        if self.pinch_zoom.get_content_rect() != content_rect {
            self.apply_image_transform(cx);
        }
        if self.pinch_zoom.is_animating() && self.zoom_next_frame == NextFrame::default() {
            self.zoom_next_frame = cx.new_next_frame();
        }

        if let (Event::Signal, Some((_background_task_id, receiver))) = (event, &mut self.receiver) {
            let mut remove_receiver = false;
            match receiver.try_recv() {
                Ok(Ok(image_buffer)) => {
                    let texture = image_buffer.into_new_texture(cx);
                    self.texture = Some(texture);
                    self.next_frame = cx.new_next_frame();
                    remove_receiver = true;
                    cx.action(ImageViewerAction::Show(
                        LoadState::FinishedBackgroundDecoding,
                    ));
                }
                Ok(Err(error)) => {
                    let error = match error {
                        ImageError::JpgDecode(_)
                        | ImageError::PngDecode(_)
                        | ImageError::GifDecode(_)
                        | ImageError::WebpDecode(_)
                        | ImageError::BmpDecode(_)
                        | ImageError::QoiDecode(_) => ImageViewerError::UnsupportedFormat,
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
        if animator_action.must_redraw() {
            self.view.redraw(cx);
        }

        // When the hide animation finishes, make the overlay views invisible
        // so their stale areas don't consume events.
        if self.is_hiding_overlay && !self.animator.is_track_animating(id!(ui_animator)) {
            self.is_hiding_overlay = false;
            self.view.view(cx, ids!(button_group_view)).set_visible(cx, false);
            self.view.view(cx, ids!(metadata_view)).set_visible(cx, false);
            self.view.redraw(cx);
        }

        if self.next_frame.is_event(event).is_some() {
            self.display_current_image(cx);
        }
        if self.refit_next_frame.is_event(event).is_some() {
            self.apply_image_transform(cx);
        }
        if let Some(ne) = self.rotation_next_frame.is_event(event) {
            self.advance_rotation(cx, ne.time);
        }

        if self.hide_ui_timer.is_event(event).is_some() {
            self.hide_overlay_ui(cx);
        }
        if self.single_tap_timer.is_event(event).is_some() {
            self.single_tap_timer = Timer::empty();
            self.pinch_zoom.clear_last_tap();
            if self.ui_overlay_visible {
                self.hide_overlay_ui(cx);
            } else {
                self.show_overlay_ui(cx, true);
            }
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        // Handle a changed (or unknown/first) image container size 
        let is_first = self.image_container_size.length() == 0.0;
        if is_first || self.needs_refit {
            self.needs_refit = false;
            let container = cx.peek_walk_turtle(walk);
            if container.size.x > 0.0 && container.size.y > 0.0 {
                self.image_container_size = container.size;
                self.pinch_zoom.set_viewport(container);
                if is_first {
                    self.next_frame = cx.new_next_frame();
                } else {
                    self.refit_next_frame = cx.new_next_frame();
                }
            }
        }

        // Position the overlays based on the animated `ui_overlay_slide` value,
        // in which 0.0 means fully visible and 1.0 means fully off-screen.
        let slide = self.ui_overlay_slide as f64;
        let insets = cx.display_context.safe_area_insets;
        let button_top_visible = 20.0_f64.max(insets.top);
        let button_right        = 20.0_f64.max(insets.right);
        let meta_top            = 20.0_f64.max(insets.top);
        let meta_left           = 20.0_f64.max(insets.left);
        let meta_right          = 20.0_f64.max(insets.right);
        let meta_bottom_visible = 20.0_f64.max(insets.bottom);
        let button_top = button_top_visible - (slide * 220.0); // visible → -200
        let meta_bottom = meta_bottom_visible - (slide * 320.0); // visible → -300
        if let Some(mut button_group_view) = self.view(cx, ids!(button_group_view)).borrow_mut() {
            button_group_view.walk.margin.top = button_top;
            button_group_view.walk.margin.right = button_right;
        }
        if let Some(mut metadata_view) = self.view(cx, ids!(metadata_view)).borrow_mut() {
            metadata_view.walk.margin = Inset {
                top: meta_top,
                left: meta_left,
                right: meta_right,
                bottom: meta_bottom,
            };
        }

        self.view.draw_walk(cx, scope, walk)
    }
}

impl MatchEvent for ImageViewer {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        // The parent Modal itself owns the Escape/back press handling logic,
        // so we don't have to do any of that here.
        // We just have to react to that happening by handling the modal being dismissed.
        if actions.iter().any(|a| matches!(a.downcast_ref(), Some(ModalAction::Dismissed))) {
            self.reset(cx);
        }
        for action in actions {
            // Handle any changes to the window size / rotation orientation.
            if let WindowAction::WindowGeomChange(_) = action.as_widget_action().cast() {
                self.needs_refit = true;
                self.view.redraw(cx);
                break;
            }
        }

        if self.view.button(cx, ids!(close_button)).clicked(actions) {
            self.reset(cx);
            cx.action(ImageViewerAction::Hide);
        }

        let mut was_overlay_button_clicked = false;
        if self.view.button(cx, ids!(zoom_to_fit_button)).clicked(actions) {
            was_overlay_button_clicked = true;
            self.pinch_zoom.animate_zoom_to_fit();
        }
        if self.view.button(cx, ids!(zoom_out_button)).clicked(actions) {
            was_overlay_button_clicked = true;
            self.pinch_zoom.animate_zoom_by(1.0 / ZOOM_STEP);
        }

        if self.view.button(cx, ids!(zoom_in_button)).clicked(actions) {
            was_overlay_button_clicked = true;
            self.pinch_zoom.animate_zoom_by(ZOOM_STEP);
        }

        if self.view.button(cx, ids!(rotate_cw_button)).clicked(actions) {
            was_overlay_button_clicked = true;
            self.start_rotation(cx, 90.0);
        }

        if let Some(info) = self.downloadable.as_ref() {
            if self.view.button(cx, ids!(download_button)).clicked(actions) {
                was_overlay_button_clicked = true;
                if let Some(bytes) = self.loaded_bytes.clone() {
                    save_loaded_attachment(info.filename.clone(), bytes);
                } else {
                    start_attachment_download(info.clone(), None);
                }
            }
            if self.view.button(cx, ids!(share_button)).clicked(actions) {
                was_overlay_button_clicked = true;
                if let Some(bytes) = self.loaded_bytes.clone() {
                    share_loaded_attachment(info, bytes);
                } else {
                    start_attachment_share(info.clone(), None);
                }
            }
        }

        // Restart the auto-hide timer if any overlay button was clicked.
        if was_overlay_button_clicked && !self.mouse_over_overlay_ui {
            cx.stop_timer(self.hide_ui_timer);
            self.hide_ui_timer = cx.start_timeout(SHOW_UI_DURATION);
        }

        for action in actions.iter() {
            match action.downcast_ref() {
                Some(ImageViewerFetchAction::Loaded(bytes)) => {
                    cx.action(ImageViewerAction::Show(LoadState::Loaded(bytes.clone())));
                }
                Some(ImageViewerFetchAction::Failed(error)) => {
                    cx.action(ImageViewerAction::Show(LoadState::Error(error.clone())));
                }
                None => {}
            }
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
                        self.loaded_bytes = Some(image_bytes.clone());
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
    /// Returns the rects of the UI overlay's button bar and metadata panel while it's showing.
    fn get_overlay_ui_rects(&self, cx: &mut Cx) -> Option<[Rect; 2]> {
        (self.ui_overlay_visible || self.is_hiding_overlay).then(|| [
            self.view.view(cx, ids!(button_group_rounded_view)).area().rect(cx),
            self.view.view(cx, ids!(metadata_rounded_view)).area().rect(cx),
        ])
    }

    fn is_over_overlay_ui(&self, cx: &mut Cx, abs: DVec2) -> bool {
        self.get_overlay_ui_rects(cx).iter().flatten().any(|rect| rect.contains(abs))
    }

    /// Handles a tap on the image or on the background around it.
    ///
    /// Only a tap toggles the UI overlay. A drag, a pinch, or a double tap never does.
    fn handle_released_press(&mut self, cx: &mut Cx, released_press: ReleasedPress) {
        let ReleasedPress::Tap { abs, double_tap_wait } = released_press else { return };
        if self.is_over_overlay_ui(cx, abs) {
            return;
        }
        if self.pinch_zoom.get_content_rect().contains(abs) {
            // This might be the first tap of a double tap, so wait to see if it happens.
            self.single_tap_timer = cx.start_timeout(double_tap_wait);
        } else {
            self.reset(cx);
            cx.action(ImageViewerAction::Hide);
        }
    }

    fn stop_single_tap_timer(&mut self, cx: &mut Cx) {
        cx.stop_timer(self.single_tap_timer);
        self.single_tap_timer = Timer::empty();
    }

    /// Shows the UI overlay (buttons + metadata) and optionally starts the auto-hide timer.
    fn show_overlay_ui(&mut self, cx: &mut Cx, start_auto_hide_timer: bool) {
        if !self.ui_overlay_visible {
            self.ui_overlay_visible = true;
            self.is_hiding_overlay = false;
            self.view.view(cx, ids!(button_group_view)).set_visible(cx, true);
            self.view.view(cx, ids!(metadata_view)).set_visible(cx, true);
            self.animator_play(cx, ids!(ui_animator.show));
            self.view.redraw(cx);
        }
        cx.stop_timer(self.hide_ui_timer);
        if start_auto_hide_timer && !self.mouse_over_overlay_ui {
            self.hide_ui_timer = cx.start_timeout(SHOW_UI_DURATION);
        }
    }

    /// Hides the UI overlay (buttons + metadata) with an animated slide-out.
    /// The views are kept visible during animation; `handle_event` sets them
    /// invisible once the animation finishes.
    fn hide_overlay_ui(&mut self, cx: &mut Cx) {
        self.ui_overlay_visible = false;
        self.is_hiding_overlay = true;
        cx.stop_timer(self.hide_ui_timer);
        self.animator_play(cx, ids!(ui_animator.hide));
        self.view.redraw(cx);
    }

    /// Reset state.
    pub fn reset(&mut self, cx: &mut Cx) {
        self.rotation_step = 0; // Reset to upright (0°)
        self.current_angle = 0.0;
        self.rotation_target_angle = 0.0;
        self.rotation_anim_start_time = None;
        self.is_animating_rotation = false; // Reset animation state
        self.pinch_zoom.reset();
        self.zoom_next_frame = NextFrame::default();
        self.stop_single_tap_timer(cx);
        self.mouse_cursor_hover_over_image = false; // Reset hover state
        self.last_mouse_pos = DVec2::default();
        self.receiver = None;
        self.is_loaded = false;
        self.loaded_bytes = None;
        self.image_container_size = DVec2::new();
        self.ui_overlay_visible = true;
        self.mouse_over_overlay_ui = false;
        self.is_hiding_overlay = false;
        cx.stop_timer(self.hide_ui_timer);
        self.hide_ui_timer = Timer::empty();
        self.view.view(cx, ids!(button_group_view)).set_visible(cx, true);
        self.view.view(cx, ids!(metadata_view)).set_visible(cx, true);
        // Snap to fully visible (no animation on reset).
        self.animator_cut(cx, ids!(ui_animator.show));
        let rotated_image_ref = self
            .view
            .image(cx, ids!(rotated_image_container.rotated_image));
        rotated_image_ref.set_texture(cx, None);
    }

    /// Displays an image in the image viewer widget.
    ///
    /// The image is displayed in the center of the widget. If the image is larger than the widget, it is scaled down to fit the widget while retaining its aspect ratio.
    pub fn show_loaded(&mut self, cx: &mut Cx, image_bytes: &Arc<[u8]>) {
        if self.receiver.is_some() {
            return;
        }
        // SVG is a vector format: load it straight into the image widget (which
        // renders it natively and stays crisp at any zoom) instead of decoding to
        // a raster texture on a worker thread.
        if looks_like_svg(image_bytes) {
            let rotated_image = self.image(cx, ids!(rotated_image));
            let load_state = match rotated_image.load_image_from_data(cx, image_bytes) {
                Ok(()) => {
                    self.texture = None;
                    self.next_frame = cx.new_next_frame();
                    LoadState::FinishedBackgroundDecoding
                }
                Err(_) => LoadState::Error(ImageViewerError::BadData),
            };
            cx.action(ImageViewerAction::Show(load_state));
            return;
        }
        if let Some(new_value) = self.background_task_id.checked_add(1) {
            self.background_task_id = new_value;
        }
        let (sender, receiver) = std::sync::mpsc::channel();
        self.receiver = Some((self.background_task_id, receiver));
        let image_bytes2 = Arc::clone(image_bytes);
        let spawned = cx.spawn_worker(move || {
            let _ = sender.send(decode_image_from_data(&image_bytes2));
            SignalToUI::set_ui_signal();
        });
        if let Err(e) = spawned {
            error!("Failed to spawn the image decoding thread: {e:?}");
        }
    }

    /// Displays an image in the image viewer widget using the provided texture.
    /// 
    /// `Texture` is an optional `Texture` that can be set to display an image. If `None`, the image is cleared.
    pub fn display_current_image(&mut self, cx: &mut Cx) {
        if self.image_container_size.length() == 0.0 {
            return;
        }
        let rotated_image = self.image(cx, ids!(rotated_image));
        // Natural (unrotated) size: the texture's dimensions for raster images,
        // or the intrinsic SVG size when an SVG has been loaded into the widget.
        let natural = if self.texture.is_some() {
            let texture = self.texture.clone();
            rotated_image.set_texture(cx, texture);
            self.texture
                .as_ref()
                .and_then(|t| t.get_format(cx).vec_width_height())
                .map(|(w, h)| DVec2 { x: w as f64, y: h as f64 })
                .unwrap_or_default()
        } else {
            rotated_image
                .size_in_pixels(cx)
                .map(|(w, h)| DVec2 { x: w as f64, y: h as f64 })
                .unwrap_or_default()
        };
        if natural.x == 0.0 || natural.y == 0.0 {
            return;
        }
        self.natural_dimension = natural;
        if !self.is_animating_rotation {
            self.current_angle = f64::from(self.rotation_step) * 90.0;
        }
        self.apply_image_transform(cx);
    }

    /// Puts the image where it currently belongs, at its current size and rotation.
    fn apply_image_transform(&mut self, cx: &mut Cx) {
        let (w, h) = (self.natural_dimension.x, self.natural_dimension.y);
        if w <= 0.0 || h <= 0.0 || self.image_container_size.length() == 0.0 {
            return;
        }
        // What gets zoomed and panned is the box around the rotated image.
        let rad = self.current_angle.to_radians();
        let (ca, sa) = (rad.cos().abs(), rad.sin().abs());
        self.pinch_zoom.set_content_size(dvec2(w * ca + h * sa, w * sa + h * ca));
        let rect = self.pinch_zoom.get_content_rect();
        let scale = self.pinch_zoom.get_content_scale();
        if let Some(mut image) = self.view.image(cx, ids!(rotated_image)).borrow_mut() {
            image.walk.abs_pos = Some(rect.pos);
            image.walk.width = Size::Fixed(rect.size.x);
            image.walk.height = Size::Fixed(rect.size.y);
            image.draw_bg.rotation = self.current_angle as f32;
            image.draw_bg.image_dim_w = (w * scale) as f32;
            image.draw_bg.image_dim_h = (h * scale) as f32;
        }
        self.view.area().redraw(cx);
    }

    /// Starts a rotation animation, with a target of `deg` additional degrees beyond the current rotation.
    fn start_rotation(&mut self, cx: &mut Cx, deg: f64) {
        if self.is_animating_rotation || self.natural_dimension.x <= 0.0 {
            return;
        }
        self.is_animating_rotation = true;
        self.rotation_anim_from = self.current_angle;
        self.rotation_target_angle = self.current_angle + deg;
        self.rotation_anim_start_time = None;
        self.rotation_next_frame = cx.new_next_frame();
    }

    /// Advances the rotation animation by one frame.
    ///
    /// Returns true while the animation is still running.
    fn advance_rotation(&mut self, cx: &mut Cx, now: f64) {
        let start = *self.rotation_anim_start_time.get_or_insert(now);
        let t = ((now - start) / ROTATION_ANIMATION_DURATION_SECS).clamp(0.0, 1.0);
        let eased = t * t * (3.0 - 2.0 * t); // smoothstep
        self.current_angle =
            self.rotation_anim_from + (self.rotation_target_angle - self.rotation_anim_from) * eased;
        if t >= 1.0 {
            self.current_angle = self.rotation_target_angle;
            self.is_animating_rotation = false;
            self.rotation_step = self.rotation_target_angle.div_euclid(90.0).rem_euclid(4.0) as i8;
        } else {
            self.rotation_next_frame = cx.new_next_frame();
        }
        self.apply_image_transform(cx);
    }

    /// Shows a loading message in the footer.
    ///
    /// The loading spinner is shown, the error icon is hidden, and the
    /// status label is set to "Loading...".
    pub fn show_loading(&mut self, cx: &mut Cx) {
        let footer = self.view.view(cx, ids!(image_layer.footer));
        footer.view(cx, ids!(image_viewer_loading_spinner_view))
            .set_visible(cx, true);
        footer.label(cx, ids!(image_viewer_status_label))
            .set_text(cx, "Loading...");
        footer.view(cx, ids!(image_viewer_forbidden_view))
            .set_visible(cx, false);
        footer.set_visible(cx, true);
        // Snap the overlay to visible immediately on initial open (no animation).
        self.ui_overlay_visible = true;
        self.is_hiding_overlay = false;
        self.view.view(cx, ids!(button_group_view)).set_visible(cx, true);
        self.view.view(cx, ids!(metadata_view)).set_visible(cx, true);
        self.animator_cut(cx, ids!(ui_animator.show));
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
        let footer = self.view.view(cx, ids!(image_layer.footer));
        footer.view(cx, ids!(image_viewer_loading_spinner_view))
            .set_visible(cx, false);
        footer.view(cx, ids!(image_viewer_forbidden_view))
            .set_visible(cx, true);
        footer.label(cx, ids!(image_viewer_status_label))
            .set_text(cx, &error.to_string());
        footer.set_visible(cx, true);
    }

    /// Hides the footer of the image viewer.
    pub fn hide_footer(&mut self, cx: &mut Cx) {
        let footer = self.view.view(cx, ids!(image_layer.footer));
        footer.set_visible(cx, false);
    }

    /// Sets the metadata view in the image viewer with the provided metadata.
    ///
    /// The image_name_and_size and username labels handle their own overflow
    /// via `max_lines: 2` + `text_overflow: Ellipsis` in the layout.
    pub fn set_metadata(&mut self, cx: &mut Cx, metadata: &ImageViewerMetaData) {
        let meta_view = self.view.view(cx, ids!(metadata_view));
        let human_readable_size = format_decimal_file_size(metadata.image_file_size);
        let display_text = format!("{} ({})", metadata.image_name, human_readable_size);
        meta_view
            .label(cx, ids!(image_name_and_size))
            .set_text(cx, &display_text);
        if let Some(timestamp) = metadata.timestamp {
            meta_view
                .timestamp(cx, ids!(avatar_timestamp_view.timestamp))
                .set_date_time(cx, timestamp);
        }

        self.loaded_bytes = None;
        self.downloadable = metadata.downloadable.clone();
        self.view.button(cx, ids!(download_button))
            .set_visible(cx, self.downloadable.is_some());

        if let Some((timeline_kind, event_timeline_item)) = &metadata.avatar_parameter {
            let (sender, _) = self.view.avatar(cx, ids!(avatar_timestamp_view.avatar)).set_avatar_and_get_username(
                cx,
                timeline_kind,
                event_timeline_item.sender(),
                Some(event_timeline_item.sender_profile()),
                event_timeline_item.event_id(),
                false,
            );
            meta_view
                .label(cx, ids!(username_label_view.username))
                .set_text(cx, &sender);
        }
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
    // Optional avatar parameter containing info about the timeline
    // and the event to be used for the avatar.
    pub avatar_parameter: Option<(TimelineKind, EventTimelineItem)>,
    pub timestamp: Option<DateTime<Local>>,
    pub image_name: String,
    // Image size in bytes
    pub image_file_size: u64,
    /// When `Some`, the overlay's download button is shown.
    pub downloadable: Option<DownloadableAttachment>,
}
