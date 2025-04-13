use makepad_widgets::*;

use super::audio_message_ui::AudioMessageUIAction;

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::styles::*;

    // width & height is 0 because it is just a template.
    pub AudioPlaybackWindow = {{AudioPlaybackWindow}} {
        debug: true
        visible: false,
        width: 120., height: 60.,
        flow: Right,
        info = <Label> {
            width: 90.,
            text: "[Querying current audio info...]"
            draw_text: {
                color: #0
                text_style: {
                    font_size: 10.
                }
            }
        }
        close_button = <RobrixCloseButton> {
            width: 30, height: 30
        }
    }
}

#[derive(Live, LiveHook, Widget)]
struct AudioPlaybackWindow {
    #[deref] view: View,
}

impl Widget for AudioPlaybackWindow {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.match_event(cx, event);
        self.view.handle_event(cx, event, scope);
    }
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl MatchEvent for AudioPlaybackWindow {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        let close_button = self.view.button(id!(close_button));

        if close_button.clicked(actions) {
            self.visible = false;
            self.redraw(cx);
        }
        for action in actions {
            if let Some(AudioMessageUIAction::ToPlaybackWindowAction(info)) = action.downcast_ref() {
                self.view.label(id!(info)).set_text(cx, info);
                self.view.visible = true;
                self.visible = true;
                self.redraw(cx);
                log!("22222222");
            }
        }
    }
}
