//! A text input with a microphone button inside it, for native speech-to-text dictation.
//!
//! Dictated words go into the text as ordinary undo-able edits, and the user can keep
//! typing, moving the caret, or composing with an IME while dictation is running.

use std::sync::{OnceLock, atomic::{AtomicBool, AtomicU64, Ordering}};
use makepad_widgets::{thread::SignalToUI, *};
use robius_speech::{NativeSpeechEvent, NativeSpeechSession, Replacement, SpeechErrorKind};
use crate::shared::popup_list::{PopupKind, enqueue_popup_notification};
use super::speech_input::{SpeechInput, SpeechPhase};

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    // Style the inner input via `text_input +: {..}`, but set its padding with `text_padding`;
    // while the microphone is shown, `mic_gutter` replaces its right padding to make room for it.
    mod.widgets.SpeechTextInput = #(SpeechTextInput::register_widget(vm)) {
        width: Fill, height: Fit
        flow: Overlay
        text_padding: 10
        mic_gutter: 40
        mic_clearance: 36
        scroll_bar_inset: Inset{top: 3, right: 4, bottom: 3}
        mic_tooltip: "Dictate"

        text_input := RobrixTextInput {}

        speech_overlay := View {
            width: Fill, height: Fill
            align: Align{x: 1.0, y: 1.0}
            // No vertical padding: the button is nearly as tall as a
            // single-line input, so any would push it past the bottom edge.
            padding: Inset{top: 0, bottom: 0, left: 4, right: 3}

            // One widget for both states: the icon at rest, a meter while recording.
            speech_button := RobrixIconButton {
                width: 32, height: 32
                padding: 7
                spacing: 0
                grab_key_focus: false
                enable_long_press: false
                icon_walk: Walk{width: 18, height: 18}
                draw_icon +: {
                    svg: crate_resource("self://resources/icons/microphone.svg")
                    color: #555
                }
                draw_bg +: {
                    color: #0000
                    color_hover: #xE0E8F0
                    color_down: #xD0D8E8
                    // Set from Rust: 1.0 while recording, plus the three
                    // most recent microphone levels, each normalized 0..=1.
                    recording: instance(0.0)
                    level_0: instance(0.0)
                    level_1: instance(0.0)
                    level_2: instance(0.0)
                    bar_color: instance(vec4(1.0, 1.0, 1.0, 1.0))
                    blend: fn(under: vec4, over: vec4) -> vec4 {
                        let alpha = over.w + under.w * (1.0 - over.w)
                        let color = over.xyz * over.w + under.xyz * under.w * (1.0 - over.w)
                        return vec4(color / max(alpha, 0.0001), alpha)
                    }
                    pixel: fn() {
                        let sdf = Sdf2d.viewport(self.pos * self.rect_size)
                        let hovered = vec4(self.color_hover.xyz, self.color_hover.w * self.hover)
                        let pressed = vec4(self.color_down.xyz, self.color_down.w * self.down)
                        let face = self.blend(self.blend(self.color, hovered), pressed)
                        sdf.box(0.5, 0.5, self.rect_size.x - 1.0, self.rect_size.y - 1.0, self.border_radius)
                        sdf.fill(face)
                        if self.recording > 0.5 {
                            // Three bars, centred in the button and scaled to its size.
                            let unit = self.rect_size.y / 32.0
                            let bar = 3.0 * unit
                            let gap = 3.0 * unit
                            let x = (self.rect_size.x - (bar * 3.0 + gap * 2.0)) * 0.5
                            let middle = self.rect_size.y * 0.5
                            let h0 = (4.0 + self.level_0 * 13.0) * unit
                            let h1 = (4.0 + self.level_1 * 13.0) * unit
                            let h2 = (4.0 + self.level_2 * 13.0) * unit
                            sdf.box(x, middle - h0 * 0.5, bar, h0, 1.5 * unit)
                            sdf.fill(self.bar_color)
                            sdf.box(x + bar + gap, middle - h1 * 0.5, bar, h1, 1.5 * unit)
                            sdf.fill(self.bar_color)
                            sdf.box(x + (bar + gap) * 2.0, middle - h2 * 0.5, bar, h2, 1.5 * unit)
                            sdf.fill(self.bar_color)
                        }
                        return sdf.result
                    }
                }
            }
        }
    }
}

// Only one session can run at a time, so these track that session across all `SpeechTextInput`s.
/// The widget UID of the `SpeechTextInput` whose session is starting or listening, or 0 if none.
static LISTENER: AtomicU64 = AtomicU64::new(0);
/// Whether a stopped session is still transcribing its last words.
static IS_FINISHING: AtomicBool = AtomicBool::new(false);
/// The time (as `f64` bits) of the `Escape` key press that last stopped dictation.
static STOPPING_ESCAPE: AtomicU64 = AtomicU64::new(u64::MAX);

/// Cancels all dictation, e.g., when navigation hides the text input being dictated into.
pub fn cancel_all_dictation() {
    robius_speech::cancel_all();
    // Wake every `SpeechTextInput` up so that it notices its session has ended.
    SignalToUI::set_ui_signal();
}

/// Whether the given `Escape` key press stops dictation, in which case nothing else should act on it.
///
/// This is for key *down* handlers; anything acting on the key's release, or on a text input's
/// `Escaped` action, should use [`escape_stopped_dictation()`] instead.
pub fn escape_stops_dictation(key: &KeyEvent) -> bool {
    LISTENER.load(Ordering::Relaxed) != 0 || STOPPING_ESCAPE.load(Ordering::Relaxed) == key.time.to_bits()
}

/// Whether the `Escape` key press being delivered now is the one that stopped dictation,
/// so that its release, and the `Escaped` action it produced, should be ignored too.
pub fn escape_stopped_dictation() -> bool {
    STOPPING_ESCAPE.load(Ordering::Relaxed) != u64::MAX
}

/// Whether this platform has native speech recognition, checked once per app run.
fn is_speech_supported() -> bool {
    static IS_SPEECH_SUPPORTED: OnceLock<bool> = OnceLock::new();
    *IS_SPEECH_SUPPORTED.get_or_init(|| {
        let supported = NativeSpeechSession::is_supported();
        log!("Native speech-to-text input is {} (engine: {}).",
            if supported { "available" } else { "unavailable" },
            robius_speech::engine_name(),
        );
        supported
    })
}

/// A text input with a microphone button for speech-to-text dictation.
///
/// Its inner `text_input` is an ordinary `TextInput`; check its actions via [`SpeechTextInputRef::text_input_ref()`].
#[derive(Script, Widget)]
pub struct SpeechTextInput {
    #[source] source: ScriptObjectRef,
    #[deref] view: View,

    /// The inner text input's padding, not counting the microphone's gutter.
    #[live] text_padding: Inset,
    /// The inner text input's right padding while the microphone button is shown.
    #[live] mic_gutter: f64,
    /// How far above the bottom edge the text input's scroll bar ends while the microphone is shown.
    #[live] mic_clearance: f64,
    /// Space between the text input's scroll bar and its borders; the microphone deepens the bottom.
    #[live] scroll_bar_inset: Inset,
    /// The microphone button's tooltip while no session is running.
    #[live] mic_tooltip: String,
    /// Whether to drop the sentence punctuation that recognizers end each transcript with,
    /// e.g., for search filters that match names.
    #[live] drop_trailing_punctuation: bool,

    /// The currently-running speech-to-text dictation session.
    #[rust] speech: Option<SpeechInput>,
    /// The phase shown by the microphone button, or `None` when no session is running.
    #[rust] shown_phase: Option<SpeechPhase>,
    /// The three most recent microphone levels shown in the button, oldest first.
    #[rust] shown_levels: Option<[f32; 3]>,
    /// Whether the recognizer turned out to be unavailable after all.
    #[rust] is_mic_hidden: bool,
    /// Whether this widget's session set [`IS_FINISHING`].
    #[rust] holds_finishing: bool,
}

impl ScriptHook for SpeechTextInput {
    fn on_after_apply(&mut self, vm: &mut ScriptVm, apply: &Apply, _scope: &mut Scope, _value: ScriptValue) {
        if apply.is_eval() || apply.is_animate() { return; }
        let show_mic = is_speech_supported() && !self.is_mic_hidden;
        vm.with_cx_mut(|cx| {
            self.show_mic(cx, show_mic);
            // Re-applying the DSL resets the button to its idle look, so restyle it for any running session.
            self.shown_phase = None;
            self.shown_levels = None;
            self.update_speech_controls(cx);
        });
    }
}

impl Drop for SpeechTextInput {
    fn drop(&mut self) {
        self.clear_finishing();
        let _ = LISTENER.compare_exchange(self.widget_uid().0, 0, Ordering::Relaxed, Ordering::Relaxed);
    }
}

impl Widget for SpeechTextInput {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        // Pressing `Escape` will stop speech recording/recognition regardless of key focus,
        // and the key press is kept from every text input, not just this one.
        if let Event::KeyDown(key) = event && key.key_code == KeyCode::Escape {
            if escape_stops_dictation(key) {
                STOPPING_ESCAPE.store(key.time.to_bits(), Ordering::Relaxed);
                match self.phase() {
                    Some(SpeechPhase::Listening) => self.stop(),
                    Some(SpeechPhase::Starting) => self.cancel(cx),
                    _ => {}
                }
                self.update_speech_controls(cx);
                return;
            }
            // This press isn't ours, so whatever acts on its release shouldn't ignore it.
            STOPPING_ESCAPE.store(u64::MAX, Ordering::Relaxed);
        }

        self.handle_speech_event(cx, event);
        self.view.handle_event(cx, event, scope);

        if let Event::Actions(actions) = event
            && self.speech_button().clicked(actions)
        {
            self.toggle(cx);
        }

        // Now that the text in the input is up to date (after handling this event),
        // we can add any dictated words to it.
        // But we still wait if an IME is composing, or the user is dragging a selection
        // since we don't want those two things to conflict.
        let Some(speech) = self.speech.as_mut() else { return };
        let input = self.view.child_by_path(ids!(text_input)).as_text_input();
        if input.is_composing() || cx.fingers.is_area_captured(input.area()) {
            speech.dictation.interrupt();
            return;
        }
        let selection = input.selection();
        if let Some(replacement) = speech.dictation.settle(&input.text(), selection.start().index..selection.end().index) {
            self.apply_replacement(cx, replacement);
        }
        if self.speech.as_ref().is_some_and(|speech| speech.ended) {
            self.cancel(cx);
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }

    fn text(&self) -> String {
        self.text_input_ref().text()
    }

    /// Replaces the text, which ends any dictation into it.
    fn set_text(&mut self, cx: &mut Cx, text: &str) {
        self.cancel(cx);
        self.text_input_ref().set_text(cx, text);
    }

    fn set_key_focus(&self, cx: &mut Cx) {
        self.text_input_ref().set_key_focus(cx);
    }

    fn key_focus(&self, cx: &Cx) -> bool {
        self.text_input_ref().key_focus(cx)
    }
}

impl SpeechTextInput {
    fn text_input_ref(&self) -> TextInputRef {
        self.view.child_by_path(ids!(text_input)).as_text_input()
    }

    fn speech_button(&self) -> ButtonRef {
        self.view.child_by_path(ids!(speech_button)).as_button()
    }

    /// Returns the phase of the current speech recognition session, if one is running.
    fn phase(&self) -> Option<SpeechPhase> {
        self.speech.as_ref().filter(|speech| !speech.ended).map(|speech| speech.phase)
    }

    /// Starts, stops, or cancels dictation, as a click on the microphone button does.
    fn toggle(&mut self, cx: &mut Cx) {
        match self.phase() {
            Some(SpeechPhase::Listening) => self.stop(),
            Some(SpeechPhase::Starting) => self.cancel(cx),
            // Its last words are still on their way, so don't throw them away.
            Some(SpeechPhase::Finishing) => {}
            None => {
                let input = self.text_input_ref();
                input.set_key_focus(cx);
                let started = match SpeechInput::start(&input) {
                    // Only one session can run at a time, so the newest microphone press wins,
                    // unless the other session has stopped recording and is just finishing up.
                    Err(error) if error.kind() == SpeechErrorKind::Busy && !IS_FINISHING.load(Ordering::Relaxed) => {
                        cancel_all_dictation();
                        SpeechInput::start(&input)
                    }
                    started => started,
                };
                match started {
                    Ok(speech) => self.speech = Some(speech),
                    Err(error) => enqueue_popup_notification(format!("Speech input: {error}"), PopupKind::Error, Some(8.0)),
                }
            }
        }
        self.update_speech_controls(cx);
    }

    /// Stops recording, but keeps the session until its last words have been transcribed.
    fn stop(&mut self) {
        if let Some(speech) = self.speech.as_mut() {
            speech.stop();
            IS_FINISHING.store(true, Ordering::Relaxed);
            self.holds_finishing = true;
        }
    }

    /// Stop and discard any pending speech-to-text callbacks (with recognition text results).
    ///
    /// Any already-displayed text will stay in the text input.
    fn cancel(&mut self, cx: &mut Cx) {
        self.speech = None;
        self.clear_finishing();
        self.update_speech_controls(cx);
    }

    /// Marks the session as ended, committing any words held back while the user was editing.
    fn end_session(&mut self, cx: &mut Cx) {
        let replacement = self.speech.as_mut().and_then(|speech| {
            speech.ended = true;
            speech.dictation.transcript("", true)
        });
        if let Some(replacement) = replacement {
            self.apply_replacement(cx, replacement);
        }
        self.clear_finishing();
    }

    fn clear_finishing(&mut self) {
        if std::mem::take(&mut self.holds_finishing) {
            IS_FINISHING.store(false, Ordering::Relaxed);
        }
    }

    /// Puts a dictated replacement text snippet into the text input.
    ///
    /// This goes into the text input as an ordinary series of undo-able text edits.
    fn apply_replacement(&mut self, cx: &mut Cx, replacement: Replacement) {
        let undo = if replacement.continues { text_input::UndoGroup::Extend } else { text_input::UndoGroup::New };
        match self.text_input_ref().replace_range(cx, replacement.range, &replacement.text, undo) {
            Ok(()) => {
                if let Some(speech) = self.speech.as_mut() {
                    speech.dictation.applied();
                }
            }
            // If the text replacement was refused due to a live IME composition or the input filter,
            // nothing gets lost, so no worries.
            // The next transcript will still include the same words that weren't applied.
            Err(error) => log!("Speech input could not update the text: {error:?}"),
        }
        self.redraw(cx);
    }

    /// Shows or hides the microphone button, and the gutter it needs.
    fn show_mic(&mut self, cx: &mut Cx, show: bool) {
        self.view.view(cx, ids!(speech_overlay)).set_visible(cx, show);
        let right = if show { self.mic_gutter } else { self.text_padding.right };
        let padding = Inset { right, ..self.text_padding };
        let mut scroll_bar_inset = self.scroll_bar_inset;
        if show {
            scroll_bar_inset.bottom = self.mic_clearance;
        }
        let mut input = self.text_input_ref();
        script_apply_eval!(cx, input, {padding: #(padding), scroll_bar_inset: #(scroll_bar_inset)});
    }

    fn update_speech_controls(&mut self, cx: &mut Cx) {
        let new_phase = self.phase();
        let uid = self.widget_uid().0;
        if matches!(new_phase, Some(SpeechPhase::Starting | SpeechPhase::Listening)) {
            LISTENER.store(uid, Ordering::Relaxed);
        } else {
            let _ = LISTENER.compare_exchange(uid, 0, Ordering::Relaxed, Ordering::Relaxed);
        }
        let levels = self.speech.as_ref().map(|speech| speech.levels);
        // Starting or ending a session completely re-styles the whole button;
        // a phase change only affects the tooltip, so only that needs redrawing.
        let is_active = new_phase.is_some();
        if is_active != self.shown_phase.is_some() {
            let bg = if is_active { vec4(0.08, 0.08, 0.08, 1.0) } else { vec4(0.0, 0.0, 0.0, 0.0) };
            let hover = if is_active { vec4(0.25, 0.25, 0.25, 1.0) } else { vec4(0.88, 0.91, 0.94, 1.0) };
            let down = if is_active { hover } else { vec4(0.82, 0.85, 0.91, 1.0) };
            // The soundwave animation replaces the icon rather than sitting beside it,
            // so we just hide the microphone icon while recording.
            let icon = if is_active { vec4(0.0, 0.0, 0.0, 0.0) } else { vec4(0.2, 0.2, 0.2, 1.0) };
            let recording = if is_active { 1.0 } else { 0.0 };
            let mut button = self.speech_button();
            script_apply_eval!(cx, button, {
                draw_icon +: {color: #(icon)}
                draw_bg +: {
                    color: #(bg), color_hover: #(hover), color_down: #(down),
                    recording: #(recording)
                }
            });
            self.redraw(cx);
        }
        if new_phase != self.shown_phase {
            self.shown_phase = new_phase;
            self.redraw(cx);
        }
        if levels != self.shown_levels {
            self.shown_levels = levels;
            if let Some(levels) = levels {
                let mut button = self.speech_button();
                script_apply_eval!(cx, button, {
                    draw_bg +: {level_0: #(levels[0]), level_1: #(levels[1]), level_2: #(levels[2])}
                });
            }
        }
    }

    fn handle_speech_event(&mut self, cx: &mut Cx, event: &Event) {
        // Leaving the app ends dictation. Typing in the text input does not.
        if self.speech.is_some() && matches!(event, Event::Background | Event::Shutdown) {
            self.cancel(cx);
        } else if let Some(speech) = self.speech.as_mut() {
            // This event may be about to change the text, and we read new
            // speech results below, before it gets there. So hold the words back.
            speech.dictation.interrupt();
        }
        let events = self.speech.as_mut().map(|speech| speech.poll()).unwrap_or_default();
        for event in events {
            match event {
                NativeSpeechEvent::Transcript { text, is_final } => {
                    let text = if self.drop_trailing_punctuation {
                        text.trim_end().trim_end_matches(['.', '!', '?', '。', '！', '？'])
                    } else {
                        &text
                    };
                    // Returns nothing while held: the keystroke hasn't reached the text
                    // input yet, so writing now would drag the caret out from under it.
                    let Some(speech) = self.speech.as_mut() else { break };
                    if let Some(replacement) = speech.dictation.transcript(text, is_final) {
                        self.apply_replacement(cx, replacement);
                    }
                }
                // The session is dropped once this event has been dispatched, so
                // a final utterance that arrived alongside its end still lands.
                NativeSpeechEvent::Stopped => {
                    self.end_session(cx);
                    break;
                }
                NativeSpeechEvent::Error(error) => {
                    self.end_session(cx);
                    // Only hide the microphone once the recognizer is really gone:
                    // Unavailable can also mean something retryable, like lost network.
                    if error.kind() == SpeechErrorKind::Unavailable
                        && !NativeSpeechSession::is_supported()
                    {
                        self.is_mic_hidden = true;
                        self.show_mic(cx, false);
                    }
                    enqueue_popup_notification(format!("Speech input: {error}"), PopupKind::Error, Some(8.0));
                    break;
                }
                _ => {}
            }
        }
        self.update_speech_controls(cx);

        let button = self.speech_button();
        let area = button.area();
        match event.hits(cx, area) {
            Hit::FingerHoverIn(_) => {
                cx.widget_action(button.widget_uid(), TooltipAction::HoverIn {
                    // Name the shortcut only while it does something, and name what
                    // it does: while finishing there is nothing left for it to stop.
                    text: match self.phase() {
                        Some(SpeechPhase::Listening) => "Stop speech input (Esc)".into(),
                        Some(SpeechPhase::Starting) => "Cancel speech input (Esc)".into(),
                        Some(SpeechPhase::Finishing) => "Finishing transcription…".into(),
                        None => self.mic_tooltip.clone(),
                    },
                    widget_rect: area.rect(cx),
                    options: CalloutTooltipOptions { position: TooltipPosition::Top, ..Default::default() },
                });
            }
            Hit::FingerHoverOut(_) => cx.widget_action(button.widget_uid(), TooltipAction::HoverOut),
            _ => {}
        }
    }
}

impl SpeechTextInputRef {
    /// Returns the inner text input, e.g., to check for its actions like `changed()` or `returned()`.
    pub fn text_input_ref(&self) -> TextInputRef {
        self.child_by_path(ids!(text_input)).as_text_input()
    }

    /// Stops dictation, discarding words not yet transcribed; words already in the text input stay.
    pub fn cancel_dictation(&self, cx: &mut Cx) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.cancel(cx);
        }
    }

    /// Like [`Self::cancel_dictation()`] for callers without a `Cx`:
    /// releases the microphone now, and cleans up upon the next event.
    pub fn release_microphone(&self) {
        if let Some(speech) = self.borrow().as_ref().and_then(|inner| inner.speech.as_ref()) {
            speech.cancel();
        }
    }

}
