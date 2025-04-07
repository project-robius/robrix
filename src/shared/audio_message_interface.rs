//! A widget for controlling audio playback in timeline.
//! We only implement the interface here.
//! The true audio playback is in `src/audio_player.rs`

use makepad_widgets::*;
use matrix_sdk::ruma::events::room::message::AudioInfo;

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::styles::*;
    use crate::shared::icon_button::*;

    pub AudioMessageInterface = {{AudioMessageInterface}} {
        width: Fill, height: Fit,
        flow: Down,

        info = <Label> {
            text: "[Fetching Audio Info...]"
            draw_text: {
                color: #0
                text_style: {
                    font_size: 11.
                }
            }
        }

        v = <View> {
            height: 35,
            flow: Right,
            spacing: 20,

            button = <Button> {
                width: 35, height: Fill,
                draw_bg: {
                    // Define a color here
                    instance playing: 0.
                    instance color: #1F1F1F
                    fn pixel(self) -> vec4 {
                        let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                        if self.playing < 0.1 {
                            sdf.line_to(self.rect_size.x, self.rect_size.y / 2.0);
                            sdf.line_to(0., self.rect_size.y);
                            sdf.close_path();
                            sdf.fill(self.color);
                        } else {
                            let length = self.rect_size.x / 3.0;
                            let height = self.rect_size.y;
                            sdf.rect(0.0, 0.0, length, height);
                            sdf.rect(length * 2.0, 0.0, length, height);
                            sdf.fill(self.color);
                        }
                        return sdf.result
                    }
                }
            }
            <View> {
                width: 35, height: Fill,
                align: {x: 0.5, y: 0.5}
                stop_button = <Button> {
                    width: 32., height: 32.,
                    draw_bg: {
                        instance color: #1F1F1F
                        fn pixel(self) -> vec4 {
                            return self.color;
                        }
                    }
                }
            }
        }
    }
}
#[derive(Debug, Clone, Copy, Default)]
enum Status {
    #[default] Stopping,
    Pausing,
    Playing,
}

#[derive(Debug, Clone, DefaultNone)]
pub enum AudioMessageInterfaceAction {
    Play(WidgetUid),
    None
}

#[derive(Live, Widget, LiveHook)]
pub struct AudioMessageInterface {
    #[deref] view: View,
}

impl Drop for AudioMessageInterface {
    fn drop(&mut self) {

    }
}


impl Widget for AudioMessageInterface {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.match_event(cx, event);
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl MatchEvent for AudioMessageInterface {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        let button = self.view.button(id!(v.button));
        let stop_button = self.view.button(id!(v.stop_button));

        if button.clicked(actions) {

        }

        if stop_button.clicked(actions) {

        }
    }
}
