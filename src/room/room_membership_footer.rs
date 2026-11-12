//! A `RoomMembershipFooter` widget is shown in place of the message input bar
//! when the user is no longer in a room that they still have open.
//!
//! It tells the user whether they left, were removed, or were banned,
//! and offers a way to get back into the room if there is one.

use makepad_widgets::*;
use matrix_sdk::{RoomState, ruma::OwnedRoomId};

use crate::{
    home::{add_room::KnockResultAction, invite_screen::JoinRoomResultAction},
    room::membership::{JoinMethod, RoomMembership},
    sliding_sync::{MatrixRequest, submit_async_request},
};

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*


    mod.widgets.RoomMembershipFooter = set_type_default() do #(RoomMembershipFooter::register_widget(vm)) {
        // Its background comes from the RoomInputBar's `notices` view.
        ..mod.widgets.View

        visible: false,
        width: Fill, height: Fit
        flow: Down,
        align: Align{x: 0.5}
        // Uneven so the content looks centered: the RoomInputBar's -4 bottom margin hides 4 of the bottom padding,
        // and the text has more space above its letters than below them.
        padding: Inset{top: 16, bottom: 24, left: 20, right: 20}
        spacing: 12

        membership_status := Label {
            width: Fill, height: Fit,
            flow: Flow.Right{wrap: true},
            align: Align{x: 0.5}
            draw_text +: {
                color: (COLOR_TEXT)
                text_style: REGULAR_TEXT {font_size: 12}
            }
        }

        join_button := RobrixPositiveIconButton {
            padding: 15,
            draw_icon.svg: (ICON_JOIN_ROOM)
            icon_walk: Walk{width: 17, height: 17, margin: Inset{left: -2, right: -1} }
        }
    }
}

/// What the footer's button does when clicked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum JoinButtonFunction {
    Join,
    Knock,
}

/// Returns the status text to show for the given membership,
/// and the text and function of the button, if the user can do anything about it.
fn footer_content(membership: &RoomMembership) -> (String, Option<(&'static str, JoinButtonFunction)>) {
    let by = membership.removed_by.as_ref().map(|by| format!(" by {by}")).unwrap_or_default();
    let reason = membership.reason.as_ref()
        .map(|reason| format!("\nReason: {reason}"))
        .unwrap_or_default();

    let (status, how_to_rejoin, button) = match membership.state {
        RoomState::Joined => return (String::from("You're in this room."), None),
        RoomState::Banned => return (format!("You were banned from this room{by}.{reason}"), None),
        RoomState::Knocked => return (
            String::from("You asked to join this room. Someone in it has to let you in."),
            None,
        ),
        RoomState::Invited => return (
            String::from("You were invited back into this room."),
            Some(("Accept invite", JoinButtonFunction::Join)),
        ),
        RoomState::Left => {
            let status = if membership.removed_by.is_some() {
                format!("You were removed from this room{by}.{reason}")
            } else {
                String::from("You left this room.")
            };
            match membership.join_method {
                JoinMethod::Join => (status, "", Some(("Re-join room", JoinButtonFunction::Join))),
                JoinMethod::Knock => (status, "", Some(("Knock to re-join", JoinButtonFunction::Knock))),
                JoinMethod::InviteOrOtherRoom => (
                    status,
                    "\nTo re-join, you need an invite or to be in another room that this room allows.",
                    None,
                ),
                JoinMethod::Invite => (status, "\nTo re-join, you need an invite.", None),
            }
        }
    };
    (status + how_to_rejoin, button)
}

/// A view that shows why the user is no longer in a room, and a button to get back into it if they can.
#[derive(Script, ScriptHook, Widget)]
pub struct RoomMembershipFooter {
    #[deref] view: View,
    /// The room that the user is no longer in.
    #[rust] room_id: Option<OwnedRoomId>,
    #[rust] membership: Option<RoomMembership>,
    /// Whether we're waiting on a join or knock request that the user made from this footer.
    #[rust] is_request_pending: bool,
}

impl Widget for RoomMembershipFooter {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if let Event::Actions(actions) = event {
            if self.view.button(cx, ids!(join_button)).clicked(actions) {
                self.send_join_request(cx);
            }
            // The backend shows a popup with the result, so we only need to re-enable the button.
            if self.is_request_pending && let Some(room_id) = self.room_id.as_ref() {
                let is_request_done = actions.iter().any(|action| match action.downcast_ref() {
                    Some(JoinRoomResultAction::Joined { room_id: id, .. })
                    | Some(JoinRoomResultAction::Failed { room_id: id, .. }) => id == room_id,
                    _ => match action.downcast_ref() {
                        Some(KnockResultAction::Knocked { room_or_alias_id: id, .. })
                        | Some(KnockResultAction::Failed { room_or_alias_id: id, .. }) => id.as_str() == room_id.as_str(),
                        _ => false,
                    },
                });
                if is_request_done {
                    self.is_request_pending = false;
                    self.update_content(cx);
                }
            }
        }
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl RoomMembershipFooter {
    /// Shows the user's membership in the given room.
    fn show(&mut self, cx: &mut Cx, room_id: &OwnedRoomId, membership: &RoomMembership) {
        // A request that we're waiting on is only for the room and membership that we had when it was sent.
        if self.room_id.as_ref() != Some(room_id) || self.membership.as_ref() != Some(membership) {
            self.is_request_pending = false;
        }
        self.room_id = Some(room_id.clone());
        self.membership = Some(membership.clone());
        self.update_content(cx);
        self.set_visible(cx, true);
    }

    /// Sets the status text and the button based on the current membership.
    fn update_content(&mut self, cx: &mut Cx) {
        let Some(membership) = self.membership.as_ref() else { return };
        let (status, button) = footer_content(membership);
        self.view.label(cx, ids!(membership_status)).set_text(cx, &status);
        let join_button = self.view.button(cx, ids!(join_button));
        join_button.set_visible(cx, button.is_some());
        if let Some((text, function)) = button {
            join_button.set_text(cx, match (self.is_request_pending, function) {
                (false, _) => text,
                (true, JoinButtonFunction::Join) => "Joining…",
                (true, JoinButtonFunction::Knock) => "Knocking…",
            });
            join_button.set_enabled(cx, !self.is_request_pending);
            join_button.reset_hover(cx);
        }
        self.redraw(cx);
    }

    /// Asks the backend to join or knock on the room, whichever the button currently offers.
    fn send_join_request(&mut self, cx: &mut Cx) {
        let (Some(room_id), Some(membership)) = (self.room_id.clone(), self.membership.as_ref()) else { return };
        if self.is_request_pending { return; }
        match footer_content(membership).1 {
            Some((_, JoinButtonFunction::Join)) => submit_async_request(MatrixRequest::JoinRoom {
                room_id,
                alias: None,
                via: membership.via.clone(),
            }),
            Some((_, JoinButtonFunction::Knock)) => submit_async_request(MatrixRequest::Knock {
                room_or_alias_id: room_id.into(),
                reason: None,
                server_names: membership.via.clone(),
            }),
            None => return,
        }
        self.is_request_pending = true;
        self.update_content(cx);
    }

    /// Hides the footer and forgets the room it was showing.
    fn hide(&mut self, cx: &mut Cx) {
        self.set_visible(cx, false);
        self.room_id = None;
        self.membership = None;
        self.is_request_pending = false;
    }
}

impl RoomMembershipFooterRef {
    /// See [`RoomMembershipFooter::show()`].
    pub fn show(&self, cx: &mut Cx, room_id: &OwnedRoomId, membership: &RoomMembership) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.show(cx, room_id, membership);
    }

    /// See [`RoomMembershipFooter::hide()`].
    pub fn hide(&self, cx: &mut Cx) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.hide(cx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::room::membership::RemovedBy;
    use matrix_sdk::ruma::owned_user_id;

    fn membership(state: RoomState, join_method: JoinMethod) -> RoomMembership {
        RoomMembership { state, is_space: false, removed_by: None, reason: None, join_method, via: Vec::new() }
    }

    #[test]
    fn leaving_on_our_own_offers_whatever_the_join_rule_allows() {
        let (status, button) = footer_content(&membership(RoomState::Left, JoinMethod::Join));
        assert_eq!(status, "You left this room.");
        assert_eq!(button, Some(("Re-join room", JoinButtonFunction::Join)));

        let (_, button) = footer_content(&membership(RoomState::Left, JoinMethod::Knock));
        assert_eq!(button, Some(("Knock to re-join", JoinButtonFunction::Knock)));

        for join_method in [JoinMethod::Invite, JoinMethod::InviteOrOtherRoom] {
            let (status, button) = footer_content(&membership(RoomState::Left, join_method));
            assert!(status.starts_with("You left this room.\nTo re-join, you need an invite"));
            assert_eq!(button, None);
        }
    }

    #[test]
    fn being_kicked_or_banned_names_who_did_it_and_why() {
        let mut kicked = membership(RoomState::Left, JoinMethod::Join);
        kicked.removed_by = Some(RemovedBy {
            user_id: owned_user_id!("@mod:example.org"),
            display_name: Some("Mod".into()),
        });
        kicked.reason = Some("spam".into());
        let (status, button) = footer_content(&kicked);
        assert_eq!(status, "You were removed from this room by Mod (@mod:example.org).\nReason: spam");
        assert_eq!(button, Some(("Re-join room", JoinButtonFunction::Join)));

        let mut banned = RoomMembership { state: RoomState::Banned, reason: None, ..kicked };
        banned.removed_by.as_mut().unwrap().display_name = None;
        let (status, button) = footer_content(&banned);
        assert_eq!(status, "You were banned from this room by @mod:example.org.");
        // A banned user can't re-join, even if the room is public.
        assert_eq!(button, None);
    }

    #[test]
    fn other_states_only_offer_to_accept_an_invite() {
        assert_eq!(
            footer_content(&membership(RoomState::Invited, JoinMethod::Invite)).1,
            Some(("Accept invite", JoinButtonFunction::Join)),
        );
        assert_eq!(footer_content(&membership(RoomState::Knocked, JoinMethod::Knock)).1, None);
        assert_eq!(footer_content(&membership(RoomState::Joined, JoinMethod::Join)).1, None);
    }
}
