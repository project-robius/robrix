use std::{collections::HashMap, sync::{Arc, LazyLock, Mutex, RwLock}};
use makepad_widgets::*;

use crate::shared::audio_message_interface::AudioMessageInterfaceAction;

#[derive(Debug, Clone, Default)]
pub struct Audio {
    pub data: Arc<[u8]>,
    pub channels: u16,
    pub bit_depth: u16
}

type Audios = HashMap<WidgetUid, (Audio, Arc<Mutex<usize>>)>;

pub static AUDIO_SET: LazyLock<RwLock<Audios>> = LazyLock::new(||{
    RwLock::new(HashMap::new())
});

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::styles::*;
    use crate::shared::icon_button::*;

    pub AudioPlayer = {{AudioPlayer}} {
        width: 0., height: 0.,
        visible: false,
    }
}

#[derive(Debug, Clone, Default)]
pub enum Selected {
    Playing(WidgetUid, usize),
    Paused(WidgetUid, usize),
    #[default]
    None,
}

#[derive(Live, LiveHook, Widget)]
pub struct AudioPlayer {
    #[deref] view: View,
    #[rust] audio: Arc<Mutex<Audio>>,
    #[rust] selected: Arc<Mutex<Selected>>,
}

impl Drop for AudioPlayer {
    fn drop(&mut self) {
        // todo!()
    }
}

impl Widget for AudioPlayer {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.match_event(cx, event);
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl MatchEvent for AudioPlayer {
    fn handle_startup(&mut self, cx: &mut Cx) {
        let audio = self.audio.clone();
        let selected = self.selected.clone();
        let mut pos = 44;
        cx.audio_output(0, move |_audio_info, output_buffer|{
            let mut selected_mg = selected.lock().unwrap();
            let audio_mg = audio.lock().unwrap();
            if let Selected::Playing(uid, _) = selected_mg.clone() {
                let audio = audio_mg.clone();
                match (audio.channels, audio.bit_depth) {
                    (2, 16) => {
                        // stereo 16bit
                        output_buffer.zero();
                        let (left, right) = output_buffer.stereo_mut();
                        let mut i = 0;
                        while i < left.len() {
                            let left_i16 = i16::from_le_bytes([audio.data[pos], audio.data[pos + 1]]);
                            let right_i16 = i16::from_le_bytes([audio.data[pos + 2], audio.data[pos + 3]]);

                            left[i] = left_i16 as f32 / i16::MAX as f32;
                            right[i] = right_i16 as f32 / i16::MAX as f32;
                            *selected_mg = Selected::Playing(uid, pos + 4);
                            pos += 4;
                            i += 1;
                        }
                    }
                    (2, 24) => {
                        // stereo 24bit
                        output_buffer.zero();
                            let (left, right) = output_buffer.stereo_mut();
                            let mut i = 0;
                            while i < left.len() {
                                let left_i32 = i32::from_le_bytes([0, audio.data[pos], audio.data[pos + 1], audio.data[pos + 2]]);
                                let right_i32 = i32::from_le_bytes([0, audio.data[pos + 3], audio.data[pos + 4], audio.data[pos + 5]]);

                                left[i] = left_i32 as f32 / i32::MAX as f32;
                                right[i] = right_i32 as f32 / i32::MAX as f32;
                                *selected_mg = Selected::Playing(uid, pos + 6);
                                pos += 6;
                                i += 1;
                            }
                    }
                    _ => { }
                }
                if pos > audio.data.len() {
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
    fn handle_actions(&mut self, _cx: &mut Cx, actions: &Actions) {
        for action in actions {
            match action.downcast_ref() {
                Some(AudioMessageInterfaceAction::Play(new_uid)) => {
                    let selected =self.selected.clone().lock().unwrap().clone();
                    match selected {
                        Selected::Playing(current_uid, current_pos) => {
                            if &current_uid != new_uid {
                                if let Some((_, old_pos)) = AUDIO_SET.write().unwrap().get(&current_uid) {
                                    *old_pos.lock().unwrap() = current_pos;
                                }
                                if let Some((audio, _)) = AUDIO_SET.read().unwrap().get(new_uid) {
                                    *self.audio.lock().unwrap() = audio.clone();
                                    *self.selected.lock().unwrap() = Selected::Playing(*new_uid, 44);
                                }
                            }
                        }
                        Selected::Paused(current_uid, current_pos) => {
                            if let Some((_, old_pos)) = AUDIO_SET.write().unwrap().get_mut(&current_uid) {
                                *old_pos.lock().unwrap() = current_pos;
                            }

                            if let Some((audio, _)) = AUDIO_SET.read().unwrap().get(new_uid) {
                                *self.audio.lock().unwrap() = audio.clone();
                                *self.selected.lock().unwrap() = Selected::Playing(*new_uid, 44);
                            }
                        }
                        Selected::None => {
                            if let Some((audio, _old_pos)) = AUDIO_SET.write().unwrap().get_mut(new_uid) {
                                *self.selected.lock().unwrap() = Selected::Playing(*new_uid, 44);
                                *self.audio.lock().unwrap() = audio.clone();
                            }
                        }
                    }
                }
                Some(AudioMessageInterfaceAction::Pause(new_uid)) => {
                    let selected =self.selected.clone().lock().unwrap().clone();
                    if let Selected::Playing(current_uid, current_pos) = selected {
                        if &current_uid == new_uid {
                            if let Some((_, old_pos)) = AUDIO_SET.write().unwrap().get(&current_uid) {
                                *old_pos.lock().unwrap() = current_pos;
                            }
                            *self.selected.lock().unwrap() = Selected::Paused(*new_uid, current_pos);
                        }
                    }
                }
                Some(AudioMessageInterfaceAction::Stop(new_uid)) => {
                    let selected =self.selected.clone().lock().unwrap().clone();
                    match selected {
                        Selected::Playing(current_uid, _current_pos) => {
                            if &current_uid == new_uid {
                                if let Some((_, old_pos)) = AUDIO_SET.write().unwrap().get(&current_uid) {
                                    *old_pos.lock().unwrap() = 44;
                                }
                                *self.selected.lock().unwrap() = Selected::None;
                            }
                        }
                        Selected::Paused(current_uid, _current_pos) => {
                            if &current_uid == new_uid {
                                if let Some((_, old_pos)) = AUDIO_SET.write().unwrap().get(&current_uid) {
                                    *old_pos.lock().unwrap() = 44;
                                }
                            }
                            *self.selected.lock().unwrap() = Selected::None;
                        }
                        _ => { }
                    }
                }
                _ => { }
            }
        }
    }
}

pub fn insert_new_audio(audio_control_interface_uid: WidgetUid, data: Arc<[u8]>, channels: &u16, bit_depth: &u16) {
    let audio = Audio {
        data,
        channels: *channels,
        bit_depth: *bit_depth
    };
    let pos = Arc::new(Mutex::new(44));
    AUDIO_SET.write().unwrap().insert(audio_control_interface_uid, (audio, pos));
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
