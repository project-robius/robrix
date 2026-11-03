use std::fmt::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use futures_util::StreamExt;
use makepad_widgets::{error, log, Cx, SignalToUI};
use matrix_sdk_base::crypto::{AcceptedProtocols, CancelInfo, EmojiShortAuthString, IdentityState};
use matrix_sdk::{
    encryption::{
        identities::UserIdentity, verification::{SasState, SasVerification, Verification, VerificationRequest, VerificationRequestState}, VerificationState}, ruma::{
        events::{
            key::verification::{request::ToDeviceKeyVerificationRequestEvent, VerificationMethod},
            room::message::{MessageType, OriginalSyncRoomMessageEvent},
        },
        OwnedUserId, UserId,
    }, Client, Room
};
use tokio::{runtime::Handle, sync::mpsc::{UnboundedReceiver, UnboundedSender}};

use crate::{
    home::{identity_reset_banner::IdentityReset, room_screen::TimelineUpdate},
    profile::user_verification_cache::{enqueue_user_verification_update, UserVerificationState, UserVerificationUpdate},
    shared::popup_list::{enqueue_popup_notification, PopupKind},
    sliding_sync::refresh_encryption_info_of_messages,
};

/// Whether a verification flow is currently in progress. See [`VerificationInProgress`].
static VERIFICATION_IN_PROGRESS: AtomicBool = AtomicBool::new(false);

/// A guard type to prevent multiple simultaneous verifications.
struct VerificationInProgress;
impl VerificationInProgress {
    /// Returns `None` if another verification flow is already in progress.
    fn start() -> Option<Self> {
        VERIFICATION_IN_PROGRESS
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
            .then_some(Self)
    }
}
impl Drop for VerificationInProgress {
    fn drop(&mut self) {
        VERIFICATION_IN_PROGRESS.store(false, Ordering::Release);
    }
}

/// A guard that posts [`UserVerificationRequestAction::Done`] when dropped,
/// so the UI always hears about it, even if sending the request failed.
struct SendingVerificationRequest(OwnedUserId);
impl Drop for SendingVerificationRequest {
    fn drop(&mut self) {
        Cx::post_action(UserVerificationRequestAction::Done(self.0.clone()));
    }
}

/// Actions about sending a verification request to another user.
///
/// These are NOT widget actions.
#[derive(Debug)]
pub enum UserVerificationRequestAction {
    /// We need a DM room with this user to send them the request, but we don't have one,
    /// so we're waiting for the user to tell us whether we should create one.
    ConfirmDmRoomCreation {
        user_id: OwnedUserId,
        response_sender: UnboundedSender<VerificationUserResponse>,
    },
    /// We've either sent a verification request to this user or failed to.
    Done(OwnedUserId),
}

#[derive(Clone, Debug)]
pub enum VerificationStateAction {
    Update(VerificationState),
}


pub fn add_verification_event_handlers_and_sync_client(client: Client) -> tokio::task::JoinHandle<()> {
    let mut verification_state_subscriber = client.encryption().verification_state();
    log!("Initial verification state is {:?}", verification_state_subscriber.get());
    let verification_state_handle = Handle::current().spawn(async move {
        while let Some(state) = verification_state_subscriber.next().await {
            log!("Received a verification state update: {state:?}");
            Cx::post_action(VerificationStateAction::Update(state));
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

    verification_state_handle
}

/// Keeps the UI's cache of users' verification states up to date as their identities change.
pub fn handle_user_identities_subscriber(client: Client) -> tokio::task::JoinHandle<()> {
    Handle::current().spawn(async move {
        let Some(own_user_id) = client.user_id().map(ToOwned::to_owned) else { return };
        let identity_updates = match client.encryption().user_identities_stream().await {
            Ok(stream) => stream,
            Err(e) => {
                error!("Failed to subscribe to user identity updates: {e:?}");
                return;
            }
        };
        let mut identity_updates = std::pin::pin!(identity_updates);
        while let Some(updates) = identity_updates.next().await {
            if updates.new.contains_key(&own_user_id) || updates.changed.contains_key(&own_user_id) {
                enqueue_user_verification_update(UserVerificationUpdate::OwnIdentityChanged);
                refresh_encryption_info_of_messages(None).await;
                continue;
            }
            let changed_user_ids: Vec<OwnedUserId> = updates.changed.keys().cloned().collect();
            let states: Vec<_> = updates.new.into_iter()
                .chain(updates.changed)
                .map(|(user_id, identity)| (user_id, UserVerificationState::from(&identity)))
                .collect();
            if !states.is_empty() {
                enqueue_user_verification_update(UserVerificationUpdate::States(states));
            }
            if !changed_user_ids.is_empty() {
                refresh_encryption_info_of_messages(Some(&changed_user_ids)).await;
            }
        }
    })
}

/// Sends the members of the given room whose identities were reset to that room's timeline,
/// first right away and then again whenever that list changes.
pub async fn identity_resets_subscriber_handler(room: Room, sender: crossbeam_channel::Sender<TimelineUpdate>) {
    // Identity resets only matter in encrypted rooms, and watching for them means loading every member's identity.
    if !room.latest_encryption_state().await.is_ok_and(|state| state.is_encrypted()) {
        return;
    }
    let changes = match room.subscribe_to_identity_status_changes().await {
        Ok(changes) => changes,
        Err(e) => {
            error!("Failed to subscribe to identity changes in room {}: {e:?}", room.room_id());
            return;
        }
    };
    let mut changes = std::pin::pin!(changes);
    // Each member whose identity was reset, and whether we had verified them before that.
    let mut resets: Vec<(OwnedUserId, bool)> = Vec::new();
    loop {
        let mut identity_resets = Vec::with_capacity(resets.len());
        for (user_id, was_verified) in &resets {
            let display_name = room.get_member_no_sync(user_id).await.ok().flatten()
                .and_then(|member| member.display_name().map(ToOwned::to_owned));
            identity_resets.push(IdentityReset { user_id: user_id.clone(), display_name, was_verified: *was_verified });
        }
        if sender.send(TimelineUpdate::IdentityResets(identity_resets)).is_err() {
            return;
        }
        SignalToUI::set_ui_signal();

        let Some(changes) = changes.next().await else { return };
        for change in changes {
            resets.retain(|(user_id, _)| *user_id != change.user_id);
            match change.changed_to {
                IdentityState::PinViolation => resets.push((change.user_id, false)),
                IdentityState::VerificationViolation => resets.push((change.user_id, true)),
                IdentityState::Pinned | IdentityState::Verified => { }
            }
        }
    }
}


async fn dump_devices(user_id: &UserId, client: &Client) -> String {
    let our_devices = match client.encryption().get_user_devices(user_id).await {
        Ok(d) => d,
        Err(e) => return format!("Couldn't get the list of devices for user {user_id}: {e}"),
    };
    let mut devices = String::new();
    for device in our_devices.devices() {
        let current = client.device_id().is_some_and(|id| id == device.device_id());
        let _ = writeln!(&mut devices,
            "    {:<10} {:<30} {:<}{}",
            device.device_id(),
            device.display_name().unwrap_or("(unknown name)"),
            if device.is_verified() { "✅" } else { "❌" },
            if current { " <-- this device" } else { "" },
        );
    }
    format!("Currently-known devices of user {user_id}:\n{}",
        if devices.is_empty() { "    (none)" } else { &devices },
    )
}


async fn sas_verification_handler(
    client: Client,
    sas: SasVerification,
    mut response_receiver: UnboundedReceiver<VerificationUserResponse>,
    _in_progress: VerificationInProgress,
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

    // Poll the modal's responses alongside the stream of verification state updates,
    // so that we can actually handle the user canceling the verification at any point.
    enum KeyConfirmation {
        NotYetAsked,
        WaitingOnUser,
        UserAnswered,
    }
    let mut key_confirmation = KeyConfirmation::NotYetAsked;

    loop {
        let state = tokio::select! {
            response = response_receiver.recv() => {
                match response {
                    Some(VerificationUserResponse::Accept)
                        if matches!(key_confirmation, KeyConfirmation::WaitingOnUser) =>
                    {
                        key_confirmation = KeyConfirmation::UserAnswered;
                        log!("User confirmed SAS verification keys");
                        let sas2 = sas.clone();
                        Handle::current().spawn(async move {
                            if let Err(e) = sas2.confirm().await {
                                log!("Failed to confirm SAS verification keys; error: {:?}", e);
                                Cx::post_action(VerificationAction::SasConfirmationError(Arc::new(e)));
                            }
                        });
                    }
                    // An old or duplicate `Accept` shouldn't be treated as us confirming the keys match.
                    Some(VerificationUserResponse::Accept) => { }
                    Some(VerificationUserResponse::Mismatch) => {
                        log!("User reported that the SAS keys did not match");
                        if !sas.is_done() {
                            let _ = sas.mismatch().await;
                        }
                        return;
                    }
                    // `None` means the modal was dismissed, meaning that verification was cancelled.
                    Some(VerificationUserResponse::Cancel) | None => {
                        log!("User cancelled the SAS verification");
                        // Don't cancel a verification that already finished successfully.
                        if !sas.is_done() {
                            let _ = sas.cancel().await;
                        }
                        return;
                    }
                }
                continue;
            }
            state = stream.next() => match state {
                Some(state) => state,
                None => return,
            },
        };

        match state {
            SasState::Created { .. }
            | SasState::Started { .. } => { } // we've already passed these states

            SasState::Accepted { accepted_protocols } => Cx::post_action(
                VerificationAction::SasAccepted(accepted_protocols)
            ),

            SasState::KeysExchanged { emojis, decimals } => match key_confirmation {
                KeyConfirmation::NotYetAsked => {
                    Cx::post_action(VerificationAction::KeysExchanged { emojis, decimals });
                    log!("Waiting for user to confirm SAS verification keys...");
                    key_confirmation = KeyConfirmation::WaitingOnUser;
                }
                KeyConfirmation::WaitingOnUser | KeyConfirmation::UserAnswered => {
                    log!("The other side confirmed that the displayed keys matched.");
                }
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
    // A self-verification request we just sent can get delivered to us via homeserver sync,
    // so we must ignore that instead of treating it as a new request.
    if request.we_started() {
        return;
    }
    let mut stream = request.changes();
    let state = request.state();
    log!("Received a verification request; {state:?}, room {:?}", request.room_id());
    if matches!(state, VerificationRequestState::Cancelled(_) | VerificationRequestState::Done) {
        return;
    }
    let Some(in_progress) = VerificationInProgress::start() else {
        log!("Declining verification request: another verification is already in progress.");
        let _ = request.cancel().await;
        return;
    };
    let (sender, mut response_receiver) = tokio::sync::mpsc::unbounded_channel::<VerificationUserResponse>();
    Cx::post_action(
        VerificationAction::RequestReceived(
            VerificationRequestActionState {
                request: request.clone(),
                // Don't clone this sender, as we rely on it being dropped to wake up the recv side
                response_sender: sender,
            }
        )
    );

    let mut accepted = false;
    // If the other side starts SAS before the user clicks Yes,
    // that's fine, we just wait til they click Yes locally
    let mut early_sas = None;
    let sas = loop {
        tokio::select! {
            response = response_receiver.recv() => match response {
                Some(VerificationUserResponse::Accept) if !accepted => {
                    // We currently only support SAS verification.
                    match request.accept_with_methods(vec![VerificationMethod::SasV1]).await {
                        Ok(()) => {
                            accepted = true;
                            Cx::post_action(VerificationAction::RequestAccepted);
                            if let Some(sas) = early_sas.take() {
                                break sas;
                            }
                        }
                        Err(e) => {
                            Cx::post_action(VerificationAction::RequestAcceptError(Arc::new(e)));
                            return;
                        }
                    }
                }
                Some(VerificationUserResponse::Accept) => { }
                // `None` means the modal went away, which we treat as a cancel too.
                Some(VerificationUserResponse::Cancel | VerificationUserResponse::Mismatch) | None => {
                    log!("User cancelled the verification request.");
                    if let Err(e) = request.cancel().await {
                        Cx::post_action(VerificationAction::RequestCancelError(Arc::new(e)));
                    }
                    return;
                }
            },
            state = stream.next() => {
                let Some(state) = state else { return };
                match state {
                    VerificationRequestState::Created { .. }
                    | VerificationRequestState::Requested { .. }
                    | VerificationRequestState::Ready { .. } => { }
                    VerificationRequestState::Transitioned { verification } => match verification {
                        // We only support SAS verification.
                        Verification::SasV1(sas) => {
                            log!("Verification request transitioned to SAS V1.");
                            if accepted {
                                break sas;
                            }
                            early_sas = Some(sas);
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
                        return;
                    }
                    VerificationRequestState::Done => {
                        log!("Verification request is done!");
                        Cx::post_action(VerificationAction::RequestCompleted);
                        return;
                    }
                }
            }
        }
    };

    sas_verification_handler(client, sas, response_receiver, in_progress).await;
}


/// Sends a self-verification request to the user's other logged-in sessions,
pub async fn request_self_verification_handler(client: Client) {
    let Some(user_id) = client.user_id() else {
        enqueue_popup_notification("Can't verify this device: you are not logged in.", PopupKind::Error, Some(5.0));
        return;
    };

    // Get a fresh copy of the user's device list, don't rely on a cached one
    let identity = match client.encryption().request_user_identity(user_id).await {
        Ok(Some(identity)) => identity,
        Ok(None) => {
            enqueue_popup_notification(
                "Can't verify this device yet: no cross-signing identity exists. \
                Verify from another client first, or reset your identity.",
                PopupKind::Error,
                Some(8.0),
            );
            return;
        }
        Err(e) => {
            error!("Failed to get own user identity for self-verification: {e:?}");
            enqueue_popup_notification(format!("Couldn't start verification: {e}"), PopupKind::Error, Some(6.0));
            return;
        }
    };

    // If there's no other signed device to verify against, show an appropriate error or warning.
    match client.encryption().get_user_devices(user_id).await {
        Ok(devices) => {
            let mut other_devices = devices.devices()
                .filter(|d| Some(d.device_id()) != client.device_id() && !d.is_dehydrated())
                .peekable();
            if other_devices.peek().is_none() {
                enqueue_popup_notification(
                    "Cannot verify this device because it's the only one logged into this account.\n\n\
                    Sign in to this account on another device or Matrix client, and then unlock it \
                    with your recovery key or security phrase, then use that to verify this device.",
                    PopupKind::Error,
                    None,
                );
                return;
            }
            // Signing this device needs cross-signing keys, so only a cross-signed session can do it.
            if !other_devices.any(|d| d.is_cross_signed_by_owner()) {
                enqueue_popup_notification(
                    "You don't have any other devices/sessions with cross-signing keys. \
                    Verification might succeed, but this device will stay unverified.\n\n\
                    Unlock another session with your recovery key or security phrase, then try again.",
                    PopupKind::Warning,
                    Some(20.0),
                );
            }
        }
        Err(e) => error!("Couldn't list our own devices before self-verification: {e:?}"),
    }

    outgoing_verification_request_handler(client, identity, None).await;
}


/// Sends a verification request to another user in our DM room with them,
/// which the SDK creates (or re-invites them to) if needed.
pub async fn request_user_verification_handler(client: Client, user_id: OwnedUserId) {
    let sending = SendingVerificationRequest(user_id.clone());

    // Only a device with our user-signing key can sign (verify) another user's identity.
    if !client.encryption().cross_signing_status().await.is_some_and(|status| status.has_user_signing) {
        enqueue_popup_notification(
            "You need to verify this device before you can verify other users.\n\n\
            You can verify it in Settings.",
            PopupKind::Error,
            Some(8.0),
        );
        return;
    }

    // Get a fresh copy of their identity, don't rely on a cached one
    let identity = match client.encryption().request_user_identity(&user_id).await {
        Ok(Some(identity)) => identity,
        Ok(None) => {
            enqueue_popup_notification(
                format!("Can't verify {user_id} yet, since they haven't set up cross-signing."),
                PopupKind::Error,
                Some(8.0),
            );
            return;
        }
        Err(e) => {
            error!("Failed to get the identity of {user_id} for verification: {e:?}");
            enqueue_popup_notification(format!("Couldn't start verification: {e}"), PopupKind::Error, Some(6.0));
            return;
        }
    };

    // The SDK would create a new DM room with them on its own, so we ask the user first.
    if client.get_dm_room(&user_id).is_none() {
        let (response_sender, mut response_receiver) = tokio::sync::mpsc::unbounded_channel::<VerificationUserResponse>();
        Cx::post_action(UserVerificationRequestAction::ConfirmDmRoomCreation { user_id, response_sender });
        if !matches!(response_receiver.recv().await, Some(VerificationUserResponse::Accept)) {
            return;
        }
    }

    outgoing_verification_request_handler(client, identity, Some(sending)).await;
}


/// Sends a verification request to the given identity, then runs that verification flow.
async fn outgoing_verification_request_handler(
    client: Client,
    identity: UserIdentity,
    sending: Option<SendingVerificationRequest>,
) {
    // Now we try to start the verification flow.
    let Some(in_progress) = VerificationInProgress::start() else {
        enqueue_popup_notification(
            "A verification request is already in progress. Finish or cancel it first.",
            PopupKind::Error,
            Some(6.0),
        );
        return;
    };

    let request = match identity.request_verification_with_methods(vec![VerificationMethod::SasV1]).await {
        Ok(request) => request,
        Err(e) => {
            error!("Failed to send verification request to {}: {e:?}", identity.user_id());
            enqueue_popup_notification(format!("Couldn't send verification request: {e}"), PopupKind::Error, Some(6.0));
            return;
        }
    };
    log!("Sent verification request to {}, flow ID: {}", identity.user_id(), request.flow_id());

    // we use the same verification modal as we do for incoming requests.
    let (sender, mut response_receiver) = tokio::sync::mpsc::unbounded_channel::<VerificationUserResponse>();
    Cx::post_action(VerificationAction::RequestReceived(
        VerificationRequestActionState {
            request: request.clone(),
            response_sender: sender,
        }
    ));
    drop(sending);

    // Wait for another session to respond, then start SAS verification.
    let mut stream = request.changes();
    let sas = loop {
        // Use `select` so we can receive a cancel request from the modal
        tokio::select! {
            // The user cancelled from the modal while we were waiting on another session.
            response = response_receiver.recv() => {
                if !matches!(response, Some(VerificationUserResponse::Accept)) {
                    let _ = request.cancel().await;
                    return;
                }
            }
            state = stream.next() => {
                let Some(state) = state else { return };
                match state {
                    VerificationRequestState::Created { .. }
                    | VerificationRequestState::Requested { .. } => { }
                    // Another session accepted, so we can now start SAS
                    VerificationRequestState::Ready { .. } => match request.start_sas().await {
                        Ok(Some(sas)) => break sas,
                        // If the other side already started SAS, handle it in the `Transitioned` state below.
                        Ok(None) => { }
                        Err(e) => {
                            Cx::post_action(VerificationAction::RequestAcceptError(Arc::new(e)));
                            return;
                        }
                    }
                    VerificationRequestState::Transitioned { verification } => match verification {
                        Verification::SasV1(sas) => break sas,
                        unsupported => {
                            Cx::post_action(VerificationAction::RequestTransitionedToUnsupportedMethod(unsupported));
                            return;
                        }
                    }
                    VerificationRequestState::Cancelled(info) => {
                        Cx::post_action(VerificationAction::RequestCancelled(info));
                        return;
                    }
                    VerificationRequestState::Done => {
                        Cx::post_action(VerificationAction::RequestCompleted);
                        return;
                    }
                }
            }
        }
    };

    sas_verification_handler(client, sas, response_receiver, in_progress).await;
}


/// Actions related to verification that should be handled by the top-level app context.
#[derive(Clone, Debug)]
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
    Mismatch,
}
