use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*


    // A modal dialog that displays registration progress
    // Note: Matrix SDK's register function has a built-in 60s timeout
    mod.widgets.RegisterStatusModal = #(RegisterStatusModal::register_widget(vm)) {
        width: Fit,
        height: Fit,
        align: Align{x: 0.5}

        RoundedView {
            // Keep parity with LoginStatusModal width by averaging both legacy widths.
            width: ((320+250)/2),
            height: Fit,
            flow: Down,
            align: Align{x: 0.5}
            padding: 25,
            spacing: 10,

            show_bg: true
            draw_bg +: {
                color: #CCC
                border_radius: 3.0
            }

            View {
                width: Fill,
                height: Fit,
                flow: Right
                padding: Inset{top: 0, bottom: 10}
                align: Align{x: 0.5, y: 0.0}

                title := Label {
                    text: "Registration Status"
                    draw_text +: {
                        text_style: TITLE_TEXT {font_size: 13},
                        color: #000
                    }
                }
            }

            status := Label {
                width: Fill
                margin: Inset{top: 5, bottom: 5}
                align: Align{x: 0.5, y: 0.0}
                flow: Flow.Right{wrap: true}
                draw_text +: {
                    text_style: REGULAR_TEXT {
                        font_size: 11.5,
                    },
                    color: #000
                },
                text: "Registering account, please wait..."
            }

            View {
                width: Fill,
                height: Fit,
                flow: Right
                align: Align{x: 1.0}
                margin: Inset{top: 10}

                cancel_button := RobrixIconButton {
                    align: Align{x: 0.5, y: 0.5}
                    width: Fit, height: Fit
                    padding: 12
                    text: "Cancel"
                }
            }
        }
    }
}

#[derive(Script, ScriptHook, Widget)]
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
        let abort_clicked = self.view.button(cx, ids!(cancel_button)).clicked(actions);

        if abort_clicked || modal_dismissed {
            // Send action to close the modal with appropriate was_internal flag
            // Note: This doesn't actually cancel the registration request (still running in background)
            cx.action(RegisterStatusModalAction::Close {
                was_internal: abort_clicked 
            });
        }
    }
}

#[derive(Clone, Default, Debug)]
pub enum RegisterStatusModalAction {
    /// The modal requested to be closed
    Close {
        /// Whether the modal was closed by clicking an internal button (Abort)
        /// or being dismissed externally (ESC or click outside)
        was_internal: bool,
    },
    #[default]
    None,
}
