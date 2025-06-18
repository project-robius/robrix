use makepad_widgets::*;
use crate::shared::styles::{COLOR_ACTIVE_PRIMARY, COLOR_DISABLE_GRAY, COLOR_PRIMARY, COLOR_SECONDARY, COLOR_TEXT};

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
                    },
                    color: #000000,
                    wrap: Word
                },
                text: "Are you sure you want to logout?"
            }

            <View> {
                width: Fill,
                height: Fit,
                flow: Right,
                align: {x: 0.5, y: 0.5},
                spacing: 10.0,

                cancel_button = <RobrixIconButton> {
                    width: Fit, height: Fit,
                    padding: 10,
                    draw_bg: {
                        color: (COLOR_SECONDARY)
                    },
                    text: "Cancel"
                    draw_text: {
                        color: (COLOR_TEXT)
                        text_style: <REGULAR_TEXT> {font_size: 14}
                    },
                }

                confirm_button = <RobrixIconButton> {
                    width: Fit, height: Fit,
                    padding: 10,
                    draw_bg: { color: (COLOR_ACTIVE_PRIMARY) },
                    text: "Confirm"
                    draw_text: {
                        color: (COLOR_PRIMARY)
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
    #[rust(false)] is_loading: bool,
}

#[derive(Clone, Debug)]
pub enum LogoutConfirmModalAction {
    Open,
    Cancel,
    Confirm,
    LogoutSuccess,
    LogoutFailed(String),
}

impl Widget for LogoutConfirmModal {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if self.is_loading {
            return;
        }
        self.view.handle_event(cx, event, scope);
        self.widget_match_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl WidgetMatchEvent for LogoutConfirmModal {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions, _scope: &mut Scope) {
        let cancel_button = self.button(id!(cancel_button));
        let confirm_button = self.button(id!(confirm_button));
        
        let modal_dismissed = actions
            .iter()
            .any(|a| matches!(a.downcast_ref(), Some(ModalAction::Dismissed)));

        if modal_dismissed && self.is_loading {
            return
        }

        let cancel_button_clicked = cancel_button.clicked(actions) ;
        if cancel_button_clicked { 
            cx.action(LogoutConfirmModalAction::Cancel);
        }
        if confirm_button.clicked(actions) && !self.is_loading {
            self.is_loading = true;
            self.set_message(cx, "Waiting for logout...");
            self.update_button_states(cx);
            cx.action(LogoutConfirmModalAction::Confirm);
        }
    }
}

impl LogoutConfirmModal {
    /// Sets the message text displayed in the body of the modal.
    pub fn set_message(&mut self, cx: &mut Cx, message: &str) {
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

    fn update_button_states(&mut self, cx: &mut Cx) {
        let cancel_button = self.button(id!(cancel_button));
        let confirm_button = self.button(id!(confirm_button));
        
        if self.is_loading {
            cancel_button.apply_over(cx, live! {
                draw_bg: { color: (COLOR_SECONDARY) },
                draw_text: { color: (COLOR_DISABLE_GRAY) },
                enabled: false
            });
            confirm_button.apply_over(cx, live! {
                draw_bg: { color: (COLOR_DISABLE_GRAY) },
                draw_text: { color: (COLOR_SECONDARY) },
                enabled: false
            });
        } else {
            cancel_button.apply_over(cx, live! {
                draw_bg: { color: (COLOR_SECONDARY) },
                draw_text: { color: (COLOR_TEXT) },
                enabled: true
            });
            confirm_button.apply_over(cx, live! {
                draw_bg: { color: (COLOR_ACTIVE_PRIMARY) },
                draw_text: { color: (COLOR_PRIMARY) },
                enabled: true
            });
        }
    }

    pub fn set_loading(&mut self, cx: &mut Cx, is_loading: bool) {
        self.is_loading = is_loading;
        self.update_button_states(cx);
        if !is_loading {
            self.set_message(cx, "Are you sure you want to logout?");
        }
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

    /// See [`LogoutConfirmModal::set_loading()`].
    pub fn set_loading(&self, cx: &mut Cx, is_loading: bool) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_loading(cx, is_loading);
        }
    }

}