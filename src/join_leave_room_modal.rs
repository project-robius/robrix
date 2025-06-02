use makepad_widgets::*;
use matrix_sdk::ruma::OwnedRoomId;

use crate::{home::invite_screen::{InviteDetails, JoinRoomAction, LeaveRoomAction}, room::BasicRoomDetails, shared::popup_list::enqueue_popup_notification, sliding_sync::{submit_async_request, MatrixRequest}, utils::{self, room_name_or_id}};

live_design! {
    use link::theme::*;
    use link::widgets::*;

    use crate::shared::styles::*;
    use crate::shared::icon_button::RobrixIconButton;

    pub JoinLeaveRoomModal = {{JoinLeaveRoomModal}} {
        width: Fit
        height: Fit

        <RoundedView> {
            flow: Down
            width: 400
            height: Fit
            padding: {top: 25, right: 40, bottom: 20, left: 40}
            spacing: 10

            show_bg: true
            draw_bg: {
                color: #fff
                border_radius: 3.0
            }

            title_view = <View> {
                width: Fill,
                height: Fit,
                padding: {top: 0, bottom: 20}
                align: {x: 0.5, y: 0.0}

                title = <Label> {
                    flow: RightWrap,
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

                description = <Label> {
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
                    padding: {top: 20, bottom: 20}
                    align: {x: 1.0, y: 0.5}
                    spacing: 20

                    cancel_button = <RobrixIconButton> {
                        align: {x: 0.5, y: 0.5}
                        padding: 15,
                        draw_icon: {
                            svg_file: (ICON_BLOCK_USER)
                            color: (COLOR_DANGER_RED),
                        }
                        icon_walk: {width: 16, height: 16, margin: {left: -2, right: -1} }
        
                        draw_bg: {
                            border_color: (COLOR_DANGER_RED),
                            color: #fff0f0 // light red
                        }
                        text: "Cancel"
                        draw_text:{
                            color: (COLOR_DANGER_RED),
                        }
                    }

                    accept_button = <RobrixIconButton> {
                        align: {x: 0.5, y: 0.5}
                        padding: 15,
                        draw_icon: {
                            svg_file: (ICON_CHECKMARK)
                            color: (COLOR_ACCEPT_GREEN),
                        }
                        icon_walk: {width: 16, height: 16, margin: {left: -2, right: -1} }

                        draw_bg: {
                            border_color: (COLOR_ACCEPT_GREEN),
                            color: #f0fff0 // light green
                        }
                        text: "Yes"
                        draw_text:{
                            color: (COLOR_ACCEPT_GREEN),
                        }
                    }
                }

                tip = <Label> {
                    padding: 0,
                    width: Fill,
                    height: Fit,
                    flow: RightWrap,
                    align: {x: 0.5}
                    draw_text: {
                        text_style: <REGULAR_TEXT>{
                            font_size: 9,
                        },
                        color: #A,
                        wrap: Word
                    }
                    text: "Tip: hold Shift when clicking a button to bypass this prompt."
                }
            }
        }
    }
}

#[derive(Live, LiveHook, Widget)]
pub struct JoinLeaveRoomModal {
    #[deref] view: View,
    #[rust] kind: Option<JoinLeaveModalKind>,
    #[rust] is_final: bool,
}

/// Kinds of content that can be shown and handled by the [`JoinLeaveRoomModal`].
#[derive(Clone, Debug)]
pub enum JoinLeaveModalKind {
    /// The user wants to accept an invite to join a new room.
    AcceptInvite(InviteDetails),
    /// The user wants to reject an invite to a room.
    RejectInvite(InviteDetails),
    /// The user wants to join a room that they have not joined yet.
    JoinRoom(BasicRoomDetails),
    /// The user wants to leave an already-joined room.
    LeaveRoom(BasicRoomDetails),
}
impl JoinLeaveModalKind {
    pub fn room_id(&self) -> &OwnedRoomId {
        match self {
            JoinLeaveModalKind::AcceptInvite(invite) => &invite.room_id,
            JoinLeaveModalKind::RejectInvite(invite) => &invite.room_id,
            JoinLeaveModalKind::JoinRoom(room) => &room.room_id,
            JoinLeaveModalKind::LeaveRoom(room) => &room.room_id,
        }
    }

    pub fn room_name(&self) -> Option<&str> {
        match self {
            JoinLeaveModalKind::AcceptInvite(invite) => invite.room_name.as_deref(),
            JoinLeaveModalKind::RejectInvite(invite) => invite.room_name.as_deref(),
            JoinLeaveModalKind::JoinRoom(room) => room.room_name.as_deref(),
            JoinLeaveModalKind::LeaveRoom(room) => room.room_name.as_deref(),
        }
    }
}

/// Actions handled by the parent widget of the [`JoinLeaveRoomModal`].
#[derive(Clone, Debug, DefaultNone)]
pub enum JoinLeaveRoomModalAction {
    /// The modal should be opened by its parent widget
    Open(JoinLeaveModalKind),
    /// The modal requested its parent widget to close.
    Close {
        /// Whether the modal was canceled (aborted) by clicking the cancel button,
        /// or if it was closed after completing a full sequence.
        was_canceled: bool,
    },
    None,
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
        let accept_button = self.view.button(id!(accept_button));
        let cancel_button = self.view.button(id!(cancel_button));

        if cancel_button.clicked(actions) {
            // Inform the parent widget to close this modal.
            cx.action(JoinLeaveRoomModalAction::Close { was_canceled: true });
            self.reset_state();
            return;
        }

        let Some(kind) = self.kind.as_ref() else { return };

        let mut needs_redraw = false;
        if accept_button.clicked(actions) {
            if self.is_final {
                cx.action(JoinLeaveRoomModalAction::Close { was_canceled: false });
                self.reset_state();
                return;
            }
            else {
                let title: &str;
                let description: String;
                let accept_button_text: &str;
                match kind {
                    JoinLeaveModalKind::AcceptInvite(invite) => {
                        title = "Accepting this invite...";
                        description = format!(
                            "Accepting an invitation to join \"{}\".\n\n\
                            Waiting for confirmation from the homeserver...",
                            room_name_or_id(invite.room_name.as_ref(), &invite.room_id),
                        );
                        accept_button_text = "Joining...";
                        submit_async_request(MatrixRequest::JoinRoom {
                            room_id: invite.room_id.clone(),
                        });
                    }
                    JoinLeaveModalKind::RejectInvite(invite) => {
                        title = "Rejecting this invite...";
                        description = format!(
                            "Rejecting an invitation to join \"{}\".\n\n\
                            Waiting for confirmation from the homeserver...",
                            room_name_or_id(invite.room_name.as_ref(), &invite.room_id),
                        );
                        accept_button_text = "Rejecting...";
                        submit_async_request(MatrixRequest::LeaveRoom {
                            room_id: invite.room_id.clone(),
                        });
                    }
                    JoinLeaveModalKind::JoinRoom(room) => {
                        title = "Joining this room...";
                        description = format!(
                            "Joining \"{}\".\n\n\
                            Waiting for confirmation from the homeserver...",
                            room_name_or_id(room.room_name.as_ref(), &room.room_id),
                        );
                        accept_button_text = "Joining...";
                        submit_async_request(MatrixRequest::JoinRoom {
                            room_id: room.room_id.clone(),
                        });
                    }
                    JoinLeaveModalKind::LeaveRoom(room) => {
                        title = "Leaving this room...";
                        description = format!(
                            "Leaving \"{}\".\n\n\
                            Waiting for confirmation from the homeserver...",
                            room_name_or_id(room.room_name.as_ref(), &room.room_id),
                        );
                        accept_button_text = "Leaving...";
                        submit_async_request(MatrixRequest::LeaveRoom {
                            room_id: room.room_id.clone(),
                        });
                    }
                }

                self.view.label(id!(title)).set_text(cx, &title);
                self.view.label(id!(description)).set_text(cx, &description);
                self.view.label(id!(tip)).set_text(cx, "");
                accept_button.set_text(cx, accept_button_text);
                accept_button.set_enabled(cx, false);
                needs_redraw = true;
            }
        }

        for action in actions {
            match action.downcast_ref() {
                Some(JoinRoomAction::Joined { room_id }) if room_id == kind.room_id() => {
                    enqueue_popup_notification("Successfully joined room.".into());
                    self.view.label(id!(title)).set_text(cx, "Joined room!");
                    self.view.label(id!(description)).set_text(cx, &format!(
                        "Successfully joined \"{}\".",
                        room_name_or_id(kind.room_name(), room_id),
                    ));
                    accept_button.set_enabled(cx, true);
                    accept_button.set_text(cx, "Okay"); // TODO: set color to blue (like login button)
                    cancel_button.set_visible(cx, false);
                    self.is_final = true;
                }
                Some(JoinRoomAction::Failed { room_id, error }) if room_id == kind.room_id() => {
                    self.view.label(id!(title)).set_text(cx, "Error joining room!");
                    let msg = utils::join_leave_error_to_string(error, kind.room_name(), true, false)
                        .unwrap_or_else(|| format!("Failed to join room: {error}"));
                    self.view.label(id!(description)).set_text(cx, &msg);
                    enqueue_popup_notification(msg);
                    accept_button.set_enabled(cx, true);
                    accept_button.set_text(cx, "Okay"); // TODO: set color to blue (like login button)
                    cancel_button.set_visible(cx, false);
                    self.is_final = true;
                }
                _ => {}
            }
            match action.downcast_ref() {
                Some(LeaveRoomAction::Left { room_id }) if room_id == kind.room_id() => {
                    let title: &str;
                    let description: String;
                    let popup_msg: String;
                    match kind {
                        JoinLeaveModalKind::AcceptInvite(_) | JoinLeaveModalKind::RejectInvite(_) => {
                            title = "Rejected invite!";
                            description = format!(
                                "Successfully rejected invite to \"{}\".",
                                room_name_or_id(kind.room_name(), room_id),
                            );
                            popup_msg = "Successfully rejected invite.".into();
                        }
                        JoinLeaveModalKind::JoinRoom(_) | JoinLeaveModalKind::LeaveRoom(_) => {
                            title = "Left room!";
                            description = format!(
                                "Successfully left \"{}\".",
                                room_name_or_id(kind.room_name(), room_id),
                            );
                            popup_msg = "Successfully left room.".into();
                        }
                    }
                    self.view.label(id!(title)).set_text(cx, &title);
                    self.view.label(id!(description)).set_text(cx, &description);
                    enqueue_popup_notification(popup_msg);
                    accept_button.set_enabled(cx, true);
                    accept_button.set_text(cx, "Okay"); // TODO: set color to blue (like login button)
                    cancel_button.set_visible(cx, false);
                    self.is_final = true;
                }
                Some(LeaveRoomAction::Failed { room_id, error }) if room_id == kind.room_id() => {
                    let title: &str;
                    let description: String;
                    let popup_msg: String;
                    match kind {
                        JoinLeaveModalKind::AcceptInvite(_) | JoinLeaveModalKind::RejectInvite(_) => {
                            title = "Error rejecting invite!";
                            description = utils::join_leave_error_to_string(error, kind.room_name(), false, true)
                                .unwrap_or_else(|| format!(
                                    "Failed to reject invite to \"{}\".",
                                    room_name_or_id(kind.room_name(), room_id),
                                ));
                            popup_msg = "Failed to reject invite.".into();
                        }
                        JoinLeaveModalKind::JoinRoom(_) | JoinLeaveModalKind::LeaveRoom(_) => {
                            title = "Error leaving room!";
                            description = utils::join_leave_error_to_string(error, kind.room_name(), false, true)
                                .unwrap_or_else(|| format!(
                                    "Failed to leave \"{}\": {error}",
                                    room_name_or_id(kind.room_name(), room_id),
                                ));
                            popup_msg = "Failed to leave room.".into();
                        }
                    }

                    self.view.label(id!(title)).set_text(cx, title);
                    self.view.label(id!(description)).set_text(cx, &description);
                    enqueue_popup_notification(popup_msg);
                    accept_button.set_enabled(cx, true);
                    accept_button.set_text(cx, "Okay"); // TODO: set color to blue (like login button)
                    cancel_button.set_visible(cx, false);
                    self.is_final = true;
                }
                _ => {}
            }
        }

        if needs_redraw {
            self.redraw(cx);
        }
    }
}

impl JoinLeaveRoomModal {
    fn reset_state(&mut self) {
        self.kind = None;
        self.is_final = false;
    }

    fn set_kind(
        &mut self,
        cx: &mut Cx,
        kind: JoinLeaveModalKind,
    ) {
        log!("Initializing JoinLeaveRoomModal with kind: {kind:?}");
        let title: &str;
        let description: String;
        let tip_button: &str;

        match &kind {
            JoinLeaveModalKind::AcceptInvite(invite) => {
                title = "Accept this invite?";
                description = format!(
                    "Are you sure you want to accept this invite to join \"{}\"?",
                    room_name_or_id(invite.room_name.as_ref(), &invite.room_id),
                );
                tip_button = "Join";
            }
            JoinLeaveModalKind::RejectInvite(invite) => {
                title = "Reject this invite?";
                description = format!(
                    "Are you sure you want to reject this invite to join \"{}\"?\n\n\
                    If this is a private room, you won't be able to join this room \
                    without being re-invited to it.",
                    room_name_or_id(invite.room_name.as_ref(), &invite.room_id)
                );
                tip_button = "Reject";
            }
            JoinLeaveModalKind::JoinRoom(room) => {
                title = "Join this room?";
                description = format!(
                    "Are you sure you want to join \"{}\"?",
                    room_name_or_id(room.room_name.as_ref(), &room.room_id)
                );
                tip_button = "Join";
            }
            JoinLeaveModalKind::LeaveRoom(room) => {
                title = "Leave this room?";
                description = format!(
                    "Are you sure you want to leave \"{}\"?\n\n\
                    If this is a private room, you won't be able to join this room \
                    without being re-invited to it.",
                    room_name_or_id(room.room_name.as_ref(), &room.room_id)
                );
                tip_button = "Leave";
            }
        }

        self.view.label(id!(title)).set_text(cx, &title);
        self.view.label(id!(description)).set_text(cx, &description);
        self.view.label(id!(tip)).set_text(cx, &format!(
            "Tip: hold Shift when clicking the \"{tip_button}\" button to bypass this prompt.",
        ));

        let accept_button = self.button(id!(accept_button));
        let cancel_button = self.button(id!(cancel_button));
        accept_button.set_text(cx, "Yes");
        accept_button.set_enabled(cx, true);
        accept_button.set_visible(cx, true);
        cancel_button.set_text(cx, "Cancel");
        cancel_button.set_enabled(cx, true);
        cancel_button.set_visible(cx, true);

        self.kind = Some(kind);
        self.is_final = false;
    }
}

impl JoinLeaveRoomModalRef {
    pub fn set_kind(
        &self,
        cx: &mut Cx,
        kind: JoinLeaveModalKind,
    ) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_kind(cx, kind);
        }
    }
}
