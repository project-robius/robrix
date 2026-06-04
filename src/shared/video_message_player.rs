//! Inline video message player widget.
//!
//! Owns the single platform video session per message (one Makepad `Video`
//! widget child) and the shared `Arc<Mutex<...>>` state handles that the
//! sibling `VideoMessagePlayerModal` reads and writes when maximised.
//!
//! Control logic (playable-mime detection, slider drag, mute/restore,
//! maximise toggle) is factored into pure functions
//! (`should_show_unplayable_overlay`, `apply_volume_action`, etc.) so it
//! can be unit-tested without a Makepad `Cx`.

use std::{
    path::PathBuf,
    sync::mpsc::Receiver,
    sync::{Arc, Mutex},
};

use makepad_widgets::*;
use matrix_sdk::ruma::{events::room::MediaSource, OwnedMxcUri};
use matrix_sdk::media::MediaFormat;

pub use crate::event_preview::VideoSummary;
use crate::{
    media_cache::{MediaCache, MediaCacheEntry},
    shared::video_message_player_modal::VideoMessagePlayerModalAction,
    utils,
};

// Blurhash / placeholder helpers — previously lived in the now-deleted
// `shared::robrix_video` module. Kept here because this is the only
// caller after that wrapper widget was inlined into raw `Video`.

pub fn cap_blurhash_dimensions(width: u32, height: u32, max: u32) -> (u32, u32) {
    if width == 0 || height == 0 {
        return (0, 0);
    }
    if width <= max && height <= max {
        return (width, height);
    }
    let aspect_ratio = width as f32 / height as f32;
    if height > max && aspect_ratio <= 16.0 / 9.0 {
        return ((max as f32 * aspect_ratio).floor() as u32, max);
    }
    (max, (max as f32 / aspect_ratio).floor() as u32)
}

pub fn decode_blurhash_to_rgba(blurhash: &str, width: u32, height: u32) -> Option<Vec<u8>> {
    if blurhash.is_empty() || width == 0 || height == 0 {
        return None;
    }
    blurhash::decode(blurhash, width, height, 1.0).ok()
}

pub fn placeholder_fallback_color() -> [u8; 4] {
    [0x22, 0x22, 0x22, 0xFF]
}

// ============================================================================
// State types
// ============================================================================

#[derive(Clone, Copy, Debug, Default)]
pub struct VideoPlayerState {
    pub playing: bool,
    pub position_ms: u64,
}

#[derive(Clone, Copy, Debug)]
pub struct VideoVolumeState {
    pub muted: bool,
    pub level: f32,
    pub restore_level: f32,
}

impl Default for VideoVolumeState {
    fn default() -> Self {
        Self {
            muted: false,
            level: 0.8,
            restore_level: 0.8,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum VolumeAction {
    Mute,
    Unmute,
    SetLevel(f32),
}

// ============================================================================
// Shared state aliases — used by the modal widget.
// ============================================================================

pub type SharedPlayerState = Arc<Mutex<VideoPlayerState>>;
pub type SharedVolumeState = Arc<Mutex<VideoVolumeState>>;

/// `(width, height, rgba_pixels)` produced by the blurhash decoder worker
/// and shipped through a `Receiver` into the video player.
pub type BlurhashDecoded = (u32, u32, Vec<u8>);

// ============================================================================
// Cross-widget actions
// ============================================================================

#[derive(Clone, Debug)]
pub enum VideoPlaybackAction {
    /// Broadcast whenever a new video begins playback. Other video
    /// players observe this and pause themselves if their uid does not
    /// match — same "single-active track" model the audio player uses.
    ActiveTrackChanged {
        now_playing: WidgetUid,
    },
    ResumeInlineAfterModal {
        inline_uid: WidgetUid,
    },
}

// ============================================================================
// Pure helpers
// ============================================================================

/// Returns `true` for case-folded, parameter-stripped mime values the
/// platform decoder is known to handle.
pub fn is_playable_mime(mime: &str) -> bool {
    let normalized = mime
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    matches!(
        normalized.as_str(),
        "video/mp4" | "video/quicktime" | "video/x-m4v" | "video/webm" | "video/ogg"
    )
}

/// Whether the unplayable-overlay should be drawn. If the summary has
/// no mime we *optimistically* show the player and let the decoder
/// fail loudly during prepare, so this returns `false` in that case.
pub fn should_show_unplayable_overlay(summary: &VideoSummary) -> bool {
    match summary.mime.as_deref() {
        Some(mime) => !is_playable_mime(mime),
        None => false,
    }
}

pub fn infer_video_extension(filename: &str, mime: Option<&str>) -> &'static str {
    let from_filename = filename
        .rsplit_once('.')
        .map(|(_, ext)| ext.trim().to_ascii_lowercase())
        .and_then(|ext| match ext.as_str() {
            "mp4" | "m4v" | "mov" | "webm" | "ogv" | "ogg" => Some(ext),
            _ => None,
        });

    match from_filename.as_deref() {
        Some("mp4") => "mp4",
        Some("m4v") => "m4v",
        Some("mov") => "mov",
        Some("webm") => "webm",
        Some("ogv") => "ogv",
        Some("ogg") => "ogg",
        _ => mime.and_then(video_extension_from_mime).unwrap_or("mp4"),
    }
}

fn video_extension_from_mime(mime: &str) -> Option<&'static str> {
    match mime
        .to_ascii_lowercase()
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
    {
        "video/mp4" => Some("mp4"),
        "video/x-m4v" => Some("m4v"),
        "video/quicktime" => Some("mov"),
        "video/webm" => Some("webm"),
        "video/ogg" => Some("ogv"),
        _ => None,
    }
}

/// Apply a volume action to `VideoVolumeState`. Mute snapshots the
/// current level into `restore_level`; Unmute restores it with a 0.05
/// minimum guard so a previously-silent slider doesn't unmute to zero.
pub fn apply_volume_action(state: &mut VideoVolumeState, action: VolumeAction) {
    match action {
        VolumeAction::Mute => {
            state.restore_level = state.level;
            state.level = 0.0;
            state.muted = true;
        }
        VolumeAction::Unmute => {
            state.level = state.restore_level.max(0.05);
            state.muted = false;
        }
        VolumeAction::SetLevel(value) => {
            state.level = value.clamp(0.0, 1.0);
            state.muted = state.level == 0.0;
        }
    }
}

// ============================================================================
// Single-active broadcaster (mirrors the audio player's pattern)
// ============================================================================

static ACTIVE_VIDEO: Mutex<Option<WidgetUid>> = Mutex::new(None);

fn set_active_video(uid: WidgetUid) {
    if let Ok(mut guard) = ACTIVE_VIDEO.lock() {
        if guard.as_ref() != Some(&uid) {
            *guard = Some(uid);
            // Broadcast so the other players can pause themselves.
            Cx::post_action(VideoPlaybackAction::ActiveTrackChanged { now_playing: uid });
        }
    }
}

#[cfg(test)]
#[derive(Debug, PartialEq)]
enum PosterLayerDecision {
    SetPosterTexture,
    DecodeBlurhash { width: u32, height: u32 },
    SetSolidFallback([u8; 4]),
}

#[cfg(test)]
fn poster_layer_decision(
    entry: &MediaCacheEntry,
    blurhash: Option<&str>,
    dimensions: Option<(u32, u32)>,
) -> PosterLayerDecision {
    if matches!(entry, MediaCacheEntry::Loaded(_)) {
        return PosterLayerDecision::SetPosterTexture;
    }

    if let (Some(blurhash), Some((width, height))) = (blurhash, dimensions) {
        let (width, height) = cap_blurhash_dimensions(
            width,
            height,
            crate::home::room_screen::BLURHASH_IMAGE_MAX_SIZE,
        );
        if decode_blurhash_to_rgba(blurhash, width, height).is_some() {
            return PosterLayerDecision::DecodeBlurhash { width, height };
        }
    }

    PosterLayerDecision::SetSolidFallback(placeholder_fallback_color())
}

#[cfg(test)]
#[derive(Debug, PartialEq)]
enum VideoFileLayerDecision {
    SetSourceUrl(PathBuf),
    DisablePlay,
    SetInlineError(String),
}

#[cfg(test)]
fn video_file_layer_decision(
    entry: &MediaCacheEntry,
    format: &MediaFormat,
    mxc_uri: &OwnedMxcUri,
    source_path: PathBuf,
) -> VideoFileLayerDecision {
    match (entry, format) {
        (MediaCacheEntry::Loaded(_), MediaFormat::File) => {
            VideoFileLayerDecision::SetSourceUrl(source_path)
        }
        (MediaCacheEntry::Failed(status_code), _) => VideoFileLayerDecision::SetInlineError(
            format!("Failed to fetch video from {mxc_uri} (HTTP {status_code})"),
        ),
        _ => VideoFileLayerDecision::DisablePlay,
    }
}

// ============================================================================
// Live design
// ============================================================================

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.VIDEO_ICON_PLAY     = crate_resource("self://resources/icons/play.svg")
    mod.widgets.VIDEO_ICON_PAUSE    = crate_resource("self://resources/icons/pause.svg")
    mod.widgets.VIDEO_ICON_FORBIDDEN= crate_resource("self://resources/icons/forbidden.svg")
    mod.widgets.VIDEO_ICON_MAXIMISE = crate_resource("self://resources/icons/maximise.svg")
    mod.widgets.VIDEO_ICON_VOL_ON   = crate_resource("self://resources/icons/volume_on.svg")
    mod.widgets.VIDEO_ICON_VOL_OFF  = crate_resource("self://resources/icons/volume_off.svg")

    mod.widgets.VideoMessagePlayer = #(VideoMessagePlayer::register_widget(vm)) {
        width: Fill { min: 240, max: 520 }
        height: Fit
        flow: Down
        spacing: 6

        surface := View {
            width: Fill
            height: 292
            flow: Overlay
            show_bg: true
            draw_bg +: {
                color: #x111827
                border_radius: 8.0
            }

            robrix_video := Video {
                width: Fill
                height: Fill
                show_controls: true
                show_idle_thumbnail: true
            }

            unplayable_overlay := View {
                width: Fill
                height: Fill
                visible: false
                align: Align{x: 0.5, y: 0.5}
                forbidden_icon := Icon {
                    width: 48
                    height: 48
                    draw_icon +: {
                        svg: (mod.widgets.VIDEO_ICON_FORBIDDEN)
                        color: #xff4444
                    }
                    icon_walk: Walk{width: 48, height: 48}
                }
            }

            controls := View {
                width: Fill
                height: Fill
                flow: Overlay
                padding: 8

                maximise_button := Button {
                    width: 36
                    height: 36
                    text: ""
                    spacing: 0
                    padding: 0
                    align: Align{x: 0.5, y: 0.5}
                    icon_walk: Walk{width: 18, height: 18}
                    draw_icon +: {
                        svg: (mod.widgets.VIDEO_ICON_MAXIMISE)
                        color: #xffffff
                    }
                    draw_bg +: {
                        border_radius: 5.0
                        color: #x111827
                        color_hover: #x374151
                        color_down: #x111827
                    }
                }

                mute_button := Button {
                    width: 36
                    height: 36
                    margin: Inset{left: 99999}      // push to top-right edge
                    text: ""
                    spacing: 0
                    padding: 0
                    align: Align{x: 0.5, y: 0.5}
                    icon_walk: Walk{width: 18, height: 18}
                    draw_icon +: {
                        svg: (mod.widgets.VIDEO_ICON_VOL_ON)
                        color: #xffffff
                    }
                    draw_bg +: {
                        border_radius: 5.0
                        color: #x111827
                        color_hover: #x374151
                        color_down: #x111827
                        color_disabled: #x737A85
                    }
                }
            }
        }

        error_label := Label {
            width: Fill
            height: Fit
            visible: false
            flow: Flow.Right { wrap: true }
            draw_text +: { color: #xff4444 }
        }
    }
}

// ============================================================================
// Widget
// ============================================================================

#[derive(Script, Widget, ScriptHook)]
pub struct VideoMessagePlayer {
    #[deref]
    view: View,

    /// When false, the maximise button in the top-left of the controls
    /// overlay is hidden. The embedded `VideoMessagePlayer` inside
    /// `VideoMessagePlayerModal` sets this to false because it is
    /// already shown at its maximised size.
    #[live(true)]
    show_maximise_button: bool,

    // Per-message metadata.
    #[rust]
    summary: Option<VideoSummary>,
    #[rust]
    video_source: Option<MediaSource>,
    #[rust]
    poster_source: Option<MediaSource>,
    #[rust]
    loaded_video: Option<OwnedMxcUri>,
    #[rust]
    loaded_source_url: Option<PathBuf>,
    #[rust]
    loaded_poster: Option<OwnedMxcUri>,
    #[rust]
    poster_texture: Option<Texture>,
    #[rust]
    blurhash: Option<String>,
    #[rust]
    blurhash_dimensions: Option<(u32, u32)>,
    #[rust]
    blurhash_decode_key: Option<(String, u32, u32)>,
    #[rust]
    blurhash_texture_key: Option<(String, u32, u32)>,
    #[rust]
    blurhash_receiver: Option<Receiver<Option<BlurhashDecoded>>>,
    #[rust]
    play_enabled: bool,

    // Shared state — these Arcs are cloned and handed to the modal on
    // maximise so both views observe the same playback / volume / ui
    // state through `Arc<Mutex<...>>`.
    #[rust]
    player_state: SharedPlayerState,
    #[rust]
    volume_state: SharedVolumeState,

    #[rust]
    slider_drag_was_playing: Option<bool>,
}

impl Widget for VideoMessagePlayer {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if matches!(event, Event::Signal) {
            self.poll_blurhash_receiver(cx);
        }
        if let Event::Actions(actions) = event {
            for action in actions {
                if let Some(VideoPlaybackAction::ActiveTrackChanged { now_playing }) =
                    action.downcast_ref::<VideoPlaybackAction>()
                {
                    if *now_playing != self.widget_uid() {
                        self.pause_for_other_video(cx);
                    }
                }
                if let Some(VideoPlaybackAction::ResumeInlineAfterModal { inline_uid }) =
                    action.downcast_ref::<VideoPlaybackAction>()
                {
                    if *inline_uid == self.widget_uid() {
                        self.begin_inline_after_modal(cx);
                    }
                }
            }

            if self
                .view
                .button(cx, ids!(surface.controls.mute_button))
                .clicked(actions)
            {
                self.toggle_mute(cx);
            }

            if self
                .view
                .button(cx, ids!(surface.controls.maximise_button))
                .clicked(actions)
            {
                self.emit_maximise(cx);
            }
        }

        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

// ============================================================================
// Population + control helpers
// ============================================================================

impl VideoMessagePlayer {
    pub fn populate_from_summary(
        &mut self,
        cx: &mut Cx,
        summary: VideoSummary,
        video_source: MediaSource,
        poster_source: Option<MediaSource>,
        media_cache: &mut MediaCache,
    ) -> bool {
        self.populate_from_summary_and_blurhash(
            cx,
            summary,
            video_source,
            poster_source,
            None,
            None,
            media_cache,
        )
    }

    pub fn populate_from_summary_and_blurhash(
        &mut self,
        cx: &mut Cx,
        summary: VideoSummary,
        video_source: MediaSource,
        poster_source: Option<MediaSource>,
        blurhash: Option<String>,
        blurhash_dimensions: Option<(u32, u32)>,
        media_cache: &mut MediaCache,
    ) -> bool {
        self.summary = Some(summary);
        self.video_source = Some(video_source);
        self.poster_source = poster_source.or_else(|| self.video_source.clone());
        self.loaded_video = None;
        self.blurhash = blurhash;
        self.blurhash_dimensions = blurhash_dimensions;
        self.slider_drag_was_playing = None;

        self.apply_summary_state(cx);
        let poster_drawn = self.populate_poster(cx, media_cache);
        let video_drawn = self.is_unplayable() || self.ensure_video_loaded(cx, media_cache);
        if poster_drawn && !video_drawn {
            log!("set_thumbnail_texture[populate]: poster_drawn=true, video_drawn=false, has_texture={}", self.poster_texture.is_some());
            self.video_ref(cx)
                .set_thumbnail_texture(cx, self.poster_texture.clone());
        }
        self.sync_controls(cx);
        poster_drawn && video_drawn
    }

    /// Populate the player from a file path that has already been
    /// resolved by another player (typically the inline timeline
    /// player). Used by `VideoMessagePlayerModal`, which receives the
    /// loaded path through `VideoMessagePlayerModalAction::Open` and
    /// has no `MediaCache` to look up.
    pub fn populate_from_loaded_url(
        &mut self,
        cx: &mut Cx,
        summary: VideoSummary,
        source_url: PathBuf,
        blurhash: Option<String>,
    ) {
        self.summary = Some(summary);
        self.blurhash = blurhash;
        self.loaded_source_url = Some(source_url.clone());
        self.play_enabled = true;
        self.slider_drag_was_playing = None;

        let video = self.video_ref(cx);
        video.set_source(VideoDataSource::Filesystem {
            path: source_url.to_string_lossy().into_owned(),
        });
        video.should_dispatch_texture_updates(true);

        self.apply_summary_state(cx);
        self.sync_controls(cx);
    }

    fn apply_summary_state(&mut self, cx: &mut Cx) {
        let unplayable = self.is_unplayable();
        self.view(cx, ids!(surface.unplayable_overlay))
            .set_visible(cx, unplayable);
        self.view
            .button(cx, ids!(surface.controls.mute_button))
            .set_enabled(cx, !unplayable);
        self.view
            .button(cx, ids!(surface.controls.maximise_button))
            .set_visible(cx, self.show_maximise_button);
        self.view(cx, ids!(error_label)).set_visible(cx, false);
    }

    fn populate_poster(&mut self, cx: &mut Cx, media_cache: &mut MediaCache) -> bool {
        let Some(MediaSource::Plain(mxc_uri)) = self.poster_source.clone() else {
            self.apply_blurhash_or_fallback(cx);
            return true;
        };
        if self.loaded_poster.as_ref() == Some(&mxc_uri) {
            return true;
        }
        match media_cache.try_get_media_or_fetch(&MediaSource::Plain(mxc_uri.clone()), utils::MEDIA_THUMBNAIL_FORMAT.into()) {
            (MediaCacheEntry::Loaded(data), _) => {
                match crate::shared::image_viewer::get_png_or_jpg_image_buffer(data.to_vec()) {
                    Ok(image_buffer) => {
                        let texture = image_buffer.into_new_texture(cx);
                        log!("set_thumbnail_texture[populate_poster]: loaded poster from mxc={}", mxc_uri);
                        self.video_ref(cx)
                            .set_thumbnail_texture(cx, Some(texture.clone()));
                        self.poster_texture = Some(texture);
                        self.loaded_poster = Some(mxc_uri);
                        true
                    }
                    Err(_) => {
                        self.apply_blurhash_or_fallback(cx);
                        true
                    }
                }
            }
            (MediaCacheEntry::Requested, _) => {
                self.apply_blurhash_or_fallback(cx);
                false
            }
            (MediaCacheEntry::Failed(_), _) => {
                self.apply_blurhash_or_fallback(cx);
                true
            }
        }
    }

    fn ensure_video_loaded(&mut self, cx: &mut Cx, media_cache: &mut MediaCache) -> bool {
        let Some(MediaSource::Plain(mxc_uri)) = self.video_source.clone() else {
            self.show_error(cx, "Encrypted video is not supported yet.");
            return false;
        };
        if self.loaded_video.as_ref() == Some(&mxc_uri) {
            return true;
        }
        match media_cache.try_get_media_or_fetch(&MediaSource::Plain(mxc_uri.clone()), MediaFormat::File) {
            (MediaCacheEntry::Loaded(data), MediaFormat::File) => {
                let mut path = media_cache.path_for(&mxc_uri);
                if path.extension().is_none() {
                    if let Some(summary) = self.summary.as_ref() {
                        path.set_extension(infer_video_extension(
                            &summary.filename,
                            summary.mime.as_deref(),
                        ));
                    }
                }
                if let Err(error) = std::fs::write(&path, &data) {
                    self.show_error(cx, &format!("Failed to stage video file: {error}"));
                    self.set_play_enabled(cx, false);
                    return false;
                }
                let video = self.video_ref(cx);
                video.set_source(VideoDataSource::Filesystem {
                    path: path.to_string_lossy().into_owned(),
                });
                video.should_dispatch_texture_updates(true);
                self.loaded_source_url = Some(path);
                self.loaded_video = Some(mxc_uri);
                self.set_play_enabled(cx, true);
                self.view(cx, ids!(error_label)).set_visible(cx, false);
                true
            }
            (MediaCacheEntry::Requested, _) | (MediaCacheEntry::Loaded(_), _) => {
                self.set_play_enabled(cx, false);
                false
            }
            (MediaCacheEntry::Failed(status_code), _) => {
                self.set_play_enabled(cx, false);
                self.show_error(
                    cx,
                    &format!("Failed to fetch video from {mxc_uri} (HTTP {status_code})"),
                );
                true
            }
        }
    }

    fn pause_for_other_video(&mut self, cx: &mut Cx) {
        let was_playing = self
            .player_state
            .lock()
            .ok()
            .map(|g| g.playing)
            .unwrap_or(false);
        if was_playing {
            self.video_ref(cx).pause_playback(cx);
        }
        if let Ok(mut s) = self.player_state.lock() {
            s.playing = false;
        }
        self.sync_controls(cx);
    }

    fn toggle_mute(&mut self, cx: &mut Cx) {
        let new_muted = if let Ok(mut volume) = self.volume_state.lock() {
            let action = if volume.muted {
                VolumeAction::Unmute
            } else {
                VolumeAction::Mute
            };
            apply_volume_action(&mut volume, action);
            volume.muted
        } else {
            false
        };
        let _ = new_muted;
        if new_muted {
            self.video_ref(cx).mute_playback(cx);
        } else {
            self.video_ref(cx).unmute_playback(cx);
        }
        self.sync_controls(cx);
    }

    fn emit_maximise(&mut self, cx: &mut Cx) {
        let Some(summary) = self.summary.clone() else {
            return;
        };
        let Some(source_url) = self.loaded_source_url.clone() else {
            return;
        };
        let position_ms = self.video_ref(cx).current_position_ms() as u64;
        self.video_ref(cx).stop_and_cleanup_resources(cx);
        if let Ok(mut state) = self.player_state.lock() {
            state.playing = false;
        }
        self.sync_controls(cx);
        cx.action(VideoMessagePlayerModalAction::Open {
            inline_uid: self.widget_uid(),
            source_url,
            blurhash: self.blurhash.clone(),
            summary,
            position_ms,
        });
    }

    fn show_error(&mut self, cx: &mut Cx, text: &str) {
        self.view.label(cx, ids!(error_label)).set_text(cx, text);
        self.view(cx, ids!(error_label)).set_visible(cx, true);
        self.view(cx, ids!(surface.controls.slider_row))
            .set_visible(cx, false);
    }

    fn is_unplayable(&self) -> bool {
        self.summary
            .as_ref()
            .is_some_and(should_show_unplayable_overlay)
    }

    // No-op while Makepad's Video widget drives play/pause/slider state.
    // When custom controls are re-introduced, restore the previous body from
    // git history (it pulled state from `player_state` / `volume_state` and
    // pushed it to the play/pause buttons, slider, and elapsed label).
    fn sync_controls(&mut self, _cx: &mut Cx) {}

    fn set_play_enabled(&mut self, cx: &mut Cx, enabled: bool) {
        self.play_enabled = enabled;
        self.view
            .button(cx, ids!(surface.controls.center_controls.play_button))
            .set_enabled(cx, enabled);
        self.view
            .button(cx, ids!(surface.controls.center_controls.pause_button))
            .set_enabled(cx, enabled);
    }

    fn apply_blurhash_or_fallback(&mut self, cx: &mut Cx) {
        if let (Some(blurhash), Some((width, height))) =
            (self.blurhash.as_deref(), self.blurhash_dimensions)
        {
            let (width, height) = cap_blurhash_dimensions(
                width,
                height,
                crate::home::room_screen::BLURHASH_IMAGE_MAX_SIZE,
            );
            let key = (blurhash.to_string(), width, height);
            if self.blurhash_texture_key.as_ref() == Some(&key)
                || self.blurhash_decode_key.as_ref() == Some(&key)
            {
                return;
            }
            self.blurhash_decode_key = Some(key);
            let blurhash = blurhash.to_string();
            let (sender, receiver) = std::sync::mpsc::channel();
            self.blurhash_receiver = Some(receiver);
            cx.spawn_thread(move || {
                let result = decode_blurhash_to_rgba(&blurhash, width, height)
                    .map(|data| (width, height, data));
                let _ = sender.send(result);
                SignalToUI::set_ui_signal();
            });
            return;
        }
        let color = placeholder_fallback_color();
        let texture = ImageBuffer::new(&color, 1, 1)
            .ok()
            .map(|buf| buf.into_new_texture(cx));
        log!("set_thumbnail_texture[apply_blurhash_or_fallback]: fallback color, has_texture={}", texture.is_some());
        self.video_ref(cx).set_thumbnail_texture(cx, texture);
    }

    fn poll_blurhash_receiver(&mut self, cx: &mut Cx) {
        let Some(receiver) = self.blurhash_receiver.as_ref() else {
            return;
        };
        let Ok(result) = receiver.try_recv() else {
            return;
        };
        self.blurhash_receiver = None;
        match result {
            Some((width, height, data)) => {
                if let Ok(buffer) = ImageBuffer::new(&data, width as usize, height as usize) {
                    let texture = buffer.into_new_texture(cx);
                    log!("set_thumbnail_texture[poll_blurhash]: decoded blurhash {}x{}", width, height);
                    self.video_ref(cx).set_thumbnail_texture(cx, Some(texture));
                    self.blurhash_texture_key = self.blurhash_decode_key.take();
                }
            }
            None => {
                self.blurhash_decode_key = None;
                let color = placeholder_fallback_color();
                let texture = ImageBuffer::new(&color, 1, 1)
                    .ok()
                    .map(|buf| buf.into_new_texture(cx));
                log!("set_thumbnail_texture[poll_blurhash]: decode failed, fallback color, has_texture={}", texture.is_some());
                self.video_ref(cx).set_thumbnail_texture(cx, texture);
            }
        }
    }

    pub fn video_ref(&self, cx: &mut Cx) -> VideoRef {
        self.view.video(cx, ids!(surface.robrix_video))
    }

    pub fn loaded_source_url(&self) -> Option<PathBuf> {
        self.loaded_source_url.clone()
    }

    fn begin_inline_after_modal(&mut self, cx: &mut Cx) {
        self.video_ref(cx).begin_playback(cx);
        if let Ok(mut state) = self.player_state.lock() {
            state.playing = true;
            state.position_ms = 0;
        }
        self.sync_controls(cx);
        set_active_video(self.widget_uid());
    }
}

// ============================================================================
// Ref API
// ============================================================================

impl VideoMessagePlayerRef {
    pub fn populate_from_summary(
        &self,
        cx: &mut Cx,
        summary: VideoSummary,
        video_source: MediaSource,
        poster_source: Option<MediaSource>,
        media_cache: &mut MediaCache,
    ) -> bool {
        self.borrow_mut().is_some_and(|mut inner| {
            inner.populate_from_summary(cx, summary, video_source, poster_source, media_cache)
        })
    }

    pub fn populate_from_summary_and_blurhash(
        &self,
        cx: &mut Cx,
        summary: VideoSummary,
        video_source: MediaSource,
        poster_source: Option<MediaSource>,
        blurhash: Option<String>,
        blurhash_dimensions: Option<(u32, u32)>,
        media_cache: &mut MediaCache,
    ) -> bool {
        self.borrow_mut().is_some_and(|mut inner| {
            inner.populate_from_summary_and_blurhash(
                cx,
                summary,
                video_source,
                poster_source,
                blurhash,
                blurhash_dimensions,
                media_cache,
            )
        })
    }

    pub fn robrix_video(&self, cx: &mut Cx) -> VideoRef {
        self.borrow()
            .map(|inner| inner.video_ref(cx))
            .unwrap_or_default()
    }

    pub fn populate_from_loaded_url(
        &self,
        cx: &mut Cx,
        summary: VideoSummary,
        source_url: PathBuf,
        blurhash: Option<String>,
    ) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.populate_from_loaded_url(cx, summary, source_url, blurhash);
        }
    }

    pub fn set_play_button_text(&self, _cx: &mut Cx, _text: &str) {}
}
