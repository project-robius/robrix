//! `Tap`, the first tap of what might become a double tap (or a mouse's double click).

use makepad_widgets::DVec2;

/// The second tap of a double tap has to go down within
/// this many seconds of the first one going down.
pub const DOUBLE_TAP_TIMEOUT: f64 = 0.3;
/// The second tap also has to go down at least
/// this many seconds after the first one came back up.
const DOUBLE_TAP_MIN_GAP: f64 = 0.04;
/// How close together both taps of a double tap have to be, in points.
const DOUBLE_TAP_SLOP: f64 = 100.0;
/// The same as [`DOUBLE_TAP_SLOP`], but for both clicks of a mouse's double click.
const DOUBLE_CLICK_SLOP: f64 = 5.0;

#[derive(Clone, Copy, Debug)]
pub struct Tap {
    pub down_abs: DVec2,
    pub down_time: f64,
    pub up_time: f64,
}

impl Tap {
    /// Returns whether a press that went down at `down_abs` and `down_time` is this tap's second tap.
    pub fn is_followed_by(&self, down_abs: DVec2, down_time: f64, is_mouse: bool) -> bool {
        let slop = if is_mouse { DOUBLE_CLICK_SLOP } else { DOUBLE_TAP_SLOP };
        down_time - self.down_time < DOUBLE_TAP_TIMEOUT
            && down_time - self.up_time >= DOUBLE_TAP_MIN_GAP
            && (down_abs - self.down_abs).length() < slop
    }
}
