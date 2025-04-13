use makepad_widgets::*;

use super::audio_message_ui::AudioMessageUIAction;

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::styles::*;
    use crate::shared::icon_button::*;

    // width & height is 0 because it is just a template.
    pub AudioPlaybackWindow = {{AudioPlaybackWindow}} {
        visible: false,
        width: 100., height: 50.,
        flow: Down,
        info = <Label> {
            text: "[Querying current audio info...]"
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


#[derive(Live, LiveHook, Widget)]
struct AudioPlaybackWindow {
    #[deref] view: View
}

impl Widget for AudioPlaybackWindow {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
    }
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl MatchEvent for AudioPlaybackWindow {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        for action in actions {
            if let Some(AudioMessageUIAction::ShowPlayBackWindow(id)) = action.downcast_ref() {
                self.visible = true;
                self.redraw(cx);
            }
        }
    }
}
