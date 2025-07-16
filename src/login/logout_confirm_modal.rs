use std::sync::atomic::Ordering;

use makepad_widgets::{makepad_futures::channel::oneshot::Sender, *};
use crate::sliding_sync::{submit_async_request, MatrixRequest, LOGOUT_POINT_OF_NO_RETURN};

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
    /// Whether the modal is in a final state, meaning the user can only click "Okay" to close it.
    ///
    /// * Set to `Some(true)` after a successful logout Action
    /// * Set to `Some(false)` after a logout error occurs.
    /// * Set to `None` when the user is still able to interact with the modal.
    #[rust] final_success: Option<bool>,
}

/// Actions handled by the parent widget of the [`LogoutConfirmModal`].
#[derive(Clone, Debug, DefaultNone)]
pub enum LogoutConfirmModalAction {
    /// The modal should be opened
    Open,
    /// The modal requested its parent widget to close.
    Close {
        /// `True` if the modal was closed after a successful logout action.
        /// `False` if the modal was dismissed or closed after a failure/error.
        successful: bool,
        /// Whether the modal was dismissed by the user clicking an internal button.
        was_internal: bool,
    },
    None,
}

/// Actions related to logout process 
#[derive(Clone, DefaultNone)]
pub enum LogoutAction {
    /// A positive response from the backend Matrix task to the logout.
    LogoutSuccess,
    /// A negative response from the backend Matrix task to the logout.
    LogoutFailure(String),
    /// Signal to clean up App-state 
    CleanAppState {
        on_clean_appstate: Sender<bool>
    },
    /// Signal that the application is in an invalid state and needs to be restarted.
    /// This happens when critical components have been cleaned up during a previous
    /// logout attempt that reached the point of no return, but the app wasn't restarted.
    ApplicationRequiresRestart {
        /// Indicates which critical component is missing
        missing_component: MissingComponentType,
    },
    /// Progress update from the logout state machine
    ProgressUpdate {
        message: String,
        percentage: u8,
    },
    None,
}

impl std::fmt::Debug for LogoutAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LogoutAction::LogoutSuccess => write!(f, "LogoutSuccess"),
            LogoutAction::LogoutFailure(msg) => write!(f, "LogoutFailure({})", msg),
            LogoutAction::CleanAppState { .. } => write!(f, "CleanAppState"),
            LogoutAction::ApplicationRequiresRestart { missing_component } => {
                write!(f, "ApplicationRequiresRestart({:?})", missing_component)
            }
            LogoutAction::ProgressUpdate { message, percentage } => {
                write!(f, "ProgressUpdate({}, {}%)", message, percentage)
            }
            LogoutAction::None => write!(f, "None"),
        }
    }
}

/// Indicates which critical component is missing after a partial logout,
/// requiring application restart to restore functionality.
#[derive(Clone, Copy, Debug, DefaultNone)]
pub enum MissingComponentType {
    /// The Matrix client has been disposed
    ClientMissing,
    /// The sync service has been disposed
    SyncServiceMissing,
    None,
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
        
        let modal_dismissed = actions.iter().any(|a| matches!(a.downcast_ref(), Some(ModalAction::Dismissed)));
        let cancel_clicked = cancel_button.clicked(actions); 
        
        if cancel_clicked || modal_dismissed {
            cx.action(LogoutConfirmModalAction::Close { successful: false, was_internal: cancel_clicked });
            self.reset_state(cx);
            return;
        }

        let mut needs_redraw = false;
        if confirm_button.clicked(actions) {
            if let Some(successful) = self.final_success {
                if LOGOUT_POINT_OF_NO_RETURN.load(Ordering::Acquire) && !successful {
                    log!("User requested immediate restart after unrecoverable logout error");
                    std::process::exit(0);
                }

                cx.action(LogoutConfirmModalAction::Close { successful, was_internal: true });
                self.reset_state(cx);
                return;
            } else {
                self.set_message(cx, "Waiting for logout...");
                confirm_button.set_enabled(cx, false);
                submit_async_request(MatrixRequest::Logout { is_desktop: cx.display_context.is_desktop() });
                needs_redraw = true;
            }
        }

        for action in actions {
            if let Some(LogoutAction::LogoutSuccess) = action.downcast_ref() {
                // Logout was successful
                self.final_success = Some(true);
                self.set_message(cx, "Logout successful!");
                let confirm_button = self.button(id!(confirm_button));
                confirm_button.set_text(cx, "Close");
                confirm_button.set_enabled(cx, true);
                
                let cancel_button = self.button(id!(cancel_button));
                cancel_button.set_visible(cx, false);
                
                needs_redraw = true;
            }
            
            if let Some(LogoutAction::LogoutFailure(error)) = action.downcast_ref() {
                if LOGOUT_POINT_OF_NO_RETURN.load(Ordering::Acquire) {
                    self.label(id!(title)).set_text(cx, "Logout error, please restart Robrix.");
                    self.set_message(cx, "The logout process encountered an error when communicating with the homeserver. Since your login session has been partially invalidated, Robrix must restart in order to continue to properly function.");

                    let confirm_button = self.button(id!(confirm_button));
                    confirm_button.set_text(cx, "Restart now");
                    confirm_button.apply_over(cx, live!{
                        draw_bg: {
                            color: #xE23A3A
                        }
                    });
                    confirm_button.set_enabled(cx, true);

                    let cancel_button = self.button(id!(cancel_button));
                    cancel_button.set_visible(cx, true);
                    cancel_button.set_text(cx, "Restart later");
                    cancel_button.apply_over(cx, live!{
                        draw_bg: {
                            color: #x3A78E2
                        }
                    });
                    cancel_button.set_enabled(cx, true);

                } else {
                    self.set_message(cx, &format!("Logout failed: {}", error));
                    let confirm_button = self.button(id!(confirm_button));
                    confirm_button.set_text(cx, "Okay");
                    confirm_button.set_enabled(cx, true);

                    let cancel_button = self.button(id!(cancel_button));
                    cancel_button.set_visible(cx, false);
                }

                self.final_success = Some(false);
                needs_redraw = true;
            }

            if let Some(LogoutAction::ApplicationRequiresRestart { .. }) = action.downcast_ref() {
                self.label(id!(title)).set_text(cx, "Logout error, please restart Robrix.");
                self.set_message(cx, "Application is in an inconsistent state and needs to be restarted to continue.");
    
                let confirm_button = self.button(id!(confirm_button));
                confirm_button.set_text(cx, "Restart now");
                confirm_button.apply_over(cx, live!{
                    draw_bg: {
                        color: #xE23A3A
                    }
                });
                confirm_button.set_enabled(cx, true);

                let cancel_button = self.button(id!(cancel_button));
                cancel_button.set_visible(cx, true);
                cancel_button.set_text(cx, "Restart later");
                cancel_button.apply_over(cx, live!{
                    draw_bg: {
                        color: #x3A78E2
                    }
                });
                cancel_button.set_enabled(cx, true);
                
                self.final_success = Some(false);
                needs_redraw = true;
            }

            if let Some(LogoutAction::ProgressUpdate { message, percentage }) = action.downcast_ref() {
                // Just update the message text to show progress
                self.set_message(cx, &format!("{} ({}%)", message, percentage));
                // Disable buttons during logout
                self.button(id!(confirm_button)).set_enabled(cx, false);
                self.button(id!(cancel_button)).set_enabled(cx, false);
                needs_redraw = true;
            }

        }

        if needs_redraw {
            self.redraw(cx);
        }

    }
}

impl LogoutConfirmModal {
    /// Sets the message text displayed in the body of the modal.
    pub fn set_message(&mut self, cx: &mut Cx, message: &str) {
        self.label(id!(message)).set_text(cx, message);
    }

    fn reset_state(&mut self, cx: &mut Cx) {
        let cancel_button = self.button(id!(cancel_button));
        let confirm_button = self.button(id!(confirm_button));
        self.final_success = None;
        self.set_message(cx, "Are you sure you want to logout?");
        confirm_button.set_enabled(cx, true);
        confirm_button.set_text(cx, "Confirm");
        cancel_button.set_visible(cx, true);
        cancel_button.set_enabled(cx, true);
        self.redraw(cx);
    }

}


impl LogoutConfirmModalRef {
    /// See [`LogoutConfirmModal::set_message()`].
    pub fn set_message(&self, cx: &mut Cx, message: &str) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_message(cx, message);
        }
    }

    pub fn reset_state(&self,cx: &mut Cx) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.reset_state(cx);
        }
    }

}