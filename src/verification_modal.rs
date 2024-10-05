use makepad_widgets::*;
use matrix_sdk::encryption::verification::{Verification, VerificationRequest};
use tokio::sync::mpsc::UnboundedSender;

use crate::verification::{VerificationAction, VerificationUserResponse};

live_design! {
    import makepad_widgets::base::*;
    import makepad_widgets::theme_desktop_dark::*;
    import makepad_draw::shader::std::*;

    import crate::shared::styles::*;
    import crate::shared::widgets::MolyButton;

    VerificationModal = {{VerificationModal}} {
        width: Fit
        height: Fit

        wrapper = <RoundedView> {
            flow: Down
            width: 600
            height: Fit
            padding: {top: 44, right: 30 bottom: 30 left: 50}
            spacing: 10

            show_bg: true
            draw_bg: {
                color: #fff
                radius: 3
            }

            <View> {
                width: Fill,
                height: Fit,
                flow: Right

                padding: {top: 8, bottom: 20}

                title = <View> {
                    width: Fit,
                    height: Fit,

                    <Label> {
                        text: "Verification Request"
                        draw_text: {
                            text_style: <TITLE_TEXT>{font_size: 13},
                            color: #000
                        }
                    }
                }

                filler_x = <View> {width: Fill, height: Fit}

                // The "X" close button on the top right corner.
                close_button = <RobrixIconButton> {
                    width: Fit,
                    height: Fit,
                    align: {x: 1.0, y: 0.0},
                    margin: 7,
                    padding: 15,

                    draw_icon: {
                        svg_file: (ICON_CLOSE),
                        fn get_color(self) -> vec4 {
                            return #x0;
                        }
                    }
                    icon_walk: {width: 14, height: 14}
                }
            }

            body = <View> {
                width: Fill,
                height: Fit,
                flow: Down,
                spacing: 40,

                prompt = <Label> {
                    width: Fill
                    draw_text: {
                        text_style: <REGULAR_TEXT>{
                            font_size: 10,
                            height_factor: 1.3
                        },
                        color: #000
                        wrap: Word
                    }
                }

                actions = <View> {
                    width: Fill, height: Fit
                    flow: Right,
                    align: {x: 1.0, y: 0.5}
                    spacing: 20

                    cancel_button = <RobrixIconButton> {
                        draw_icon: {
                            svg_file: (ICON_BLOCK_USER)
                            color: (COLOR_DANGER_RED),
                        }
                        icon_walk: {width: 16, height: 16, margin: {left: -2, right: -1} }
        
                        draw_bg: {
                            border_color: (COLOR_DANGER_RED),
                            color: #fff0f0 // light red
                        }
                        text: "Cancel"
                        draw_text:{
                            color: (COLOR_DANGER_RED),
                        }
                    }

                    accept_button = <RobrixIconButton> {
                        draw_icon: {
                            svg_file: (ICON_CHECKMARK)
                            color: (COLOR_ACCEPT_GREEN),
                        }
                        icon_walk: {width: 16, height: 16, margin: {left: -2, right: -1} }
        
                        draw_bg: {
                            border_color: (COLOR_ACCEPT_GREEN),
                            color: #f0fff0 // light green
                        }
                        text: "Yes"
                        draw_text:{
                            color: (COLOR_ACCEPT_GREEN),
                        }
                    }
                }
            }
        }
    }
}

#[derive(Live, LiveHook, Widget)]
pub struct VerificationModal {
    #[deref] view: View,
    #[rust] state: Option<VerificationModalState>,
    /// Whether the modal is in a "final" state,
    /// meaning that the verification process has ended
    /// and that any further interaction with it should close the modal.
    #[rust(false)] is_final: bool,
}

struct VerificationModalState {
    request: VerificationRequest,
    response_sender: UnboundedSender<VerificationUserResponse>,
}

#[derive(Clone, Debug, DefaultNone)]
pub enum VerificationModalAction {
    None,
    Close,
}

impl Widget for VerificationModal {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        self.widget_match_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk.with_abs_pos(DVec2 { x: 0., y: 0. }))
    }
}

impl WidgetMatchEvent for VerificationModal {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions, scope: &mut Scope) {
        let widget_uid = self.widget_uid();
        let accept_button = self.button(id!(accept_button));
        let cancel_button = self.button(id!(cancel_button));

        if cancel_button.clicked(actions)
            || self.button(id!(close_button)).clicked(actions)
            || actions.iter().find(|a| matches!(a.downcast_ref(), Some(ModalAction::Dismissed))).is_some()
        {
            if let Some(state) = self.state.as_ref() {
                let _ = state.response_sender.send(VerificationUserResponse::Cancel);
            }
            cx.widget_action(widget_uid, &scope.path, VerificationModalAction::Close);
            self.reset_state();
        }

        if self.button(id!(accept_button)).clicked(actions) {
            if self.is_final {
                cx.widget_action(widget_uid, &scope.path, VerificationModalAction::Close);
                self.reset_state();
            } else {
                if let Some(state) = self.state.as_ref() {
                    let _ = state.response_sender.send(VerificationUserResponse::Accept);
                }
            }
        }

        for action in actions {
            match action.as_widget_action().cast() {
                VerificationAction::None => { }
            
                VerificationAction::RequestReceived { request, response_sender } => {
                    log!("Shouldn't happen - received a redundant verification request: {:?}", request);
                }

                VerificationAction::RequestCancelled(cancel_info) => {
                    self.label(id!(prompt)).set_text(
                        &format!("Verification request was cancelled: {}", cancel_info.reason())
                    );
                    accept_button.set_enabled(true);
                    accept_button.set_text("Ok");
                    cancel_button.set_visible(false);
                    self.is_final = true;
                }

                VerificationAction::RequestAccepted => {
                    self.label(id!(prompt)).set_text(
                        "You successfully accepted the verification request.\n\n\
                        Waiting for the other device to agree on verification methods..."
                    );
                    accept_button.set_enabled(false);
                    accept_button.set_text("Waiting...");
                    cancel_button.set_text("Cancel");
                    cancel_button.set_enabled(true);
                    cancel_button.set_visible(true);
                }

                VerificationAction::RequestAcceptError(error) => {
                    self.label(id!(prompt)).set_text(&format!(
                        "Error accepting verification request: {}\n\n\
                        Please try the verification process again.",
                        error,
                    ));
                    accept_button.set_enabled(true);
                    accept_button.set_text("Ok");
                    cancel_button.set_visible(false);
                    self.is_final = true;
                }

                VerificationAction::RequestCancelError(error) => {
                    self.label(id!(prompt)).set_text(
                        &format!("Error cancelling verification request: {}.", error)
                    );
                    accept_button.set_enabled(true);
                    accept_button.set_text("Ok");
                    cancel_button.set_visible(false);
                    self.is_final = true;
                }

                VerificationAction::RequestTransitionedToUnsupportedMethod(method) => {
                    self.label(id!(prompt)).set_text(
                        &format!(
                            "Verification request transitioned to unsupported method: {}\n\nPlease try the verification process again.",
                            match method {
                                Verification::SasV1(_) => "Short Authentication String",
                                // Verification::QrV1(_) => "QR Code",
                                other => "Unknown",
                            },
                        )
                    );
                    accept_button.set_enabled(true);
                    accept_button.set_text("Ok");
                    cancel_button.set_visible(false);
                    self.is_final = true;
                }

                VerificationAction::SasAccepted(accepted_protocols) => {
                    self.label(id!(prompt)).set_text(&format!(
                        "Both sides have accepted the same verification method(s).\n\n\
                        Waiting for both devices to exchange keys..."
                    ));
                    accept_button.set_enabled(false);
                    accept_button.set_text("Waiting...");
                    cancel_button.set_text("Cancel");
                    cancel_button.set_enabled(true);
                    cancel_button.set_visible(true);
                }

                VerificationAction::KeysExchanged { emojis, decimals } => {
                    self.label(id!(prompt)).set_text(&format!(
                        "Keys have been exchanged. Please verify the following:\n\n\
                        - Emojis: {:?}\n\
                        - Decimals: {:?}\n\n\
                        Do these keys match?",
                        emojis, decimals
                    ));
                    accept_button.set_enabled(true);
                    accept_button.set_text("Yes");
                    cancel_button.set_text("No");
                    cancel_button.set_enabled(true);
                    cancel_button.set_visible(true);
                }

                VerificationAction::SasConfirmed => {
                    self.label(id!(prompt)).set_text(
                        "You successfully confirmed the Short Auth Strings.\n\n\
                        Waiting for the other device to confirm..."
                    );
                    accept_button.set_enabled(false);
                    accept_button.set_text("Waiting...");
                    cancel_button.set_text("Cancel");
                    cancel_button.set_enabled(true);
                    cancel_button.set_visible(true);
                }

                VerificationAction::SasConfirmationError(error) => {
                    self.label(id!(prompt)).set_text(
                        &format!("Error confirming keys: {}\n\nPlease try the verification process again.", error)
                    );
                    accept_button.set_text("Ok");
                    accept_button.set_enabled(true);
                    cancel_button.set_visible(false);
                    self.is_final = true;
                }

                VerificationAction::RequestCompleted => {
                    self.label(id!(prompt)).set_text("Verification completed successfully!");
                    accept_button.set_text("Ok");
                    accept_button.set_enabled(true);
                    cancel_button.set_visible(false);
                }
            }
        }
    }
}

impl VerificationModal {
    fn reset_state(&mut self) {
        self.state = None;
        self.is_final = false;
    }
}

impl VerificationModalRef {
    pub fn initialize_with_data(
        &self,
        request: VerificationRequest,
        response_sender: UnboundedSender<VerificationUserResponse>,
    ) {
        if let Some(mut inner) = self.borrow_mut() {
            let prompt_text = if request.is_self_verification() {
                format!("Do you wish to verify your own device?")
            } else {
                if let Some(room_id) = request.room_id() {
                    format!("Do you wish to verify user {} in room {}?",
                        request.other_user_id(),
                        room_id,
                    )
                } else {
                    format!("Do you wish to verify user {}?",
                        request.other_user_id()
                    )
                }
            };
            self.label(id!(prompt)).set_text(&prompt_text);

            let accept_button = self.button(id!(accept_button));
            let cancel_button = self.button(id!(cancel_button));
            accept_button.set_text("Yes");
            accept_button.set_enabled(true);
            accept_button.set_visible(true);
            cancel_button.set_text("Cancel");
            cancel_button.set_enabled(true);
            cancel_button.set_visible(true);

            inner.state = Some(VerificationModalState { request, response_sender });
            inner.is_final = false;
        }
    }
}
