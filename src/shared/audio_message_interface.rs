//! A widget for controlling audio playback in timeline.
//! We only implement the interface here.
//! The true audio playback is in `src/audio_player.rs`

use makepad_widgets::*;

use crate::audio_player::{AUDIO_SET, SHOULD_PLAY};

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
            visible: false

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

#[derive(Live, Widget, LiveHook)]
pub struct AudioMessageInterface {
    #[deref] view: View,
    #[rust(false)] fully_fetched: bool,
    #[rust(false)] is_playing: bool,
}

impl Drop for AudioMessageInterface {
    fn drop(&mut self) {
        // todo!()
    }
}


impl Widget for AudioMessageInterface {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if self.fully_fetched {
            self.view(id!(v)).set_visible(cx, true);
        }
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
            let mut audio_set_wg = AUDIO_SET.write().unwrap();
            audio_set_wg.get_mut(&self.widget_uid()).unwrap().1 = !self.is_playing;

            let is_playing = if self.is_playing {
                0.
            } else {
                *SHOULD_PLAY.write().unwrap() = true;
                1.
            };

            button.apply_over(cx, live! {
                draw_bg: {
                    playing: (is_playing)
                }
            });
            self.is_playing = !self.is_playing;
        }

        if stop_button.clicked(actions) {
            let mut audio_set_wg = AUDIO_SET.write().unwrap();
            audio_set_wg.get_mut(&self.widget_uid()).unwrap().1 = false;
            self.is_playing = false;
        }
    }
}

impl AudioMessageInterface {
    fn mark_fully_fetched(&mut self) {
        self.fully_fetched = true
    }
}

impl AudioMessageInterfaceRef {
    pub fn mark_fully_fetched(&self) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.mark_fully_fetched()
    }
}
