//! Handles zoom and pan gestures for pinch to zoom, e.g., for an image viewer.
//!
//! This turns the fingers (or the mouse) that are pressed down on some zoomable content
//! into a zoom level and an offset for that content. This is modeled after how
//! Android and iOS work, and we favor android in places where they disagree.
//! 
//! * Pinching zooms about the point between the fingers and follows them around.
//! * Dragging a finger pans the content, and letting go of it while moving flings it.
//! * The content's edges never come away from the viewport's edges,
//!   and content that is smaller than the viewport stays centered.
//! * A double tap zooms in on the tapped spot, or back out to fit.
//! * A double tap that is held and then dragged zooms with only that finger.
//! * Unlike on either platform, the content can be zoomed out to a tenth of its
//!   fitted size, and zoomed in without limit. Zooming out beyond that resists
//!   like a rubber band, and springs back once released.

use std::cell::Cell;

use makepad_widgets::{
    event::{TouchPoint, TouchState},
    scroll_motion::{estimate_release_velocity, push_sample, stretch_displayed, stretch_raw, FrameClock, ScrollSample, FLING_MIN_TOTAL_DELTA, RUBBER_BAND_TOUCH_RANGE},
    *,
};

/// How far a pointer can move and still be tapping, in points.
const TOUCH_SLOP: f64 = 8.0;
/// A press that is held for this long is no longer a tap, in seconds.
const LONG_PRESS_TIMEOUT: f64 = 0.4;
/// The second tap of a double tap has to go down within
/// this many seconds of the first one going down.
const DOUBLE_TAP_TIMEOUT: f64 = 0.3;
/// The second tap also has to go down at least
/// this many seconds after the first one came back up.
const DOUBLE_TAP_MIN_GAP: f64 = 0.04;
/// How close together both taps of a double tap have to be, in points.
const DOUBLE_TAP_SLOP: f64 = 100.0;
/// The same as [`DOUBLE_TAP_SLOP`], but for both clicks of a mouse's double click.
const DOUBLE_CLICK_SLOP: f64 = 5.0;

/// The zoom level at which the content is fitted to the viewport.
const FIT_ZOOM: f64 = 1.0;
/// The content can be zoomed out until it's this much of its fitted size.
const MIN_ZOOM: f64 = 0.1;
/// A double tap zooms in until the content fills the viewport,
/// or to this many times its fitted size if that is more.
const DOUBLE_TAP_ZOOM: f64 = 2.0;
/// Zooming out beyond the minimum resists like a rubber band does,
/// and never gets this many percent beyond it.
const MAX_UNDERZOOM_PERCENT: f64 = 40.0;

/// A drag only flings the content if it let go while moving at least this fast
/// along either axis, in points per second.
const MIN_FLING_SPEED: f64 = 50.0;
const MAX_FLING_SPEED: f64 = 8000.0;
/// How quickly Android's flings slow down: they lose 22% of their speed
/// over each tenth of the time that they have left.
const FLING_DECELERATION_RATE: f64 = 2.3582018154259448;
/// The friction of Android's flings (0.015), as the deceleration that it
/// causes in points per second squared (there are 160 points to an inch).
const FLING_FRICTION: f64 = 0.015 * 9.80665 * 39.37 * 160.0 * 0.84;
/// Where Android's flings stop speeding along and start easing out.
const FLING_INFLEXION: f64 = 0.35;
/// A pointer that hasn't moved for this many seconds
/// before it lets go has stopped, so it flings nothing.
const POINTER_STOPPED_TIME: f64 = 0.04;

/// How fast a double tap's zoom animation is, as the square root of the
/// spring stiffness that Element X on Android uses for it (400).
const ZOOM_SPRING_SPEED: f64 = 20.0;
/// How fast the zoom level springs back up to the minimum, as the square root
/// of the spring stiffness that Element X on Android uses for it (1500).
const SPRING_BACK_SPEED: f64 = 38.7;
/// A spring animation is over once this little of it is left.
const SPRING_END_FRACTION: f64 = 0.001;

/// The ID of the mouse's pointer, which no touch will ever have.
const MOUSE_UID: u64 = u64::MAX;

/// What a press turned out to be, once it was released.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum ReleasedPress {
    /// A tap, which is only known to be a single tap once
    /// `double_tap_wait` seconds go by without another press.
    Tap {
        abs: DVec2,
        double_tap_wait: f64,
    },
    /// The second tap of a double tap, which we've started zooming for.
    DoubleTap,
    /// Not a tap, or nothing of ours was released.
    #[default]
    None,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Pointer {
    uid: u64,
    abs: DVec2,
}

#[derive(Clone, Copy, Debug)]
struct Tap {
    down_abs: DVec2,
    down_time: f64,
    up_time: f64,
}

#[derive(Clone, Copy, Debug, Default)]
enum Gesture {
    /// One pointer went down and has stayed put so far, so it could still be a tap.
    PossibleTap {
        down_abs: DVec2,
        down_time: f64,
        is_second_tap: bool,
    },
    /// One pointer is dragging the content around.
    Drag,
    /// Several pointers went down, but haven't moved enough yet to pinch.
    PinchWithinSlop {
        start_focus: DVec2,
        start_span: f64,
    },
    /// Several pointers are zooming and moving the content.
    Pinch,
    /// The second tap of a double tap is being dragged up or down to zoom.
    OneFingerZoom {
        anchor_abs: DVec2,
    },
    #[default]
    None,
}

#[derive(Clone, Copy, Debug, Default)]
enum Animation {
    /// The content keeps moving the way that a drag let go of it, slowing down until
    /// it has gone the `distance` along each axis, which takes that axis' `duration`.
    Fling {
        clock: FrameClock,
        from_offset: DVec2,
        distance: DVec2,
        duration: DVec2,
    },
    /// The content is zooming and moving over to a destination.
    Spring {
        clock: FrameClock,
        speed: f64,
        from_zoom: f64,
        to_zoom: f64,
        from_offset: DVec2,
        to_offset: DVec2,
    },
    #[default]
    None,
}

pub struct PinchZoom {
    /// Where the content gets shown.
    viewport: Rect,
    /// The content's own size, before it gets fitted or zoomed.
    content_size: DVec2,
    /// How much bigger the content is than when fitted to the viewport.
    zoom: f64,
    /// How far the content's center is from the viewport's center.
    offset: DVec2,
    pointers: Vec<Pointer>,
    /// The pointers as of the previous update, so we can track how they moved since.
    prev_pointers: Vec<Pointer>,
    gesture: Gesture,
    animation: Animation,
    last_tap: Option<Tap>,
    velocity_samples_x: Vec<ScrollSample>,
    velocity_samples_y: Vec<ScrollSample>,
    /// The point that we last zoomed in/out around, so we stay anchored to it.
    zoom_focus_abs: Option<DVec2>,
}

impl Default for PinchZoom {
    fn default() -> Self {
        Self {
            viewport: Rect::default(),
            content_size: DVec2::default(),
            zoom: FIT_ZOOM,
            offset: DVec2::default(),
            pointers: Vec::new(),
            prev_pointers: Vec::new(),
            gesture: Gesture::None,
            animation: Animation::None,
            last_tap: None,
            velocity_samples_x: Vec::new(),
            velocity_samples_y: Vec::new(),
            zoom_focus_abs: None,
        }
    }
}

impl PinchZoom {
    /// Sets where the content gets shown, in absolute coordinates.
    ///
    /// The same part of the content stays in the middle of the viewport.
    pub fn set_viewport(&mut self, viewport: Rect) {
        let old_fit_scale = self.get_fit_scale();
        self.viewport = viewport;
        let fit_scale = self.get_fit_scale();
        if old_fit_scale > 0.0 && fit_scale > 0.0 {
            self.offset *= fit_scale / old_fit_scale;
        }
        // An animation in progress was headed for somewhere in the old viewport.
        self.animation = Animation::None;
        self.offset = self.clamp_offset(self.offset, self.zoom);
        self.animate_zoom_to(self.zoom.max(MIN_ZOOM), self.viewport.center(), SPRING_BACK_SPEED);
    }

    /// Sets the content's own size, before it gets fitted or zoomed.
    pub fn set_content_size(&mut self, content_size: DVec2) {
        self.content_size = content_size;
        // An animation is already headed for somewhere within the edges, and keeps to them.
        if !self.is_animating() {
            self.offset = self.clamp_offset(self.offset, self.zoom);
        }
    }

    /// Goes back to fitting the content to the viewport, with nothing in progress.
    pub fn reset(&mut self) {
        *self = Self {
            viewport: self.viewport,
            content_size: self.content_size,
            ..Self::default()
        };
    }

    /// Returns where the content currently is, in absolute coordinates.
    pub fn get_content_rect(&self) -> Rect {
        let size = self.content_size * self.get_content_scale();
        Rect {
            pos: self.viewport.center() + self.offset - size * 0.5,
            size,
        }
    }

    /// Returns how many points each of the content's pixels currently covers.
    pub fn get_content_scale(&self) -> f64 {
        self.get_fit_scale() * self.zoom
    }

    pub fn is_animating(&self) -> bool {
        !matches!(self.animation, Animation::None)
    }

    pub fn has_pointers_down(&self) -> bool {
        !self.pointers.is_empty()
    }

    /// Handles the touches of a `TouchUpdateEvent` that occurred at the given `time`.
    ///
    /// `can_accept` decides whether a touch that has just started is one of ours.
    pub fn handle_touch_update(
        &mut self,
        time: f64,
        touches: &[TouchPoint],
        mut can_accept: impl FnMut(&TouchPoint) -> bool,
    ) -> ReleasedPress {
        std::mem::swap(&mut self.pointers, &mut self.prev_pointers);
        self.pointers.clear();
        let mut has_new_press = false;
        let mut released_abs = None;
        for touch in touches {
            let is_ours = self.prev_pointers.iter().any(|pointer| pointer.uid == touch.uid);
            match touch.state {
                TouchState::Start => {
                    if !can_accept(touch) {
                        continue;
                    }
                    has_new_press = true;
                }
                TouchState::Move | TouchState::Stable => {
                    if !is_ours {
                        continue;
                    }
                }
                TouchState::Stop => {
                    if is_ours {
                        released_abs = Some(touch.abs);
                    }
                    continue;
                }
                // A touch that the system took away can't tap or fling, as it wasn't let go of.
                TouchState::Cancel => continue,
            }
            self.pointers.push(Pointer { uid: touch.uid, abs: touch.abs });
        }
        self.handle_pointers_changed(time, has_new_press, released_abs)
    }

    /// Handles the mouse's primary button being pressed (`Start`),
    /// moved while pressed (`Move`), and released (`Stop`).
    pub fn handle_mouse(
        &mut self,
        state: TouchState,
        abs: DVec2,
        time: f64,
        can_accept: bool,
    ) -> ReleasedPress {
        let mouse = TouchPoint {
            state,
            abs,
            time,
            uid: MOUSE_UID,
            rotation_angle: 0.0,
            force: 0.0,
            radius: DVec2::default(),
            handled: Cell::new(Area::Empty),
            sweep_lock: Cell::new(Area::Empty),
        };
        self.handle_touch_update(time, &[mouse], |_| can_accept)
    }

    /// Handles a step of a trackpad's pinch, which zooms about the mouse cursor.
    pub fn handle_trackpad_pinch(&mut self, phase: PinchPhase, abs: DVec2, scale: f64) {
        if matches!(phase, PinchPhase::Begin) {
            self.animation = Animation::None;
        }
        self.zoom_about(abs, abs, scale);
        if matches!(phase, PinchPhase::End) {
            self.animate_zoom_to(self.zoom.max(MIN_ZOOM), abs, SPRING_BACK_SPEED);
        }
    }

    /// Forgets the latest tap, so that the next one can't be the second tap of a double tap.
    pub fn clear_last_tap(&mut self) {
        self.last_tap = None;
    }

    /// Immediately zooms by the given `factor` about the given point.
    pub fn zoom_by(&mut self, factor: f64, anchor_abs: DVec2) {
        self.animation = Animation::None;
        let zoom = (self.zoom * factor).max(MIN_ZOOM);
        self.offset = self.get_anchored_offset(anchor_abs, anchor_abs, zoom);
        self.zoom = zoom;
    }

    /// Smoothly zooms by the given `factor` about the middle of the viewport.
    pub fn animate_zoom_by(&mut self, factor: f64) {
        let to_zoom = (self.get_zoom_headed_for() * factor).max(MIN_ZOOM);
        self.animate_zoom_to(to_zoom, self.viewport.center(), ZOOM_SPRING_SPEED);
    }

    /// Smoothly zooms back to the content being fitted to the viewport.
    pub fn animate_zoom_to_fit(&mut self) {
        self.animate_zoom_to(FIT_ZOOM, self.viewport.center(), ZOOM_SPRING_SPEED);
    }

    /// Moves any animation that's in progress along to the given `time` of a `NextFrame`.
    pub fn advance_animation(&mut self, time: f64) {
        match self.animation {
            Animation::Fling { mut clock, from_offset, distance, duration } => {
                let elapsed = clock.advance(time);
                let travel = dvec2(
                    distance.x * get_fling_progress(elapsed / duration.x),
                    distance.y * get_fling_progress(elapsed / duration.y),
                );
                let offset = self.clamp_offset(from_offset + travel, self.zoom);
                // An edge stops the content dead, and the fling too once nothing can move.
                self.animation = if offset != self.offset && elapsed < duration.x.max(duration.y) {
                    Animation::Fling { clock, from_offset, distance, duration }
                } else {
                    Animation::None
                };
                self.offset = offset;
            }
            Animation::Spring { mut clock, speed, from_zoom, to_zoom, from_offset, to_offset } => {
                // This is how much is left to go on a spring that never overshoots.
                let spring_time = clock.advance(time) * speed;
                let remaining = (1.0 + spring_time) * (-spring_time).exp();
                if remaining < SPRING_END_FRACTION {
                    self.zoom = to_zoom;
                    self.offset = self.clamp_offset(to_offset, to_zoom);
                    self.animation = Animation::None;
                    return;
                }
                self.zoom = to_zoom + (from_zoom - to_zoom) * remaining;
                // The content's edges stay put along the way too, not only at both ends.
                let offset = DVec2::from_lerp(to_offset, from_offset, remaining);
                self.offset = self.clamp_offset(offset, self.zoom);
                self.animation = Animation::Spring { clock, speed, from_zoom, to_zoom, from_offset, to_offset };
            }
            Animation::None => {}
        }
    }

    /// Acts upon `pointers` having been updated from what is in `prev_pointers`.
    fn handle_pointers_changed(
        &mut self,
        time: f64,
        has_new_press: bool,
        released_abs: Option<DVec2>,
    ) -> ReleasedPress {
        let has_same_pointers = !has_new_press
            && self.pointers.len() == self.prev_pointers.len()
            && self.pointers.iter().all(|pointer| {
                self.prev_pointers.iter().any(|prev| prev.uid == pointer.uid)
            });
        if has_same_pointers {
            self.handle_pointers_moved(time);
            return ReleasedPress::None;
        }

        // A pointer came or went, so everything starts over from where the pointers
        // are now, such that the content doesn't jump.
        let old_gesture = std::mem::take(&mut self.gesture);
        match *self.pointers.as_slice() {
            [] => return self.handle_all_pointers_released(old_gesture, time, released_abs),
            [pointer] => {
                // A press that catches a flinging content stops it, and isn't a tap.
                let is_flinging = matches!(self.animation, Animation::Fling { .. });
                self.gesture = if has_new_press && !is_flinging {
                    let slop = if pointer.uid == MOUSE_UID { DOUBLE_CLICK_SLOP } else { DOUBLE_TAP_SLOP };
                    Gesture::PossibleTap {
                        down_abs: pointer.abs,
                        down_time: time,
                        is_second_tap: self.last_tap.is_some_and(|tap| {
                            time - tap.down_time < DOUBLE_TAP_TIMEOUT
                                && time - tap.up_time >= DOUBLE_TAP_MIN_GAP
                                && (pointer.abs - tap.down_abs).length() < slop
                        }),
                    }
                } else {
                    Gesture::Drag
                };
                self.velocity_samples_x.clear();
                self.velocity_samples_y.clear();
                push_sample(&mut self.velocity_samples_x, pointer.abs.x, time);
                push_sample(&mut self.velocity_samples_y, pointer.abs.y, time);
            }
            _ => {
                self.gesture = match old_gesture {
                    Gesture::Drag | Gesture::Pinch | Gesture::OneFingerZoom { .. } => Gesture::Pinch,
                    _ => {
                        let (start_focus, start_span) = get_focus_and_span(&self.pointers);
                        Gesture::PinchWithinSlop { start_focus, start_span }
                    }
                };
            }
        }
        // A tap in the making leaves a zoom animation to finish.
        if !matches!(self.gesture, Gesture::PossibleTap { .. }) {
            self.animation = Animation::None;
        }
        self.last_tap = None;
        ReleasedPress::None
    }

    /// Pans or zooms the content by however much the pointers have moved.
    fn handle_pointers_moved(&mut self, time: f64) {
        let (Some(&pointer), Some(&prev_pointer)) = (self.pointers.first(), self.prev_pointers.first()) else { return };
        let (abs, mut prev_abs) = (pointer.abs, prev_pointer.abs);
        if self.pointers.len() == 1 {
            // A still finger gets reported as moving too, which would crowd out the samples.
            if abs != prev_abs {
                push_sample(&mut self.velocity_samples_x, abs.x, time);
                push_sample(&mut self.velocity_samples_y, abs.y, time);
            }
            // A press that has moved too far is no longer a tap.
            if let Gesture::PossibleTap { down_abs, is_second_tap, .. } = self.gesture
                && (abs - down_abs).length() > TOUCH_SLOP
            {
                self.gesture = if is_second_tap && pointer.uid != MOUSE_UID {
                    Gesture::OneFingerZoom { anchor_abs: down_abs }
                } else {
                    Gesture::Drag
                };
                // The content comes along from wherever an animation has it right now.
                self.animation = Animation::None;
                // We catch up with the pointer, so it stays on the content that it pressed.
                prev_abs = down_abs;
            }
        }
        match self.gesture {
            Gesture::Drag => {
                self.offset = self.clamp_offset(self.offset + abs - prev_abs, self.zoom);
            }
            Gesture::PinchWithinSlop { start_focus, start_span } => {
                let (focus, span) = get_focus_and_span(&self.pointers);
                if (span - start_span).abs() > 2.0 * TOUCH_SLOP
                    || (focus - start_focus).length() > TOUCH_SLOP
                {
                    self.gesture = Gesture::Pinch;
                }
            }
            Gesture::Pinch => {
                let (prev_focus, prev_span) = get_focus_and_span(&self.prev_pointers);
                let (focus, span) = get_focus_and_span(&self.pointers);
                let factor = if prev_span > 0.0 && span > 0.0 { span / prev_span } else { 1.0 };
                self.zoom_about(prev_focus, focus, factor);
            }
            Gesture::OneFingerZoom { anchor_abs } => {
                // Dragging down zooms in and dragging up zooms out, by the square root
                // of how many times farther from the double tap the finger has gotten.
                let get_zoom_at = |y: f64| {
                    let zoom = ((y - anchor_abs.y).abs() / TOUCH_SLOP).max(1.0).sqrt();
                    if y < anchor_abs.y { 1.0 / zoom } else { zoom }
                };
                self.zoom_about(anchor_abs, anchor_abs, get_zoom_at(abs.y) / get_zoom_at(prev_abs.y));
            }
            Gesture::PossibleTap { .. } | Gesture::None => {}
        }
    }

    /// Ends the given `gesture`, which may tap, fling, or spring back.
    fn handle_all_pointers_released(
        &mut self,
        gesture: Gesture,
        time: f64,
        released_abs: Option<DVec2>,
    ) -> ReleasedPress {
        let mut released_press = ReleasedPress::None;
        match gesture {
            Gesture::PossibleTap { down_abs, down_time, is_second_tap } => {
                if let Some(abs) = released_abs
                    && time - down_time < LONG_PRESS_TIMEOUT
                {
                    if is_second_tap {
                        // Zoomed in or out, a double tap goes back to fit. From fit, it zooms in.
                        let to_zoom = if self.get_zoom_headed_for() == FIT_ZOOM {
                            let fitted_size = self.content_size * self.get_fit_scale();
                            let fill_zoom = (self.viewport.size.x / fitted_size.x)
                                .max(self.viewport.size.y / fitted_size.y);
                            DOUBLE_TAP_ZOOM.max(fill_zoom)
                        } else {
                            FIT_ZOOM
                        };
                        self.animate_zoom_to(to_zoom, down_abs, ZOOM_SPRING_SPEED);
                        released_press = ReleasedPress::DoubleTap;
                    } else {
                        self.last_tap = Some(Tap { down_abs, down_time, up_time: time });
                        released_press = ReleasedPress::Tap {
                            abs,
                            double_tap_wait: (DOUBLE_TAP_TIMEOUT - (time - down_time)).max(0.0),
                        };
                    }
                }
            }
            Gesture::Drag => {
                let has_stopped = self.velocity_samples_x.last()
                    .is_none_or(|sample| time - sample.time > POINTER_STOPPED_TIME);
                let (velocity_x, travel_x) = estimate_release_velocity(&self.velocity_samples_x);
                let (velocity_y, travel_y) = estimate_release_velocity(&self.velocity_samples_y);
                if !has_stopped
                    && released_abs.is_some()
                    && self.zoom >= MIN_ZOOM
                    && velocity_x.abs().max(velocity_y.abs()) >= MIN_FLING_SPEED
                    && dvec2(travel_x, travel_y).length() > FLING_MIN_TOTAL_DELTA
                {
                    // Like on Android, each axis is flung on its own, which takes
                    // however long and goes however far its own speed calls for.
                    let (distance_x, duration_x) = get_fling_distance_and_duration(velocity_x);
                    let (distance_y, duration_y) = get_fling_distance_and_duration(velocity_y);
                    self.animation = Animation::Fling {
                        clock: FrameClock::default(),
                        from_offset: self.offset,
                        distance: dvec2(distance_x, distance_y),
                        duration: dvec2(duration_x, duration_y),
                    };
                }
            }
            _ => {}
        }
        if !self.is_animating() {
            self.animate_zoom_to(
                self.zoom.max(MIN_ZOOM),
                self.zoom_focus_abs.unwrap_or(self.viewport.center()),
                SPRING_BACK_SPEED,
            );
        }
        released_press
    }

    /// Zooms by the given `factor`, which is resisted below the minimum zoom,
    /// while moving the content that was under `from_abs` to be under `to_abs`.
    fn zoom_about(&mut self, from_abs: DVec2, to_abs: DVec2, factor: f64) {
        // The rubber band works on how many percent below the minimum a zoom level is.
        let get_excess = |zoom: f64| (MIN_ZOOM / zoom - 1.0).max(0.0) * 100.0;
        let get_zoom = |excess: f64| MIN_ZOOM / (1.0 + excess / 100.0);
        let length = MAX_UNDERZOOM_PERCENT / RUBBER_BAND_TOUCH_RANGE;
        // We take the resistance off of where we are, zoom, and then put it back on.
        let raw_zoom = factor * if self.zoom < MIN_ZOOM {
            get_zoom(stretch_raw(get_excess(self.zoom), length, true))
        } else {
            self.zoom
        };
        let zoom = if raw_zoom < MIN_ZOOM {
            get_zoom(stretch_displayed(get_excess(raw_zoom), length, true))
        } else {
            raw_zoom
        };
        if !zoom.is_finite() || zoom <= 0.0 {
            return;
        }
        self.offset = self.get_anchored_offset(from_abs, to_abs, zoom);
        self.zoom = zoom;
        self.zoom_focus_abs = Some(to_abs);
    }

    /// Starts a spring animation to the given zoom level, such that the content
    /// under `anchor_abs` stays there for as long as the content's edges allow.
    fn animate_zoom_to(&mut self, to_zoom: f64, anchor_abs: DVec2, speed: f64) {
        let to_offset = self.get_anchored_offset(anchor_abs, anchor_abs, to_zoom);
        if !to_zoom.is_finite() || (to_zoom == self.zoom && to_offset == self.offset) {
            return;
        }
        self.animation = Animation::Spring {
            clock: FrameClock::default(),
            speed,
            from_zoom: self.zoom,
            to_zoom,
            from_offset: self.offset,
            to_offset,
        };
    }

    /// Returns the offset that, at the given zoom level, puts the content that is now
    /// under `from_abs` under `to_abs`, as closely as the content's edges allow.
    fn get_anchored_offset(&self, from_abs: DVec2, to_abs: DVec2, zoom: f64) -> DVec2 {
        let center = self.viewport.center();
        let offset = to_abs - center - (from_abs - center - self.offset) * (zoom / self.zoom);
        self.clamp_offset(offset, zoom)
    }

    /// Keeps the content's edges from coming away from the viewport's edges, and
    /// centers the content along each axis that it is smaller than the viewport.
    fn clamp_offset(&self, offset: DVec2, zoom: f64) -> DVec2 {
        let content_size = self.content_size * (self.get_fit_scale() * zoom);
        let max_offset = (content_size - self.viewport.size) * 0.5;
        dvec2(
            offset.x.clamp(-max_offset.x.max(0.0), max_offset.x.max(0.0)),
            offset.y.clamp(-max_offset.y.max(0.0), max_offset.y.max(0.0)),
        )
    }

    /// Returns how many points each of the content's pixels covers when fitted to the viewport.
    fn get_fit_scale(&self) -> f64 {
        if self.content_size.x <= 0.0 || self.content_size.y <= 0.0 {
            return 0.0;
        }
        (self.viewport.size.x / self.content_size.x)
            .min(self.viewport.size.y / self.content_size.y)
    }

    /// Returns the zoom level that an animation is on its way to, or else the current one.
    fn get_zoom_headed_for(&self) -> f64 {
        match self.animation {
            Animation::Spring { to_zoom, .. } => to_zoom,
            _ => self.zoom,
        }
    }
}

/// Returns how far a fling that starts out at the given velocity goes,
/// and how many seconds it takes to get there, as per Android's `OverScroller`.
fn get_fling_distance_and_duration(velocity: f64) -> (f64, f64) {
    let velocity = velocity.clamp(-MAX_FLING_SPEED, MAX_FLING_SPEED);
    if velocity == 0.0 {
        return (0.0, 0.0);
    }
    let deceleration = (FLING_INFLEXION * velocity.abs() / FLING_FRICTION).ln();
    let distance = FLING_FRICTION
        * (FLING_DECELERATION_RATE / (FLING_DECELERATION_RATE - 1.0) * deceleration).exp();
    (distance.copysign(velocity), (deceleration / (FLING_DECELERATION_RATE - 1.0)).exp())
}

/// Returns how much of its distance a fling has gone after the given fraction of its duration,
/// along Android's fling curve: a bezier pulled towards (inflexion / 2, 0.5), then (inflexion, 1).
fn get_fling_progress(time_fraction: f64) -> f64 {
    if time_fraction.is_nan() || time_fraction >= 1.0 {
        return 1.0;
    }
    let get_curve_at = |at: f64, first_pull: f64, second_pull: f64| {
        3.0 * at * (1.0 - at) * ((1.0 - at) * first_pull + at * second_pull) + at * at * at
    };
    // Find the point along the curve that is at the given time, which is its x.
    let (mut before, mut after) = (0.0, 1.0);
    for _ in 0..40 {
        let middle = (before + after) / 2.0;
        if get_curve_at(middle, FLING_INFLEXION / 2.0, FLING_INFLEXION) > time_fraction {
            after = middle;
        } else {
            before = middle;
        }
    }
    get_curve_at((before + after) / 2.0, 0.5, 1.0)
}

/// Returns the point in the middle of the given pointers, and how far apart they are.
///
/// With two pointers, the span is exactly the distance between them.
fn get_focus_and_span(pointers: &[Pointer]) -> (DVec2, f64) {
    let count = pointers.len().max(1) as f64;
    let focus = pointers.iter().fold(DVec2::default(), |sum, pointer| sum + pointer.abs) / count;
    let deviation = pointers.iter().fold(DVec2::default(), |sum, pointer| {
        sum + dvec2((pointer.abs.x - focus.x).abs(), (pointer.abs.y - focus.y).abs())
    }) / count;
    (focus, 2.0 * deviation.length())
}
