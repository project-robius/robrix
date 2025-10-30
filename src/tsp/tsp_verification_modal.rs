
use makepad_widgets::*;
use tsp_sdk::AsyncSecureStore;

use crate::{shared::styles::*, sliding_sync::current_user_id, tsp::{submit_tsp_request, TspRequest, TspVerificationDetails}};

live_design! {
    link tsp_enabled

    use link::theme::*;
    use link::widgets::*;

    use crate::shared::styles::*;
    use crate::shared::icon_button::RobrixIconButton;

    pub TspVerificationModal = {{TspVerificationModal}} {
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
                    text: "TSP Verification Request"
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
                        text: "Ignore Request"
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
                        text: "Accept Request"
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
pub struct TspVerificationModal {
    #[deref] view: View,
    #[rust] state: TspVerificationModalState,
}

#[derive(Default)]
enum TspVerificationModalState {
    #[default]
    Initial,
    ReceivedRequest {
        details: TspVerificationDetails,
        wallet_db: AsyncSecureStore,
    },
    RequestAccepted {
        details: TspVerificationDetails,
        wallet_db: AsyncSecureStore,
    },
    RequestVerified,
    RequestDeclined,
}
impl TspVerificationModalState {
    fn details(&self) -> Option<&TspVerificationDetails> {
        match self {
            TspVerificationModalState::ReceivedRequest { details, .. }
            | TspVerificationModalState::RequestAccepted { details, .. } => Some(details),
            _ => None,
        }
    }
}

/// Actions emitted by or send to the `TspVerificationModal`.
#[derive(Debug)]
pub enum TspVerificationModalAction {
    /// Emitted by the `TspVerificationModal` when it should be closed by its parent widget.
    Close,
    /// The result of sending a DID association response.
    /// This action is sent from the background TSP worker task.
    SentDidAssociationResponse {
        details: TspVerificationDetails,
        result: Result<(), anyhow::Error>,
    },
}

impl Widget for TspVerificationModal {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        self.widget_match_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl WidgetMatchEvent for TspVerificationModal {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions, _scope: &mut Scope) {
        let accept_button = self.button(ids!(accept_button));
        let cancel_button = self.button(ids!(cancel_button));

        let cancel_button_clicked = cancel_button.clicked(actions);
        let modal_dismissed = actions
            .iter()
            .any(|a| matches!(a.downcast_ref(), Some(ModalAction::Dismissed)));

        if cancel_button_clicked || modal_dismissed {
            match &self.state {
                TspVerificationModalState::ReceivedRequest { details, wallet_db }
                | TspVerificationModalState::RequestAccepted { details, wallet_db } => {
                    submit_tsp_request(TspRequest::RespondToDidAssociationRequest {
                        details: details.clone(),
                        wallet_db: wallet_db.clone(),
                        accepted: false,
                    });
                }
                _ => {}
            }

            // If the modal was dismissed by clicking outside of it, we MUST NOT emit
            // a `TspVerificationModalAction::Close` action, as that would cause
            // an infinite action feedback loop.
            if !modal_dismissed {
                cx.action(TspVerificationModalAction::Close);
            }
            return;
        }

        let prompt_label = self.view.label(ids!(prompt));
        if accept_button.clicked(actions) {
            let current_state = std::mem::take(&mut self.state);
            let new_state: TspVerificationModalState;
            match current_state {
                TspVerificationModalState::ReceivedRequest { details, wallet_db } => {
                    // Here, we need to confirm that the receiving VID (our VID) is actually in
                    // the wallet. If not, we need to show an error instructing the user
                    // to add that VID to their wallet first and then retry the verification process.
                    // Then, we need to send a negative response to the initiator of the request.
                    let error_text = if !wallet_db.has_private_vid(&details.responding_vid).is_ok_and(|v| v) {
                        Some(format!(
                            "Error: the VID \"{}\" was not found in your current wallet.\n\n\
                            Either the requestor has the wrong VID for you, or you have not yet added that VID to your wallet.\n\n\
                            Once you have addressed this, please retry the verification process again.",
                            details.responding_vid
                        ))
                    } else if current_user_id().as_ref() != Some(&details.responding_user_id) {
                        Some(format!(
                            "Error: the verification request was intended for Matrix User ID \"{}\", \
                            but you are not logged in as that user.\n\n\
                            Either the requestor has the wrong Matrix ID for you, or you are logged into a different account.\n\n\
                            Once you have addressed this, please retry the verification process again.",
                            details.responding_user_id
                        ))
                    } else {
                        None
                    };
                    if let Some(error_text) = error_text {
                        prompt_label.set_text(cx, &error_text);
                        submit_tsp_request(TspRequest::RespondToDidAssociationRequest {
                            details: details.clone(),
                            wallet_db: wallet_db.clone(),
                            accepted: false,
                        });
                        cancel_button.set_visible(cx, false);
                        accept_button.apply_over(cx, live!(
                            text: "Okay",
                            draw_bg: {
                                color: (COLOR_ACTIVE_PRIMARY),
                            },
                            draw_icon: {
                                color: (COLOR_PRIMARY),
                            }
                            draw_text: {
                                color: (COLOR_PRIMARY),
                            },
                        ));
                        new_state = TspVerificationModalState::RequestDeclined;
                    }
                    else {
                        let prompt = format!("You have accepted the TSP verification request.\n\n\
                            Please confirm that the following code matches for both users:\n\n\
                            Code: \"{}\"\n",
                            details.random_str,
                        );
                        prompt_label.set_text(cx, &prompt);
                        accept_button.set_text(cx, "Yes, they match!");
                        new_state = TspVerificationModalState::RequestAccepted { details, wallet_db };
                    }
                }

                TspVerificationModalState::RequestAccepted { details, wallet_db } => {
                    submit_tsp_request(TspRequest::RespondToDidAssociationRequest {
                        details: details.clone(),
                        wallet_db: wallet_db.clone(),
                        accepted: true,
                    });
                    let prompt_text = "You have confirmed the TSP verification request.\n\nSending a response now...";
                    prompt_label.set_text(cx, prompt_text);
                    accept_button.set_enabled(cx, false);
                    // stay in this same state until we get an acknowledgment back 
                    // that we sent the response (the `SentDidAssociationResponse` action).
                    new_state = TspVerificationModalState::RequestAccepted { details, wallet_db };
                }

                TspVerificationModalState::Initial
                | TspVerificationModalState::RequestDeclined
                | TspVerificationModalState::RequestVerified => {
                    cx.action(TspVerificationModalAction::Close);
                    return;
                }
            }
            self.state = new_state;
        }

        for action in actions {
            match action.downcast_ref() {
                Some(TspVerificationModalAction::SentDidAssociationResponse { details, result }) 
                    if self.state.details().is_some_and(|d| d == details) =>
                {
                    match result {
                        Ok(()) => {
                            self.label(ids!(prompt)).set_text(cx, "The TSP verification process has completed successfully.\n\nYou may now close this.");
                            self.state = TspVerificationModalState::RequestVerified;
                        }
                        Err(e) => {
                            self.label(ids!(prompt)).set_text(cx, &format!("Error: failed to complete the TSP verification process:\n\n{e}"));
                            self.state = TspVerificationModalState::RequestDeclined;
                        }
                    }
                    cancel_button.set_visible(cx, false);
                    accept_button.apply_over(cx, live!(
                        enabled: true,
                        text: "Okay",
                        draw_bg: {
                            color: (COLOR_ACTIVE_PRIMARY),
                        },
                        draw_icon: {
                            color: (COLOR_PRIMARY),
                        }
                        draw_text: {
                            color: (COLOR_PRIMARY),
                        }
                    ));
                    self.redraw(cx);
                }
                _ => {}
            }
        }
    }
}

impl TspVerificationModal {
    fn initialize_with_details(
        &mut self,
        cx: &mut Cx,
        details: TspVerificationDetails,
        wallet_db: AsyncSecureStore,
    ) {
        log!("Initializing TSP verification modal with: {:?}", details);
        let prompt_text = format!("Matrix User \"{}\" is requesting to verify your identity via TSP.\n\
            Their TSP identity is: \"{}\".\n\n\
            They want to verify your TSP identity \"{}\" associated with Matrix User ID \"{}\".\n\n\
            If you recognize these details, would you like to accept this request?",
            details.initiating_user_id,
            details.initiating_vid,
            details.responding_vid,
            details.responding_user_id,
        );
        self.label(ids!(prompt)).set_text(cx, &prompt_text);

        let accept_button = self.button(ids!(accept_button));
        let cancel_button = self.button(ids!(cancel_button));
        accept_button.set_text(cx, "Accept Request");
        accept_button.set_enabled(cx, true);
        accept_button.set_visible(cx, true);
        accept_button.reset_hover(cx);
        cancel_button.set_text(cx, "Ignore (Decline)");
        cancel_button.set_enabled(cx, true);
        cancel_button.set_visible(cx, true);
        cancel_button.reset_hover(cx);

        self.state = TspVerificationModalState::ReceivedRequest {
            details,
            wallet_db,
        };
    }
}

impl TspVerificationModalRef {
    /// Initialize this modal with the details of a TSP verification request.
    pub fn initialize_with_details(
        &self,
        cx: &mut Cx, 
        details: TspVerificationDetails,
        wallet_db: AsyncSecureStore,
    ) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.initialize_with_details(cx, details, wallet_db);
        }
    }
}
