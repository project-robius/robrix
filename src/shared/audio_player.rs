use std::{collections::HashMap, io::Cursor, sync::Arc};

use makepad_widgets::*;
use rodio::{cpal::Stream, Decoder, OutputStream, OutputStreamHandle, Sink};

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::styles::*;
    use crate::shared::icon_button::*;

    pub AudioPlayer = {{AudioPlayer}} {
        width: Fill, height: Fit,
        flow: Down,
        spacing: 20,

        audio_info = <Label> {
            text: ""
            draw_text: {
                color: #0
                text_style: {
                    font_size: 11.
                }
            }
        }

        v = <View> {
            height: Fit
            flow: Right

            play_button = <RobrixButton> {
                width: 40, height: 40,
                draw_bg: {
                    // Define a color here
                    color: #0
                    fn pixel(self) -> vec4 {
                        let sdf = Sdf2d::viewport(self.pos * self.rect_size);

                        let length = self.rect_size.x / 3.0;
                        let height = self.rect_size.y;
                        sdf.rect(0.0, 0.0, length, height);
                        sdf.rect(length * 2.0, 0.0, length, height);
                        sdf.fill(self.color);

                        return sdf.result
                    }
                }
            }

            pause_button = <RobrixButton> {
                width: 40, height: 40,
                draw_bg: {
                    // Define a color here
                    color: #0
                    fn pixel(self) -> vec4 {
                        let sdf = Sdf2d::viewport(self.pos * self.rect_size);

                        sdf.line_to(self.rect_size.x, self.rect_size.y / 2.0);
                        sdf.line_to(0., self.rect_size.y);
                        sdf.close_path();
                        sdf.fill(self.color);

                        return sdf.result
                    }
                }
            }
        }
    }
}

#[derive(Live, Widget, LiveHook)]
pub struct AudioPlayer {
    #[deref] view: View,
    #[rust] audio_data: Option<Arc<[u8]>>,
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
    fn handle_actions(&mut self, _cx: &mut Cx, actions: &Actions) {
        let play_button = self.view.button(id!(play_button));
        let pause_button = self.view.button(id!(pause_button));


        if play_button.clicked(actions) {

            let audio_data = self.audio_data.clone();

            std::thread::spawn(move ||{
                let Ok((stream, stream_handle)) = OutputStream::try_default() else { return };
                Box::leak(Box::new(stream));
                let Some(audio_data) = audio_data else { return };

                let cursor = Cursor::new(audio_data);

                let Ok(decoder) = Decoder::new(cursor) else { log!("Cannot get decoder"); return };

                let Ok(sink) = Sink::try_new(&stream_handle) else { log!("Cannot get sink"); return };

                log!("Ready to play");

                sink.append(decoder);
                sink.play();
            }).join().unwrap()
        }

        if pause_button.clicked(actions) {
            log!("pause_button clicked");
        }

        for action in actions {

        }
    }
}


impl AudioPlayer {
    fn set_data(&mut self, audio_data: Arc<[u8]>) {
        log!("INSERTED");
        self.audio_data = Some(audio_data);
    }
    fn play(&self) {

    }
    fn pause(&self) {

    }
}

impl AudioPlayerRef {
    pub fn set_data(&self, audio_data: Arc<[u8]>) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.set_data(audio_data);
    }
}
