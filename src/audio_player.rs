use std::{collections::HashMap, sync::{Arc, LazyLock, Mutex, RwLock}};
use makepad_widgets::*;

#[derive(Debug, Clone)]
pub struct Audio {
    pub data: Arc<[u8]>,
    pub pos: usize,
    pub channels: u16,
    pub bit_depth: u16
}

type Audios = HashMap<WidgetUid, (Arc<Mutex<Audio>>, bool)>;

pub static AUDIO_SET: LazyLock<RwLock<Audios>> = LazyLock::new(||{
    RwLock::new(HashMap::new())
});

pub static SHOULD_PLAY: LazyLock<RwLock<bool>> = LazyLock::new(||{
    RwLock::new(false)
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

#[derive(Live, LiveHook, Widget)]
pub struct AudioPlayer {
    #[deref] view: View,
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
        cx.audio_output(0, |_audio_info, output_buffer|{
            if *SHOULD_PLAY.read().unwrap() {
            log!("run again");
            AUDIO_SET.write().unwrap().iter_mut().for_each(|(audio_control_interface_uid, (audio, is_playing))|{
                if *is_playing {
                    let mut mg = audio.lock().unwrap();
                    log!("Play time: uid: {:?}, audio_data_len: {}", audio_control_interface_uid, mg.data.len());
                    let (channels, bit_depth) = (mg.channels, mg.bit_depth);
                    match (channels, bit_depth) {
                        (2, 16) => {
                            // stereo 16bit
                            output_buffer.zero();
                            let (left, right) = output_buffer.stereo_mut();
                            let mut i = 0;
                            while i < left.len() {
                                let left_i16 = i16::from_le_bytes([mg.data[mg.pos], mg.data[mg.pos + 1]]);
                                let right_i16 = i16::from_le_bytes([mg.data[mg.pos + 2], mg.data[mg.pos + 3]]);

                                left[i] = left_i16 as f32 / i16::MAX as f32;
                                right[i] = right_i16 as f32 / i16::MAX as f32;
                                mg.pos += 4;
                                i += 1;
                            }
                        }
                        (2, 24) => {
                            // stereo 24bit
                            output_buffer.zero();
                                let (left, right) = output_buffer.stereo_mut();
                                let mut i = 0;
                                while i < left.len() {
                                    let left_i32 = i32::from_le_bytes([0, mg.data[mg.pos], mg.data[mg.pos + 1], mg.data[mg.pos + 2]]);
                                    let right_i32 = i32::from_le_bytes([0, mg.data[mg.pos + 3], mg.data[mg.pos + 4], mg.data[mg.pos + 5]]);

                                    left[i] = left_i32 as f32 / i32::MAX as f32;
                                    right[i] = right_i32 as f32 / i32::MAX as f32;
                                    mg.pos += 6;
                                    i += 1;
                                }
                        }
                        _ => { }
                    }
                    *is_playing = false;
                }
            });
            }
        });
    }

    fn handle_audio_devices(&mut self, cx: &mut Cx, devices: &AudioDevicesEvent) {
        cx.use_audio_outputs(&devices.default_output())
    }
}

pub fn insert_new_audio(audio_control_interface_uid: WidgetUid, audio_data: Arc<[u8]>, channels: &u16, bit_depth: &u16) {
    log!("Insert time: uid: {:?}, audio_data_len: {}", audio_control_interface_uid, audio_data.len());
    let audio = Audio {
        data: audio_data,
        pos: 0,
        channels: *channels,
        bit_depth: *bit_depth
    };
    AUDIO_SET.write().unwrap().insert(audio_control_interface_uid, (Arc::new(Mutex::new(audio)), false));
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
