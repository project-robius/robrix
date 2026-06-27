use std::borrow::Cow;

use makepad_widgets::*;
use matrix_sdk::encryption::verification::Verification;

use crate::verification::{VerificationAction, VerificationRequestActionState, VerificationUserResponse};

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*


    mod.widgets.VerificationEmojiCell = View {
        width: Fit, height: Fit
        flow: Down
        align: Align{x: 0.5}
        padding: Inset{top: 6, right: 6, bottom: 6, left: 6}
        spacing: 4

        symbol := Label {
            width: Fit, height: Fit
            align: Align{x: 0.5}
            draw_text +: {
                text_style: REGULAR_TEXT {font_size: 30},
                color: #000
            }
        }
        description := Label {
            width: Fit{max: FitBound.Abs(72.0)}, height: Fit
            flow: Flow.Right{wrap: true}
            align: Align{x: 0.5}
            draw_text +: {
                text_style: REGULAR_TEXT {font_size: 9},
                color: #000
            }
        }
    }

    mod.widgets.VerificationModal = set_type_default() do #(VerificationModal::register_widget(vm)) {
        ..mod.widgets.SmallModal

        title := ModalTitle {
            text: "Verification Request"
        }

        body := ModalBody {}

        // SAS V1 always produces exactly 7 emojis, so 7 cells are declared up front.
        emojis_view := View {
            width: Fill, height: Fit
            flow: Flow.Right{wrap: true}
            align: Align{x: 0.5}
            spacing: 10
            margin: Inset{top: 15, bottom: 5}
            visible: false

            emoji0 := mod.widgets.VerificationEmojiCell {}
            emoji1 := mod.widgets.VerificationEmojiCell {}
            emoji2 := mod.widgets.VerificationEmojiCell {}
            emoji3 := mod.widgets.VerificationEmojiCell {}
            emoji4 := mod.widgets.VerificationEmojiCell {}
            emoji5 := mod.widgets.VerificationEmojiCell {}
            emoji6 := mod.widgets.VerificationEmojiCell {}
        }

        question := ModalBody {
            margin: Inset{top: 10}
            visible: false
        }

        buttons_view := ModalButtonsRow {
            margin: Inset{top: 30}

            accept_button := RobrixPositiveIconButton {
                align: Align{x: 0.5, y: 0.5}
                padding: 15,
                draw_icon.svg: (ICON_CHECKMARK)
                icon_walk: Walk{width: 16, height: 16, margin: Inset{left: -2, right: -1} }
                text: "Yes"
            }

            cancel_button := RobrixNegativeIconButton {
                align: Align{x: 0.5, y: 0.5}
                padding: 15,
                draw_icon.svg: (ICON_FORBIDDEN)
                icon_walk: Walk{width: 16, height: 16, margin: Inset{left: -2, right: -1} }
                text: "Cancel"
            }
        }
    }
}

#[derive(Script, ScriptHook, Widget)]
pub struct VerificationModal {
    #[deref] view: View,
    #[rust] state: Option<VerificationRequestActionState>,
    /// Whether the modal is in a "final" state,
    /// meaning that the verification process has ended
    /// and that any further interaction with it should close the modal.
    #[rust(false)] is_final: bool,
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
        let accept_button = self.button(cx, ids!(accept_button));
        let cancel_button = self.button(cx, ids!(cancel_button));

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
                // Outgoing verification requests start with the accept button hidden
                // since we're still in the waiting state then, so show it now
                accept_button.set_visible(cx, true);
                // The emoji grid and its question prompt are only relevant during
                // the `KeysExchanged` emoji step; hide them by default and let that
                // branch re-show them, so they never leak into other states.
                self.view.view(cx, ids!(emojis_view)).set_visible(cx, false);
                self.label(cx, ids!(question)).set_visible(cx, false);
                match verification_action {
                    VerificationAction::RequestCancelled(cancel_info) => {
                        self.label(cx, ids!(body)).set_text(
                            cx,
                            &format!("Verification request was cancelled: {}", cancel_info.reason())
                        );
                        accept_button.set_enabled(cx, true);
                        accept_button.set_text(cx, "Ok");
                        cancel_button.set_visible(cx, false);
                        self.is_final = true;
                    }

                    VerificationAction::RequestAccepted => {
                        self.label(cx, ids!(body)).set_text(
                            cx,
                            "You successfully accepted the verification request.\n\n\
                            Waiting for the other device to agree on verification methods..."
                        );
                        accept_button.set_enabled(cx, false);
                        accept_button.set_text(cx, "Waiting...");
                        cancel_button.set_text(cx, "Cancel");
                        cancel_button.set_enabled(cx, true);
                        cancel_button.set_visible(cx, true);
                    }

                    VerificationAction::RequestAcceptError(error) => {
                        self.label(cx, ids!(body)).set_text(cx, 
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
                        self.label(cx, ids!(body)).set_text(
                            cx,
                            &format!("Error cancelling verification request: {}.", error)
                        );
                        accept_button.set_enabled(cx, true);
                        accept_button.set_text(cx, "Ok");
                        cancel_button.set_visible(cx, false);
                        self.is_final = true;
                    }

                    VerificationAction::RequestTransitionedToUnsupportedMethod(method) => {
                        self.label(cx, ids!(body)).set_text(
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
                        self.label(cx, ids!(body)).set_text(
                            cx,
                            "Both sides have accepted the same verification method(s).\n\n\
                            Waiting for both devices to exchange keys..."
                        );
                        accept_button.set_enabled(cx, false);
                        accept_button.set_text(cx, "Waiting...");
                        cancel_button.set_text(cx, "Cancel");
                        cancel_button.set_enabled(cx, true);
                        cancel_button.set_visible(cx, true);
                    }

                    VerificationAction::KeysExchanged { emojis, decimals } => {
                        if let Some(emoji_list) = emojis {
                            self.label(cx, ids!(body)).set_text(
                                cx,
                                "Keys have been exchanged. Please verify the following emoji:",
                            );
                            // SAS V1 always yields 7 emojis.
                            for index in 0 .. 7 {
                                let content = emoji_list.emojis
                                    .get(index)
                                    .map(|em| (em.symbol, em.description));
                                self.populate_emoji_cell(cx, index, content);
                            }
                            self.view.view(cx, ids!(emojis_view)).set_visible(cx, true);
                            self.label(cx, ids!(question)).set_text(cx, "Do these emoji keys match?");
                            self.label(cx, ids!(question)).set_visible(cx, true);
                        } else {
                            let text = format!(
                                "Keys have been exchanged. Please verify the following numbers:\n\
                                \n   {}\n   {}\n   {}\n\n\
                                Do these number keys match?",
                                decimals.0, decimals.1, decimals.2,
                            );
                            self.label(cx, ids!(body)).set_text(cx, &text);
                        }
                        accept_button.set_enabled(cx, true);
                        accept_button.set_text(cx, "They match");
                        cancel_button.set_text(cx, "They don't match");
                        cancel_button.set_enabled(cx, true);
                        cancel_button.set_visible(cx, true);
                    }

                    VerificationAction::SasConfirmed => {
                        self.label(cx, ids!(body)).set_text(
                            cx,
                            "You successfully confirmed the Short Auth String keys.\n\n\
                            Waiting for the other device to confirm..."
                        );
                        accept_button.set_enabled(cx, false);
                        accept_button.set_text(cx, "Waiting...");
                        cancel_button.set_text(cx, "Cancel");
                        cancel_button.set_enabled(cx, true);
                        cancel_button.set_visible(cx, true);
                    }

                    VerificationAction::SasConfirmationError(error) => {
                        self.label(cx, ids!(body)).set_text(
                            cx,
                            &format!("Error confirming keys: {}\n\nPlease retry the verification process.", error)
                        );
                        accept_button.set_text(cx, "Ok");
                        accept_button.set_enabled(cx, true);
                        cancel_button.set_visible(cx, false);
                        self.is_final = true;
                    }

                    VerificationAction::RequestCompleted => {
                        self.label(cx, ids!(body)).set_text(
                            cx,
                            "Verification completed successfully! \
                            Now you can read or send messages securely, and anyone you chat \
                            with can also trust this device.",
                        );
                        accept_button.set_text(cx, "Ok");
                        accept_button.set_enabled(cx, true);
                        cancel_button.set_visible(cx, false);
                        self.is_final = true;
                    }
                    _ => { }
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

    fn populate_emoji_cell(&mut self, cx: &mut Cx, index: usize, content: Option<(&str, &str)>) {
        let (symbol_text, description_text, visible) = match content {
            Some((symbol, description)) => (symbol, description, true),
            None => ("", "", false),
        };
        let cell = match index {
            0 => self.view.view(cx, ids!(emoji0)),
            1 => self.view.view(cx, ids!(emoji1)),
            2 => self.view.view(cx, ids!(emoji2)),
            3 => self.view.view(cx, ids!(emoji3)),
            4 => self.view.view(cx, ids!(emoji4)),
            5 => self.view.view(cx, ids!(emoji5)),
            6 => self.view.view(cx, ids!(emoji6)),
            _ => return,
        };
        cell.label(cx, ids!(symbol)).set_text(cx, symbol_text);
        cell.label(cx, ids!(description)).set_text(cx, description_text);
        cell.set_visible(cx, visible);
    }

    fn initialize_with_data(
        &mut self,
        cx: &mut Cx,
        state: VerificationRequestActionState,
    ) {
        log!("Initializing verification modal with state: {:?}", state);
        let request = &state.request;
        // `we_started` means this is an outgoing request we just sent, so we don't need
        // to accept it, but rather just wait for another device to accept & respond.
        let we_started = request.we_started();
        let prompt_text = if we_started {
            Cow::from("Send a verification request to your other logged-in devices.\n\n\
                Accept it on one of those devices to continue verifying this device.")
        } else if request.is_self_verification() {
            Cow::from("Do you wish to verify your own device?")
        } else if let Some(room_id) = request.room_id() {
            format!("Do you wish to verify user {} in room {}?",
                request.other_user_id(),
                room_id,
            ).into()
        } else {
            format!("Do you wish to verify user {}?",
                request.other_user_id()
            ).into()
        };
        self.label(cx, ids!(body)).set_text(cx, &prompt_text);
        // Ensure the emoji grid from any prior verification is not shown
        // on the initial prompt screen.
        self.view.view(cx, ids!(emojis_view)).set_visible(cx, false);
        self.label(cx, ids!(question)).set_visible(cx, false);

        let accept_button = self.button(cx, ids!(accept_button));
        let cancel_button = self.button(cx, ids!(cancel_button));
        accept_button.set_text(cx, "Yes");
        accept_button.set_enabled(cx, !we_started);
        accept_button.set_visible(cx, !we_started);
        cancel_button.set_text(cx, "Cancel");
        cancel_button.set_enabled(cx, true);
        cancel_button.set_visible(cx, true);

        self.state = Some(state);
        self.is_final = false;
    }
}

impl VerificationModalRef {
    pub fn initialize_with_data(
        &self,
        cx: &mut Cx, 
        state: VerificationRequestActionState,
    ) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.initialize_with_data(cx, state);
        }
    }
}
