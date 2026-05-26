//! A standalone widget for Matrix audio messages.
//!
//! Audio decoding is a thin port of
//! `/Users/alanpoon/Documents/rust/makepad/examples/media_player/src/decoder.rs`,
//! and the single-active mixer is a thin port of
//! `/Users/alanpoon/Documents/rust/makepad/examples/media_player/src/player.rs`.

#![allow(dead_code)]

use std::{
    fmt,
    io::Cursor,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex, OnceLock,
    },
    thread,
};

use bytesize::ByteSize;
use makepad_widgets::{
    makepad_platform::audio::{AudioBuffer, AudioInfo},
    *,
};
use matrix_sdk::{
    media::MediaFormat,
    ruma::{events::room::MediaSource, OwnedMxcUri},
};
use symphonia::{
    core::{
        audio::{AudioBufferRef, SampleBuffer},
        codecs::{DecoderOptions, CODEC_TYPE_NULL},
        errors::Error as SymphoniaError,
        formats::FormatOptions,
        io::MediaSourceStream,
        meta::MetadataOptions,
        probe::Hint,
    },
    default::{get_codecs, get_probe},
};

use crate::{
    event_preview::{format_mmss, infer_audio_extension, AudioSummary},
    media_cache::{MediaCache, MediaCacheEntry},
};

// ============================================================================
// Audio decoding
// ============================================================================

#[derive(Clone, Debug)]
pub struct DecodedPcm {
    pub sample_rate: u32,
    pub channels: usize,
    pub interleaved_samples: Vec<f32>,
}

impl DecodedPcm {
    pub fn frame_count(&self) -> usize {
        self.interleaved_samples.len() / self.channels
    }
}

#[derive(Debug)]
pub enum DecodeError {
    Probe(String),
    MissingTrack,
    UnsupportedTrack,
    Decode(String),
    Empty,
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Probe(err) => write!(f, "failed to probe audio: {err}"),
            Self::MissingTrack => write!(f, "no audio track found"),
            Self::UnsupportedTrack => write!(f, "audio track is missing required parameters"),
            Self::Decode(err) => write!(f, "failed to decode audio: {err}"),
            Self::Empty => write!(f, "decoded audio did not contain samples"),
        }
    }
}

impl std::error::Error for DecodeError {}

pub fn decode_audio(bytes: &[u8], hint_ext: &str) -> Result<DecodedPcm, DecodeError> {
    let cursor = Cursor::new(bytes.to_vec());
    let media_source = MediaSourceStream::new(Box::new(cursor), Default::default());
    let mut hint = Hint::new();
    hint.with_extension(hint_ext);

    let probed = get_probe()
        .format(
            &hint,
            media_source,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .map_err(|err| DecodeError::Probe(err.to_string()))?;

    let mut format = probed.format;
    let track = format
        .tracks()
        .iter()
        .find(|track| track.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or(DecodeError::MissingTrack)?;

    let track_id = track.id;

    let mut decoder = get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|err| DecodeError::Decode(err.to_string()))?;

    let mut sample_rate = track.codec_params.sample_rate.unwrap_or_default();
    let mut samples = Vec::new();
    let mut sample_buffer = None;

    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(SymphoniaError::IoError(err))
                if err.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(SymphoniaError::ResetRequired) => {
                return Err(DecodeError::Decode("decoder reset required".to_string()));
            }
            Err(err) => return Err(DecodeError::Decode(err.to_string())),
        };

        if packet.track_id() != track_id {
            continue;
        }

        let decoded = match decoder.decode(&packet) {
            Ok(decoded) => decoded,
            Err(SymphoniaError::DecodeError(_)) => continue,
            Err(err) => return Err(DecodeError::Decode(err.to_string())),
        };

        sample_rate = decoded.spec().rate;
        append_as_stereo(&decoded, &mut sample_buffer, &mut samples);
    }

    if samples.is_empty() {
        return Err(DecodeError::Empty);
    }

    Ok(DecodedPcm {
        sample_rate,
        channels: 2,
        interleaved_samples: samples,
    })
}

fn append_as_stereo(
    decoded: &AudioBufferRef<'_>,
    sample_buffer: &mut Option<SampleBuffer<f32>>,
    output: &mut Vec<f32>,
) {
    let spec = *decoded.spec();
    let duration = decoded.capacity() as u64;
    let buffer = sample_buffer.get_or_insert_with(|| SampleBuffer::<f32>::new(duration, spec));
    if buffer.capacity() < decoded.capacity() {
        *buffer = SampleBuffer::<f32>::new(duration, spec);
    }
    buffer.copy_interleaved_ref(decoded.clone());

    let channels = spec.channels.count();
    for frame in buffer.samples().chunks(channels) {
        let left = frame.first().copied().unwrap_or(0.0);
        let right = frame.get(1).copied().unwrap_or(left);
        output.push(left);
        output.push(right);
    }
}

// ============================================================================
// Single active audio-message playback controller
// ============================================================================

#[derive(Clone, Debug)]
pub struct PlayerState {
    pub cursor_frames: f64,
    pub playing: bool,
}

impl Default for PlayerState {
    fn default() -> Self {
        Self {
            cursor_frames: 0.0,
            playing: false,
        }
    }
}

#[derive(Clone, Debug)]
pub enum AudioPlaybackAction {
    ActiveTrackChanged { now_playing: WidgetUid },
}

type ActiveTrack = Option<(WidgetUid, Arc<DecodedPcm>, Arc<Mutex<PlayerState>>)>;

static ACTIVE_TRACK: OnceLock<Mutex<ActiveTrack>> = OnceLock::new();
static AUDIO_OUTPUT_REGISTERED: OnceLock<()> = OnceLock::new();
static AUDIO_OUTPUT_REGISTRATION_COUNT: AtomicUsize = AtomicUsize::new(0);

fn active_track() -> &'static Mutex<ActiveTrack> {
    ACTIVE_TRACK.get_or_init(|| Mutex::new(None))
}

pub fn set_active(
    cx: &mut Cx,
    uid: WidgetUid,
    decoded: Arc<DecodedPcm>,
    state: Arc<Mutex<PlayerState>>,
) {
    ensure_audio_output(cx);
    set_active_track(uid, decoded, state);
    Cx::post_action(AudioPlaybackAction::ActiveTrackChanged { now_playing: uid });
}

fn set_active_track(uid: WidgetUid, decoded: Arc<DecodedPcm>, state: Arc<Mutex<PlayerState>>) {
    *active_track().lock().unwrap() = Some((uid, decoded, state));
}

fn ensure_audio_output(cx: &mut Cx) {
    AUDIO_OUTPUT_REGISTERED.get_or_init(|| {
        AUDIO_OUTPUT_REGISTRATION_COUNT.fetch_add(1, Ordering::Relaxed);
        cx.audio_output(0, move |info, output| {
            output.zero();
            let Some((_uid, decoded, state)) = active_track()
                .lock()
                .ok()
                .and_then(|guard| guard.clone())
            else {
                return;
            };
            if let Ok(mut state) = state.lock() {
                mix_audio_output(&mut state, &decoded, info, output);
            }
        });
    });
}

pub fn audio_output_registered() -> bool {
    AUDIO_OUTPUT_REGISTERED.get().is_some()
}

pub fn audio_output_registration_count() -> usize {
    AUDIO_OUTPUT_REGISTRATION_COUNT.load(Ordering::Relaxed)
}

pub fn active_track_uid() -> Option<WidgetUid> {
    active_track()
        .lock()
        .unwrap()
        .as_ref()
        .map(|(uid, _, _)| *uid)
}

pub fn fill_audio_output(
    state: &mut PlayerState,
    source: &DecodedPcm,
    info: AudioInfo,
    output: &mut AudioBuffer,
) {
    output.zero();
    mix_audio_output(state, source, info, output);
}

pub fn mix_audio_output(
    state: &mut PlayerState,
    source: &DecodedPcm,
    info: AudioInfo,
    output: &mut AudioBuffer,
) {
    if !state.playing || source.interleaved_samples.is_empty() {
        return;
    }

    let output_frames = output.frame_count();
    let output_channels = output.channel_count();
    let source_frames = source.frame_count();
    if source_frames == 0 || output_channels == 0 {
        state.playing = false;
        state.cursor_frames = 0.0;
        return;
    }

    let src_step = source.sample_rate as f64 / info.sample_rate;

    for frame in 0..output_frames {
        let src_pos = state.cursor_frames;
        if src_pos >= source_frames as f64 {
            state.playing = false;
            state.cursor_frames = 0.0;
            break;
        }

        let src_idx = src_pos.floor() as usize;
        let frac = (src_pos - src_idx as f64) as f32;
        let next_idx = (src_idx + 1).min(source_frames - 1);

        let left = interpolate_sample(source, src_idx, next_idx, frac, 0);
        let right = interpolate_sample(source, src_idx, next_idx, frac, 1);

        output.data[frame] += left;
        if output_channels > 1 {
            output.data[frame + output_frames] += right;
        }
        for channel in 2..output_channels {
            output.data[channel * output_frames + frame] += 0.5 * (left + right);
        }

        state.cursor_frames += src_step;
    }

    if state.cursor_frames >= source_frames as f64 {
        state.playing = false;
        state.cursor_frames = 0.0;
    }
}

fn interpolate_sample(
    source: &DecodedPcm,
    src_idx: usize,
    next_idx: usize,
    frac: f32,
    channel: usize,
) -> f32 {
    let current = source.interleaved_samples[src_idx * 2 + channel];
    let next = source.interleaved_samples[next_idx * 2 + channel];
    current + (next - current) * frac
}

// ============================================================================
// AudioMessagePlayer widget
// ============================================================================

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.ICON_AUDIO_PLAY = crate_resource("self://resources/icons/play.svg")
    mod.widgets.ICON_AUDIO_PAUSE = crate_resource("self://resources/icons/pause.svg")

    mod.widgets.AudioMessagePlayer = #(AudioMessagePlayer::register_widget(vm)) {
        width: Fill { min: 240, max: 460 }
        height: Fit
        flow: Right
        spacing: 12
        padding: Inset{top: 10 bottom: 10 left: 12 right: 12}
        align: Align{y: 0.5}
        show_bg: true
        draw_bg +: {
            color: #xf3f4f6
            border_radius: 8.0
        }

        // Bug 1 (stuck play icon): DrawSvg::load_svg is a one-shot loader — once `svg_loaded`
        // is true it ignores any new `svg:` value. Runtime-swapping `draw_icon.svg` therefore
        // never re-parses, leaving the play icon visible forever. Workaround: stack two buttons
        // and toggle `.visible`, since each button keeps its statically-loaded SVG.
        //
        // Bug 2 (button turns white after click-then-leave): the plain Button has a focus
        // animator that latches `focus: 1.0` after click. With no `color_focus` override the
        // shader mixes toward the theme's near-white focus color. The focus animator override
        // below pins `focus` to 0.0 in both states — same trick RobrixIconButton uses
        // (see src/shared/icon_button.rs:22-40).
        play_button_container := View {
            width: 44
            height: 44
            flow: Right

            play_button := Button {
                width: Fill
                height: Fill
                text: ""
                spacing: 0
                padding: 0
                align: Align{x: 0.5 y: 0.5}
                icon_walk: Walk{width: 16 height: 16 margin: Inset{left: 2}}
                animator +: {
                    focus: {
                        default: @off
                        off: AnimatorState {
                            from: {all: Forward {duration: 0.0}}
                            apply: {
                                draw_bg: {focus: 0.0}
                                draw_text: {focus: 0.0}
                            }
                        }
                        on: AnimatorState {
                            from: {all: Forward {duration: 0.0}}
                            apply: {
                                draw_bg: {focus: 0.0}
                                draw_text: {focus: 0.0}
                            }
                        }
                    }
                }
                draw_icon +: {
                    svg: (mod.widgets.ICON_AUDIO_PLAY)
                    color: #xffffff
                }
                draw_bg +: {
                    border_radius: 5.0
                    color: #x111827
                    color_hover: #x374151
                    color_down: #x111827
                    color_disabled: #737A85
                }
            }

            pause_button := Button {
                width: Fill
                height: Fill
                visible: false
                text: ""
                spacing: 0
                padding: 0
                align: Align{x: 0.5 y: 0.5}
                icon_walk: Walk{width: 14 height: 16}
                animator +: {
                    focus: {
                        default: @off
                        off: AnimatorState {
                            from: {all: Forward {duration: 0.0}}
                            apply: {
                                draw_bg: {focus: 0.0}
                                draw_text: {focus: 0.0}
                            }
                        }
                        on: AnimatorState {
                            from: {all: Forward {duration: 0.0}}
                            apply: {
                                draw_bg: {focus: 0.0}
                                draw_text: {focus: 0.0}
                            }
                        }
                    }
                }
                draw_icon +: {
                    svg: (mod.widgets.ICON_AUDIO_PAUSE)
                    color: #xffffff
                }
                draw_bg +: {
                    border_radius: 5.0
                    color: #x111827
                    color_hover: #x374151
                    color_down: #x111827
                    color_disabled: #737A85
                }
            }
        }

        details := View {
            width: Fill
            height: Fit
            flow: Down
            spacing: 5

            filename_label := Label {
                width: Fill
                height: Fit
                flow: Flow.Right{wrap: true}
                draw_text +: {
                    text_style: MESSAGE_TEXT_STYLE { font_size: 12.0 }
                    color: #x111827
                }
            }

            subtitle_label := Label {
                width: Fill
                height: Fit
                flow: Flow.Right{wrap: true}
                draw_text +: {
                    text_style: MESSAGE_TEXT_STYLE { font_size: 10.0 }
                    color: #x6b7280
                }
            }

            scrubber_row := View {
                width: Fill
                height: Fit
                flow: Right
                spacing: 8
                align: Align{y: 0.5}

                slider := SliderMinimal {
                    width: Fill
                    height: 20
                    min: 0.0
                    max: 1.0
                    step: 0.0
                    default: 0.0
                    precision: 2
                    hover_actions_enabled: false
                    draw_bg +: {
                        offset_y: uniform(8.0)
                        handle_size: uniform(8.0)
                        val_color: #x111827
                        handle_color: #x111827
                    }
                }

                elapsed_label := Label {
                    width: 46
                    height: Fit
                    text: "00:00"
                    draw_text +: {
                        text_style: MESSAGE_TEXT_STYLE { font_size: 10.0 }
                        color: #x374151
                    }
                }
            }
        }
    }
}

#[derive(Script, Widget, ScriptHook)]
pub struct AudioMessagePlayer {
    #[deref]
    view: View,
    #[rust]
    filename: String,
    #[rust]
    total_duration_secs: Option<f64>,
    #[rust]
    total_size_bytes: Option<u64>,
    #[rust]
    mime: Option<String>,
    #[rust]
    media_source: Option<MediaSource>,
    #[rust]
    decoded: Option<Arc<DecodedPcm>>,
    #[rust(Arc::new(Mutex::new(PlayerState::default())))]
    state: Arc<Mutex<PlayerState>>,
    #[rust]
    decode_error: Option<String>,
    #[rust]
    decode_request: Option<DecodeRequestKey>,
    #[rust]
    slider_drag: Option<SliderDragState>,
    #[rust]
    next_frame: NextFrame,
}

impl Widget for AudioMessagePlayer {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if self.next_frame.is_event(event).is_some() {
            self.sync_progress_ui(cx);
            if self.is_playing() {
                self.next_frame = cx.new_next_frame();
            }
        }

        if let Event::Actions(actions) = event {
            for action in actions {
                if let Some(action) = action.downcast_ref::<AudioDecodeAction>() {
                    self.handle_decode_action(cx, action);
                }
                if let Some(AudioPlaybackAction::ActiveTrackChanged { now_playing }) =
                    action.downcast_ref::<AudioPlaybackAction>()
                {
                    if *now_playing != self.widget_uid() {
                        self.pause_for_other_track(cx);
                    }
                }
            }

            let play_button = self
                .view
                .button(cx, ids!(play_button_container.play_button));
            let pause_button = self
                .view
                .button(cx, ids!(play_button_container.pause_button));
            if play_button.clicked(actions) || pause_button.clicked(actions) {
                self.toggle_playback(cx);
            }

            let slider = self.view.slider(cx, ids!(details.scrubber_row.slider));
            if let Some(action) = actions.find_widget_action(slider.widget_uid()) {
                match action.cast() {
                    SliderAction::StartSlide => {
                        self.on_slider_start(cx, slider.value().unwrap_or(0.0))
                    }
                    SliderAction::Slide(value) | SliderAction::TextSlide(value) => {
                        self.on_slider_move(cx, value)
                    }
                    SliderAction::EndSlide(value) => self.on_slider_end(cx, value),
                    _ => {}
                }
            }
        }

        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl AudioMessagePlayer {
    fn populate_from_summary(
        &mut self,
        cx: &mut Cx,
        summary: AudioSummary,
        media_source: MediaSource,
        media_cache: &mut MediaCache,
    ) {
        let AudioSummary {
            filename,
            mime,
            duration_secs,
            size_bytes,
            caption_html: _caption_html,
        } = summary;
        self.filename = filename;
        self.total_duration_secs = duration_secs;
        self.total_size_bytes = size_bytes;
        self.mime = mime;
        self.media_source = Some(media_source.clone());
        self.decode_error = None;

        self.set_labels(cx);
        self.set_ready_enabled(cx, self.decoded.is_some());
        self.view(cx, ids!(details.scrubber_row))
            .set_visible(cx, true);

        match media_source {
            MediaSource::Plain(mxc_uri) => self.populate_from_mxc(cx, mxc_uri, media_cache),
            MediaSource::Encrypted(encrypted) => {
                self.show_decode_error(
                    cx,
                    format!("Encrypted audio is not supported yet: {:?}", encrypted.url),
                );
            }
        }
    }

    fn populate_from_mxc(
        &mut self,
        cx: &mut Cx,
        mxc_uri: OwnedMxcUri,
        media_cache: &mut MediaCache,
    ) {
        match media_cache.try_get_media_or_fetch(&mxc_uri, MediaFormat::File) {
            (MediaCacheEntry::Loaded(data), MediaFormat::File) => {
                self.begin_decode(cx, mxc_uri, data);
            }
            (MediaCacheEntry::Loaded(_), _) | (MediaCacheEntry::Requested, _) => {
                self.show_loading(cx);
            }
            (MediaCacheEntry::Failed(_), _) => {
                self.show_decode_error(cx, "Failed to fetch audio".to_string());
            }
        }
    }

    fn begin_decode(&mut self, cx: &mut Cx, mxc_uri: OwnedMxcUri, data: Arc<[u8]>) {
        let key = DecodeRequestKey {
            mxc_uri: mxc_uri.clone(),
            filename: self.filename.clone(),
        };
        if self.decoded.is_some() && self.decode_request.as_ref() == Some(&key) {
            self.set_ready_enabled(cx, true);
            return;
        }
        if self.decode_request.as_ref() == Some(&key) && self.decoded.is_none() {
            self.show_loading(cx);
            return;
        }

        self.decoded = None;
        self.decode_request = Some(key.clone());
        self.show_loading(cx);

        let uid = self.widget_uid();
        let hint = infer_audio_extension(&self.filename, self.mime.as_deref()).to_string();
        thread::spawn(move || {
            let action = match decode_audio(&data, &hint) {
                Ok(decoded) => AudioDecodeAction::DecodeReady {
                    uid,
                    key,
                    decoded: Arc::new(decoded),
                },
                Err(error) => AudioDecodeAction::DecodeFailed {
                    uid,
                    key,
                    error: error.to_string(),
                },
            };
            Cx::post_action(action);
        });
    }

    fn handle_decode_action(&mut self, cx: &mut Cx, action: &AudioDecodeAction) {
        match action {
            AudioDecodeAction::DecodeReady { uid, key, decoded }
                if *uid == self.widget_uid() && self.decode_request.as_ref() == Some(key) =>
            {
                self.decoded = Some(decoded.clone());
                self.decode_error = None;
                self.set_ready_enabled(cx, true);
                self.sync_progress_ui(cx);
            }
            AudioDecodeAction::DecodeFailed { uid, key, error }
                if *uid == self.widget_uid() && self.decode_request.as_ref() == Some(key) =>
            {
                self.decoded = None;
                self.show_decode_error(
                    cx,
                    if error.is_empty() {
                        "Unsupported audio format".to_string()
                    } else {
                        "Unsupported audio format".to_string()
                    },
                );
            }
            _ => {}
        }
    }

    fn toggle_playback(&mut self, cx: &mut Cx) {
        let Some(decoded) = self.decoded.clone() else {
            return;
        };
        let playing = {
            let mut state = self.state.lock().unwrap();
            if state.cursor_frames >= decoded.frame_count() as f64 {
                state.cursor_frames = 0.0;
            }
            state.playing = !state.playing;
            state.playing
        };
        if playing {
            set_active(cx, self.widget_uid(), decoded, self.state.clone());
            self.next_frame = cx.new_next_frame();
        }
        self.set_play_button(cx, playing);
        self.sync_progress_ui(cx);
    }

    fn on_slider_start(&mut self, cx: &mut Cx, normalized_pos: f64) {
        let Some(decoded) = self.decoded.as_ref() else {
            return;
        };
        let was_playing = {
            let mut state = self.state.lock().unwrap();
            let was_playing = state.playing;
            apply_slider_drag(
                &mut state,
                normalized_pos,
                decoded.frame_count(),
                DragPhase::Start { was_playing },
            );
            was_playing
        };
        self.slider_drag = Some(SliderDragState { was_playing });
        self.set_play_button(cx, false);
        self.sync_progress_ui(cx);
    }

    fn on_slider_move(&mut self, cx: &mut Cx, normalized_pos: f64) {
        let Some(decoded) = self.decoded.as_ref() else {
            return;
        };
        let mut state = self.state.lock().unwrap();
        apply_slider_drag(
            &mut state,
            normalized_pos,
            decoded.frame_count(),
            DragPhase::Move,
        );
        drop(state);
        self.sync_progress_ui(cx);
    }

    fn on_slider_end(&mut self, cx: &mut Cx, normalized_pos: f64) {
        let Some(decoded) = self.decoded.clone() else {
            return;
        };
        let was_playing = self.slider_drag.take().is_some_and(|drag| drag.was_playing);
        let playing = {
            let mut state = self.state.lock().unwrap();
            apply_slider_drag(
                &mut state,
                normalized_pos,
                decoded.frame_count(),
                DragPhase::End { was_playing },
            );
            state.playing
        };
        if playing {
            set_active(cx, self.widget_uid(), decoded, self.state.clone());
            self.next_frame = cx.new_next_frame();
        }
        self.set_play_button(cx, playing);
        self.sync_progress_ui(cx);
    }

    fn sync_progress_ui(&mut self, cx: &mut Cx) {
        let (cursor_frames, playing) = {
            let state = self.state.lock().unwrap();
            (state.cursor_frames, state.playing)
        };
        let duration = self
            .decoded
            .as_ref()
            .map(|decoded| decoded.frame_count() as f64 / decoded.sample_rate as f64)
            .or(self.total_duration_secs)
            .unwrap_or(0.0);
        let elapsed = self
            .decoded
            .as_ref()
            .map(|decoded| cursor_frames / decoded.sample_rate as f64)
            .unwrap_or(0.0);
        let normalized = if duration > 0.0 {
            (elapsed / duration).clamp(0.0, 1.0)
        } else {
            0.0
        };

        self.view
            .slider(cx, ids!(details.scrubber_row.slider))
            .set_value(cx, normalized);
        self.view
            .label(cx, ids!(details.scrubber_row.elapsed_label))
            .set_text(cx, &format_mmss(elapsed));
        self.set_play_button(cx, playing);
    }

    fn pause_for_other_track(&mut self, cx: &mut Cx) {
        if let Ok(mut state) = self.state.lock() {
            state.playing = false;
        }
        self.set_play_button(cx, false);
    }

    fn is_playing(&self) -> bool {
        self.state.lock().is_ok_and(|state| state.playing)
    }

    fn set_labels(&mut self, cx: &mut Cx) {
        self.view
            .label(cx, ids!(details.filename_label))
            .set_text(cx, &self.filename);
        self.view
            .label(cx, ids!(details.subtitle_label))
            .set_text(cx, &self.subtitle());
    }

    fn subtitle(&self) -> String {
        let duration = self
            .total_duration_secs
            .map(format_mmss)
            .unwrap_or_else(|| "00:00".to_string());
        match self.total_size_bytes {
            Some(size) => format!("{duration} ({})", ByteSize::b(size)),
            None => duration,
        }
    }

    fn set_ready_enabled(&mut self, cx: &mut Cx, ready: bool) {
        self.view
            .button(cx, ids!(play_button_container.play_button))
            .set_enabled(cx, ready);
        self.view
            .button(cx, ids!(play_button_container.pause_button))
            .set_enabled(cx, ready);
        self.view(cx, ids!(details.scrubber_row))
            .set_visible(cx, true);
        if !ready {
            self.view
                .label(cx, ids!(details.scrubber_row.elapsed_label))
                .set_text(cx, "00:00");
        }
    }

    fn show_loading(&mut self, cx: &mut Cx) {
        self.set_ready_enabled(cx, false);
        self.view
            .label(cx, ids!(details.subtitle_label))
            .set_text(cx, &self.subtitle());
    }

    fn show_decode_error(&mut self, cx: &mut Cx, error: String) {
        self.decode_error = Some(error);
        self.view
            .button(cx, ids!(play_button_container.play_button))
            .set_enabled(cx, false);
        self.view
            .button(cx, ids!(play_button_container.pause_button))
            .set_enabled(cx, false);
        self.view(cx, ids!(details.scrubber_row))
            .set_visible(cx, false);
        self.view
            .label(cx, ids!(details.subtitle_label))
            .set_text(cx, "Unsupported audio format");
    }

    fn set_play_button(&mut self, cx: &mut Cx, playing: bool) {
        // Toggle visibility on the stacked play/pause buttons. Mutating
        // `draw_icon.svg` at runtime does not work — DrawSvg::load_svg is
        // a one-shot loader and ignores subsequent svg changes.
        self.view
            .button(cx, ids!(play_button_container.play_button))
            .set_visible(cx, !playing);
        self.view
            .button(cx, ids!(play_button_container.pause_button))
            .set_visible(cx, playing);
    }
}

impl AudioMessagePlayerRef {
    pub(crate) fn populate_from_summary(
        &self,
        cx: &mut Cx,
        summary: AudioSummary,
        media_source: MediaSource,
        media_cache: &mut MediaCache,
    ) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.populate_from_summary(cx, summary, media_source, media_cache);
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct DecodeRequestKey {
    mxc_uri: OwnedMxcUri,
    filename: String,
}

#[derive(Clone, Debug)]
enum AudioDecodeAction {
    DecodeReady {
        uid: WidgetUid,
        key: DecodeRequestKey,
        decoded: Arc<DecodedPcm>,
    },
    DecodeFailed {
        uid: WidgetUid,
        key: DecodeRequestKey,
        error: String,
    },
}

#[derive(Clone, Copy, Debug)]
pub struct SliderDragState {
    pub was_playing: bool,
}

#[derive(Clone, Copy, Debug)]
pub enum DragPhase {
    Start { was_playing: bool },
    Move,
    End { was_playing: bool },
}

pub fn apply_slider_drag(
    state: &mut PlayerState,
    normalized_pos: f64,
    source_frames: usize,
    phase: DragPhase,
) {
    let normalized_pos = normalized_pos.clamp(0.0, 1.0);
    state.cursor_frames = normalized_pos * source_frames as f64;
    match phase {
        DragPhase::Start { .. } | DragPhase::Move => {
            state.playing = false;
        }
        DragPhase::End { was_playing } => {
            state.playing = was_playing && state.cursor_frames < source_frames as f64;
        }
    }
}
