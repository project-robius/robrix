//! App-wide preferences and related types.

use makepad_widgets::*;
use serde::{Deserialize, Serialize};

/// App-wide user preferences controlled by the App Settings UI.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AppPreferences {
    /// Forces the HomeScreen `AdaptiveView` into a particular layout,
    /// or falls back to the default automatic width-based layout.
    #[serde(default, deserialize_with = "crate::utils::deserialize_or_default")]
    pub view_mode: ViewModeOverride,
    /// * If `true` (default), plain Enter sends the message (Shift+Enter inserts a newline).
    /// * If `false`, Cmd+Enter (Apple platforms) / Ctrl+Enter (other platforms) sends the
    ///   message and plain Enter inserts a newline. This is only relevant for physical keyboards;
    ///   virtual/soft keyboards always insert a newline upon Enter.
    #[serde(default = "default_send_on_enter", deserialize_with = "deserialize_send_on_enter")]
    pub send_on_enter: bool,
    /// Max height of image thumbnails in the room timeline.
    #[serde(default, deserialize_with = "crate::utils::deserialize_or_default")]
    pub thumbnail_max_height: ThumbnailMaxHeight,
    /// UI-wide zoom level, which scaled the entire UI (not just text).
    #[serde(default, deserialize_with = "crate::utils::deserialize_or_default")]
    pub ui_zoom: UiZoom,

    // Note: if you add a new preference here, be sure to add a new
    // function `on_<NEW_PREFERENCE>_changed` and update `broadcast_all()`.
}

impl Default for AppPreferences {
    fn default() -> Self {
        Self {
            view_mode: ViewModeOverride::default(),
            send_on_enter: true,
            thumbnail_max_height: ThumbnailMaxHeight::default(),
            ui_zoom: UiZoom::default(),
        }
    }
}

impl AppPreferences {
    /// Broadcasts the current `view_mode` to listening widgets.
    ///
    /// Call this whenever the `view_mode` preference has just changed.
    pub fn on_view_mode_changed(&self, cx: &mut Cx) {
        cx.global::<AppPreferencesGlobal>().0.view_mode = self.view_mode;
        cx.action(AppPreferencesAction::ViewModeChanged(self.view_mode));
    }

    /// Broadcasts the current `send_on_enter` value to listening widgets.
    ///
    /// Call this whenever `send_on_enter` has just changed.
    pub fn on_send_on_enter_changed(&self, cx: &mut Cx) {
        cx.global::<AppPreferencesGlobal>().0.send_on_enter = self.send_on_enter;
        cx.action(AppPreferencesAction::SendOnEnterChanged(self.send_on_enter));
    }

    /// Broadcasts the current `thumbnail_max_height` to listening widgets.
    ///
    /// Approach: `mod.widgets.IMG_MSG_FIT` is a single shared
    /// `Size::Fit{max: ...}` heap object referenced by every Image widget
    /// inside an `ImageMessage` / `CondensedImageMessage` via
    /// `height: (mod.widgets.IMG_MSG_FIT)`. Because DSL field assignment
    /// stores the object *reference* (a 64-bit `ScriptValue` holding the
    /// heap index), every widget's `walk.height` points to the same slot.
    /// Mutating `IMG_MSG_FIT.max` in place is therefore observed by every
    /// holder on the next re-apply — no chain-walking, no per-derivative
    /// copies.
    ///
    /// `cx.request_script_reapply()` then fires `Event::ScriptReapply`,
    /// which walks the widget tree with `Apply::ScriptReapply`. Each Image's
    /// `Size::script_apply` re-reads `max` from the shared `IMG_MSG_FIT`
    /// object and updates the widget's `walk.height`.
    pub fn on_thumbnail_max_height_changed(&self, cx: &mut Cx) {
        cx.global::<AppPreferencesGlobal>().0.thumbnail_max_height = self.thumbnail_max_height;
        let px = self.thumbnail_max_height.to_pixels() as f64;
        // The `use mod.prelude.widgets.*` is required so `FitBound`
        // resolves in runtime script scope.
        script_eval!(cx, {
            use mod.prelude.widgets.*
            mod.widgets.IMG_MSG_FIT.max = FitBound.Abs(#(px))
        });

        // Now that we've updated the `IMG_MSG_FIT.max` object in place,
        // we need to instruct every widget that uses this object to re-read
        // the new value and update their whole widget tree accordingly.
        cx.request_script_reapply();
    }

    /// Applies the current `ui_zoom` value by overriding the window's dpi factor.
    pub fn on_ui_zoom_changed(&self, cx: &mut Cx) {
        cx.global::<AppPreferencesGlobal>().0.ui_zoom = self.ui_zoom;
        let window_id = CxWindowPool::id_zero();
        let dpi_override = if self.ui_zoom.is_default() {
            None
        } else {
            let window = &cx.windows[window_id];
            let baseline = window.os_dpi_factor.unwrap_or(window.window_geom.dpi_factor);
            Some(baseline * self.ui_zoom.multiplier())
        };
        cx.set_window_dpi_override(window_id, dpi_override);
        cx.action(AppPreferencesAction::UiZoomChanged(self.ui_zoom));
    }

    /// Broadcasts every preference to listening widgets.
    ///
    /// Used upon app-state restore so every listener picks up the loaded
    /// values without having to poll `AppState` every draw, and also
    /// after every `Event::LiveEdit` so a hot-reloaded `script_mod!` block
    /// doesn't clobber our runtime heap overrides.
    pub fn broadcast_all(&self, cx: &mut Cx) {
        self.on_view_mode_changed(cx);
        self.on_send_on_enter_changed(cx);
        self.on_thumbnail_max_height_changed(cx);
        self.on_ui_zoom_changed(cx);
    }
}

/// Forces the main `HomeScreen` layout into a specific variant.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ViewModeOverride {
    /// Select desktop/mobile based on window width.
    #[default]
    Automatic,
    /// Always use the wide "desktop" layout.
    ForceWide,
    /// Always use the narrow "mobile" layout.
    ForceNarrow,
}

impl ViewModeOverride {
    pub fn from_index(index: usize) -> Self {
        match index {
            1 => Self::ForceWide,
            2 => Self::ForceNarrow,
            _ => Self::Automatic,
        }
    }
    pub fn to_index(self) -> usize {
        match self {
            Self::Automatic => 0,
            Self::ForceWide => 1,
            Self::ForceNarrow => 2,
        }
    }

    /// Returns a closure for use in `AdaptiveView::set_variant_selector`
    /// that selects this view mode override.
    pub fn variant_selector(self) -> impl FnMut(&mut Cx, &Vec2d) -> LiveId + 'static {
        move |cx: &mut Cx, _parent_size: &Vec2d| match self {
            Self::Automatic => {
                if cx.display_context.is_desktop()
                    || !cx.display_context.is_screen_size_known()
                {
                    live_id!(Desktop)
                } else {
                    live_id!(Mobile)
                }
            }
            Self::ForceWide => live_id!(Desktop),
            Self::ForceNarrow => live_id!(Mobile),
        }
    }
}

/// The maximum height (in pixels) of image thumbnails in the room timeline.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThumbnailMaxHeight {
    /// 200 pixels.
    Small,
    /// 300 pixels.
    #[default]
    Medium,
    /// 400 pixels.
    Large,
    /// A user-specified maximum height in pixels.
    Custom(u32),
}

impl ThumbnailMaxHeight {
    /// Returns the max height in pixels.
    pub fn to_pixels(&self) -> u32 {
        match self {
            Self::Small => 200,
            Self::Medium => 300,
            Self::Large => 400,
            Self::Custom(v) => *v,
        }
    }
}

/// `send_on_enter` defaults to `true`, unlike the typical `false` bool value.
fn default_send_on_enter() -> bool {
    true
}

fn deserialize_send_on_enter<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    Ok(serde_json::from_value(value).unwrap_or_else(|_| default_send_on_enter()))
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct UiZoom(pub f32);

impl UiZoom {
    pub const MIN: f32 = 0.25;
    pub const MAX: f32 = 3.00;
    pub const DEFAULT: f32 = 1.00;

    /// Step size for keyboard shortcuts.
    pub const STEP: f32 = 0.02;
    /// Step size for the app settings +/- buttons.
    pub const BUTTON_STEP: f32 = 0.05;

    /// Create a new zoom value that is properly clamped.
    pub fn new(value: f32) -> Self {
        let v = if value.is_finite() { value } else { Self::DEFAULT };
        Self(v.clamp(Self::MIN, Self::MAX))
    }

    pub fn multiplier(self) -> f64 {
        self.0 as f64
    }

    /// Returns whether this zoom value is within 0.01 of the default 100%.
    pub fn is_default(self) -> bool {
        (self.0 - Self::DEFAULT).abs() < 0.01
    }

    pub fn zoom_in_by(self, delta: f32) -> Self {
        Self::new(self.0 + delta)
    }

    pub fn zoom_out_by(self, delta: f32) -> Self {
        Self::new(self.0 - delta)
    }

    pub fn reset() -> Self {
        Self(Self::DEFAULT)
    }

    pub fn format_percent(self) -> String {
        let pct = self.0 * 100.0;
        let rounded = pct.round();
        if (pct - rounded).abs() < 0.05 {
            format!("{}%", rounded as i32)
        } else {
            format!("{:.1}%", pct)
        }
    }
}

impl Default for UiZoom {
    fn default() -> Self {
        Self(Self::DEFAULT)
    }
}

/// Actions emitted when an app-wide preference changes so other parts of the
/// app can react.
///
/// Note: the thumbnail max-height preference is *not* an action — it mutates
/// the shared `mod.widgets.IMG_MSG_FIT` heap object in place and relies on
/// `cx.request_script_reapply()` to propagate the change to every Image
/// widget via `Apply::Reload`.
#[derive(Debug, Clone)]
pub enum AppPreferencesAction {
    ViewModeChanged(ViewModeOverride),
    SendOnEnterChanged(bool),
    UiZoomChanged(UiZoom),
}

/// A `Cx` global mirror of the current [`AppPreferences`].
///
/// Kept in sync by the individual `on_*_changed` methods (and thus by
/// [`AppPreferences::broadcast_all`] at app-state restore). Widgets that
/// need to read a preference value at construction time — where
/// `scope.data` is not yet populated with `AppState` — can read it from
/// here via `cx.global::<AppPreferencesGlobal>()`.
#[derive(Default, Clone)]
pub struct AppPreferencesGlobal(pub AppPreferences);

/// Returns whether the UI should currently behave as the wide "desktop"
/// layout, honoring any `ForceWide` / `ForceNarrow` user override.
pub fn effective_is_desktop(cx: &mut Cx) -> bool {
    match cx.global::<AppPreferencesGlobal>().0.view_mode {
        ViewModeOverride::ForceWide => true,
        ViewModeOverride::ForceNarrow => false,
        ViewModeOverride::Automatic => {
            cx.display_context.is_desktop() || !cx.display_context.is_screen_size_known()
        }
    }
}
