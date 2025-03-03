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
            height: 35,
            flow: Right
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

        let Some(sink) = self.sink.clone() else {log!("No sink, return"); return };
        let Some(audio_data) = self.audio_data.clone() else {log!("No audio aata, return"); return };

        if button.clicked(actions) {
            let (media_handle_request, playing, new_status) = match self.status {
                Status::Playing => {
                    (MatrixRequest::MediaHandle {
                        sender: None,
                        media_data: None,
                        widget_uid: self.widget_uid(),
                        sink: Some(sink.clone()),
                        on_handle:|_audio_data, _sender, sink|{
                            let sink = sink.unwrap();
                            pause_sink(sink)
                        }
                    },
                    0.,
                    Status::Pausing
                    )
                },
                Status::Pausing => {
                    (MatrixRequest::MediaHandle {
                        sender: None,
                        media_data: None,
                        widget_uid: self.widget_uid(),
                        sink: Some(sink.clone()),
                        on_handle:|_audio_data, _sender, sink|{
                            let sink = sink.unwrap();
                            play_sink(sink)
                        }
                    },
                    1.,
                    Status::Playing
                    )
                },
                Status::Stopping => {
                    (MatrixRequest::MediaHandle {
                        sender: None,
                        media_data: Some(audio_data.clone()),
                        widget_uid: self.widget_uid(),
                        sink: Some(sink.clone()),
                        on_handle:|audio_data, _sender, sink|{
                            let sink = sink.unwrap();
                            let data = audio_data.unwrap();
                            init_and_play_sink(data, sink)
                        }
                    },
                    1.,
                    Status::Playing
                    )
                }
            };

            submit_async_request(media_handle_request);

            self.view.button(id!(v.button)).apply_over(cx, live! {
                draw_bg: {
                    playing: (playing)
                }
            });
            self.status = new_status;
        }

        if stop_button.clicked(actions) {
            stop_sink(sink.clone());

            self.view.button(id!(v.button)).apply_over(cx, live! {
                draw_bg: {
                    playing: 0.
                }
            });
            self.status = Status::Stopping;
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

pub fn play_sink(sink: Arc<Sink>) {
    sink.as_ref().play();
}

pub fn init_and_play_sink(audio_data: Arc<[u8]>, sink: Arc<Sink>) {
    sink.as_ref().stop();
    let cursor = Cursor::new(audio_data);
    let decoder = Decoder::new(cursor).unwrap();
    sink.as_ref().append(decoder);
    sink.as_ref().play();
}
