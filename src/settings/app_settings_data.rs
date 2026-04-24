//! App-wide settings/preferences and related types.

use makepad_widgets::*;
use serde::{Deserialize, Serialize};

/// App-wide user preferences controlled by the App Settings UI.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppPreferences {
    /// Forces the HomeScreen `AdaptiveView` into a particular layout,
    /// or falls back to the default automatic width-based layout.
    #[serde(default)]
    pub view_mode: ViewModeOverride,
    /// * If `true` (default), plain Enter sends the message (Shift+Enter inserts a newline).
    /// * If `false`, Cmd+Enter (macOS) / Ctrl+Enter (other platforms) sends the
    /// message and plain Enter inserts a newline.
    #[serde(default)]
    pub send_on_enter: bool,
    /// Max height of image thumbnails in the room timeline.
    #[serde(default)]
    pub thumbnail_max_height: ThumbnailMaxHeight,

    // Note: if you add a new preference here, be sure to add a new
    // function `on_<NEW_PREFERENCE>_changed` and update `broadcast_all()`.
}

impl Default for AppPreferences {
    fn default() -> Self {
        Self {
            view_mode: ViewModeOverride::default(),
            send_on_enter: true,
            thumbnail_max_height: ThumbnailMaxHeight::default(),
        }
    }
}

impl AppPreferences {
    /// Broadcasts the current `view_mode` to listening widgets.
    ///
    /// Call this whenever the `view_mode` setting has just changed.
    pub fn on_view_mode_changed(&self, cx: &mut Cx) {
        cx.global::<AppPreferencesGlobal>().0.view_mode = self.view_mode;
        cx.action(AppSettingsAction::ViewModeChanged(self.view_mode));
    }

    /// Broadcasts the current `send_on_enter` value to listening widgets.
    ///
    /// Call this whenever `send_on_enter` has just changed.
    pub fn on_send_on_enter_changed(&self, cx: &mut Cx) {
        cx.global::<AppPreferencesGlobal>().0.send_on_enter = self.send_on_enter;
        cx.action(AppSettingsAction::SendOnEnterChanged(self.send_on_enter));
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
    /// which walks the widget tree with `Apply::Reload`. Each Image's
    /// `Size::script_apply` re-reads `max` from the shared `IMG_MSG_FIT`
    /// object and updates the widget's `walk.height`.
    ///
    /// For `ThumbnailMaxHeight::Unlimited` we set `max` to `nil`, which
    /// `Option<FitBound>::script_apply` maps to `None` — i.e. `Fit{max: None}`,
    /// truly unbounded.
    pub fn on_thumbnail_max_height_changed(&self, cx: &mut Cx) {
        cx.global::<AppPreferencesGlobal>().0.thumbnail_max_height = self.thumbnail_max_height;
        match self.thumbnail_max_height.to_pixels() {
            Some(px) => {
                let px = px as f64;
                // The `use mod.prelude.widgets.*` is required so `FitBound`
                // resolves in runtime script scope.
                script_eval!(cx, {
                    use mod.prelude.widgets.*
                    mod.widgets.IMG_MSG_FIT.max = FitBound.Abs(#(px))
                });
            }
            None => {
                script_eval!(cx, {
                    mod.widgets.IMG_MSG_FIT.max = nil
                });
            }
        }

        // Now that we've updated the `IMG_MSG_FIT.max` object in place,
        // we need to instruct every widget that uses this object to re-read
        // the new value and update their whole widget tree accordingly.
        cx.request_script_reapply();
    }

    /// Broadcasts every preference to listening widgets.
    ///
    /// Used at app-state restore so every listener picks up the loaded
    /// values without having to poll `AppState` every draw, and also
    /// after every `Event::LiveEdit` so a hot-reloaded `script_mod!` block
    /// doesn't clobber our runtime heap overrides.
    pub fn broadcast_all(&self, cx: &mut Cx) {
        self.on_view_mode_changed(cx);
        self.on_send_on_enter_changed(cx);
        self.on_thumbnail_max_height_changed(cx);
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
    #[default]
    Small,
    /// 400 pixels.
    Medium,
    /// No maximum height (not recommended).
    Unlimited,
    /// A user-specified maximum height in pixels.
    Custom(u32),
}

impl ThumbnailMaxHeight {
    /// Returns the max height in pixels, or `None` if unlimited.
    pub fn to_pixels(&self) -> Option<u32> {
        match self {
            Self::Small => Some(200),
            Self::Medium => Some(400),
            Self::Unlimited => None,
            Self::Custom(v) => Some(*v),
        }
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
pub enum AppSettingsAction {
    ViewModeChanged(ViewModeOverride),
    SendOnEnterChanged(bool),
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
