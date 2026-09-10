//! A confirmation modal for blocking or unblocking a user.

use makepad_widgets::*;
use matrix_sdk::ruma::{OwnedRoomId, OwnedUserId};
use crate::{home::rooms_list::{InviteState, set_invite_state}, sliding_sync::{MatrixRequest, submit_async_request}};

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*


    mod.widgets.BlockUserModal = set_type_default() do #(BlockUserModal::register_widget(vm)) {
        ..mod.widgets.SmallModal

        title := ModalTitle {}

        body := ModalBody {}

        ModalBody {
            margin: Inset{top: 15}
            draw_text +: {
                color: (COLOR_FG_DANGER_RED)
            }
            text: "Changing your blocked users will reload all room timelines from scratch. You may lose your viewing position in each room."
        }

        buttons_view := ModalButtonsRow {
            cancel_button := RobrixNeutralIconButton {
                width: Fit{min: FitBound.Abs(120.0)},
                align: Align{x: 0.5, y: 0.5}
                padding: 15,
                draw_icon.svg: (ICON_CLOSE)
                icon_walk: Walk{width: 16, height: 16, margin: Inset{left: -2, right: -1} }
                text: "Cancel"
            }

            block_button := RobrixNegativeIconButton {
                // Grows past 120 for the longer "Reject & Block" label.
                width: Fit{min: FitBound.Abs(120)},
                align: Align{x: 0.5, y: 0.5}
                padding: 15,
                draw_icon.svg: (ICON_FORBIDDEN)
                icon_walk: Walk{width: 16, height: 16, margin: Inset{left: -2, right: -1} }
                text: "Block"
            }

            unblock_button := RobrixPositiveIconButton {
                visible: false,
                width: Fit{min: FitBound.Abs(120)},
                align: Align{x: 0.5, y: 0.5}
                padding: 15,
                draw_icon.svg: (ICON_CHECKMARK)
                icon_walk: Walk{width: 16, height: 16, margin: Inset{left: -2, right: -1} }
                text: "Unblock"
            }
        }
    }
}

/// A pending block/unblock action that the user must confirm.
#[derive(Clone, Debug)]
#[doc(alias("ignore", "unignore"))]
pub struct BlockUserRequest {
    pub user_id: OwnedUserId,
    /// Shown in the prompt instead of the user ID, if known.
    pub display_name: Option<String>,
    /// Whether to block (`true`) or unblock (`false`) the user.
    pub block: bool,
    /// Also reject this user's pending invite to this room, if set.
    pub reject_invite_to: Option<OwnedRoomId>,
}
impl BlockUserRequest {
    /// Returns the name to show in the prompt, falling back to the user ID.
    pub fn displayable_name(&self) -> &str {
        self.display_name
            .as_deref()
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| self.user_id.as_str())
    }
}

/// Actions handled by the parent widget of the [`BlockUserModal`].
#[derive(Clone, Debug)]
pub enum BlockUserModalAction {
    /// The modal should be opened by its parent widget to confirm the given request.
    Open(BlockUserRequest),
    /// The user confirmed the request; the block, and any invite rejection, has been submitted.
    Confirmed(BlockUserRequest),
    /// The modal requested its parent widget to close it.
    Close,
}


#[derive(Script, ScriptHook, Widget)]
pub struct BlockUserModal {
    #[deref] view: View,
    #[rust] request: Option<BlockUserRequest>,
}

impl Widget for BlockUserModal {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        self.widget_match_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl WidgetMatchEvent for BlockUserModal {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions, _scope: &mut Scope) {
        let cancel_clicked = self.view.button(cx, ids!(cancel_button)).clicked(actions);
        if cancel_clicked
            || actions.iter().any(|a| matches!(a.downcast_ref(), Some(ModalAction::Dismissed)))
        {
            self.request = None;
            // Dismissing by clicking outside already closes the modal, and emitting
            // `Close` for it would cause an infinite action loop.
            if cancel_clicked {
                cx.action(BlockUserModalAction::Close);
            }
            return;
        }

        let confirmed = self.view.button(cx, ids!(block_button)).clicked(actions)
            || self.view.button(cx, ids!(unblock_button)).clicked(actions);
        if !confirmed { return }
        let Some(request) = self.request.take() else { return };

        submit_async_request(MatrixRequest::BlockUser {
            user_id: request.user_id.clone(),
            block: request.block,
        });
        if let Some(room_id) = request.reject_invite_to.as_ref() {
            submit_async_request(MatrixRequest::LeaveRoom { room_id: room_id.clone() });
            set_invite_state(cx, room_id, InviteState::WaitingForLeaveResult);
        }
        cx.action(BlockUserModalAction::Confirmed(request));
        cx.action(BlockUserModalAction::Close);
    }
}

impl BlockUserModal {
    fn set_info(&mut self, cx: &mut Cx, request: BlockUserRequest) {
        let name = request.displayable_name();
        let (title, body, confirm_text) = if request.reject_invite_to.is_some() {
            (
                "Reject this invite and block?",
                format!(
                    "Are you sure you want to reject this invite and block {name}?\n\n\
                    You won't see any messages or invites from them in any room."
                ),
                "Reject & Block",
            )
        } else if request.block {
            (
                "Block this user?",
                format!(
                    "Are you sure you want to block {name}?\n\n\
                    You won't see any messages or invites from them in any room."
                ),
                "Block",
            )
        } else {
            (
                "Unblock this user?",
                format!(
                    "Are you sure you want to unblock {name}?\n\n\
                    You'll start seeing their messages and invites again."
                ),
                "Unblock",
            )
        };

        self.view.label(cx, ids!(title)).set_text(cx, title);
        self.view.label(cx, ids!(body)).set_text(cx, &body);

        let block_button = self.view.button(cx, ids!(block_button));
        block_button.set_visible(cx, request.block);
        block_button.set_text(cx, confirm_text);
        block_button.reset_hover(cx);

        let unblock_button = self.view.button(cx, ids!(unblock_button));
        unblock_button.set_visible(cx, !request.block);
        unblock_button.set_text(cx, confirm_text);
        unblock_button.reset_hover(cx);

        self.view.button(cx, ids!(cancel_button)).reset_hover(cx);
        self.request = Some(request);
        self.redraw(cx);
    }
}

impl BlockUserModalRef {
    /// See [`BlockUserModal::set_info()`].
    pub fn set_info(&self, cx: &mut Cx, request: BlockUserRequest) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.set_info(cx, request);
    }
}
