//! A native speech-to-text dictation session, scoped to a single text input.

use std::cell::{Cell, RefCell};
use std::sync::{Arc, atomic::{AtomicU32, Ordering}, mpsc::{self, Receiver}};
use std::time::{Duration, Instant};
use robius_speech::{Dictation, NativeSpeechEvent, NativeSpeechOptions, NativeSpeechSession, SpeechError, SpeechErrorKind};
use makepad_widgets::{thread::SignalToUI, *};

/// This is the default "sentinel" value that indicates there is no `level` value yet.
///
/// This indicates no microphone soundwave level should be drawn, because a real
/// level value normalized (0.0..=1.0) and stored as bit values in a u32.
const NO_LEVEL: u32 = u32::MAX;

/// How long to wait for recording to actually begin. Long, because the platform
/// may be showing a permission prompt that the user hasn't answered yet.
const START_TIMEOUT: Duration = Duration::from_secs(120);

/// How long to wait for the last transcript after asking the recognizer to stop.
const FINISH_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Clone, Copy, PartialEq)]
pub(super) enum SpeechPhase {
    Starting,
    Listening,
    Finishing,
}

pub(super) struct SpeechInput {
    session: Option<NativeSpeechSession>,
    receiver: RefCell<Option<Receiver<NativeSpeechEvent>>>,
    cancelled: Cell<bool>,
    /// The latest microphone level the recognizer has reported that we haven't drawn yet.
    ///
    /// This is wrapped in an `Arc` so it can be shared with the recognizer's own OS thread.
    latest_level: Arc<AtomicU32>,
    pub phase: SpeechPhase,
    phase_since: Instant,
    /// Tracks where dictated words go in the text input, and which ones are already there.
    pub dictation: Dictation,
    /// True once the recognizer has stopped. We keep the session around until its
    /// last words have been added to the text input, then drop it.
    pub ended: bool,
    pub levels: [f32; 3],
}

impl SpeechInput {
    fn new(input: &TextInputRef, receiver: Option<Receiver<NativeSpeechEvent>>, phase: SpeechPhase) -> Self {
        let selection = input.selection();
        Self {
            session: None,
            receiver: RefCell::new(receiver),
            cancelled: Cell::new(false),
            latest_level: Arc::new(AtomicU32::new(NO_LEVEL)),
            phase,
            phase_since: Instant::now(),
            dictation: Dictation::new(&input.text(), selection.start().index..selection.end().index),
            ended: false,
            levels: [0.0; 3],
        }
    }

    pub fn start(input: &TextInputRef) -> Result<Self, SpeechError> {
        let mut this = Self::new(input, None, SpeechPhase::Starting);
        this.start_native()?;
        Ok(this)
    }

    fn start_native(&mut self) -> Result<(), SpeechError> {
        let (sender, receiver) = mpsc::channel();
        let signal = SignalToUI::new();
        let level = self.latest_level.clone();
        self.session = Some(NativeSpeechSession::start(NativeSpeechOptions::default(), move |event| {
            if let NativeSpeechEvent::AudioLevel(value) = event {
                // Only draw the latest level value, don't queue them up.
                level.store(value.clamp(0.0, 1.0).to_bits(), Ordering::Relaxed);
                signal.set();
            } else if sender.send(event).is_ok() {
                signal.set();
            }
        })?);
        *self.receiver.get_mut() = Some(receiver);
        Ok(())
    }

    pub fn stop(&mut self) {
        if let Some(session) = &self.session {
            session.stop();
        }
        self.phase = SpeechPhase::Finishing;
        self.phase_since = Instant::now();
    }

    pub fn cancel(&self) {
        self.cancelled.set(true);
        self.receiver.borrow_mut().take();
        if let Some(session) = &self.session {
            session.cancel();
        }
    }

    pub fn poll(&mut self) -> Vec<NativeSpeechEvent> {
        if self.ended { return Vec::new(); }
        if self.cancelled.get() { return vec![NativeSpeechEvent::Stopped]; }
        let reported_level = self.latest_level.swap(NO_LEVEL, Ordering::Relaxed);
        if reported_level != NO_LEVEL {
            let level = f32::from_bits(reported_level);
            // A bar's amplitude should fall gradually rather than snap down,
            // so quiet speech still looks like smooth soundwave movement.
            self.levels = [self.levels[1], self.levels[2], level.max(self.levels[2] * 0.65)];
        }
        let mut events = Vec::new();
        if let Some(receiver) = self.receiver.get_mut() {
            loop {
                match receiver.try_recv() {
                    Ok(event) => events.push(event),
                    // App-wide cancellation drops the native callback sender, so the channel gets dc'd.
                    Err(mpsc::TryRecvError::Disconnected) => {
                        events.push(NativeSpeechEvent::Stopped);
                        break;
                    }
                    Err(mpsc::TryRecvError::Empty) => break,
                }
            }
        }
        for event in &events {
            if matches!(event, NativeSpeechEvent::Started) && self.phase == SpeechPhase::Starting {
                self.phase = SpeechPhase::Listening;
                self.phase_since = Instant::now();
            }
        }
        // A backstop for a platform that never sends a terminal event.
        let timeout = match self.phase {
            SpeechPhase::Starting => Some(START_TIMEOUT),
            SpeechPhase::Finishing => Some(FINISH_TIMEOUT),
            // The user can stay quiet for as long as they like.
            SpeechPhase::Listening => None,
        };
        if events.is_empty() && timeout.is_some_and(|timeout| self.phase_since.elapsed() > timeout) {
            return vec![NativeSpeechEvent::Error(SpeechError::new(
                SpeechErrorKind::Other,
                "Speech input timed out. Any words already transcribed were kept; please try again.",
            ))];
        }
        events
    }
}
