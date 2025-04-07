use std::{collections::HashMap, sync::{Arc, LazyLock, RwLock}};
use makepad_widgets::*;

use crate::shared::audio_message_interface::AudioMessageInterfaceAction;

#[derive(Debug, Clone)]
pub struct Audio {
    pub data: Arc<[u8]>,
    pub pos: usize,
}

type Audios = HashMap<WidgetUid, Audio>;

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
#[derive(Debug, Clone, Copy, Default)]
enum Status {
    #[default] Stopping,
    Playing(WidgetUid),
}

#[derive(Live, LiveHook, Widget)]
pub struct AudioPlayer {
    #[deref] view: View,
    #[rust] audio: Option<Audio>,
    #[rust] status: Status
}

impl Drop for AudioPlayer {
    fn drop(&mut self) {
        todo!()
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
        cx.audio_output(0, |_audio_info, audio_buffer|{

        });
    }

    fn handle_audio_devices(&mut self, cx: &mut Cx, devices: &AudioDevicesEvent) {
        cx.use_audio_outputs(&devices.default_output())
    }

    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        for action in actions {
            if let Some(AudioMessageInterfaceAction::Play(audio_message_interface_uid)) = action.downcast_ref() {
                AUDIO_SET.read().unwrap().get(audio_message_interface_uid).inspect(|audio|{
                    self.audio = Some(audio.clone().clone());
                });
            }
        }
    }
}

pub fn insert_new_audio(audio_control_interface_uid: WidgetUid, audio_data: Arc<[u8]>) {
    let audio = Audio {
        data: audio_data,
        pos: 0
    };
    AUDIO_SET.write().unwrap().insert(audio_control_interface_uid, audio);
}
