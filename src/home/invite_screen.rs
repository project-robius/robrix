//! An `InviteScreen` widget shows information about a room invite.
//!
//! This is similar to how a `RoomScreen` shows the full timeline of a joined room,
//! but it only shows a simple summary of a room the current user has been invited to,
//! with buttons to accept or decline the invitation.

use std::ops::Deref;
use makepad_widgets::*;
use matrix_sdk::{RoomState, ruma::OwnedRoomId};

use crate::{app::AppStateAction, avatar_cache::{self, AvatarCacheEntry}, home::rooms_list::RoomsListRef, join_leave_room_modal::{JoinLeaveModalKind, JoinLeaveRoomModalAction}, room::{BasicRoomDetails, FetchedRoomAvatar}, shared::{avatar::AvatarWidgetRefExt, restore_status_view::RestoreStatusViewWidgetExt}, sliding_sync::{submit_async_request, MatrixRequest}, utils::{self, RoomNameId}};

use super::rooms_list::{AcceptedInviteKind, InviteState, InviterInfo, RoomsListAction, get_invited_rooms, set_invite_state};


script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*


    mod.widgets.InviteScreen = set_type_default() do #(InviteScreen::register_widget(vm)) {
        ..mod.widgets.SolidView

        // make this a ScrollYView
        scroll_bars: mod.widgets.ScrollBars {
            show_scroll_x: false show_scroll_y: true
            scroll_bar_y.drag_scrolling: true
        }

        width: Fill,
        height: Fill,
        flow: Down,
        align: Align{x: 0.5, y: 0}
        padding: Inset{left: 20, right: 20, top: 50}
        spacing: 0,

        show_bg: true,
        draw_bg +: {
            color: mod.widgets.COLOR_PRIMARY_DARKER
        }

        restore_status_view := RestoreStatusView {}

        // This view is only shown if `inviter` is Some.
        inviter_view := View {
            width: Fill, height: Fit
            align: Align{x: 0.5, y: 0}
            spacing: 10,
            flow: Down,


            inviter_avatar := Avatar {
                width: 30,
                height: 30,
                text_view +: {
                    text +: {
                        draw_text +: {
                            text_style: TITLE_TEXT { font_size: 10.0 }
                        }
                    }
                }
            }


            inviter_name := Label {
                width: Fill, height: Fit,
                align: Align{x: 0.5, y: 0},
                margin: Inset{top: 2}
                padding: 0,
                flow: Flow.Right{wrap: true},
                text: ""
                draw_text +: {
                    text_style: TITLE_TEXT {
                        font_size: 15,
                    },
                    color: #000
                }
            }

            inviter_user_id := Label {
                width: Fill, height: Fit,
                align: Align{x: 0.5, y: 0},
                margin: Inset{top: -3},
                flow: Flow.Right{wrap: true},
                text: ""
                draw_text +: {
                    text_style: TITLE_TEXT {
                        font_size: 10,
                    },
                    color: #888
                }
            }

            LineH {
                width: 240,
                draw_bg.color: (COLOR_DIVIDER)
            }
        }

        invite_message := Label {
            margin: Inset{top: 15, bottom: 15},
            width: Fill, height: Fit,
            align: Align{x: 0.5, y: 0},
            flow: Flow.Right{wrap: true},
            text: "",
            draw_text +: {
                text_style: REGULAR_TEXT {
                    font_size: 15,
                },
                color: #000
            }
        }

        room_view := View {
            width: Fill, height: Fit
            align: Align{x: 0.5, y: 0}
            spacing: 10,
            flow: Down,

            room_avatar := Avatar {
                width: 40,
                height: 40,

                text_view +: {
                    text +: {
                        draw_text +: {
                            text_style: TITLE_TEXT { font_size: 13.0 }
                        }
                    }
                }
            }

            room_name := Label {
                width: Fill, height: Fit,
                align: Align{x: 0.5, y: 0},
                text: ""
                // margin: Inset{top: 3}
                flow: Flow.Right{wrap: true},
                draw_text +: {
                    text_style: TITLE_TEXT {
                        font_size: 18,
                    },
                    color: #000
                }
            }
        }

        buttons := View {
            width: Fill, height: Fit
            // We'd like to use RightWrap, but it doesn't work with x-centered alignment
            // flow: Flow.Right{wrap: true},
            flow: Right,
            align: Align{x: 0.5, y: 0.5}
            margin: Inset{top: 20}
            spacing: 40

            cancel_button := RobrixNegativeIconButton {
                align: Align{x: 0.5, y: 0.5}
                padding: 15,
                draw_icon.svg: (ICON_FORBIDDEN)
                icon_walk: Walk{width: 16, height: 16, margin: Inset{left: -2, right: -1} }
                text: "Reject Invite"
            }

            accept_button := RobrixPositiveIconButton {
                align: Align{x: 0.5, y: 0.5}
                padding: 15,
                draw_icon.svg: (ICON_CHECKMARK)
                icon_walk: Walk{width: 16, height: 16, margin: Inset{left: -2, right: -1} }
                text: "Join Room"
            }
        }

        completion_label := Label {
            width: Fill, height: Fit,
            align: Align{x: 0.5, y: 0},
            margin: Inset{top: 10, bottom: 10},
            flow: Flow.Right{wrap: true},
            draw_text +: {
                color: (COLOR_FG_ACCEPT_GREEN),
                text_style: theme.font_bold {font_size: 12}
            }
            text: ""
        }

        View {
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


/// Actions that tell an `InviteScreen` to refresh part of its content.
#[derive(Debug)]
pub enum InviteScreenAction {
    /// We've fetched more info about who sent this invite.
    InviterInfoUpdated {
        room_id: OwnedRoomId,
        inviter_info: InviterInfo,
    },
    /// We've determined whether this invite is for a space.
    IsSpace {
        room_id: OwnedRoomId,
        is_space: bool,
    },
}


/// A view that shows information about a room that the user has been invited to.
#[derive(Script, ScriptHook, Widget)]
pub struct InviteScreen {
    #[deref] view: View,

    #[rust] invite_state: InviteState,
    #[rust] info: Option<InviteDetails>,
    /// The name and ID of the invited room.
    #[rust] room_name_id: Option<RoomNameId>,
    #[rust] is_loaded: bool,
    #[rust] all_rooms_loaded: bool,
    #[rust] is_space: bool,
}

impl Widget for InviteScreen {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if let Event::Signal = event {
            // We use the avatar cache to populate the inviter's avatar.
            avatar_cache::process_avatar_updates(cx);

            // Otherwise, a Signal just means that the room might've been received from the homeserver.
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

        // Handle any updates to this invite screen, e.g., it's been loaded or more inviter info has been fetched.
        if let Event::Actions(actions) = event {
            for action in actions {
                if let Some(AppStateAction::RoomLoadedSuccessfully { room_name_id, .. }) = action.downcast_ref() {
                    if self.room_name_id.as_ref().is_some_and(|current| current.room_id() == room_name_id.room_id()) {
                        self.set_displayed_invite(cx, room_name_id);
                        break;
                    }
                    continue;
                }
                if let Some(InviteScreenAction::InviterInfoUpdated { room_id, inviter_info }) = action.downcast_ref() {
                    if self.room_name_id.as_ref().is_some_and(|r| r.room_id() == room_id) {
                        if let Some(info) = self.info.as_mut() {
                            info.inviter = Some(inviter_info.clone());
                        }
                        self.redraw(cx);
                    }
                    continue;
                }
                if let Some(InviteScreenAction::IsSpace { room_id, is_space }) = action.downcast_ref() {
                    if self.room_name_id.as_ref().is_some_and(|r| r.room_id() == room_id) {
                        self.is_space = *is_space;
                        self.redraw(cx);
                    }
                    continue;
                }
            }

            let Some(info) = self.info.as_ref() else { return; };
            // Handle button clicks to accept or decline the invite
            if let Some(modifiers) = self.view.button(cx, ids!(cancel_button)).clicked_modifiers(actions) {
                if modifiers.shift {
                    submit_async_request(MatrixRequest::LeaveRoom {
                        room_id: info.room_id().clone(),
                    });
                    self.invite_state = InviteState::WaitingForLeaveResult;
                    set_invite_state(cx, info.room_id(), InviteState::WaitingForLeaveResult);
                } else {
                    cx.action(JoinLeaveRoomModalAction::Open {
                        kind: JoinLeaveModalKind::RejectInvite(info.clone()),
                        show_tip: true,
                    });
                }
            }
            if let Some(modifiers) = self.view.button(cx, ids!(accept_button)).clicked_modifiers(actions) {
                if modifiers.shift {
                    submit_async_request(MatrixRequest::JoinRoom {
                        room_id: info.room_id().clone(),
                    });
                    self.invite_state = InviteState::WaitingForJoinResult;
                    set_invite_state(cx, info.room_id(), InviteState::WaitingForJoinResult);
                } else {
                    cx.action(JoinLeaveRoomModalAction::Open {
                        kind: JoinLeaveModalKind::AcceptInvite(info.clone()),
                        show_tip: true,
                    });
                }
            }

            for action in actions {
                match action.downcast_ref() {
                    // The success/failure popups are shown by the backend task, which is
                    // more consistent than doing it here since the user may have left this screen already.
                    Some(JoinRoomResultAction::Joined { room_id }) if room_id == info.room_id() => {
                        self.invite_state = InviteState::WaitingForJoinedRoom;
                        continue;
                    }
                    Some(JoinRoomResultAction::Failed { room_id, .. }) if room_id == info.room_id() => {
                        self.invite_state = InviteState::WaitingOnUserInput;
                        continue;
                    }
                    _ => {}
                }

                match action.downcast_ref() {
                    Some(LeaveRoomResultAction::Left { room_id }) if room_id == info.room_id() => {
                        self.invite_state = InviteState::RoomLeft;
                        continue;
                    }
                    Some(LeaveRoomResultAction::Failed { room_id, .. }) if room_id == info.room_id() => {
                        self.invite_state = InviteState::WaitingOnUserInput;
                        continue;
                    }
                    _ => {}
                }

                if let Some(JoinLeaveRoomModalAction::Close { room_id, .. }) = action.downcast_ref() {
                    if room_id == info.room_id() {
                        // check the latest invite state for this room, as it may have changed
                        // even after the modal was closed.
                        let current = get_invited_rooms(cx)
                            .borrow()
                            .get(room_id)
                            .map(|invite| invite.invite_state);
                        if let Some(state) = current {
                            self.invite_state = state;
                        }
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
            let mut restore_status_view = self.view.restore_status_view(cx, ids!(restore_status_view));
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
        let inviter_view = self.view.view(cx, ids!(inviter_view));
        let (is_visible, invite_text) = if let Some(inviter) = info.inviter.as_ref() {
            let inviter_avatar = inviter_view.avatar(cx, ids!(inviter_avatar));
            let mut drew_avatar = false;
            if let Some(uri) = inviter.avatar_url.as_ref()
                && let AvatarCacheEntry::Loaded(data) = avatar_cache::get_or_fetch_avatar(cx, uri)
            {
                drew_avatar = inviter_avatar.show_image(
                    cx,
                    None, // don't make this avatar clickable.
                    |cx, img| utils::load_avatar_image(&img, cx, &(uri.clone(), data).into()),
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
            let inviter_name = inviter_view.label(cx, ids!(inviter_name));
            let inviter_user_id = inviter_view.label(cx, ids!(inviter_user_id));
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
        self.view.label(cx, ids!(invite_message)).set_text(cx, invite_text);

        // Second, populate the room info, if we have it.
        let room_view = self.view.view(cx, ids!(room_view));
        let room_avatar = room_view.avatar(cx, ids!(room_avatar));
        match &info.room_avatar() {
            FetchedRoomAvatar::Text(text) => {
                room_avatar.show_text(
                    cx,
                    None,
                    None, // don't make this avatar clickable.
                    text,
                );
            }
            FetchedRoomAvatar::Image(avatar_image) => {
                let _ = room_avatar.show_image(
                    cx,
                    None, // don't make this avatar clickable.
                    |cx, img| utils::load_avatar_image(&img, cx, avatar_image),
                );
            }
        }
        let invite_room_label = info.room_name_id().to_string();
        room_view.label(cx, ids!(room_name)).set_text(cx, &invite_room_label);

        // Third, set the buttons' text based on the invite state.
        let cancel_button = self.view.button(cx, ids!(cancel_button));
        let accept_button = self.view.button(cx, ids!(accept_button));
        let join_text = match self.is_space { true => "Join Space", false => "Join Room" };
        match self.invite_state {
            InviteState::WaitingOnUserInput => {
                cancel_button.set_enabled(cx, true);
                accept_button.set_enabled(cx, true);
                cancel_button.set_text(cx, "Reject Invite");
                accept_button.set_text(cx, join_text);
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
                accept_button.set_text(cx, join_text);
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
                self.view.label(cx, ids!(completion_label)).set_text(
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
        if let Some(invite) = get_invited_rooms(cx)
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
            self.is_space = invite.is_space;
            self.is_loaded = true;
            self.all_rooms_loaded = true;
            self.redraw(cx);
        }
        // If this invite has already been accepted (e.g., in another client, or while Robrix was offline),
        // we need to handle that and upgrade this screen to the corresponding joined room's RoomScreen.
        else if cx.has_global::<RoomsListRef>()
            && cx.get_global::<RoomsListRef>().get_room_state(room_name_id.room_id()) == Some(RoomState::Joined)
        {
            // Use the joined room's most current name, of course.
            let room_name_id = cx.get_global::<RoomsListRef>()
                .get_room_name(room_name_id.room_id())
                .unwrap_or_else(|| room_name_id.clone());

            self.is_loaded = true;
            cx.widget_action(
                self.widget_uid(),
                RoomsListAction::InviteAccepted { room_name_id, kind: AcceptedInviteKind::Room },
            );
            return;
        }

        let restore_status_view = self.view.restore_status_view(cx, ids!(restore_status_view));
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

    pub fn hide_displayed_invite(&mut self, cx: &mut Cx) {
        let cancel_button = self.view.button(cx, ids!(cancel_button));
        cancel_button.set_visible(cx, true);
        cancel_button.reset_hover(cx);
        let accept_button = self.view.button(cx, ids!(accept_button));
        accept_button.set_visible(cx, true);
        accept_button.reset_hover(cx);
        self.view.label(cx, ids!(completion_label)).set_text(cx, "");
        self.room_name_id = None;
        self.info = None;
        self.invite_state = InviteState::default();
        self.is_space = false;
        self.is_loaded = false;
        self.all_rooms_loaded = false;
        self.view.restore_status_view(cx, ids!(restore_status_view)).set_visible(cx, false);
        self.redraw(cx);
    }
}

impl InviteScreenRef {
    /// See [`InviteScreen::set_displayed_invite()`].
    pub fn set_displayed_invite(&self, cx: &mut Cx, room_name_id: &RoomNameId) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_displayed_invite(cx, room_name_id);
        }
    }

    pub fn hide_displayed_invite(&self, cx: &mut Cx) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.hide_displayed_invite(cx);
        }
    }
}
