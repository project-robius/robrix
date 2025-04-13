//! Audio controller, which just manages audio playback.

use std::{collections::{hash_map::Entry, HashMap}, sync::{Arc, LazyLock, Mutex, RwLock}};
use makepad_widgets::*;
use matrix_sdk_ui::timeline::TimelineEventItemId;

use super::audio_message_ui::AudioMessageUIAction;

type Audios = HashMap<TimelineEventItemId, (Audio, Arc<Mutex<usize>>, Arc<Mutex<bool>>)>;

pub static AUDIO_SET: LazyLock<RwLock<Audios>> = LazyLock::new(||{
    RwLock::new(HashMap::new())
});

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::styles::*;
    use crate::shared::icon_button::*;

    // width & height is 0 because it is just a template.
    pub AudioController = {{AudioController}} {
        width: 0., height: 0.,
        visible: false,
    }
}

#[derive(Debug, Clone, DefaultNone)]
pub enum AudioControllerAction {
    UiToPause(TimelineEventItemId),
    None,
}

#[derive(Debug, Clone, Default)]
pub struct Audio {
    pub data: Arc<[u8]>,
    pub channels: u16,
    pub bit_depth: u16
}

#[derive(Debug, Clone, Default)]
pub enum Selected {
    Playing(TimelineEventItemId, usize),
    Paused(TimelineEventItemId, usize),
    #[default]
    None,
}

#[derive(Live, LiveHook, Widget)]
pub struct AudioController {
    #[deref] view: View,
    #[rust] audio: Arc<Mutex<Audio>>,
    #[rust] selected: Arc<Mutex<Selected>>,
}

impl Drop for AudioController {
    fn drop(&mut self) {
        // todo!()
    }
}

impl Widget for AudioController {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.match_event(cx, event);
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl MatchEvent for AudioController {
    fn handle_startup(&mut self, cx: &mut Cx) {
        let audio = self.audio.clone();
        let selected = self.selected.clone();
        cx.audio_output(0, move |_audio_info, output_buffer|{
            let mut selected_mg = selected.lock().unwrap();
            let audio_mg = audio.lock().unwrap();
            if let Selected::Playing(ref id, mut pos) = selected_mg.clone() {
                let audio = audio_mg.clone();
                let audio_data_len = audio.data.len();
                match (audio.channels, audio.bit_depth) {
                    (2, 16) => {
                        // stereo 16bit
                        output_buffer.zero();
                        let (left, right) = output_buffer.stereo_mut();
                        for i in 0..left.len() {
                            if pos + 4 < audio_data_len {
                                let left_i16 = i16::from_le_bytes([audio.data[pos], audio.data[pos + 1]]);
                                let right_i16 = i16::from_le_bytes([audio.data[pos + 2], audio.data[pos + 3]]);
                                left[i] = left_i16 as f32 / i16::MAX as f32;
                                right[i] = right_i16 as f32 / i16::MAX as f32;
                                pos += 4;
                                *selected_mg = Selected::Playing(id.clone(), pos);
                            } else {
                                break;
                            }
                        }
                    }
                    (2, 24) => {
                        // stereo 24bit
                        output_buffer.zero();
                            let (left, right) = output_buffer.stereo_mut();
                            for i in 0..left.len() {
                                if pos + 5 < audio_data_len {
                                    let left_i32 = i32::from_le_bytes([0, audio.data[pos], audio.data[pos + 1], audio.data[pos + 2]]);
                                    let right_i32 = i32::from_le_bytes([0, audio.data[pos + 3], audio.data[pos + 4], audio.data[pos + 5]]);
                                    left[i] = left_i32 as f32 / i32::MAX as f32;
                                    right[i] = right_i32 as f32 / i32::MAX as f32;
                                    pos += 6;
                                    *selected_mg = Selected::Playing(id.clone(), pos);
                                } else {
                                    break;
                                }
                            }
                    }
                    (2, 32) => {
                        // stereo 24bit
                        output_buffer.zero();
                            let (left, right) = output_buffer.stereo_mut();
                            for i in 0..left.len() {
                                if pos + 7 < audio_data_len {
                                    let left_i32 = i32::from_le_bytes([audio.data[pos], audio.data[pos + 1], audio.data[pos + 2], audio.data[pos + 3]]);
                                    let right_i32 = i32::from_le_bytes([audio.data[pos + 4], audio.data[pos + 5], audio.data[pos + 6], audio.data[pos + 7]]);
                                    left[i] = left_i32 as f32 / i32::MAX as f32;
                                    right[i] = right_i32 as f32 / i32::MAX as f32;
                                    pos += 8;
                                    *selected_mg = Selected::Playing(id.clone(), pos);
                                } else {
                                    break;
                                }
                            }
                    }
                    _ => { }
                }
                if pos + 8 > audio_data_len {
                    if let Some((_audio, _old_pos, old_playing_status)) = AUDIO_SET.read().unwrap().get(id) {
                        *old_playing_status.lock().unwrap() = false;
                    }
                    Cx::post_action(AudioControllerAction::UiToPause(id.clone()));
                    *selected_mg = Selected::None;
                }
            } else {
                output_buffer.zero();
            }
        });
    }

    fn handle_audio_devices(&mut self, cx: &mut Cx, e: &AudioDevicesEvent) {
        cx.use_audio_outputs(&e.default_output())
    }
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        for action in actions {
            match action.downcast_ref() {
                Some(AudioMessageUIAction::Play(new_id)) => {
                    let selected =self.selected.clone().lock().unwrap().clone();
                    match selected {
                        Selected::Playing(current_id, current_pos) => {
                            if current_id != new_id.clone() {
                                cx.action(AudioControllerAction::UiToPause(current_id.clone()));
                                if let Some((_audio, old_pos, old_playing_status)) = AUDIO_SET.read().unwrap().get(&current_id) {
                                    *old_pos.lock().unwrap() = current_pos;
                                    *old_playing_status.lock().unwrap() = false;
                                }
                                if let Some((audio, new_pos, new_playing_status)) = AUDIO_SET.read().unwrap().get(new_id) {
                                    *self.audio.lock().unwrap() = audio.clone();
                                    let new_pos_mg = new_pos.lock().unwrap();
                                    *new_playing_status.lock().unwrap() = true;
                                    *self.selected.lock().unwrap() = Selected::Playing(new_id.clone(), *new_pos_mg);
                                }
                            }
                        }
                        Selected::Paused(current_id, current_pos) => {
                            if &current_id == new_id {
                                *self.selected.lock().unwrap() = Selected::Playing(new_id.clone(), current_pos);
                            } else {
                                if let Some((_audio, old_pos, old_playing_status)) = AUDIO_SET.read().unwrap().get(&current_id) {
                                    *old_playing_status.lock().unwrap() = false;
                                    *old_pos.lock().unwrap() = current_pos;
                                }

                                if let Some((audio, new_pos, new_playing_status)) = AUDIO_SET.read().unwrap().get(new_id) {
                                    *self.audio.lock().unwrap() = audio.clone();
                                    let new_pos = *new_pos.lock().unwrap();
                                    *new_playing_status.lock().unwrap() = true;
                                    *self.selected.lock().unwrap() = Selected::Playing(new_id.clone(), new_pos);
                                }
                            }
                        }
                        Selected::None => {
                            if let Some((audio, _new_pos, new_playing_status)) = AUDIO_SET.read().unwrap().get(new_id) {
                                *new_playing_status.lock().unwrap() = true;
                                *self.audio.lock().unwrap() = audio.clone();
                                *self.selected.lock().unwrap() = Selected::Playing(new_id.clone(), 44);
                            }
                        }
                    }
                }
                Some(AudioMessageUIAction::Pause(new_id)) => {
                    let selected =self.selected.clone().lock().unwrap().clone();
                    if let Selected::Playing(current_id, current_pos) = selected {
                        if &current_id == new_id {
                            if let Some((_audio, old_pos, old_playing_status)) = AUDIO_SET.write().unwrap().get(&current_id) {
                                *old_playing_status.lock().unwrap() = false;
                                *old_pos.lock().unwrap() = current_pos;
                            }
                            *self.selected.lock().unwrap() = Selected::Paused(new_id.clone(), current_pos);
                        }
                    }
                }
                Some(AudioMessageUIAction::Stop(new_id)) => {
                    let selected =self.selected.clone().lock().unwrap().clone();
                    match selected {
                        Selected::Playing(current_id, _current_pos) => {
                            if &current_id == new_id {
                                if let Some((_audio, old_pos, old_playing_status)) = AUDIO_SET.write().unwrap().get(&current_id) {
                                    *old_playing_status.lock().unwrap() = false;
                                    *old_pos.lock().unwrap() = 44;
                                }
                                *self.selected.lock().unwrap() = Selected::None;
                            }
                        }
                        Selected::Paused(current_id, _current_pos) => {
                            if &current_id == new_id {
                                if let Some((_audio, old_pos, old_playing_status)) = AUDIO_SET.write().unwrap().get(&current_id) {
                                    *old_playing_status.lock().unwrap() = false;
                                    *old_pos.lock().unwrap() = 44;
                                }
                                *self.selected.lock().unwrap() = Selected::None;
                            }
                        }
                        _ => { }
                    }
                }
                _ => { }
            }
        }
    }
}

pub fn insert_new_audio_or_get(timeline_audio_event_item_id: &TimelineEventItemId, data: Arc<[u8]>, channels: u16, bit_depth: u16) -> (Audio, Arc<Mutex<usize>>, Arc<Mutex<bool>>) {
    match AUDIO_SET.write().unwrap().entry(timeline_audio_event_item_id.clone()) {
        Entry::Vacant(v) => {
            let audio = Audio {
                data,
                channels,
                bit_depth
            };
            let pos = Arc::new(Mutex::new(44));
            let is_playing = Arc::new(Mutex::new(false));
            v.insert((audio, pos, is_playing)).clone()
        }
        Entry::Occupied(o) => {
            o.get().clone()
        }
    }
}


pub fn parse_wav(data: &[u8]) -> Option<(u16, u16)> {
    // Check that the data length is sufficient, at least 44 bytes are required (standard WAV file header size)
    if data.len() < 44 {
        log!("Insufficient data length");
        return None;
    }

    // Check if the first 4 bytes are 'RIFF'.
    if &data[0..4] != b"RIFF" {
        log!("Not a `RIFF` file");
        return None;
    }

    // Check if bytes 8-11 are 'WAVE'.
    if &data[8..12] != b"WAVE" {
        log!("Not a `WAVE` file");
        return None;
    }

    // Check if bytes 12-15 are 'fmt'.
    if &data[12..16] != b"fmt " {
        log!("`fmt` block not found");
        return None;
    }

    // Read the size of the fmt block (bytes 16-19)
    let fmt_size = u32::from_le_bytes([data[16], data[17], data[18], data[19]]);
    if fmt_size < 16 {
        log!("`fmt` block size too small");
        return None;
    }

    // Read the audio format (bytes 20-21) and make sure it's PCM (value 1)
    let audio_format = u16::from_le_bytes([data[20], data[21]]);
    if audio_format != 1 {
        log!("Not a `PCM` file");
        return None;
    }

    // Extract the number of channels (bytes 22-23)
    let channels = u16::from_le_bytes([data[22], data[23]]);

    // Extract sampling rate (bytes 24-27)
    // let sample_rate = u32::from_le_bytes([data[24], data[25], data[26], data[27]]);

    // Extract bit depth (bytes 34-35)
    let bit_depth = u16::from_le_bytes([data[34], data[35]]);

    // Return the parsing result
    // Some((channels, sample_rate, bits_per_sample))
    Some((channels, bit_depth))
}
