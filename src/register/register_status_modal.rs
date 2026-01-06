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
        align: {x: 0.5}

        <RoundedView> {
            // Same width as LoginStatusModal for consistency
            width: ((320+250)/2),
            height: Fit,
            flow: Down,
            align: {x: 0.5}
            padding: 25,
            spacing: 10,

            show_bg: true
            draw_bg: {
                color: #CCC
                border_radius: 3.0
            }

            <View> {
                width: Fill,
                height: Fit,
                flow: Right
                padding: {top: 0, bottom: 10}
                align: {x: 0.5, y: 0.0}

                title = <Label> {
                    text: "Registration Status"
                    draw_text: {
                        text_style: <TITLE_TEXT>{font_size: 13},
                        color: #000
                    }
                }
            }

            status = <Label> {
                width: Fill
                margin: {top: 5, bottom: 5}
                draw_text: {
                    text_style: <REGULAR_TEXT>{
                        font_size: 11.5,
                    },
                    color: #000
                    wrap: Word
                },
                text: "Registering account, please wait..."
            }

            <View> {
                width: Fill,
                height: Fit,
                flow: Right
                align: {x: 1.0}
                margin: {top: 10}

                cancel_button = <RobrixIconButton> {
                    align: {x: 0.5, y: 0.5}
                    width: Fit, height: Fit
                    padding: 12
                    draw_bg: {
                        color: (COLOR_ACTIVE_PRIMARY)
                    }
                    draw_text: {
                        color: (COLOR_PRIMARY)
                        text_style: <REGULAR_TEXT> {}
                    }
                    text: "Cancel"
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
        // Check if modal was dismissed (ESC key or click outside)
        let modal_dismissed = actions
            .iter()
            .any(|a| matches!(a.downcast_ref(), Some(ModalAction::Dismissed)));

        // Check if Abort button was clicked
        let abort_clicked = self.view.button(ids!(cancel_button)).clicked(actions);

        if abort_clicked || modal_dismissed {
            // Send action to close the modal with appropriate was_internal flag
            // Note: This doesn't actually cancel the registration request (still running in background)
            cx.action(RegisterStatusModalAction::Close {
                was_internal: abort_clicked 
            });
        }
    }
}

#[derive(Clone, DefaultNone, Debug)]
pub enum RegisterStatusModalAction {
    /// The modal requested to be closed
    Close {
        /// Whether the modal was closed by clicking an internal button (Abort)
        /// or being dismissed externally (ESC or click outside)
        was_internal: bool,
    },
    None,
}
