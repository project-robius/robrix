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
    /// Call this whenever `thumbnail_max_height` has just changed.
    ///
    /// Updates the `mod.widgets.IMAGE_MSG_MAX_HEIGHT` DSL constant and
    /// re-assigns the `mod.widgets.ImageMessage` / `.CondensedImageMessage`
    /// templates so their `height: Fit{max: FitBound.Abs(..)}` property has
    /// the new cap baked in — otherwise the old constant is still stored
    /// inside the already-evaluated template objects. `RoomScreen`s handle
    /// the emitted action to refresh their PortalList's captured template
    /// references and resize any image widgets currently in hand.
    pub fn on_thumbnail_max_height_changed(&self, cx: &mut Cx) {
        use makepad_script::trap::NoTrap;
        let max_height = self
            .thumbnail_max_height
            .to_pixels()
            .map_or(f64::MAX, |p| p as f64);

        // Capture the template object identity before re-assignment so we
        // can verify whether `script_eval!` actually replaced it.
        let img_before = cx.with_vm(|vm| {
            let widgets = vm.heap_mut().module(id!(widgets));
            vm.heap().value(widgets, id!(ImageMessage).into(), NoTrap)
        });
        let cimg_before = cx.with_vm(|vm| {
            let widgets = vm.heap_mut().module(id!(widgets));
            vm.heap().value(widgets, id!(CondensedImageMessage).into(), NoTrap)
        });
        let const_before = cx.with_vm(|vm| {
            let widgets = vm.heap_mut().module(id!(widgets));
            vm.heap().value(widgets, id!(IMAGE_MSG_MAX_HEIGHT).into(), NoTrap)
        });
        let tl_before = cx.with_vm(|vm| {
            let widgets = vm.heap_mut().module(id!(widgets));
            vm.heap().value(widgets, id!(Timeline).into(), NoTrap)
        });
        let rs_before = cx.with_vm(|vm| {
            let widgets = vm.heap_mut().module(id!(widgets));
            vm.heap().value(widgets, id!(RoomScreen).into(), NoTrap)
        });

        // Makepad DSL templates are eagerly evaluated: references like
        // `(mod.widgets.IMAGE_MSG_MAX_HEIGHT)` and `ImageMessage :=
        // mod.widgets.ImageMessage {}` are resolved at script_mod load
        // time and the resulting object is frozen into the enclosing
        // template. That means updating `mod.widgets.IMAGE_MSG_MAX_HEIGHT`
        // alone — or even re-assigning `mod.widgets.ImageMessage` — isn't
        // enough to affect RoomScreens instantiated later: their parent
        // template (`mod.widgets.RoomScreen` → `mod.widgets.Timeline` →
        // its inner `PortalList`) still points at the stale originals.
        //
        // So we re-assign the whole template chain, innermost first, so
        // each layer's inner references re-resolve against the just-
        // updated outer ones.
        //
        // The `use mod.prelude.widgets.*` at the top is required — without
        // it, `Fit` / `FitBound` aren't in scope at runtime `script_eval!`
        // time and the height assignment silently produces a broken
        // template with an invalid height property.
        script_eval!(cx, {
            use mod.prelude.widgets.*

            mod.widgets.IMAGE_MSG_MAX_HEIGHT = #(max_height)

            mod.widgets.ImageMessage = mod.widgets.ImageMessage {
                body +: { content +: { message +: {
                    image_view +: { image +: {
                        height: Fit{max: FitBound.Abs(#(max_height))}
                    } }
                    default_image_view +: { image +: {
                        height: Fit{max: FitBound.Abs(#(max_height))}
                    } }
                } } }
            }
            mod.widgets.CondensedImageMessage = mod.widgets.CondensedImageMessage {
                body +: { content +: { message +: {
                    image_view +: { image +: {
                        height: Fit{max: FitBound.Abs(#(max_height))}
                    } }
                    default_image_view +: { image +: {
                        height: Fit{max: FitBound.Abs(#(max_height))}
                    } }
                } } }
            }
            // Re-assign `Timeline` so its inner `list`'s
            // `ImageMessage := mod.widgets.ImageMessage {}` /
            // `CondensedImageMessage := …` references re-resolve against
            // the freshly-updated module entries above.
            mod.widgets.Timeline = mod.widgets.Timeline {
                list +: {
                    ImageMessage := mod.widgets.ImageMessage {}
                    CondensedImageMessage := mod.widgets.CondensedImageMessage {}
                }
            }
            // Re-assign `RoomScreen` so its `timeline := mod.widgets.Timeline {}`
            // re-resolves against the updated `mod.widgets.Timeline` above.
            // RoomScreens instantiated from this template afterwards — e.g.,
            // dock tabs created after an app-state restore — get the new
            // cap all the way down the chain.
            mod.widgets.RoomScreen = mod.widgets.RoomScreen {
                room_screen_wrapper +: {
                    keyboard_view +: {
                        timeline := mod.widgets.Timeline {}
                    }
                }
            }
        });

        let img_after = cx.with_vm(|vm| {
            let widgets = vm.heap_mut().module(id!(widgets));
            vm.heap().value(widgets, id!(ImageMessage).into(), NoTrap)
        });
        let cimg_after = cx.with_vm(|vm| {
            let widgets = vm.heap_mut().module(id!(widgets));
            vm.heap().value(widgets, id!(CondensedImageMessage).into(), NoTrap)
        });
        let const_after = cx.with_vm(|vm| {
            let widgets = vm.heap_mut().module(id!(widgets));
            vm.heap().value(widgets, id!(IMAGE_MSG_MAX_HEIGHT).into(), NoTrap)
        });
        let tl_after = cx.with_vm(|vm| {
            let widgets = vm.heap_mut().module(id!(widgets));
            vm.heap().value(widgets, id!(Timeline).into(), NoTrap)
        });
        let rs_after = cx.with_vm(|vm| {
            let widgets = vm.heap_mut().module(id!(widgets));
            vm.heap().value(widgets, id!(RoomScreen).into(), NoTrap)
        });
        log!(
            "on_thumbnail_max_height_changed: max_height={max_height}\n  \
            IMAGE_MSG_MAX_HEIGHT const before={const_before:?} after={const_after:?}\n  \
            ImageMessage obj same={} ({:?} -> {:?})\n  \
            CondensedImageMessage obj same={} ({:?} -> {:?})\n  \
            Timeline obj same={} ({:?} -> {:?})\n  \
            RoomScreen obj same={} ({:?} -> {:?})",
            img_before.as_object() == img_after.as_object(),
            img_before.as_object(),
            img_after.as_object(),
            cimg_before.as_object() == cimg_after.as_object(),
            cimg_before.as_object(),
            cimg_after.as_object(),
            tl_before.as_object() == tl_after.as_object(),
            tl_before.as_object(),
            tl_after.as_object(),
            rs_before.as_object() == rs_after.as_object(),
            rs_before.as_object(),
            rs_after.as_object(),
        );

        cx.action(AppSettingsAction::ThumbnailMaxHeightChanged(self.thumbnail_max_height));
    }

    /// Propagates every preference to listening widgets in one go.
    ///
    /// Used at app-state restore so every listener picks up the loaded
    /// values without having to poll `AppState` every draw.
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
