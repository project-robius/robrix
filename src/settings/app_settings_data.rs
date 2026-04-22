//! App-wide settings/preferences and related types.

use makepad_widgets::*;
use serde::{Deserialize, Serialize};

/// App-wide user preferences controlled by the App Settings UI.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppPreferences {
    /// Forces the HomeScreen `AdaptiveView` into a specific layout,
    /// or falls back to the default automatic width-based selection.
    #[serde(default)]
    pub view_mode: ViewModeOverride,
    /// When `true` (default), plain Enter sends the message (Shift+Enter inserts a newline).
    /// When `false`, Cmd+Enter (macOS) / Ctrl+Enter (other platforms) sends the
    /// message and plain Enter inserts a newline.
    #[serde(default)]
    pub send_on_enter: bool,
    /// Max height of image thumbnails in the room timeline.
    #[serde(default)]
    pub thumbnail_max_height: ThumbnailMaxHeight,

    // Note: if you add a new preference here, be sure to update `broadcast_all()`.
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
    /// Propagates the current `view_mode` to listening widgets.
    ///
    /// Call this whenever `view_mode` has just changed. `HomeScreen` picks
    /// this up and reinstalls the `AdaptiveView`'s variant selector
    /// accordingly.
    pub fn on_view_mode_changed(&self, cx: &mut Cx) {
        cx.action(AppSettingsAction::ViewModeChanged(self.view_mode));
    }

    /// Propagates the current `send_on_enter` value to listening widgets.
    ///
    /// Call this whenever `send_on_enter` has just changed. `RoomInputBar`
    /// picks this up on its next draw to configure the message
    /// `TextInput`'s submit-on-Enter behavior.
    pub fn on_send_on_enter_changed(&self, cx: &mut Cx) {
        cx.action(AppSettingsAction::SendOnEnterChanged(self.send_on_enter));
    }

    /// Propagates the current `thumbnail_max_height` to listening widgets.
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
    /// For `ThumbnailMaxHeight::Unlimited` we set `max` to
    /// `FitBound.Abs(f64::MAX)` so no effective cap is applied.
    ///
    /// The `use mod.prelude.widgets.*` at the top is required — without
    /// it, `FitBound` isn't in scope at runtime `script_eval!` time.
    pub fn on_thumbnail_max_height_changed(&self, cx: &mut Cx) {
        let max_height = self
            .thumbnail_max_height
            .to_pixels()
            .map_or(f64::MAX, |p| p as f64);

        script_eval!(cx, {
            use mod.prelude.widgets.*

            mod.widgets.IMG_MSG_FIT.max = FitBound.Abs(#(max_height))
        });

        // The shared `IMG_MSG_FIT.max` was mutated in place; every widget
        // whose `walk.height` referenced this object needs a tree re-apply
        // pass to re-read the new value. The flag is coalesced — multiple
        // calls in the same frame result in exactly one
        // `Event::ScriptReapply`.
        cx.request_script_reapply();
    }

    /// Propagates every preference to listening widgets in one go.
    ///
    /// Used at app-state restore so every listener picks up the loaded
    /// values without having to poll `AppState` every draw, and also
    /// after every `Event::LiveEdit` so a hot-reloaded `script_mod!` block
    /// doesn't clobber our runtime heap overrides.
    pub fn broadcast_all(&self, cx: &mut Cx) {
        log!("AppPreferences::broadcast_all: view_mode={:?}, send_on_enter={}, thumbnail_max_height={:?}",
            self.view_mode, self.send_on_enter, self.thumbnail_max_height,
        );
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
/// app can react (e.g., apply a new view mode or redraw thumbnails).
#[derive(Debug, Clone)]
pub enum AppSettingsAction {
    ViewModeChanged(ViewModeOverride),
    SendOnEnterChanged(bool),
    ThumbnailMaxHeightChanged(ThumbnailMaxHeight),
}
