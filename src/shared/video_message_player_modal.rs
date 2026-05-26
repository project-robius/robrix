//! `VideoMessagePlayerModal` — the **inner content widget** placed inside
//! a Makepad `Modal { content +: { ... } }` shell so the user can view a
//! maximised video.
//!
//! Architecture: this widget embeds a single `VideoMessagePlayer`
//! configured with `show_maximise_button: false` (already maximised) and
//! adds a top-right close button. The embedded player owns all playback
//! state — there is no `Arc<Mutex>` mirroring against the inline
//! timeline player. Closing the modal stops its playback.
//!
//! ```ignore
//! video_message_player_modal := Modal {
//!     content +: {
//!         height: Fill, width: Fill,
//!         align: Align{x: 0.5, y: 0.5},
//!         video_message_player_modal_inner := VideoMessagePlayerModal {}
//!     }
//! }
//! ```
//!
//! Action protocol (preserved so `RoomScreen` and `app.rs` wiring does
//! not need to change):
//!   - `VideoMessagePlayerModalAction::Open { ... }` — emitted by the
//!     inline player's `maximise_button` handler. `RoomScreen` calls
//!     `inner.show(cx, ...)` then `outer.open(cx)`.
//!   - `VideoMessagePlayerModalAction::Close` — emitted by this widget
//!     when its `close_button` is clicked.
//!   - `ModalAction::Dismissed` (from the outer Makepad `Modal`) is
//!     observed here and stops embedded playback. We MUST NOT re-emit
//!     `Close` in response (infinite feedback loop).

use makepad_widgets::*;
use std::path::PathBuf;

use crate::shared::video_message_player::{
    VideoMessagePlayerRef, VideoMessagePlayerWidgetExt, VideoSummary,
};

// ============================================================================
// Live design — the card body. No scrim, no overlay draw-list: the outer
// `Modal { ... }` wrapper supplies those.
// ============================================================================

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.VIDEO_MODAL_ICON_CLOSE = crate_resource("self://resources/icons/close.svg")

    mod.widgets.VideoMessagePlayerModal =
        set_type_default() do #(VideoMessagePlayerModal::register_widget(vm))
    {
        ..mod.widgets.RoundedView

        // The card body. Sized to fit the outer Modal's `Fill, Fill,
        // align: Center` content slot with a viewport margin.
        width: Fill { max: 1600 }
        height: Fill { max: 1000 }
        margin: 40
        flow: Overlay
        align: Align{x: 0.5, y: 0.5}

        show_bg: true
        // Transparent fill that absorbs hits so a click on the video
        // area does not dismiss the outer Modal.
        draw_bg +: {
            color: #00000000
            border_radius: 6.0
            border_size: 0.0
        }

        // The embedded inline player. `show_maximise_button: false`
        // hides the in-player maximise control (we are already
        // maximised); the modal owns the close button instead.
        inner_player := mod.widgets.VideoMessagePlayer {
            width: Fill
            height: Fill
            show_maximise_button: false
            surface +: {
                height: Fill
            }
        }

        // Close button overlay, top-right.
        close_button := Button {
            width: 36
            height: 36
            margin: Inset{ left: 99999, top: 8, right: 8 }
            text: ""
            spacing: 0
            padding: 0
            align: Align{x: 0.5, y: 0.5}
            icon_walk: Walk{width: 18, height: 18}
            draw_icon +: {
                svg: (mod.widgets.VIDEO_MODAL_ICON_CLOSE)
                color: #xffffff
            }
            draw_bg +: {
                border_radius: 5.0
                color: #x111827
                color_hover: #x374151
                color_down: #x111827
            }
        }
    }
}

// ============================================================================
// Actions
// ============================================================================

/// Actions emitted by / received around `VideoMessagePlayerModal`.
#[derive(Clone, Debug)]
pub enum VideoMessagePlayerModalAction {
    /// Emitted by the inline `VideoMessagePlayer`'s `maximise_button`
    /// handler. The host calls `inner.show(cx, ...)` with this payload
    /// and then opens the outer `Modal`.
    Open {
        inline_uid: WidgetUid,
        source_url: PathBuf,
        blurhash: Option<String>,
        summary: VideoSummary,
        position_ms: u64,
    },
    /// Emitted by this widget when its `close_button` is clicked.
    /// NOT emitted in response to `ModalAction::Dismissed`.
    Close,
}

/// Relay action used by the video maximise/close flow to ask the
/// `App`-level handler (which owns the `main_window` `WindowRef`) to
/// toggle OS fullscreen.
#[derive(Clone, Debug)]
pub enum WindowFullscreenAction {
    /// Maps to `self.ui.window(cx, ids!(main_window)).fullscreen(cx)`.
    Enable,
    /// Maps to `self.ui.window(cx, ids!(main_window)).disable_fullscreen(cx)`.
    Disable,
}

// ============================================================================
// Widget
// ============================================================================

#[derive(Script, ScriptHook, Widget)]
pub struct VideoMessagePlayerModal {
    #[deref]
    view: View,

    /// Stamped on `show()`; identifies the inline player that opened
    /// this modal so `RoomScreen` can resume it on close.
    #[rust]
    origin_inline_uid: Option<WidgetUid>,
}

impl Widget for VideoMessagePlayerModal {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        self.widget_match_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl WidgetMatchEvent for VideoMessagePlayerModal {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions, _scope: &mut Scope) {
        // ---- Outer `Modal` dismissed (scrim / Escape / back-press) ----
        // Stop embedded playback. We MUST NOT emit `Close` here
        // (infinite feedback loop with the outer Modal).
        let dismissed_by_outer = actions
            .iter()
            .any(|a| matches!(a.downcast_ref(), Some(ModalAction::Dismissed)));
        if dismissed_by_outer {
            self.stop_embedded_playback(cx);
            return;
        }

        // ---- Close button: stop playback and emit `Close`. ----
        let close_button = self.view.button(cx, ids!(close_button));
        if close_button.clicked(actions) {
            self.stop_embedded_playback(cx);
            cx.action(VideoMessagePlayerModalAction::Close);
        }
    }
}

// ============================================================================
// Public methods (entry point + helpers)
// ============================================================================

impl VideoMessagePlayerModal {
    /// Populate the embedded `VideoMessagePlayer` with the loaded video
    /// path and start playback. Called by `RoomScreen` when it receives
    /// `VideoMessagePlayerModalAction::Open`.
    pub fn show(
        &mut self,
        cx: &mut Cx,
        origin_inline_uid: WidgetUid,
        source_url: PathBuf,
        blurhash: Option<String>,
        summary: VideoSummary,
    ) {
        self.origin_inline_uid = Some(origin_inline_uid);

        let inner = self.inner_player_ref(cx);
        inner.populate_from_loaded_url(cx, summary, source_url, blurhash);

        self.view
            .button(cx, ids!(close_button))
            .reset_hover(cx);

        self.view.redraw(cx);
    }

    /// Convenience accessor returning the embedded `VideoMessagePlayer`'s
    /// underlying `VideoRef`. Used by `RoomScreen` to call
    /// `.begin_playback()` and `.seek_to()` on the modal's video.
    pub fn robrix_video_ref(&self, cx: &mut Cx) -> VideoRef {
        self.inner_player_ref(cx).robrix_video(cx)
    }

    pub fn inner_player_ref(&self, cx: &mut Cx) -> VideoMessagePlayerRef {
        self.view.video_message_player(cx, ids!(inner_player))
    }

    /// `RoomScreen` calls this after `show()` to auto-start playback,
    /// and on close to halt it. The embedded `VideoMessagePlayer` owns
    /// its own playback state — we only forward to the inner `VideoRef`.
    pub fn set_playing(&mut self, cx: &mut Cx, playing: bool) {
        if playing {
            self.inner_player_ref(cx).robrix_video(cx).begin_playback(cx);
        } else {
            self.stop_embedded_playback(cx);
        }
    }

    fn stop_embedded_playback(&mut self, cx: &mut Cx) {
        let video = self.inner_player_ref(cx).robrix_video(cx);
        video.pause_playback(cx);
        video.stop_and_cleanup_resources(cx);
    }
}

// ============================================================================
// Ref API — mirrors the previous shape so `RoomScreen` integration
// (src/home/room_screen.rs) continues to compile without changes.
// ============================================================================

impl VideoMessagePlayerModalRef {
    pub fn show(
        &self,
        cx: &mut Cx,
        origin_inline_uid: WidgetUid,
        source_url: PathBuf,
        blurhash: Option<String>,
        summary: VideoSummary,
    ) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.show(cx, origin_inline_uid, source_url, blurhash, summary);
        }
    }

    pub fn robrix_video(&self, cx: &mut Cx) -> VideoRef {
        self.borrow()
            .map(|inner| inner.robrix_video_ref(cx))
            .unwrap_or_default()
    }

    pub fn set_playing(&self, cx: &mut Cx, playing: bool) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_playing(cx, playing);
        }
    }
}
