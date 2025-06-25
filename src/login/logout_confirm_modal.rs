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
    #[rust(false)] is_logging_out: bool,
    /// Flag to track if a background dismiss event has been processed.
    /// 
    /// This prevents event loops when the modal background is clicked:
    /// 1. User clicks modal background → ModalAction::Dismissed is triggered
    /// 2. We set dismiss_handled=true and emit Close event
    /// 3. Any subsequent Dismissed events are ignored to break the loop
    /// 
    /// IMPORTANT: Must call reset_state() before opening the modal
    /// to ensure background clicks can be processed again. This reset
    /// typically happens in app.rs when handling LogoutConfirmModalAction::Open.
    #[rust(false)] dismiss_handled: bool,
}

/// Actions sent to or from the logout_confrim_modal.
#[derive(Clone, Debug)]
pub enum LogoutConfirmModalAction {
    Open,
    Close,
    Confirm,
    LogoutSuccess,
    LogoutFailure(String),
}

impl Widget for LogoutConfirmModal {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
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

        if modal_dismissed && self.is_logging_out {
            return;
        }

        // Handle background click dismiss event, but only once to prevent event loops
        if modal_dismissed && !self.dismiss_handled {
            self.dismiss_handled = true;
            cx.action(LogoutConfirmModalAction::Close);
            return;
        }

        let cancel_button_clicked = cancel_button.clicked(actions) ;
        if cancel_button_clicked { 
            cx.action(LogoutConfirmModalAction::Close);
        }
        if confirm_button.clicked(actions) && !self.is_logging_out {
            self.is_logging_out = true;
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

    fn reset_state(&mut self) {
        self.dismiss_handled = false;
        self.is_logging_out = false;
    }

    fn update_button_states(&mut self, cx: &mut Cx) {
        let cancel_button = self.button(id!(cancel_button));
        let confirm_button = self.button(id!(confirm_button));
        
        if self.is_logging_out {
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
        self.is_logging_out = is_loading;
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

    /// See [`LogoutConfirmModal::set_loading()`].
    pub fn set_loading(&self, cx: &mut Cx, is_loading: bool) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_loading(cx, is_loading);
        }
    }

    pub fn reset_state(&self) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.reset_state();
        }
    }

}