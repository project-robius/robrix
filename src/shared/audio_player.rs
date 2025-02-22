use std::{io::Cursor, sync::Arc};

use makepad_widgets::*;
use rodio::{Decoder, OutputStream, Sink };

use crate::sliding_sync::{submit_async_request, MatrixRequest};

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::styles::*;
    use crate::shared::icon_button::*;

    pub AudioPlayer = {{AudioPlayer}} {
        width: Fill, height: Fit,
        flow: Down,

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
            spacing: 20,

            play_button = <RobrixButton> {
                width: 40, height: 40,
                draw_bg: {
                    // Define a color here
                    color: #1F1F1F
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

            pause_button = <RobrixButton> {
                width: 40, height: 40,
                draw_bg: {
                    color: #1F1F1F
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

            stop_button = <RobrixButton> {
                width: 40, height: 40,
                draw_bg: {
                    color: #0
                }
            }
        }
    }
}

// #[derive(Debug, Clone, Copy, DefaultNone)]
// pub enum AudioPlayerAction {
//     BeforeDrop(WidgetUid, )
//     None,
// }

// #[derive(Clone)]
// struct AudioPlayerData {
//     audio_data: Option<Arc<[u8]>>,
//     sink: Option<Arc<Sink>>,
//     audio_existing: bool,
//     inisialized: bool,
// }

#[derive(Live, Widget)]
pub struct AudioPlayer {
    #[deref] view: View,
    #[rust] audio_data: Option<Arc<[u8]>>,
    #[rust] sink: Option<Arc<Sink>>,
    #[rust(false)] audio_existing: bool,
    #[rust(false)] inisialized: bool,
}

impl Drop for AudioPlayer {
    fn drop(&mut self) {
        let _audio_data = self.audio_data.take();
        let _sink = self.sink.take();
        let _audio_existing = self.audio_existing;
        let _inisialized = false;
    }
}


impl Widget for AudioPlayer {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if !self.inisialized {
            log!("Start Init");
            let Ok((stream, stream_handle)) = OutputStream::try_default() else { return };
            Box::leak(Box::new(stream));
            let Ok(sink) = Sink::try_new(&stream_handle) else { return };
            log!("Success Init");
            self.sink = Some(Arc::new(sink));
            self.inisialized = true
        }

        self.match_event(cx, event);
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl MatchEvent for AudioPlayer {
    fn handle_actions(&mut self, _cx: &mut Cx, actions: &Actions) {
        let button_set = (id!(play_button), id!(pause_button), id!(stop_button));

        let (play_button, pause_button, stop_button) =
            (self.view.button(button_set.0), self.view.button(button_set.1),self.view.button(button_set.2));


        if play_button.clicked(actions) {
            let audio_data = self.audio_data.clone();

            if !self.audio_existing {
                submit_async_request(MatrixRequest::MediaHandle {
                    sender: None,
                    media_data: audio_data,
                    widget_uid: self.widget_uid(),
                    sink: self.sink.clone(),
                    on_handle:|audio_data, _sender, sink|{
                        let Some(sink) = sink else { return };
                        init_and_play_sink(audio_data, sink)
                    }
                });
                self.audio_existing = true
            } else {
                submit_async_request(MatrixRequest::MediaHandle {
                    sender: None,
                    media_data: audio_data,
                    widget_uid: self.widget_uid(),
                    sink: self.sink.clone(),
                    on_handle:|_audio_data, _sender, sink|{
                        let Some(sink) = sink.clone() else { return };
                        play_sink(sink)
                    }
                });
            }
        }

        if pause_button.clicked(actions) {
            submit_async_request(MatrixRequest::MediaHandle {
                sender: None,
                media_data: None,
                widget_uid: self.widget_uid(),
                sink: self.sink.clone(),
                on_handle:|_audio_data, _sender, sink|{
                    let Some(sink) = sink.clone() else { return };
                    pause_sink(sink)
                }
            });
        }

        if stop_button.clicked(actions) {
            let Some(sink) = self.sink.clone() else { return };
            stop_sink(sink);
            self.audio_existing = false;
        }
    }
}
impl LiveHook for AudioPlayer {
    fn after_new_from_doc(&mut self, _cx:&mut Cx) {
        log!("handle_startup called");
        let Ok((stream, stream_handle)) = OutputStream::try_default() else { return };
        Box::leak(Box::new(stream));
        let Ok(sink) = Sink::try_new(&stream_handle) else { return };
        log!("Success Init");
        self.sink = Some(Arc::new(sink));
    }
}


impl AudioPlayer {
    fn set_data(&mut self, audio_data: Arc<[u8]>) {
        self.audio_data = Some(audio_data);
    }
}

impl AudioPlayerRef {
    pub fn set_data(&self, audio_data: Arc<[u8]>) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.set_data(audio_data);
    }
}


pub fn play_sink(sink: Arc<Sink>) {
    sink.as_ref().play();
    // sink.as_ref().sleep_until_end();
}

pub fn stop_sink(sink: Arc<Sink>) {
    sink.as_ref().stop();
}

pub fn pause_sink(sink: Arc<Sink>) {
    sink.as_ref().pause();
}

pub fn init_and_play_sink(audio_data: Option<Arc<[u8]>>, sink: Arc<Sink>) {
    let Some(audio_data) = audio_data else { return };
    sink.as_ref().stop();
    let cursor = Cursor::new(audio_data);
    let decoder = Decoder::new(cursor).unwrap();
    log!("Ready to play");
    sink.as_ref().append(decoder);
    sink.as_ref().play();
}
