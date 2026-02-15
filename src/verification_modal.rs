use std::borrow::Cow;

use makepad_widgets::*;
use matrix_sdk::encryption::verification::Verification;

use crate::verification::{
    VerificationAction, VerificationRequestActionState, VerificationUserResponse,
};

live_design! {
    use link::theme::*;
    use link::widgets::*;

    use crate::shared::styles::*;
    use crate::shared::icon_button::RobrixIconButton;

    pub VerificationModal = {{VerificationModal}} {
        width: Fit
        height: Fit

        <RoundedView> {
            flow: Down
            width: 400
            height: Fit
            padding: {top: 25, right: 30 bottom: 30 left: 45}
            spacing: 10

            show_bg: true
            draw_bg: {
                color: #fff
                border_radius: 3.0
            }

            title = <View> {
                width: Fill,
                height: Fit,
                flow: Right
                padding: {top: 0, bottom: 40}
                align: {x: 0.5, y: 0.0}

                <Label> {
                    text: "Verification Request"
                    draw_text: {
                        text_style: <TITLE_TEXT>{font_size: 13},
                        color: #000
                    }
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
                            font_size: 11.5,
                        },
                        color: #000
                        wrap: Word
                    }
                }

                <View> {
                    width: Fill, height: Fit
                    flow: Right,
                    align: {x: 1.0, y: 0.5}
                    spacing: 20

                    cancel_button = <RobrixIconButton> {
                        align: {x: 0.5, y: 0.5}
                        padding: 15,
                        draw_icon: {
                            svg_file: (ICON_FORBIDDEN)
                            color: (COLOR_FG_DANGER_RED),
                        }
                        icon_walk: {width: 16, height: 16, margin: {left: -2, right: -1} }

                        draw_bg: {
                            border_color: (COLOR_FG_DANGER_RED),
                            color: (COLOR_BG_DANGER_RED)
                        }
                        text: "Cancel"
                        draw_text:{
                            color: (COLOR_FG_DANGER_RED),
                        }
                    }

                    accept_button = <RobrixIconButton> {
                        align: {x: 0.5, y: 0.5}
                        padding: 15,
                        draw_icon: {
                            svg_file: (ICON_CHECKMARK)
                            color: (COLOR_FG_ACCEPT_GREEN),
                        }
                        icon_walk: {width: 16, height: 16, margin: {left: -2, right: -1} }

                        draw_bg: {
                            border_color: (COLOR_FG_ACCEPT_GREEN),
                            color: (COLOR_BG_ACCEPT_GREEN)
                        }
                        text: "Yes"
                        draw_text:{
                            color: (COLOR_FG_ACCEPT_GREEN),
                        }
                    }
                }
            }
        }
    }
}

#[derive(Live, LiveHook, Widget)]
pub struct VerificationModal {
    #[deref]
    view: View,
    #[rust]
    state: Option<VerificationRequestActionState>,
    /// Whether the modal is in a "final" state,
    /// meaning that the verification process has ended
    /// and that any further interaction with it should close the modal.
    #[rust(false)]
    is_final: bool,
}

/// Actions emitted by the `VerificationModal`.
#[derive(Clone, Copy, Debug)]
pub enum VerificationModalAction {
    Close,
}

impl Widget for VerificationModal {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        self.widget_match_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl WidgetMatchEvent for VerificationModal {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions, _scope: &mut Scope) {
        let accept_button = self.button(ids!(accept_button));
        let cancel_button = self.button(ids!(cancel_button));

        let cancel_button_clicked = cancel_button.clicked(actions);
        let modal_dismissed = actions
            .iter()
            .any(|a| matches!(a.downcast_ref(), Some(ModalAction::Dismissed)));

        if cancel_button_clicked || modal_dismissed {
            if let Some(state) = self.state.as_ref() {
                let _ = state.response_sender.send(VerificationUserResponse::Cancel);
            }
            self.reset_state();

            // If the modal was dismissed by clicking outside of it, we MUST NOT emit
            // a `VerificationModalAction::Close` action, as that would cause
            // an infinite action feedback loop.
            if !modal_dismissed {
                cx.action(VerificationModalAction::Close);
            }
        }

        if accept_button.clicked(actions) {
            if self.is_final {
                cx.action(VerificationModalAction::Close);
                self.reset_state();
            } else {
                if let Some(state) = self.state.as_ref() {
                    let _ = state.response_sender.send(VerificationUserResponse::Accept);
                }
            }
        }

        let mut needs_redraw = false;
        for action in actions {
            // `VerificationAction`s come from a background thread, so they are NOT widget actions.
            // Therefore, we cannot use `as_widget_action().cast()` to match them.
            if let Some(verification_action) = action.downcast_ref::<VerificationAction>() {
                match verification_action {
                    VerificationAction::RequestCancelled(cancel_info) => {
                        self.label(ids!(prompt)).set_text(
                            cx,
                            &format!(
                                "Verification request was cancelled: {}",
                                cancel_info.reason()
                            ),
                        );
                        accept_button.set_enabled(cx, true);
                        accept_button.set_text(cx, "Ok");
                        cancel_button.set_visible(cx, false);
                        self.is_final = true;
                    }

                    VerificationAction::RequestAccepted => {
                        self.label(ids!(prompt)).set_text(
                            cx,
                            "You successfully accepted the verification request.\n\n\
                            Waiting for the other device to agree on verification methods...",
                        );
                        accept_button.set_enabled(cx, false);
                        accept_button.set_text(cx, "Waiting...");
                        cancel_button.set_text(cx, "Cancel");
                        cancel_button.set_enabled(cx, true);
                        cancel_button.set_visible(cx, true);
                    }

                    VerificationAction::RequestAcceptError(error) => {
                        self.label(ids!(prompt)).set_text(
                            cx,
                            &format!(
                                "Error accepting verification request: {}\n\n\
                                Please try the verification process again.",
                                error,
                            ),
                        );
                        accept_button.set_enabled(cx, true);
                        accept_button.set_text(cx, "Ok");
                        cancel_button.set_visible(cx, false);
                        self.is_final = true;
                    }

                    VerificationAction::RequestCancelError(error) => {
                        self.label(ids!(prompt)).set_text(
                            cx,
                            &format!("Error cancelling verification request: {}.", error),
                        );
                        accept_button.set_enabled(cx, true);
                        accept_button.set_text(cx, "Ok");
                        cancel_button.set_visible(cx, false);
                        self.is_final = true;
                    }

                    VerificationAction::RequestTransitionedToUnsupportedMethod(method) => {
                        self.label(ids!(prompt)).set_text(
                            cx,
                            &format!(
                                "Verification request transitioned to unsupported method: {}\n\nPlease try the verification process again.",
                                match method {
                                    Verification::SasV1(_) => "Short Authentication String",
                                    // Verification::QrV1(_) => "QR Code",
                                    _other => "Unknown",
                                },
                            )
                        );
                        accept_button.set_enabled(cx, true);
                        accept_button.set_text(cx, "Ok");
                        cancel_button.set_visible(cx, false);
                        self.is_final = true;
                    }

                    VerificationAction::SasAccepted(_accepted_protocols) => {
                        self.label(ids!(prompt)).set_text(
                            cx,
                            "Both sides have accepted the same verification method(s).\n\n\
                            Waiting for both devices to exchange keys...",
                        );
                        accept_button.set_enabled(cx, false);
                        accept_button.set_text(cx, "Waiting...");
                        cancel_button.set_text(cx, "Cancel");
                        cancel_button.set_enabled(cx, true);
                        cancel_button.set_visible(cx, true);
                    }

                    VerificationAction::KeysExchanged { emojis, decimals } => {
                        let text = if let Some(emoji_list) = emojis {
                            format!(
                                "Keys have been exchanged. Please verify the following emoji:\
                                \n   {}\n\n\
                                Do these emoji keys match?",
                                emoji_list
                                    .emojis
                                    .iter()
                                    .map(|em| format!("{}  ({})", em.symbol, em.description))
                                    .collect::<Vec<_>>()
                                    .join("\n   ")
                            )
                        } else {
                            format!(
                                "Keys have been exchanged. Please verify the following numbers:\n\
                                \n   {}\n   {}\n   {}\n\n\
                                Do these number keys match?",
                                decimals.0, decimals.1, decimals.2,
                            )
                        };
                        self.label(ids!(prompt)).set_text(cx, &text);
                        accept_button.set_enabled(cx, true);
                        accept_button.set_text(cx, "Yes");
                        cancel_button.set_text(cx, "No");
                        cancel_button.set_enabled(cx, true);
                        cancel_button.set_visible(cx, true);
                    }

                    VerificationAction::SasConfirmed => {
                        self.label(ids!(prompt)).set_text(
                            cx,
                            "You successfully confirmed the Short Auth String keys.\n\n\
                            Waiting for the other device to confirm...",
                        );
                        accept_button.set_enabled(cx, false);
                        accept_button.set_text(cx, "Waiting...");
                        cancel_button.set_text(cx, "Cancel");
                        cancel_button.set_enabled(cx, true);
                        cancel_button.set_visible(cx, true);
                    }

                    VerificationAction::SasConfirmationError(error) => {
                        self.label(ids!(prompt)).set_text(
                            cx,
                            &format!("Error confirming keys: {}\n\nPlease retry the verification process.", error)
                        );
                        accept_button.set_text(cx, "Ok");
                        accept_button.set_enabled(cx, true);
                        cancel_button.set_visible(cx, false);
                        self.is_final = true;
                    }

                    VerificationAction::RequestCompleted => {
                        self.label(ids!(prompt))
                            .set_text(cx, "Verification completed successfully!");
                        accept_button.set_text(cx, "Ok");
                        accept_button.set_enabled(cx, true);
                        cancel_button.set_visible(cx, false);
                        self.is_final = true;
                    }
                    _ => {}
                }
                // If we received a `VerificationAction`, we need to redraw the modal content.
                needs_redraw = true;
            }
        }

        if needs_redraw {
            self.redraw(cx);
        }
    }
}

impl VerificationModal {
    fn reset_state(&mut self) {
        self.state = None;
        self.is_final = false;
    }

    fn initialize_with_data(&mut self, cx: &mut Cx, state: VerificationRequestActionState) {
        log!("Initializing verification modal with state: {:?}", state);
        let request = &state.request;
        let prompt_text = if request.is_self_verification() {
            Cow::from("Do you wish to verify your own device?")
        } else {
            if let Some(room_id) = request.room_id() {
                format!(
                    "Do you wish to verify user {} in room {}?",
                    request.other_user_id(),
                    room_id,
                )
                .into()
            } else {
                format!("Do you wish to verify user {}?", request.other_user_id()).into()
            }
        };
        self.label(ids!(prompt)).set_text(cx, &prompt_text);

        let accept_button = self.button(ids!(accept_button));
        let cancel_button = self.button(ids!(cancel_button));
        accept_button.set_text(cx, "Yes");
        accept_button.set_enabled(cx, true);
        accept_button.set_visible(cx, true);
        cancel_button.set_text(cx, "Cancel");
        cancel_button.set_enabled(cx, true);
        cancel_button.set_visible(cx, true);

        self.state = Some(state);
        self.is_final = false;
    }
}

impl VerificationModalRef {
    pub fn initialize_with_data(&self, cx: &mut Cx, state: VerificationRequestActionState) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.initialize_with_data(cx, state);
        }
    }
}
