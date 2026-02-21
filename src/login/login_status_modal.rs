use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*


    // A modal dialog that displays the status of a login attempt.
    mod.widgets.LoginStatusModal = #(LoginStatusModal::register_widget(vm)) {
        width: Fit,
        height: Fit
        align: Align{x: 0.5}

        RoundedView {
            // Halfway between the login screen background (320 px wide)
            // and the login screen's buttons/content (250 px wide).
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
                    text: "Login Status"
                    draw_text +: {
                        text_style: TITLE_TEXT {font_size: 13},
                        color: #000
                    }
                }
            }

            status := Label {
                width: Fill
                margin: Inset{top: 5, bottom: 5}
                draw_text +: {
                    text_style: REGULAR_TEXT {
                        font_size: 11.5,
                    },
                    color: #000
                    flow: Flow.Right{wrap: true}
                }
            }

            View {
                width: Fill,
                height: Fit,
                flow: Right
                align: Align{x: 1.0}
                margin: Inset{top: 10}

                button := RobrixIconButton {
                    align: Align{x: 0.5, y: 0.5}
                    width: Fit, height: Fit
                    padding: 12
                    draw_bg +: {
                        color: (COLOR_ACTIVE_PRIMARY)
                    }
                    draw_text +: {
                        color: (COLOR_PRIMARY)
                        text_style: REGULAR_TEXT {}
                    }
                    text: "Cancel"
                }
            }
        }
    }
}

/// A modal dialog that displays the status of a login attempt.
#[derive(Script, ScriptHook, Widget)]
pub struct LoginStatusModal {
    #[deref] view: View,
}

#[derive(Clone, Debug, Default)]
pub enum LoginStatusModalAction {
    #[default]
    None,
    Close,
}

impl Widget for LoginStatusModal {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        self.widget_match_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl WidgetMatchEvent for LoginStatusModal {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions, _scope: &mut Scope) {
        let widget_uid = self.widget_uid();
        let button = self.button(cx, ids!(button));

        let modal_dismissed = actions
            .iter()
            .any(|a| matches!(a.downcast_ref(), Some(ModalAction::Dismissed)));

        if modal_dismissed || button.clicked(actions) {
            // Here, we could optionally attempt to cancel the in-flight login request.
            // But our background async task doesn't yet support that, so we do nothing atm.

            // If the modal was dismissed by clicking outside of it, we MUST NOT emit
            // a `LoginStatusModalAction::Close` action, as that would cause
            // an infinite action feedback loop.
            if !modal_dismissed {
                cx.widget_action(widget_uid,  LoginStatusModalAction::Close);
            }
        }
    }
}

impl LoginStatusModal {
    /// Sets the title text displayed in the modal.
    fn set_title(&mut self, cx: &mut Cx, title: &str) {
        self.label(cx, ids!(title)).set_text(cx, title);
    }

    /// Sets the status text displayed in the body of the modal.
    fn set_status(&mut self, cx: &mut Cx, status: &str) {
        self.label(cx, ids!(status)).set_text(cx, status);
    }

    /// Returns a reference to the modal's button, enabling you to set its properties.
    fn button_ref(&self, cx: &mut Cx) -> ButtonRef {
        self.button(cx, ids!(button))
    }
}

impl LoginStatusModalRef {
    /// See [`LoginStatusModal::set_title()`].
    pub fn set_title(&self, cx: &mut Cx, title: &str) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_title(cx, title);
        }
    }

    /// See [`LoginStatusModal::set_status()`].
    pub fn set_status(&self, cx: &mut Cx, status: &str) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_status(cx, status);
        }
    }

    /// See [`LoginStatusModal::button_ref()`].
    pub fn button_ref(&self, cx: &mut Cx) -> ButtonRef {
        self.borrow()
            .map(|inner| inner.button_ref(cx))
            .unwrap_or_default()
    }
}
