//! An `InviteScreen` widget shows information about a room invite.
//!
//! This is similar to how a `RoomScreen` shows the full timeline of a joined room,
//! but it only shows a simple summary of a room the current user has been invited to,
//! with buttons to accept or decline the invitation.

use std::ops::Deref;
use makepad_widgets::*;
use matrix_sdk::ruma::OwnedRoomId;

use crate::{app::AppStateAction, home::rooms_list::RoomsListRef, join_leave_room_modal::{JoinLeaveModalKind, JoinLeaveRoomModalAction}, room::{BasicRoomDetails, FetchedRoomAvatar}, shared::{avatar::AvatarWidgetRefExt, popup_list::{enqueue_popup_notification, PopupItem, PopupKind}, restore_status_view::RestoreStatusViewWidgetExt}, sliding_sync::{submit_async_request, MatrixRequest}, utils::{self, RoomNameId}};

use super::rooms_list::{InviteState, InviterInfo};


live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::helpers::*;
    use crate::shared::styles::*;
    use crate::shared::avatar::*;
    use crate::shared::icon_button::*;
    use crate::shared::restore_status_view::*;

    pub InviteScreen = {{InviteScreen}}<ScrollXYView> {
        width: Fill,
        height: Fill,
        flow: Down,
        align: {x: 0.5, y: 0}
        padding: {left: 20, right: 20, top: 50}
        spacing: 0,

        show_bg: true,
        draw_bg: {
            color: (COLOR_PRIMARY_DARKER),
        }
        restore_status_view = <RestoreStatusView> {}

        // This view is only shown if `inviter` is Some.
        inviter_view = <View> {
            width: Fill, height: Fit
            align: {x: 0.5, y: 0}
            spacing: 10,
            flow: Down,


            inviter_avatar = <Avatar> {
                width: 30,
                height: 30,
                text_view = { text = { draw_text: {
                    text_style: <TITLE_TEXT>{ font_size: 10.0 }
                }}}
            }


            inviter_name = <Label> {
                width: Fill, height: Fit,
                align: {x: 0.5, y: 0},
                margin: {top: 2}
                padding: 0,
                flow: RightWrap,
                text: ""
                draw_text: {
                    text_style: <TITLE_TEXT>{
                        font_size: 15,
                    },
                    color: #000
                    wrap: Word
                }
            }

            inviter_user_id = <Label> {
                width: Fill, height: Fit,
                align: {x: 0.5, y: 0},
                margin: {top: -3},
                flow: RightWrap,
                text: ""
                draw_text: {
                    text_style: <TITLE_TEXT>{
                        font_size: 10,
                    },
                    color: #888
                    wrap: Word,
                }
            }

            <LineH> {
                width: 240,
                draw_bg: {
                    color: (COLOR_DIVIDER),
                }
            }
        }

        invite_message = <Label> {
            margin: {top: 15, bottom: 15},
            width: Fill, height: Fit,
            align: {x: 0.5, y: 0},
            flow: RightWrap,
            text: "",
            draw_text: {
                text_style: <REGULAR_TEXT>{
                    font_size: 15,
                },
                color: #000
                wrap: Word
            }
        }

        room_view = <View> {
            width: Fill, height: Fit
            align: {x: 0.5, y: 0}
            spacing: 10,
            flow: Down,

            room_avatar = <Avatar> {
                width: 40,
                height: 40,

                text_view = { text = { draw_text: {
                    text_style: <TITLE_TEXT>{ font_size: 13.0 }
                }}}
            }

            room_name = <Label> {
                width: Fill, height: Fit,
                align: {x: 0.5, y: 0},
                text: ""
                // margin: {top: 3}
                flow: RightWrap,
                draw_text: {
                    text_style: <TITLE_TEXT>{
                        font_size: 18,
                    },
                    color: #000
                    wrap: Word,
                }
            }
        }

        buttons = <View> {
            width: Fill, height: Fit
            // We'd like to use RightWrap, but it doesn't work with x-centered alignment
            // flow: RightWrap,
            flow: Right,
            align: {x: 0.5, y: 0.5}
            margin: {top: 20}
            spacing: 40

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
                text: "Reject Invite"
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
                text: "Join Room"
                draw_text:{
                    color: (COLOR_FG_ACCEPT_GREEN),
                }
            }
        }

        completion_label = <Label> {
            width: Fill, height: Fit,
            align: {x: 0.5, y: 0},
            margin: {top: 10, bottom: 10},
            flow: RightWrap,
            draw_text: {
                color: (COLOR_FG_ACCEPT_GREEN),
                text_style: <THEME_FONT_BOLD>{font_size: 12}
                wrap: Word,
            }
            text: ""
        }

        <View> {
            width: Fill, height: 30,
        }
    }
}

#[derive(Clone, Debug)]
pub struct InviteDetails {
    pub room_info: BasicRoomDetails,
    pub inviter: Option<InviterInfo>,
}
impl Deref for InviteDetails {
    type Target = BasicRoomDetails;
    fn deref(&self) -> &Self::Target {
        &self.room_info
    }
}

/// Actions sent from the backend task as a result of a [`MatrixRequest::JoinRoom`].
///
/// Note that this *DOES NOT MEAN* that the room has actually been fully joined yet.
/// For that, you must wait for a [`AppStateAction::RoomLoadedSuccessfully`] action to occur.
#[derive(Debug)]
pub enum JoinRoomResultAction {
    /// The user has successfully joined the room.
    Joined {
        room_id: OwnedRoomId,
    },
    /// There was an error attempting to join the room.
    Failed {
        room_id: OwnedRoomId,
        error: matrix_sdk::Error,
    }
}

/// Actions sent from the backend task as a result of a [`MatrixRequest::LeaveRoom`].
///
/// Note that this *DOES NOT MEAN* that the room has actually been fully left yet.
#[derive(Debug)]
pub enum LeaveRoomResultAction {
    /// The user has successfully left the room.
    Left {
        room_id: OwnedRoomId,
    },
    /// There was an error attempting to leave the room.
    Failed {
        room_id: OwnedRoomId,
        error: matrix_sdk::Error,
    }
}


/// A view that shows information about a room that the user has been invited to.
#[derive(Live, LiveHook, Widget)]
pub struct InviteScreen {
    #[deref] view: View,

    #[rust] invite_state: InviteState,
    #[rust] info: Option<InviteDetails>,
    /// Whether a JoinLeaveRoomModal dialog has been displayed
    /// to allow the user to confirm their join/reject action.
    /// This is used to prevent showing multiple popup notifications
    /// (one from the JoinLeaveRoomModal, and one from this invite screen).
    #[rust] has_shown_confirmation: bool,
    /// The name and ID of the invited room.
    #[rust] room_name_id: Option<RoomNameId>,
    #[rust] is_loaded: bool,
    #[rust] all_rooms_loaded: bool,
}

impl Widget for InviteScreen {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        // Currently, a Signal event is only used to tell this widget
        // to check if the room has been loaded from the homeserver yet.
        if let Event::Signal = event {
            if let (false, Some(room_name_id), true) = (self.is_loaded, self.room_name_id.as_ref(), cx.has_global::<RoomsListRef>()) {
                let rooms_list_ref = cx.get_global::<RoomsListRef>();
                if !rooms_list_ref.is_room_loaded(room_name_id.room_id()) {
                    self.all_rooms_loaded = rooms_list_ref.all_rooms_loaded();
                    self.redraw(cx);
                    return;
                } else {
                    self.set_displayed_invite(cx, &room_name_id.clone());
                }
            }
        }

        self.view.handle_event(cx, event, scope);

        let orig_state = self.invite_state;

        // Handle button clicks to accept or decline the invite
        if let Event::Actions(actions) = event {
            // First, we quickly loop over the actions up front to handle the case
            // where this room was restored and has now been successfully loaded from the homeserver.
            for action in actions {
                if let Some(AppStateAction::RoomLoadedSuccessfully { room_name_id, .. }) = action.downcast_ref() {
                    if self.room_name_id.as_ref().is_some_and(|current| current.room_id() == room_name_id.room_id()) {
                        self.set_displayed_invite(cx, room_name_id);
                        break;
                    }
                }
            }

            let Some(info) = self.info.as_ref() else { return; };
            if let Some(modifiers) = self.view.button(ids!(cancel_button)).clicked_modifiers(actions) {
                self.invite_state = InviteState::WaitingForLeaveResult;
                if modifiers.shift {
                    submit_async_request(MatrixRequest::LeaveRoom {
                        room_id: info.room_id().clone(),
                    });
                    self.has_shown_confirmation = false;
                } else {
                    cx.action(JoinLeaveRoomModalAction::Open {
                        kind: JoinLeaveModalKind::RejectInvite(info.clone()),
                        show_tip: true,
                    });
                    self.has_shown_confirmation = true;
                }
            }
            if let Some(modifiers) = self.view.button(ids!(accept_button)).clicked_modifiers(actions) {
                self.invite_state = InviteState::WaitingForJoinResult;
                if modifiers.shift {
                    submit_async_request(MatrixRequest::JoinRoom {
                        room_id: info.room_id().clone(),
                    });
                    self.has_shown_confirmation = false;
                } else {
                    cx.action(JoinLeaveRoomModalAction::Open {
                        kind: JoinLeaveModalKind::AcceptInvite(info.clone()),
                        show_tip: true,
                    });
                    self.has_shown_confirmation = true;
                }
            }

            for action in actions {
                match action.downcast_ref() {
                    Some(JoinRoomResultAction::Joined { room_id }) if room_id == info.room_id() => {
                        self.invite_state = InviteState::WaitingForJoinedRoom;
                        if !self.has_shown_confirmation {
                            enqueue_popup_notification(PopupItem{ message: "Successfully joined room.".into(), kind: PopupKind::Success, auto_dismissal_duration: Some(5.0) });
                        }
                        continue;
                    }
                    Some(JoinRoomResultAction::Failed { room_id, error }) if room_id == info.room_id() => {
                        self.invite_state = InviteState::WaitingOnUserInput;
                        if !self.has_shown_confirmation {
                            let msg = utils::stringify_join_leave_error(error, info.room_name_id(), true, true);
                            enqueue_popup_notification(PopupItem { message: msg, kind: PopupKind::Error, auto_dismissal_duration: None });
                        }
                        continue;
                    }
                    _ => {}
                }

                match action.downcast_ref() {
                    Some(LeaveRoomResultAction::Left { room_id }) if room_id == info.room_id() => {
                        self.invite_state = InviteState::RoomLeft;
                        if !self.has_shown_confirmation {
                            enqueue_popup_notification(PopupItem { message: "Successfully rejected invite.".into(), kind: PopupKind::Success, auto_dismissal_duration: Some(5.0) });
                        }
                        continue;
                    }
                    Some(LeaveRoomResultAction::Failed { room_id, error }) if room_id == info.room_id() => {
                        self.invite_state = InviteState::WaitingOnUserInput;
                        if !self.has_shown_confirmation {
                            enqueue_popup_notification(PopupItem { message: format!("Failed to reject invite: {error}"), kind: PopupKind::Error, auto_dismissal_duration: None });
                        }
                        continue;
                    }
                    _ => {}
                }

                if let Some(JoinLeaveRoomModalAction::Close { successful, .. }) = action.downcast_ref() {
                    // If the modal didn't result in a successful join/leave,
                    // then we must reset the invite state to waiting for user input.
                    if !*successful {
                        self.invite_state = InviteState::WaitingOnUserInput;
                    }
                    continue;
                }
            }
        }

        if self.invite_state != orig_state {
            self.redraw(cx);
        }
    }


    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        if !self.is_loaded {
            let mut restore_status_view = self.view.restore_status_view(ids!(restore_status_view));
            if let Some(room_name) = &self.room_name_id {
                restore_status_view.set_content(cx, self.all_rooms_loaded, room_name);
            }
            return restore_status_view.draw(cx, scope);
        }
        let Some(info) = self.info.as_ref() else {
            // If we don't have any info, just return.
            return self.view.draw_walk(cx, scope, walk);
        };

        // First, populate the inviter info, if we have it.
        let inviter_view = self.view.view(ids!(inviter_view));
        let (is_visible, invite_text) = if let Some(inviter) = info.inviter.as_ref() {
            let inviter_avatar = inviter_view.avatar(ids!(inviter_avatar));
            let mut drew_avatar = false;
            if let Some(avatar_bytes) = inviter.avatar.as_ref() {
                drew_avatar = inviter_avatar.show_image(
                    cx,
                    None, // don't make this avatar clickable.
                    |cx, img| utils::load_png_or_jpg(&img, cx, avatar_bytes),
                ).is_ok();
            }
            if !drew_avatar {
                inviter_avatar.show_text(
                    cx,
                    None,
                    None, // don't make this avatar clickable.
                    inviter.display_name.as_deref().unwrap_or_else(|| inviter.user_id.as_str()),
                );
            }
            let inviter_name = inviter_view.label(ids!(inviter_name));
            let inviter_user_id = inviter_view.label(ids!(inviter_user_id));
            if let Some(inviter_user_name) = inviter.display_name.as_deref() {
                // If we have an inviter display name, show that *and* the user ID.
                inviter_name.set_text(cx, inviter_user_name);
                inviter_user_id.set_visible(cx, true);
                inviter_user_id.set_text(cx, inviter.user_id.as_str());
            }
            else {
                // If we only have a user ID, show it in the user_name field,
                // and hide the user ID field.
                inviter_name.set_text(cx, inviter.user_id.as_str());
                inviter_user_id.set_visible(cx, false);
            }
            (true, "has invited you to join:")
        }
        else {
            (false, "You have been invited to join:")
        };
        inviter_view.set_visible(cx, is_visible);
        self.view.label(ids!(invite_message)).set_text(cx, invite_text);

        // Second, populate the room info, if we have it.
        let room_view = self.view.view(ids!(room_view));
        let room_avatar = room_view.avatar(ids!(room_avatar));
        match &info.room_avatar() {
            FetchedRoomAvatar::Text(text) => {
                room_avatar.show_text(
                    cx,
                    None,
                    None, // don't make this avatar clickable.
                    text,
                );
            }
            FetchedRoomAvatar::Image(avatar_bytes) => {
                let _ = room_avatar.show_image(
                    cx,
                    None, // don't make this avatar clickable.
                    |cx, img| utils::load_png_or_jpg(&img, cx, avatar_bytes),
                );
            }
        }
        let invite_room_label = info.room_name_id().to_string();
        room_view.label(ids!(room_name)).set_text(cx, &invite_room_label);

        // Third, set the buttons' text based on the invite state.
        let cancel_button = self.view.button(ids!(cancel_button));
        let accept_button = self.view.button(ids!(accept_button));
        match self.invite_state {
            InviteState::WaitingOnUserInput => {
                cancel_button.set_enabled(cx, true);
                accept_button.set_enabled(cx, true);
                cancel_button.set_text(cx, "Reject Invite");
                accept_button.set_text(cx, "Join Room");
            }
            InviteState::WaitingForJoinResult => {
                cancel_button.set_enabled(cx, false);
                accept_button.set_enabled(cx, false);
                cancel_button.set_text(cx, "Reject Invite");
                accept_button.set_text(cx, "Joining...");
            }
            InviteState::WaitingForLeaveResult => {
                cancel_button.set_enabled(cx, false);
                accept_button.set_enabled(cx, false);
                cancel_button.set_text(cx, "Rejecting...");
                accept_button.set_text(cx, "Join Room");
            }
            InviteState::WaitingForJoinedRoom => {
                cancel_button.set_enabled(cx, false);
                accept_button.set_enabled(cx, false);
                cancel_button.set_text(cx, "Reject Invite");
                accept_button.set_text(cx, "Joined!");
            }
            InviteState::RoomLeft => {
                cancel_button.set_visible(cx, false);
                accept_button.set_visible(cx, false);
                self.view.label(ids!(completion_label)).set_text(
                    cx,
                    "Invite successfully rejected. You may close this invite.",
                );
            }
        }

        self.view.draw_walk(cx, scope, walk)
    }
}

impl InviteScreen {
    /// Sets the ID of the invited room that will be displayed by this screen.
    pub fn set_displayed_invite(&mut self, cx: &mut Cx, room_name_id: &RoomNameId) {
        self.room_name_id = Some(room_name_id.clone());
        if let Some(invite) = super::rooms_list::get_invited_rooms(cx)
            .borrow()
            .get(room_name_id.room_id())
        {
            self.info = Some(InviteDetails {
                room_info: BasicRoomDetails::NameAndAvatar {
                    room_name_id: room_name_id.clone(),
                    room_avatar: invite.room_avatar.clone(),
                },
                inviter: invite.inviter_info.clone(),
            });
            self.invite_state = invite.invite_state;
            self.has_shown_confirmation = false;
            self.is_loaded = true;
            self.all_rooms_loaded = true;
            self.redraw(cx);
        }

        let restore_status_view = self.view.restore_status_view(ids!(restore_status_view));
        if !self.is_loaded {
            restore_status_view.set_content(
                cx,
                self.all_rooms_loaded,
                room_name_id,
            );
            restore_status_view.set_visible(cx, true);
        } else {
            restore_status_view.set_visible(cx, false);
        }
    }
}

impl InviteScreenRef {
    /// See [`InviteScreen::set_displayed_invite()`].
    pub fn set_displayed_invite(&self, cx: &mut Cx, room_name_id: &RoomNameId) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_displayed_invite(cx, room_name_id);
        }
    }
}
