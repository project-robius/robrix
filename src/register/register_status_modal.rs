use makepad_widgets::*;

live_design! {
    use link::theme::*;
    use link::widgets::*;

    use crate::shared::styles::*;
    use crate::shared::icon_button::RobrixIconButton;

    // A modal dialog that displays registration progress
    // Note: Matrix SDK's register function has a built-in 60s timeout
    pub RegisterStatusModal = {{RegisterStatusModal}} {
        width: Fit,
        height: Fit,

        <RoundedView> {
            width: 300,
            height: Fit,
            flow: Down,
            align: {x: 0.5},
            padding: 25,
            spacing: 15,

            show_bg: true,
            draw_bg: {
                color: #FFFFFF
            }

            status = <Label> {
                width: Fill,
                align: {x: 0.5},
                draw_text: {
                    text_style: <REGULAR_TEXT>{
                        font_size: 14,
                    },
                    color: (COLOR_TEXT),
                    wrap: Word
                },
                text: "Registering account, please wait..."
            }
            
            <View> {
                width: Fill,
                height: Fit,
                flow: Right,
                align: {x: 1.0},
                margin: {top: 10}
                
                cancel_button = <RobrixIconButton> {
                    width: Fit, height: Fit,
                    padding: 10,
                    draw_bg: {
                        color: (COLOR_ACTIVE_PRIMARY)
                    },
                    text: "Abort"
                    draw_text: {
                        color: (COLOR_PRIMARY)
                        text_style: <REGULAR_TEXT> {font_size: 12}
                    }
                }
            }
        }
    }
}

#[derive(Live, LiveHook, Widget)]
pub struct RegisterStatusModal {
    #[deref] view: View,
}

impl Widget for RegisterStatusModal {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        self.match_event(cx, event);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl MatchEvent for RegisterStatusModal {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        if self.view.button(id!(cancel_button)).clicked(actions) {
            // Send action to close the modal
            // Note: This doesn't actually cancel the registration request (still running in background)
            cx.action(RegisterStatusModalAction::Close);
        }
    }
}

#[derive(Clone, DefaultNone, Debug)]
pub enum RegisterStatusModalAction {
    Close,
    None,
}