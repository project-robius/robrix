//! Moderating a room's members, and reporting messages, rooms, and users to homeserver admins.
//!
//! Everything here runs on the background matrix worker task;
//! see [`ModerationModal`](crate::moderation_modal::ModerationModal) for how the user confirms these.

use anyhow::{Result, anyhow};
use bitflags::bitflags;
use makepad_widgets::{Cx, error};
use matrix_sdk::{
    Client, Room,
    room::RoomMember,
    ruma::{
        OwnedEventId, OwnedRoomId, OwnedUserId, UserId,
        api::client::reporting::report_user,
        events::{MessageLikeEventType, room::{member::MembershipState, power_levels::RoomPowerLevels}},
        int,
    },
};

use crate::{shared::popup_list::{PopupKind, enqueue_popup_notification}, utils::RoomNameId};

/// How long to show the popup notification for a request that succeeded, in seconds.
const SUCCESS_POPUP_DURATION: f64 = 4.0;
/// How long to show the popup notification for a request that failed, in seconds.
const FAILURE_POPUP_DURATION: f64 = 8.0;

bitflags! {
    /// Possible moderation actions that the current user can perform on a room member.
    ///
    /// This is used to determine which buttons to show in the user profile pane.
    #[derive(Copy, Clone, Debug, Default)]
    pub struct MemberAbilities: u8 {
        const CanKick = 1 << 0;
        const CanBan = 1 << 1;
        const CanUnban = 1 << 2;
        const CanMute = 1 << 3;
        const CanUnmute = 1 << 4;
    }
}
impl MemberAbilities {
    /// Returns what `own_user_id` can do to `target_user_id`.
    ///
    /// The target's `membership` should be `None` if they've never been in the room.
    fn from_power_levels(
        power_levels: &RoomPowerLevels,
        own_user_id: &UserId,
        target_user_id: &UserId,
        membership: Option<&MembershipState>,
        message_type: MessageLikeEventType,
    ) -> Self {
        let mut abilities = Self::empty();
        let is_banned = matches!(membership, Some(MembershipState::Ban));
        let is_in_room = matches!(membership, Some(MembershipState::Join | MembershipState::Invite));
        let is_knocking = matches!(membership, Some(MembershipState::Knock));
        abilities.set(Self::CanKick, (is_in_room || is_knocking) && power_levels.user_can_kick_user(own_user_id, target_user_id));
        abilities.set(Self::CanBan, !is_banned && power_levels.user_can_ban_user(own_user_id, target_user_id));
        abilities.set(Self::CanUnban, is_banned && power_levels.user_can_unban_user(own_user_id, target_user_id));

        // Unmuting resets a user to the default level, which only helps if that level can send messages.
        let send_level = power_levels.for_message(message_type);
        if is_in_room
            && power_levels.users_default >= send_level
            && power_levels.user_can_change_user_power_level(own_user_id, target_user_id)
        {
            let is_muted = power_levels.for_user(target_user_id) < send_level;
            abilities.set(if is_muted { Self::CanUnmute } else { Self::CanMute }, true);
        }
        abilities
    }
}

/// The result of a [`crate::sliding_sync::MatrixRequest::GetMemberAbilities`] request.
///
/// This is NOT a widget action.
#[derive(Debug)]
pub struct MemberAbilitiesFetched {
    pub room_id: OwnedRoomId,
    pub user_id: OwnedUserId,
    pub abilities: MemberAbilities,
    /// The latest member info of this user, or `None` if they've never been in the room.
    pub room_member: Option<RoomMember>,
}

/// An action that a moderator can perform on a room member.
#[derive(Copy, Clone, Debug)]
pub enum MemberAction {
    Kick,
    Ban,
    Unban,
    /// Lowers the member's power level so that they can't send messages.
    Mute,
    /// Resets a muted member to the room's default power level.
    Unmute,
}

/// A [`MemberAction`] to perform on a specific member of a specific room.
#[derive(Clone, Debug)]
pub struct MemberModeration {
    pub room_id: OwnedRoomId,
    pub room_name: String,
    pub user_id: OwnedUserId,
    pub user_name: String,
    pub action: MemberAction,
}

/// The thing being reported to the homeserver's admins.
#[derive(Clone, Debug)]
pub enum ReportTarget {
    Message {
        room_id: OwnedRoomId,
        event_id: OwnedEventId,
    },
    Room(RoomNameId),
    User {
        user_id: OwnedUserId,
        user_name: String,
    },
}

/// Fetches what the current user can do to the given user in the given room,
/// which gets posted as a [`MemberAbilitiesFetched`] action.
pub async fn fetch_member_abilities(client: Client, room_id: OwnedRoomId, user_id: OwnedUserId) {
    let Some(room) = client.get_room(&room_id) else { return };
    let fetched: Result<_> = async {
        // This may have to fetch the room's members from the homeserver.
        let room_member = room.get_member(&user_id).await?;
        Ok((room_member, room.power_levels().await?))
    }.await;
    let (room_member, power_levels) = match fetched {
        Ok(fetched) => fetched,
        Err(e) => {
            error!("Failed to get the moderation abilities for {user_id} in {room_id}: {e:?}");
            return;
        }
    };
    let abilities = MemberAbilities::from_power_levels(
        &power_levels,
        room.own_user_id(),
        &user_id,
        room_member.as_ref().map(|member| member.membership()),
        get_message_event_type(&room),
    );
    Cx::post_action(MemberAbilitiesFetched { room_id, user_id, abilities, room_member });
}

/// Performs the given moderation of a room member, showing the result in a popup notification.
pub async fn moderate_member(client: Client, moderation: MemberModeration, reason: Option<String>) {
    let MemberModeration { room_id, user_id, user_name, action, .. } = moderation;
    let result: Result<()> = async {
        let room = get_room(&client, &room_id)?;
        let reason = reason.as_deref();
        match action {
            MemberAction::Kick => room.kick_user(&user_id, reason).await?,
            MemberAction::Ban => room.ban_user(&user_id, reason).await?,
            MemberAction::Unban => room.unban_user(&user_id, reason).await?,
            MemberAction::Mute => {
                let send_level = room.power_levels().await?.for_message(get_message_event_type(&room));
                room.update_power_levels(vec![(&user_id, send_level.saturating_sub(int!(1)))]).await?;
            }
            MemberAction::Unmute => {
                let default_level = room.power_levels().await?.users_default;
                room.update_power_levels(vec![(&user_id, default_level)]).await?;
            }
        }
        Ok(())
    }.await;

    let (verb, past_tense) = match action {
        MemberAction::Kick => ("kick", "Kicked"),
        MemberAction::Ban => ("ban", "Banned"),
        MemberAction::Unban => ("unban", "Unbanned"),
        MemberAction::Mute => ("mute", "Muted"),
        MemberAction::Unmute => ("unmute", "Unmuted"),
    };
    match result {
        Ok(()) => enqueue_popup_notification(
            format!("{past_tense} {user_name}."),
            PopupKind::Success,
            Some(SUCCESS_POPUP_DURATION),
        ),
        Err(e) => {
            error!("Failed to {verb} {user_id} in {room_id}: {e:?}");
            enqueue_popup_notification(
                format!("Failed to {verb} {user_name}: {e}"),
                PopupKind::Error,
                Some(FAILURE_POPUP_DURATION),
            );
        }
    }
}

/// Sends the given report to the homeserver's admins, showing the result in a popup notification.
pub async fn report(client: Client, target: ReportTarget, reason: Option<String>) {
    let result: Result<()> = async {
        match &target {
            ReportTarget::Message { room_id, event_id } => {
                get_room(&client, room_id)?.report_content(event_id.clone(), reason).await?;
            }
            ReportTarget::Room(room_name_id) => {
                get_room(&client, room_name_id.room_id())?.report_room(reason.unwrap_or_default()).await?;
            }
            ReportTarget::User { user_id, .. } => {
                client.send(report_user::v3::Request::new(user_id.clone(), reason.unwrap_or_default())).await?;
            }
        }
        Ok(())
    }.await;

    let description = match &target {
        ReportTarget::Message { .. } => String::from("this message"),
        ReportTarget::Room(room_name_id) => format!("\"{room_name_id}\""),
        ReportTarget::User { user_name, .. } => user_name.clone(),
    };
    match result {
        Ok(()) => enqueue_popup_notification(
            format!("Reported {description} to your homeserver's admins."),
            PopupKind::Success,
            Some(SUCCESS_POPUP_DURATION),
        ),
        Err(e) => {
            error!("Failed to report {target:?}: {e:?}");
            enqueue_popup_notification(
                format!("Failed to report {description}: {e}"),
                PopupKind::Error,
                Some(FAILURE_POPUP_DURATION),
            );
        }
    }
}

fn get_room(client: &Client, room_id: &OwnedRoomId) -> Result<Room> {
    client.get_room(room_id).ok_or_else(|| anyhow!("room {room_id} wasn't found"))
}

/// Returns the type of event that the given room's messages are sent as.
fn get_message_event_type(room: &Room) -> MessageLikeEventType {
    if room.encryption_state().is_encrypted() {
        MessageLikeEventType::RoomEncrypted
    } else {
        MessageLikeEventType::RoomMessage
    }
}
