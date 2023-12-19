use std::fmt;

use matrix_sdk::ruma::events::room::message::MessageType;
use matrix_sdk_ui::timeline::{EventTimelineItem, TimelineItemContent, TimelineDetails};

/// An extension trait that can be implemented for certain Matrix message/event types
/// to allow them to be easily formatted for various kinds of display.
pub trait DisplayerExt {
    /// Returns a struct that implements [`fmt::Display`] text-only preview (unformatted) of a Matrix event like a message.
    fn text_preview(&self) -> EventTextPreview;
}

/// A wrapper struct that implements [`fmt::Display`] for a Matrix timeline event.
pub struct EventTextPreview<'e>(pub &'e EventTimelineItem);
impl DisplayerExt for EventTimelineItem {
    fn text_preview(&self) -> EventTextPreview { EventTextPreview(self) }
}
impl<'e> fmt::Display for EventTextPreview<'e> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sender = match self.0.sender_profile() {
            TimelineDetails::Ready(profile) => profile.display_name.as_deref(),
            _ => None,
        }.unwrap_or_else(|| self.0.sender().as_str());

        match self.0.content() {
            TimelineItemContent::Message(msg) => match msg.msgtype() {
                MessageType::Audio(audio)     => write!(f, "{}: [Audio: {}]", sender, audio.body),
                MessageType::Emote(emote)     => write!(f, "{}: [Emote: {}]", sender, emote.body),
                MessageType::File(file)       => write!(f, "{}: [File: {}]", sender, file.body),
                MessageType::Image(image)     => write!(f, "{}: [Image: {}]", sender, image.body),
                MessageType::Location(loc)    => write!(f, "{}: [Location: {}]", sender, loc.body),
                MessageType::Notice(n)        => write!(f, "{}: [Notice: {}]", sender, n.body),
                MessageType::ServerNotice(sn) => write!(f, "{}: [Server notice: {}]", sender, sn.body),
                MessageType::Text(t)          => write!(f, "{}: {}", sender, t.body),
                MessageType::Video(video)     => write!(f, "{}: [Video: {}]", sender, video.body),
                MessageType::VerificationRequest(_) => write!(f, "{}: [Verification request]", sender),
                _ => Ok(()),
            },
            TimelineItemContent::RedactedMessage => f.write_str("[Message was redacted]"),
            TimelineItemContent::MembershipChange(membership_change) => f.write_str("[Membership change]"),
            TimelineItemContent::ProfileChange(profile_change) => f.write_str("[Profile change]"),
            TimelineItemContent::OtherState(other) => f.write_str("[Other state]"),
            TimelineItemContent::Sticker(s) => write!(f, "{}: [Sticker: {}]", sender, s.content().body),
            TimelineItemContent::Poll(_p) => write!(f, "{}: [Poll]", sender),
            _unhandled => {
                println!("!!! Found unknown latest event type: {:?}", _unhandled);
                write!(f, "[Unknown event]")
            }
        }
    }
}

/*
fn display_other_state(
    other_state: &timeline::OtherState,
) -> Option<String> {
    match other_state.content() {
        AnyOtherFullStateEventContent::RoomAliases(FullStateEventContent::Original { content, .. }) => {
            let mut s = format!("set this room's aliases to ");
            for alias in &content.aliases {
                s.push_str(alias.as_str());
                s.push_str(", ");
            }
            s.truncate(s.len() - 2); // remove the last trailing ", "
            Some(s)
        }
        AnyOtherFullStateEventContent::RoomAvatar(_) => {
            // TODO: handle a changed room avatar (picture)
            None
        }
        AnyOtherFullStateEventContent::RoomCanonicalAlias(FullStateEventContent::Original { content, .. }) => {
            Some(format!("set the main address of this room to {}", 
                content.alias.as_ref().map(|a| a.as_str()).unwrap_or("<unknown>")
            ))
        }
        AnyOtherFullStateEventContent::RoomCreate(FullStateEventContent::Original { content, .. }) => {
            Some(format!("created this room (v{})", content.room_version.as_str()))
        }
        AnyOtherFullStateEventContent::RoomGuestAccess(FullStateEventContent::Original { content, .. }) => {
            Some(match content.guest_access {
                GuestAccess::CanJoin => format!("has allowed guests to join this room"),
                GuestAccess::Forbidden | _ => format!("has forbidden guests from joining this room"),
            })
        }
        AnyOtherFullStateEventContent::RoomHistoryVisibility(FullStateEventContent::Original { content, .. }) => {
            let visibility = match content.history_visibility {
                HistoryVisibility::Invited => "invited users, since they were invited",
                HistoryVisibility::Joined => "joined users, since they joined",
                HistoryVisibility::Shared => "joined users, for all of time",
                HistoryVisibility::WorldReadable | _ => "anyone for all time",
            };
            Some(format!("set this room's history to be visible by {}", visibility))
        }
        AnyOtherFullStateEventContent::RoomJoinRules(FullStateEventContent::Original { content, .. }) => {
            Some(match content.join_rule {
                JoinRule::Public => format!("set this room to be joinable by anyone"),
                JoinRule::Knock => format!("set this room to be joinable by invite only or by request"),
                JoinRule::Private => format!("set this room to be private"),
                JoinRule::Restricted(_) => format!("set this room to be joinable by invite only or with restrictions"),
                JoinRule::KnockRestricted(_) => format!("set this room to be joinable by invite only or requestable with restrictions"),
                JoinRule::Invite | _ => format!("set this room to be joinable by invite only"),
            })
        }
        AnyOtherFullStateEventContent::RoomName(FullStateEventContent::Original { content, .. }) => {
            Some(format!("changed this room's name to {:?}", content.name))
        }
        AnyOtherFullStateEventContent::RoomPowerLevels(_) => {
            None
        }
        AnyOtherFullStateEventContent::RoomTopic(FullStateEventContent::Original { content, .. }) => {
            Some(format!("changed this room's topic to {:?}", content.topic))
        }
        AnyOtherFullStateEventContent::SpaceParent(_)
        | AnyOtherFullStateEventContent::SpaceChild(_) => None,
        other => {
            println!("*** Unhandled: {:?}", other);
            None
        }
    }
}
*/
