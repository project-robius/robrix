//! A background task that watches a room's info and notification modes
//! while the UI shows its room info pane (or keeps that pane's saved state).

use std::sync::Arc;
use makepad_widgets::{error, warning, Cx};
use matrix_sdk::{
    notification_settings::{IsEncrypted, IsOneToOne, NotificationSettings, RoomNotificationMode},
    Room,
};
use tokio::{runtime::Handle, sync::{broadcast::error::RecvError, mpsc::UnboundedReceiver, Notify}};

use crate::{
    room::room_info::{NOTIFICATION_MODES, RoomInfoAction, RoomInfoDetails, RoomNotificationModes},
    shared::popup_list::{enqueue_popup_notification, PopupKind},
    utils::RoomNameId,
};

/// Posts the given room's info and notification modes initially, whenever they change, and whenever
/// `resend_notifier` is notified, and sets the room's notification mode to each one from `new_modes`.
pub async fn room_info_subscriber_handler(
    room: Room,
    resend_notifier: Arc<Notify>,
    mut new_modes: UnboundedReceiver<Option<RoomNotificationMode>>,
) {
    let room_id = room.room_id().to_owned();
    Handle::current().spawn({
        let room = room.clone();
        async move {
            if let Err(error) = room.request_encryption_state().await {
                warning!("Failed to get the encryption state of room {}: {error}", room.room_id());
            }
        }
    });
    // We change notification modes with these same settings, which update themselves right away,
    // whereas other instances only see the change once the server sends it back to us.
    let notification_settings = room.client().notification_settings().await;
    let mut notification_settings_changes = notification_settings.subscribe_to_changes();
    let mut room_info_stream = room.subscribe_info();
    let mut posted_details = None;
    let mut posted_modes = None;
    let mut should_resend = true;

    loop {
        let details = get_room_info_details(&room);
        if should_resend || posted_details.as_ref() != Some(&details) {
            Cx::post_action(RoomInfoAction::Updated { room_id: room_id.clone(), details: details.clone() });
            posted_details = Some(details);
        }
        let modes = get_notification_modes(&notification_settings, &room).await;
        if should_resend || posted_modes != Some(modes) {
            Cx::post_action(RoomInfoAction::NotificationModesUpdated { room_id: room_id.clone(), modes });
            posted_modes = Some(modes);
        }

        should_resend = tokio::select! {
            room_info = room_info_stream.next() => {
                if room_info.is_none() { break }
                false
            }
            change = notification_settings_changes.recv() => {
                if matches!(change, Err(RecvError::Closed)) { break }
                false
            }
            Some(mut mode) = new_modes.recv() => {
                while let Ok(newer_mode) = new_modes.try_recv() {
                    mode = newer_mode;
                }
                // This finishes even if we're stopped, but we wait for it so that changes are made in order.
                let _ = Handle::current().spawn({
                    let (room, notification_settings) = (room.clone(), notification_settings.clone());
                    async move { set_notification_mode(&room, &notification_settings, mode).await }
                }).await;
                // Even if that failed, the UI must show the room's actual mode again.
                true
            }
            _ = resend_notifier.notified() => true,
        };
    }
}

/// Sets the notification mode of the given room, whose info isn't being watched,
/// or resets it to the default mode if `mode` is `None`.
pub async fn set_room_notification_mode(room: Room, mode: Option<RoomNotificationMode>) {
    let notification_settings = room.client().notification_settings().await;
    set_notification_mode(&room, &notification_settings, mode).await;
    Cx::post_action(RoomInfoAction::NotificationModesUpdated {
        room_id: room.room_id().to_owned(),
        modes: get_notification_modes(&notification_settings, &room).await,
    });
}

async fn set_notification_mode(room: &Room, notification_settings: &NotificationSettings, mode: Option<RoomNotificationMode>) {
    let result = match mode {
        Some(mode) => notification_settings.set_room_notification_mode(room.room_id(), mode).await,
        None => notification_settings.delete_user_defined_room_rules(room.room_id()).await,
    };
    let room_name_id = RoomNameId::from_room(room).await;
    let label = NOTIFICATION_MODES.iter().find(|(m, _)| *m == mode).map_or("", |(_, label)| *label);
    match result {
        Ok(()) => enqueue_popup_notification(
            format!("Successfully set the notifications of \"{room_name_id}\" to \"{label}\"."),
            PopupKind::Success,
            Some(4.0),
        ),
        Err(error) => {
            error!("Failed to set the notification mode of room {room_name_id:?} to {mode:?}: {error}");
            enqueue_popup_notification(
                format!("Failed to set the notifications of \"{room_name_id}\" to \"{label}\": {error}"),
                PopupKind::Error,
                None,
            );
        }
    }
}

fn get_room_info_details(room: &Room) -> RoomInfoDetails {
    let encryption_state = room.encryption_state();
    RoomInfoDetails {
        topic: room.topic(),
        alias: room.canonical_alias().or_else(|| room.alt_aliases().into_iter().next()),
        avatar_url: room.avatar_url(),
        is_encrypted: (!encryption_state.is_unknown()).then(|| encryption_state.is_encrypted()),
        num_joined_members: room.joined_members_count(),
    }
}

async fn get_notification_modes(notification_settings: &NotificationSettings, room: &Room) -> RoomNotificationModes {
    RoomNotificationModes {
        user_defined: notification_settings.get_user_defined_room_notification_mode(room.room_id()).await,
        default: notification_settings.get_default_room_notification_mode(
            IsEncrypted::from(room.encryption_state().is_encrypted()),
            IsOneToOne::from(room.active_members_count() == 2),
        ).await,
    }
}
