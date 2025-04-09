//! Audio message in timeline.
//! It just manages UI.
//! Audio playback is in `audio_controller.rs`.

use makepad_widgets::*;

use super::audio_controller::AudioControllerAction;

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::styles::*;
    use crate::shared::icon_button::*;

    pub AudioMessageUI = {{AudioMessageUI}} {
        width: Fill, height: Fit,
        flow: Down,

        fetching_info = <Label> {
            text: "[Fetching audio info...]"
            draw_text: {
                color: #0
                text_style: {
                    font_size: 11.
                }
            }
        }

        fetching_data = <View> {
            height: 35
            <Label> {
                text: "[Fetching audio data...]"
                draw_text: {
                    color: #0
                    text_style: {
                        font_size: 11.
                    }
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

#[derive(Debug, Clone, DefaultNone)]
pub enum AudioMessageUIAction {
    Play(WidgetUid),
    Stop(WidgetUid),
    Pause(WidgetUid),
    None
}

#[derive(Live, Widget, LiveHook)]
pub struct AudioMessageUI {
    #[deref] view: View,
    #[rust(false)] is_playing: bool,
}

// impl Drop for AudioMessageUI {
//     fn drop(&mut self) {
//         // todo!()
//     }
// }


impl Widget for AudioMessageUI {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.match_event(cx, event);
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl MatchEvent for AudioMessageUI {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        let button = self.view.button(id!(v.button));
        let stop_button = self.view.button(id!(v.stop_button));

        if button.clicked(actions) {
            let is_playing = if self.is_playing {
                cx.action(AudioMessageUIAction::Pause(self.widget_uid()));
                0.
            } else {
                cx.action(AudioMessageUIAction::Play(self.widget_uid()));
                1.
            };
            self.is_playing = !self.is_playing;
            button.apply_over(cx, live! {
                draw_bg: {
                    playing: (is_playing)
                }
            });
        }

        if stop_button.clicked(actions) {
            cx.action(AudioMessageUIAction::Stop(self.widget_uid()));
            self.is_playing = false;
            button.apply_over(cx, live! {
                draw_bg: {
                    playing: 0.
                }
            });
        }
        for action in actions {
            if let Some(AudioControllerAction::UiToPause(uid)) = action.downcast_ref() {
                if *uid == self.widget_uid() {
                    self.is_playing = false;
                    button.apply_over(cx, live! {
                        draw_bg: {
                            playing: 0.
                        }
                    });
                }
            }
        }
    }
}

impl AudioMessageUI {
    fn mark_fully_fetched(&mut self, cx: &mut Cx) {
        self.view(id!(fetching_data)).set_visible(cx, false);
        self.view(id!(v)).set_visible(cx, true);
    }
}

impl AudioMessageUIRef {
    pub fn mark_fully_fetched(&self, cx: &mut Cx) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.mark_fully_fetched(cx);
    }
}
