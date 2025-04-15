use makepad_widgets::*;

live_design! {
    use link::theme::*;
    use link::widgets::*;

    use crate::shared::styles::*;
    use crate::shared::icon_button::RobrixIconButton;

    // A modal dialog that displays logout confirmation
    pub LogoutConfirmModal = {{LogoutConfirmModal}} {
        width: Fit,
        height: Fit,
        align: {x: 0.5, y: 0.5},

        <RoundedView> {
            width: 300,
            height: Fit,
            flow: Down,
            align: {x: 0.5},
            padding: 25,
            spacing: 10,

            show_bg: true,
            draw_bg: {
                color: #FFFFFF
            }
            margin: 0


            <View> {
                width: Fill,
                height: Fit,
                flow: Right,
                padding: {top: 0, bottom: 10},
                align: {x: 0.5, y: 0.0},

                title = <Label> {
                    text: "Confirm Logout",
                    draw_text: {
                            text_style: <TITLE_TEXT>{font_size: 18},
                            color: #000000
                    }
                }
            }

            message = <Label> {
                width: Fill,
                margin: {top: 10, bottom: 20},
                draw_text: {
                    text_style: <REGULAR_TEXT>{
                        font_size: 14,
                        height_factor: 1.3
                    },
                    color: #000000,
                    wrap: Word
                },
                text: "Are you sure you want to logout?"
            }


            <View> {
                width: Fill,
                height: Fit,
                flow: Right, // Buttons side-by-side
                align: {x: 0.5, y: 0.5}, // Center buttons horizontally if needed, or use 1.0 to right-align
                spacing: 10.0, // Space between buttons

                // Cancel Button
                cancel_button = <RobrixIconButton> {
                    width: Fit, height: Fit,
                    padding: 10,
                    draw_bg: {
                        color: #CCCCCC
                    },
                    text: "Cancel"
                    draw_text: {
                        color: #000000,
                        text_style: <REGULAR_TEXT> {font_size: 14}
                    }, 
                }

                // Confirm Button
                confirm_button = <RobrixIconButton> {
                    width: Fit, height: Fit,
                    padding: 10,
                    draw_bg: { color: (COLOR_DANGER_RED) },
                    text: "Confirm"
                    draw_text: {
                        color: #FFFFFF
                        text_style: <REGULAR_TEXT> {font_size: 14}
                    },
                }
            }
        }
    }
}

/// A modal dialog that displays logout confirmation.
#[derive(Live, LiveHook, Widget)]
pub struct LogoutConfirmModal {
    #[deref] view: View,
}

#[derive(Clone, Debug, DefaultNone)]
pub enum LogoutConfirmModalAction {
    None,
    Cancel,
    Confirm,
}

impl Widget for LogoutConfirmModal {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        // Need to call widget_match_event BEFORE view.handle_event
        // if we want to intercept actions like ButtonAction::Clicked
        self.widget_match_event(cx, event, scope);
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl WidgetMatchEvent for LogoutConfirmModal {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions, scope: &mut Scope) {
        let widget_uid = self.widget_uid();
        let cancel_button = self.button(id!(cancel_button));
        let confirm_button = self.button(id!(confirm_button));

        // Check if cancel button was clicked
        if cancel_button.clicked(actions) {
            cx.widget_action(widget_uid, &scope.path, LogoutConfirmModalAction::Cancel);
        }
        // Check if confirm button was clicked
        else if confirm_button.clicked(actions) {
            cx.widget_action(widget_uid, &scope.path, LogoutConfirmModalAction::Confirm);
        }
    }
}

impl LogoutConfirmModal {
    /// Sets the message text displayed in the body of the modal.
    fn set_message(&mut self, cx: &mut Cx, message: &str) {
        self.label(id!(message)).set_text(cx, message);
    }

    /// Returns a reference to the cancel button
    fn cancel_button_ref(&self) -> ButtonRef {
        self.button(id!(cancel_button))
    }

    /// Returns a reference to the confirm button
    fn confirm_button_ref(&self) -> ButtonRef {
        self.button(id!(confirm_button))
    }
}


impl LogoutConfirmModalRef {
    /// See [`LogoutConfirmModal::set_message()`].
    pub fn set_message(&self, cx: &mut Cx, message: &str) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_message(cx, message);
        }
    }

    /// See [`LogoutConfirmModal::cancel_button_ref()`].
    pub fn cancel_button_ref(&self) -> ButtonRef {
        self.borrow()
            .map(|inner| inner.cancel_button_ref())
            .unwrap_or_default()
    }

    /// See [`LogoutConfirmModal::confirm_button_ref()`].
    pub fn confirm_button_ref(&self) -> ButtonRef {
        self.borrow()
            .map(|inner| inner.confirm_button_ref())
            .unwrap_or_default()
    }

    /// Shows the modal dialog.
    pub fn open(&self, cx: &mut Cx) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.view.redraw(cx);
            let widget_uid = inner.widget_uid();
            cx.widget_action(widget_uid, &Scope::empty().path, ModalAction::None);
        }
    }

    /// Hides the modal dialog.
    pub fn close(&self, cx: &mut Cx) {
        if let Some(inner) = self.borrow_mut() {
            let widget_uid = inner.widget_uid();
            cx.widget_action(widget_uid, &Scope::empty().path, ModalAction::Dismissed);
        }
    }
}