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

            button = <Button> {
                width: 40, height: 40,
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

            stop_button = <Button> {
                width: 40, height: 40,
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

// #[derive(Clone)]
// struct AudioPlayerData {
//     audio_data: Option<Arc<[u8]>>,
//     sink: Option<Arc<Sink>>,
//     audio_existing: bool,
//     inisialized: bool,
// }
#[derive(Debug, Clone, Copy, Default)]
enum Status {
    #[default] Stopping,
    Pausing,
    Playing,
}

#[derive(Live, Widget)]
pub struct AudioPlayer {
    #[deref] view: View,
    #[rust] audio_data: Option<Arc<[u8]>>,
    #[rust] sink: Option<Arc<Sink>>,
    #[rust] status: Status
}

impl Drop for AudioPlayer {
    fn drop(&mut self) {
        let _audio_data = self.audio_data.take();
        let _sink = self.sink.take();
        let _inisialized = false;
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
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        let button = self.view.button(id!(v.button));
        let stop_button = self.view.button(id!(v.stop_button));

        if button.clicked(actions) {
            let Some(sink) = self.sink.clone() else {log!("No sink, return"); return };

            submit_async_request(MatrixRequest::MediaHandle {
                sender: None,
                media_data: self.audio_data.clone(),
                widget_uid: self.widget_uid(),
                sink: Some(sink),
                on_handle:|audio_data, _sender, sink|{
                    let Some(sink) = sink else { return };
                    init_or_play_sink(audio_data, sink)
                }
            });

            let (playing, new_status) = match self.status {
                Status::Playing => {
                    (0., Status::Pausing)
                },
                Status::Pausing | Status::Stopping => {
                    (1., Status::Playing)
                }
            };

            self.view.button(id!(v.button)).apply_over(cx, live! {
                draw_bg: {
                    playing: (playing)
                }
            });
            self.status = new_status;
        }

        if stop_button.clicked(actions) {
            let Some(sink) = self.sink.clone() else { return };
            stop_sink(sink);
        }
    }
    // fn handle_audio_devices(&mut self, _cx: &mut Cx, _e:&AudioDevicesEvent) {
    //     log!("handle_audio_devices");
    //     let Ok((stream, stream_handle)) = OutputStream::try_default() else { return };
    //     Box::leak(Box::new(stream));
    //     let Ok(sink) = Sink::try_new(&stream_handle) else { return };
    //     self.sink = Some(Arc::new(sink));
    //     self.status = Status::Pausing;
    // }
}
impl LiveHook for AudioPlayer {
    fn after_new_from_doc(&mut self, _cx:&mut Cx) {
        log!("after_new_from_doc");
        let Ok((stream, stream_handle)) = OutputStream::try_default() else { return };
        Box::leak(Box::new(stream));
        let Ok(sink) = Sink::try_new(&stream_handle) else { return };
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

pub fn stop_sink(sink: Arc<Sink>) {
    sink.as_ref().stop();
}

pub fn pause_sink(sink: Arc<Sink>) {
    sink.as_ref().pause();
}

pub fn init_or_play_sink(audio_data: Option<Arc<[u8]>>, sink: Arc<Sink>) {
    if let Some(audio_data) = audio_data {
        sink.as_ref().stop();
        let cursor = Cursor::new(audio_data);
        let decoder = Decoder::new(cursor).unwrap();
        sink.as_ref().append(decoder);
        sink.as_ref().play();
    } else {
        sink.as_ref().play();
    }
}
