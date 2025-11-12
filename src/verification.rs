use std::sync::Arc;
use futures_util::StreamExt;
use makepad_widgets::{log, ActionDefaultRef, Cx, DefaultNone};
use matrix_sdk_base::crypto::{AcceptedProtocols, CancelInfo, EmojiShortAuthString};
use matrix_sdk::{
    encryption::{
        verification::{SasState, SasVerification, Verification, VerificationRequest, VerificationRequestState}, VerificationState}, ruma::{
        events::{
            key::verification::{request::ToDeviceKeyVerificationRequestEvent, VerificationMethod},
            room::message::{MessageType, OriginalSyncRoomMessageEvent},
        },
        UserId,
    }, Client
};
use tokio::{runtime::Handle, sync::mpsc::{UnboundedReceiver, UnboundedSender}};

#[derive(Clone, Debug, DefaultNone)]
pub enum VerificationStateAction {
    Update(VerificationState),
    None,
}

pub fn add_verification_event_handlers_and_sync_client(client: Client) {
    let mut verification_state_subscriber = client.encryption().verification_state();
    log!("Initial verification state is {:?}", verification_state_subscriber.get());
    Handle::current().spawn(async move {
        while let Some(state) = verification_state_subscriber.next().await {
            log!("Received a verification state update: {state:?}");
            Cx::post_action(VerificationStateAction::Update(state));
            if let VerificationState::Verified = state {
                break;
            }
        }
    });

    client.add_event_handler(
        |ev: ToDeviceKeyVerificationRequestEvent, client: Client| async move {
            if let Some(request) = client
                .encryption()
                .get_verification_request(&ev.sender, &ev.content.transaction_id)
                .await
            {
                Handle::current().spawn(request_verification_handler(client, request));
            }
            else {
                // warning!("Skipping invalid verification request from {}, transaction ID: {}\n   Content: {:?}",
                //     ev.sender, ev.content.transaction_id, ev.content,
                // );
            }
        },
    );

    client.add_event_handler(
        |ev: OriginalSyncRoomMessageEvent, client: Client| async move {
            if let MessageType::VerificationRequest(_) = &ev.content.msgtype {
                if let Some(request) = client
                    .encryption()
                    .get_verification_request(&ev.sender, &ev.event_id)
                    .await
                {
                    Handle::current().spawn(request_verification_handler(client, request));
                }
                else {
                    // warning!("Skipping invalid verification request from {}, event ID: {}\n   Content: {:?}",
                    //     ev.sender, ev.event_id, ev.content,
                    // );
                }
            }
        }
    );
}


async fn dump_devices(user_id: &UserId, client: &Client) -> String {
    let mut devices = String::new();
    for device in client.encryption().get_user_devices(user_id).await.unwrap().devices() {
        let current = client.device_id().is_some_and(|id| id == device.device_id());
        devices.push_str(&format!(
            "    {:<10} {:<30} {:<}{}\n",
            device.device_id(),
            device.display_name().unwrap_or("(unknown name)"),
            if device.is_verified() { "✅" } else { "❌" },
            if current { " <-- this device" } else { "" },
        ));
    }
    format!("Currently-known devices of user {user_id}:\n{}",
        if devices.is_empty() { "    (none)" } else { &devices },
    )
}


async fn sas_verification_handler(
    client: Client,
    sas: SasVerification,
    response_receiver: UnboundedReceiver<VerificationUserResponse>,
) {
    log!(
        "Starting verification with {} {}",
        &sas.other_device().user_id(),
        &sas.other_device().device_id()
    );
    log!("[Pre-verification] {}", dump_devices(sas.other_device().user_id(), &client).await);

    let mut stream = sas.changes();
    // Accept the SAS verification with both default methods: emoji and decimal.
    if let Err(e) = sas.accept().await {
        log!("Error accepting SAS verification request: {:?}", e);
        Cx::post_action(VerificationAction::RequestAcceptError(Arc::new(e)));
        return;
    }

    // A little trick to allow us to move the response_receiver into the async block below.
    let mut receiver_opt = Some(response_receiver);
    while let Some(state) = stream.next().await {
        match state {
            SasState::Created { .. }
            | SasState::Started { .. } => { } // we've already passed these states

            SasState::Accepted { accepted_protocols } => Cx::post_action(
                VerificationAction::SasAccepted(accepted_protocols)
            ),

            SasState::KeysExchanged { emojis, decimals } => {
                Cx::post_action(VerificationAction::KeysExchanged { emojis, decimals });
                if let Some(mut receiver) = receiver_opt.take() {
                    let sas2 = sas.clone();
                    Handle::current().spawn(async move {
                        log!("Waiting for user to confirm SAS verification keys...");
                        match receiver.recv().await {
                            Some(VerificationUserResponse::Accept) => {
                                log!("User confirmed SAS verification keys");
                                if let Err(e) = sas2.confirm().await {
                                    log!("Failed to confirm SAS verification keys; error: {:?}", e);
                                    Cx::post_action(VerificationAction::SasConfirmationError(Arc::new(e)));
                                }
                                // If successful, SAS verification will now transition to the Confirmed state,
                                // which will be sent to the main UI thread in the `SasState::Confirmed` match arm below.
                            }
                            Some(VerificationUserResponse::Cancel) | None => {
                                log!("User did not confirm SAS verification keys");
                                let _ = sas2.cancel().await;
                            }
                        }
                    });
                } else {
                    // Receiving a second `KeysExchanged` state indicates that the other device
                    // confirmed their keys match the ones we have *before* we confirmed them.
                    log!("The other side confirmed that the displayed keys matched.");
                };

            }

            SasState::Confirmed => Cx::post_action(VerificationAction::SasConfirmed),

            SasState::Done { verified_devices, verified_identities } => {
                let device = sas.other_device();
                log!("SAS verification done.
                    Devices: {verified_devices:?}
                    Identities: {verified_identities:?}",
                );
                log!(
                    "Successfully verified device {} {} {:?}",
                    device.user_id(),
                    device.device_id(),
                    device.local_trust_state()
                );
                log!("[Post-verification] {}", dump_devices(sas.other_device().user_id(), &client).await);
                // We go ahead and send the RequestCompleted action here,
                // because it is not guaranteed that the VerificationRequestState stream loop
                // will receive an update an enter the `Done` state.
                Cx::post_action(VerificationAction::RequestCompleted);
                break;
            }
            SasState::Cancelled(cancel_info) => {
                log!("SAS verification has been cancelled, reason: {}", cancel_info.reason());
                // We go ahead and send the RequestCancelled action here,
                // because it is not guaranteed that the VerificationRequestState stream loop
                // will receive an update an enter the `Cancelled` state.
                Cx::post_action(VerificationAction::RequestCancelled(cancel_info));
                break;
            }
        }
    }
}

async fn request_verification_handler(client: Client, request: VerificationRequest) {
    log!("Received a verification request in room {:?}: {:?}", request.room_id(), request.state());
    let (sender, mut response_receiver) = tokio::sync::mpsc::unbounded_channel::<VerificationUserResponse>();
    Cx::post_action(
        VerificationAction::RequestReceived(
            VerificationRequestActionState {
                request: request.clone(),
                response_sender: sender.clone(),
            }
        )
    );

    let mut stream = request.changes();

    // We currently only support SAS verification.
    let supported_methods = vec![VerificationMethod::SasV1];
    match response_receiver.recv().await {
        Some(VerificationUserResponse::Accept) => match request.accept_with_methods(supported_methods).await {
            Ok(()) => {
                Cx::post_action(VerificationAction::RequestAccepted);
                // Fall through to the stream loop below.
            }
            Err(e) => {
                Cx::post_action(VerificationAction::RequestAcceptError(Arc::new(e)));
                return;
            }
        }
        Some(VerificationUserResponse::Cancel) | None => match request.cancel().await {
            Ok(()) => { } // response will be sent in the stream loop below
            Err(e) => {
                Cx::post_action(VerificationAction::RequestCancelError(Arc::new(e)));
                return;
            }
        }
    };

    while let Some(state) = stream.next().await {
        match state {
            VerificationRequestState::Created { .. }
            | VerificationRequestState::Requested { .. }
            | VerificationRequestState::Ready { .. } => { }
            VerificationRequestState::Transitioned { verification } => match verification {
                // We only support SAS verification.
                Verification::SasV1(sas) => {
                    log!("Verification request transitioned to SAS V1.");
                    Handle::current().spawn(sas_verification_handler(client, sas, response_receiver));
                    return;
                }
                unsupported => {
                    log!("Verification request transitioned to unsupported method: {:?}", unsupported);
                    Cx::post_action(VerificationAction::RequestTransitionedToUnsupportedMethod(unsupported));
                    return;
                }
            }
            VerificationRequestState::Cancelled(info) => {
                log!("Verification request was cancelled, reason: {}", info.reason());
                Cx::post_action(VerificationAction::RequestCancelled(info));
            }
            VerificationRequestState::Done => {
                log!("Verification request is done!");
                Cx::post_action(VerificationAction::RequestCompleted);
                return;
            }
        }
    }
}


/// Actions related to verification that should be handled by the top-level app context.
#[derive(Clone, Debug, DefaultNone)]
pub enum VerificationAction {
    /// Informs the main UI thread that a verification request has been received.
    RequestReceived(VerificationRequestActionState),
    /// Informs the main UI thread that a verification request was cancelled successfully.
    RequestCancelled(CancelInfo),
    /// Informs the main UI thread that a verification request was accepted successfully.
    /// This is effectively just a status update for the sake of user awareness;
    /// the user doesn't need to do anything to respond to this, but rather only needs
    /// to wait for the verification to proceed to the next step.
    RequestAccepted,
    /// Informs the main UI thread that an error occurred while accepting a verification request.
    RequestAcceptError(Arc<matrix_sdk::Error>),
    /// Informs the main UI thread that an error occurred while cancelling a verification request.
    RequestCancelError(Arc<matrix_sdk::Error>),
    /// Informs the main UI thread that a verification request transitioned to an unsupported method.
    RequestTransitionedToUnsupportedMethod(Verification),
    /// Informs the main UI thread that the given SAS verification protocols
    /// have been accepted by both sides.
    /// This is effectively just a status update for the sake of user awareness;
    /// the user doesn't need to do anything to respond to this, but rather only needs
    /// to wait for the verification to proceed to the next step, i.e., KeysExchanged.
    SasAccepted(AcceptedProtocols),
    /// Informs the main UI thread that the SAS verification has exchanged keys with the other side.
    /// The UI should display the given keys to the user for interactive confirmation.
    KeysExchanged {
        emojis: Option<EmojiShortAuthString>,
        decimals: (u16, u16, u16),
    },
    /// Informs the main UI thread that SAS verification keys have been confirmed by the current user,
    /// and that we're just waiting for the other side to confirm too.
    SasConfirmed,
    /// Informs the main UI thread that an error occurred while confirming SAS verification keys.
    SasConfirmationError(Arc<matrix_sdk::Error>),
    /// Informs the main UI thread that a verification request has been fully completed.
    RequestCompleted,
    None,
}

/// The state included in a verification request action.
///
/// This is passed from the background async task to the main UI thread,
/// where it is extracted from the `VerificationAction` and then stored
/// in the `VerificationModal`` widget.
#[derive(Clone, Debug)]
pub struct VerificationRequestActionState {
    pub request: VerificationRequest,
    pub response_sender: UnboundedSender<VerificationUserResponse>,
}

/// Responses that the user can make to a verification request,
/// which are then sent from the main UI thread to the background async task
/// that originally received the verification request.
pub enum VerificationUserResponse {
    Accept,
    Cancel,
}
