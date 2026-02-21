//! A modal dialog for joining or leaving rooms in Matrix.
//!
//! Also used as a confirmation dialog for accepting or rejecting room invites.

use std::borrow::Cow;

use makepad_widgets::*;
use crate::ApplyOverCompat;
use matrix_sdk::ruma::OwnedRoomId;
use tokio::sync::mpsc::UnboundedSender;

use crate::{home::invite_screen::{InviteDetails, JoinRoomResultAction, LeaveRoomResultAction}, room::BasicRoomDetails, shared::popup_list::{PopupKind, enqueue_popup_notification}, sliding_sync::{MatrixRequest, submit_async_request}, space_service_sync::{SpaceRequest, SpaceRoomListAction}, utils::{self, RoomNameId}};

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*


    mod.widgets.JoinLeaveRoomModal = #(JoinLeaveRoomModal::register_widget(vm)) {
        width: Fit
        height: Fit

        RoundedView {
            flow: Down
            width: 400
            height: Fit
            padding: Inset{top: 30, right: 40, bottom: 20, left: 40}

            show_bg: true
            draw_bg +: {
                color: #fff
                border_radius: 4.0
            }

            title_view := View {
                width: Fill,
                height: Fit,
                padding: Inset{top: 0, bottom: 25}
                align: Align{x: 0.5, y: 0.0}

                title := Label {
                    flow: Flow.Right{wrap: true},
                    draw_text +: {
                        text_style: TITLE_TEXT {font_size: 13},
                        color: #000
                        flow: Flow.Right{wrap: true}
                    }
                }
            }

            body := View {
                width: Fill,
                height: Fit,
                flow: Down,

                description := Label {
                    width: Fill
                    draw_text +: {
                        text_style: REGULAR_TEXT {
                            font_size: 11.5,
                        },
                        color: #000
                        flow: Flow.Right{wrap: true}
                    }
                }

                View {
                    width: Fill, height: Fit
                    flow: Right,
                    padding: Inset{top: 20, bottom: 20}
                    align: Align{x: 1.0, y: 0.5}
                    spacing: 20

                    cancel_button := RobrixIconButton {
                        width: 120,
                        align: Align{x: 0.5, y: 0.5}
                        padding: 15,
                        draw_icon +: {
                            svg_file: (ICON_FORBIDDEN)
                            color: (COLOR_FG_DANGER_RED),
                        }
                        icon_walk: Walk{width: 16, height: 16, margin: Inset{left: -2, right: -1} }
        
                        draw_bg +: {
                            border_color: (COLOR_FG_DANGER_RED),
                            color: (COLOR_BG_DANGER_RED)
                        }
                        text: "Cancel"
                        draw_text +: {
                            color: (COLOR_FG_DANGER_RED),
                        }
                    }

                    accept_button := RobrixIconButton {
                        width: 120,
                        align: Align{x: 0.5, y: 0.5}
                        padding: 15,
                        draw_icon +: {
                            svg_file: (ICON_CHECKMARK)
                            color: (COLOR_FG_ACCEPT_GREEN),
                        }
                        icon_walk: Walk{width: 16, height: 16, margin: Inset{left: -2, right: -1} }

                        draw_bg +: {
                            border_color: (COLOR_FG_ACCEPT_GREEN),
                            color: (COLOR_BG_ACCEPT_GREEN)
                        }
                        text: "Yes"
                        draw_text +: {
                            color: (COLOR_FG_ACCEPT_GREEN),
                        }
                    }
                }

                tip_view := View {
                    width: Fill,
                    height: Fit,
                    align: Align{x: 0.5, y: 0.0}

                    tip := Label {
                        padding: 0,
                        margin: 0,
                        width: Fill,
                        height: Fit,
                        flow: Flow.Right{wrap: true},
                        align: Align{x: 0.5}
                        draw_text +: {
                            text_style: REGULAR_TEXT {
                                font_size: 9,
                            },
                            color: #A,
                            flow: Flow.Right{wrap: true}
                        }
                        text: "Tip: hold Shift when clicking a button to bypass this prompt."
                    }
                }
            }
        }
    }
}

#[derive(Script, ScriptHook, Widget)]
pub struct JoinLeaveRoomModal {
    #[deref] view: View,
    #[rust] kind: Option<JoinLeaveModalKind>,
    /// Whether the modal is in a final state, meaning the user can only click "Okay" to close it.
    ///
    /// * Set to `Some(true)` after a successful action (e.g., joining or leaving a room).
    /// * Set to `Some(false)` after a join/leave error occurs.
    /// * Set to `None` when the user is still able to interact with the modal.
    #[rust] final_success: Option<bool>,
}

/// Kinds of content that can be shown and handled by the [`JoinLeaveRoomModal`].
#[derive(Clone, Debug)]
pub enum JoinLeaveModalKind {
    /// The user wants to accept an invite to join a new room.
    AcceptInvite(InviteDetails),
    /// The user wants to reject an invite to a room.
    RejectInvite(InviteDetails),
    /// The user wants to join a room that they have not joined yet.
    JoinRoom {
        details: BasicRoomDetails,
        is_space: bool,
    },
    /// The user wants to leave an already-joined room.
    LeaveRoom(BasicRoomDetails),
    /// The user wants to leave an already-joined space.
    /// This is its own variant because it's a much more complex procedure
    /// than leaving a room. Eventually this should be moved to its own modal,
    /// e.g., in order to allow the user to select which joine rooms in the space
    /// that they also want to leave, but for now we reuse this modal for convenience.
    LeaveSpace {
        details: BasicRoomDetails,
        space_request_sender: UnboundedSender<SpaceRequest>,
    },
}
impl JoinLeaveModalKind {
    pub fn room_id(&self) -> &OwnedRoomId {
        match self {
            JoinLeaveModalKind::AcceptInvite(invite)
            | JoinLeaveModalKind::RejectInvite(invite) => invite.room_id(),
            JoinLeaveModalKind::JoinRoom { details, .. }
            | JoinLeaveModalKind::LeaveRoom(details)
            | JoinLeaveModalKind::LeaveSpace { details, .. } => details.room_id(),
        }
    }

    pub fn room_name(&self) -> &RoomNameId {
        match self {
            JoinLeaveModalKind::AcceptInvite(invite)
            | JoinLeaveModalKind::RejectInvite(invite) => invite.room_name_id(),
            JoinLeaveModalKind::JoinRoom { details, .. }
            | JoinLeaveModalKind::LeaveRoom(details)
            | JoinLeaveModalKind::LeaveSpace { details, .. } => details.room_name_id(),
        }
    }

    #[allow(unused)] // remove when we use it in navigate_to_room
    pub fn basic_room_details(&self) -> &BasicRoomDetails {
        match self {
            JoinLeaveModalKind::AcceptInvite(invite)
            | JoinLeaveModalKind::RejectInvite(invite) => &invite.room_info,
            JoinLeaveModalKind::JoinRoom { details, .. }
            | JoinLeaveModalKind::LeaveRoom(details)
            | JoinLeaveModalKind::LeaveSpace { details, .. } => details,
        }
    }
}

/// Actions handled by the parent widget of the [`JoinLeaveRoomModal`].
#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum JoinLeaveRoomModalAction {
    /// The modal should be opened by its parent widget.
    Open {
        /// The kind of action to be performed.
        kind: JoinLeaveModalKind,
        /// Whether to show the tip about holding Shift to bypass the prompt.
        show_tip: bool,
    },
    /// The modal requested its parent widget to close.
    Close {
        /// `True` if the modal was closed after a successful join/leave action.
        /// `False` if the modal was dismissed or closed after a failure/error.
        successful: bool,
        /// Whether the modal was dismissed by the user clicking an internal button.
        was_internal: bool,
    },
}


impl Widget for JoinLeaveRoomModal {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        self.widget_match_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl WidgetMatchEvent for JoinLeaveRoomModal {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions, _scope: &mut Scope) {
        let accept_button = self.view.button(cx, ids!(accept_button));
        let cancel_button = self.view.button(cx, ids!(cancel_button));

        let cancel_clicked = cancel_button.clicked(actions);
        if cancel_clicked ||
            actions.iter().any(|a| matches!(a.downcast_ref(), Some(ModalAction::Dismissed)))
        {
            // Inform other widgets that this modal has been closed.
            cx.action(JoinLeaveRoomModalAction::Close { successful: false, was_internal: cancel_clicked });
            self.reset_state();
            return;
        }

        let Some(kind) = self.kind.as_ref() else { return };
        let mut needs_redraw = false;

        if accept_button.clicked(actions) {
            if let Some(successful) = self.final_success {
                cx.action(JoinLeaveRoomModalAction::Close { successful, was_internal: true });
                self.reset_state();
                return;
            }
            else {
                let title: Cow<str>;
                let description: String;
                let accept_button_text: &str;
                match kind {
                    JoinLeaveModalKind::AcceptInvite(invite) => {
                        title = "Accepting this invite...".into();
                        description = format!(
                            "Accepting an invitation to join \"{}\".\n\n\
                            Waiting for confirmation from the homeserver...",
                            invite.room_name_id(),
                        );
                        accept_button_text = "Joining...";
                        submit_async_request(MatrixRequest::JoinRoom {
                            room_id: invite.room_id().clone(),
                        });
                    }
                    JoinLeaveModalKind::RejectInvite(invite) => {
                        title = "Rejecting this invite...".into();
                        description = format!(
                            "Rejecting an invitation to join \"{}\".\n\n\
                            Waiting for confirmation from the homeserver...",
                            invite.room_name_id(),
                        );
                        accept_button_text = "Rejecting...";
                        submit_async_request(MatrixRequest::LeaveRoom {
                            room_id: invite.room_id().clone(),
                        });
                    }
                    JoinLeaveModalKind::JoinRoom { details, is_space } => {
                        title = format!("Joining this {}...", if *is_space { "space" } else { "room" }).into();
                        description = format!(
                            "Joining \"{}\".\n\n\
                            Waiting for confirmation from the homeserver...",
                            details.room_name_id(),
                        );
                        accept_button_text = "Joining...";
                        submit_async_request(MatrixRequest::JoinRoom {
                            room_id: details.room_id().clone(),
                        });
                    }
                    JoinLeaveModalKind::LeaveRoom(room) => {
                        title = "Leaving this room...".into();
                        description = format!(
                            "Leaving \"{}\".\n\n\
                            Waiting for confirmation from the homeserver...",
                            room.room_name_id(),
                        );
                        accept_button_text = "Leaving...";
                        submit_async_request(MatrixRequest::LeaveRoom {
                            room_id: room.room_id().clone(),
                        });
                    }
                    JoinLeaveModalKind::LeaveSpace { details, space_request_sender } => {
                        title = "Leaving this space...".into();
                        description = format!(
                            "Leaving \"{}\".\n\n\
                            Waiting for confirmation from the homeserver...",
                            details.room_name_id(),
                        );
                        accept_button_text = "Leaving...";
                        if space_request_sender.send(
                            SpaceRequest::LeaveSpace { space_name_id: details.room_name_id().clone() }
                        ).is_err() {
                            enqueue_popup_notification(
                                "Failed to send leave space request.\n\nPlease restart Robrix.",
                                PopupKind::Error,
                                None,
                            );
                        }
                    }
                }

                self.view.label(cx, ids!(title)).set_text(cx, &title);
                self.view.label(cx, ids!(description)).set_text(cx, &description);
                self.view.view(cx, ids!(tip_view)).set_visible(cx, false);
                accept_button.set_text(cx, accept_button_text);
                accept_button.set_enabled(cx, false);
                needs_redraw = true;
            }
        }

        let mut new_final_success = None;
        for action in actions {
            match action.downcast_ref() {
                Some(JoinRoomResultAction::Joined { room_id }) if room_id == kind.room_id() => {
                    enqueue_popup_notification(
                        "Successfully joined room.",
                        PopupKind::Success,
                        Some(3.0),
                    );
                    self.view.label(cx, ids!(title)).set_text(cx, "Joined room!");
                    self.view.label(cx, ids!(description)).set_text(cx, &format!(
                        "Successfully joined \"{}\".",
                        kind.room_name(),
                    ));
                    new_final_success = Some(true);
                }
                Some(JoinRoomResultAction::Failed { room_id, error }) if room_id == kind.room_id() => {
                    self.view.label(cx, ids!(title)).set_text(cx, "Error joining room!");
                    let was_invite = matches!(kind, JoinLeaveModalKind::AcceptInvite(_) | JoinLeaveModalKind::RejectInvite(_));
                    let msg = utils::stringify_join_leave_error(error, kind.room_name(), true, was_invite);
                    self.view.label(cx, ids!(description)).set_text(cx, &msg);
                    enqueue_popup_notification(
                        msg,
                        PopupKind::Error,
                        None,
                    );
                    new_final_success = Some(false);
                }
                _ => {}
            }

            match action.downcast_ref() {
                Some(LeaveRoomResultAction::Left { room_id }) if room_id == kind.room_id() => {
                    let title: &str;
                    let description: String;
                    let popup_msg: Cow<'static, str>;
                    if matches!(kind, JoinLeaveModalKind::AcceptInvite(_) | JoinLeaveModalKind::RejectInvite(_)) {
                        title = "Rejected invite!";
                        description = format!(
                            "Successfully rejected invite to \"{}\".",
                            kind.room_name(),
                        );
                        popup_msg = "Successfully rejected invite.".into();
                    } else {
                        title = "Left room!";
                        description = format!(
                            "Successfully left \"{}\".",
                            kind.room_name(),
                        );
                        popup_msg = "Successfully left room.".into();
                    }
                    self.view.label(cx, ids!(title)).set_text(cx, title);
                    self.view.label(cx, ids!(description)).set_text(cx, &description);
                    enqueue_popup_notification(popup_msg, PopupKind::Success, Some(5.0));
                    new_final_success = Some(true);
                }
                Some(LeaveRoomResultAction::Failed { room_id, error }) if room_id == kind.room_id() => {
                    let title: &str;
                    let description: String;
                    let popup_msg: Cow<'static, str>;
                    if matches!(kind, JoinLeaveModalKind::AcceptInvite(_) | JoinLeaveModalKind::RejectInvite(_)) {
                        title = "Error rejecting invite!";
                        description = utils::stringify_join_leave_error(error, kind.room_name(), false, true);
                        popup_msg = "Failed to reject invite.".into();
                    } else {
                        title = "Error leaving room!";
                        description = utils::stringify_join_leave_error(error, kind.room_name(), false, false);
                        popup_msg = "Failed to leave room.".into();
                    }

                    self.view.label(cx, ids!(title)).set_text(cx, title);
                    self.view.label(cx, ids!(description)).set_text(cx, &description);
                    enqueue_popup_notification(popup_msg, PopupKind::Error, None);
                    new_final_success = Some(false);
                }
                _ => {}
            }

            if let Some(SpaceRoomListAction::LeaveSpaceResult { space_name_id, result }) = action.downcast_ref() {
                if space_name_id.room_id() == kind.room_id() {
                    let title: &str;
                    let description: String;
                    match result {
                        Ok(()) => {
                            title = "Left space!";
                            description = format!("Successfully left \"{space_name_id}\".");
                            new_final_success = Some(true);
                        }
                        Err(e) => {
                            title = "Error leaving space!";
                            description = format!("Failed to leave space \"{space_name_id}\".\n\nError: {e}");
                            new_final_success = Some(false);
                        }
                    }
                    self.view.label(cx, ids!(title)).set_text(cx, title);
                    self.view.label(cx, ids!(description)).set_text(cx, &description);
                }
            }
        }

        if let Some(success) = new_final_success {
            self.final_success = Some(success);
            needs_redraw = true;
            accept_button.apply_over(cx, live!{
                enabled: true
                text: "Okay"
                draw_bg: {
                    color: (COLOR_ACTIVE_PRIMARY),
                    border_color: (COLOR_ACTIVE_PRIMARY)
                }
                draw_text: {
                    color: (COLOR_PRIMARY)
                }
                draw_icon: {
                    color: (COLOR_PRIMARY)
                }
            });
            accept_button.reset_hover(cx);
            cancel_button.set_visible(cx, false);
        }
        if needs_redraw {
            self.redraw(cx);
        }
    }
}

impl JoinLeaveRoomModal {
    fn reset_state(&mut self) {
        self.kind = None;
        self.final_success = None;
    }

    /// Populates this modal with the proper info based on 
    /// the given `kind of join or leave action.
    fn set_kind(
        &mut self,
        cx: &mut Cx,
        kind: JoinLeaveModalKind,
        show_tip: bool,
    ) {
        log!("Showing JoinLeaveRoomModal for {kind:?}");
        let title: &str;
        let description: String;
        let tip_button: &str;

        match &kind {
            JoinLeaveModalKind::AcceptInvite(invite) => {
                title = "Accept this invite?";
                description = format!(
                    "Are you sure you want to accept this invite to join \"{}\"?",
                    invite.room_name_id(),
                );
                tip_button = "Join";
            }
            JoinLeaveModalKind::RejectInvite(invite) => {
                title = "Reject this invite?";
                description = format!(
                    "Are you sure you want to reject this invite to join \"{}\"?\n\n\
                    If this is a private room, you won't be able to join this room \
                    without being re-invited to it.",
                    invite.room_name_id()
                );
                tip_button = "Reject";
            }
            JoinLeaveModalKind::JoinRoom { details, is_space } => {
                title = if *is_space {
                    "Join this space?"
                } else {
                    "Join this room?"
                };
                description = format!(
                    "Are you sure you want to join \"{}\"?",
                    details.room_name_id()
                );
                tip_button = "Join";
            }
            JoinLeaveModalKind::LeaveRoom(room) => {
                title = "Leave this room?";
                description = format!(
                    "Are you sure you want to leave \"{}\"?\n\n\
                    If this is a private room, you won't be able to join this room \
                    without being re-invited to it.",
                    room.room_name_id()
                );
                tip_button = "Leave";
            }
            JoinLeaveModalKind::LeaveSpace { details, .. } => {
                title = "Leave this space?";
                description = format!(
                    "Are you sure you want to leave \"{}\"?\n\n\
                    If you leave this space, you will also leave any joined rooms within this space.\n\n\
                    If this is a private space, you won't be able to join this space \
                    without being re-invited to it.",
                    details.room_name_id()
                );
                tip_button = "Leave";
            }
        }

        self.view.label(cx, ids!(title)).set_text(cx, title);
        self.view.label(cx, ids!(description)).set_text(cx, &description);
        if show_tip {
            self.view.view(cx, ids!(tip_view)).set_visible(cx, true);
            self.view.label(cx, ids!(tip)).set_text(cx, &format!(
                "Tip: hold Shift when clicking the \"{tip_button}\" button to bypass this prompt.",
            ));
        } else {
            self.view.view(cx, ids!(tip_view)).set_visible(cx, false);
        }

        let accept_button = self.button(cx, ids!(accept_button));
        let cancel_button = self.button(cx, ids!(cancel_button));
        accept_button.set_text(cx, "Yes");
        accept_button.apply_over(cx, live!{
            draw_bg: {
                border_color: (COLOR_FG_ACCEPT_GREEN),
                color: (COLOR_BG_ACCEPT_GREEN)
            }
            draw_text: {
                color: (COLOR_FG_ACCEPT_GREEN)
            }
            draw_icon: {
                color: (COLOR_FG_ACCEPT_GREEN)
            }
        });
        accept_button.set_enabled(cx, true);
        accept_button.set_visible(cx, true);
        accept_button.reset_hover(cx);
        cancel_button.set_text(cx, "Cancel");
        cancel_button.set_enabled(cx, true);
        cancel_button.set_visible(cx, true);
        cancel_button.reset_hover(cx);

        self.kind = Some(kind);
        self.final_success = None;
    }
}

impl JoinLeaveRoomModalRef {
    /// Sets the details of this join/leave modal.
    pub fn set_kind(
        &self,
        cx: &mut Cx,
        kind: JoinLeaveModalKind,
        show_tip: bool,
    ) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.set_kind(cx, kind, show_tip);
    }
}
