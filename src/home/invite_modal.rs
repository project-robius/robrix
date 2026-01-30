//! A modal dialog for inviting a user to a room.

use makepad_widgets::*;
use ruma::OwnedUserId;

use crate::home::room_screen::InviteResultAction;
use crate::shared::styles::*;
use crate::sliding_sync::{MatrixRequest, submit_async_request};
use crate::utils::RoomNameId;


live_design! {
    use link::theme::*;
    use link::widgets::*;

    use crate::shared::styles::*;
    use crate::shared::helpers::*;
    use crate::shared::icon_button::RobrixIconButton;

    pub InviteModal = {{InviteModal}} {
        width: Fit
        height: Fit

        <RoundedView> {
            width: 400
            height: Fit
            align: {x: 0.5}
            flow: Down
            padding: {top: 30, right: 25, bottom: 20, left: 25}

            show_bg: true
            draw_bg: {
                color: (COLOR_PRIMARY)
                border_radius: 4.0
            }

            title_view = <View> {
                width: Fill,
                height: Fit,
                padding: {top: 0, bottom: 25}
                align: {x: 0.5, y: 0.0}

                title = <Label> {
                    flow: RightWrap,
                    draw_text: {
                        text_style: <TITLE_TEXT>{font_size: 13},
                        color: #000
                        wrap: Word
                    }
                    text: "Invite to Room"
                }
            }

            user_id_input = <SimpleTextInput> {
                draw_text: {
                    text_style: <REGULAR_TEXT>{font_size: 11},
                    color: #000
                }
                empty_text: "@user:example.org",
            }

            <View> {
                width: Fill, height: Fit
                flow: Right,
                padding: {top: 20, bottom: 10}
                align: {x: 1.0, y: 0.5}
                spacing: 20

                cancel_button = <RobrixIconButton> {
                    width: 120,
                    align: {x: 0.5, y: 0.5}
                    padding: 12,
                    draw_icon: {
                        svg_file: (ICON_FORBIDDEN)
                        color: (COLOR_TEXT),
                    }
                    icon_walk: {width: 16, height: 16, margin: {left: -2, right: -1} }

                    draw_bg: {
                        border_size: 1.0
                        border_color: (COLOR_BG_DISABLED),
                        color: (COLOR_SECONDARY)
                    }
                    text: "Cancel"
                    draw_text:{
                        color: (COLOR_TEXT),
                    }
                }

                confirm_button = <RobrixIconButton> {
                    width: 120
                    align: {x: 0.5, y: 0.5}
                    padding: 12,
                    draw_icon: {
                        svg_file: (ICON_ADD_USER)
                        color: (COLOR_FG_ACCEPT_GREEN),
                    }
                    icon_walk: {width: 16, height: 16, margin: {left: -2, right: -1} }

                    draw_bg: {
                        border_color: (COLOR_FG_ACCEPT_GREEN),
                        color: (COLOR_BG_ACCEPT_GREEN)
                    }
                    text: "Invite"
                    draw_text:{
                        color: (COLOR_FG_ACCEPT_GREEN),
                    }
                }

                okay_button = <RobrixIconButton> {
                    visible: false
                    width: 120
                    align: {x: 0.5, y: 0.5}
                    padding: 12,
                    draw_icon: {
                        svg_file: (ICON_CHECKMARK)
                        color: (COLOR_PRIMARY),
                    }
                    icon_walk: {width: 16, height: 16, margin: {left: -2, right: -1} }

                    draw_bg: {
                        color: (COLOR_ACTIVE_PRIMARY)
                    }
                    text: "Okay"
                    draw_text:{
                        color: (COLOR_PRIMARY),
                    }
                }
            }

            status_label = <Label> {
                width: Fill,
                height: Fit,
                flow: RightWrap,
                align: {x: 0.5, y: 0.0}
                draw_text: {
                    wrap: Word
                    text_style: <REGULAR_TEXT>{font_size: 11},
                    color: #000
                }
                text: ""
            }
        }
    }
}

/// Actions emitted by other widgets to show or hide the `InviteModal`.
#[derive(Clone, Debug)]
pub enum InviteModalAction {
    /// Open the modal to invite a user to the given room.
    Open(RoomNameId),
    /// Close the modal.
    Close,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
enum InviteModalState {
    /// Waiting for the user to enter a user ID.
    #[default]
    WaitingForUserInput,
    /// Waiting for the invite to be sent.
    WaitingForInvite(OwnedUserId),
    /// The invite was sent successfully.
    InviteSuccess,
    /// An error occurred while sending the invite.
    InviteError,
}


#[derive(Live, LiveHook, Widget)]
pub struct InviteModal {
    #[deref] view: View,
    #[rust] state: InviteModalState,
    #[rust] room_name_id: Option<RoomNameId>,
}

impl Widget for InviteModal {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        self.widget_match_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl WidgetMatchEvent for InviteModal {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions, _scope: &mut Scope) {
        let cancel_button = self.view.button(ids!(cancel_button));

        // Handle canceling/closing the modal.
        let cancel_clicked = cancel_button.clicked(actions);
        if cancel_clicked ||
            actions.iter().any(|a| matches!(a.downcast_ref(), Some(ModalAction::Dismissed)))
        {
            // If the modal was dismissed by clicking outside of it, we MUST NOT emit
            // a `InviteModalAction::Close` action, as that would cause
            // an infinite action feedback loop.
            if cancel_clicked {
                cx.action(InviteModalAction::Close);
            }
            return;
        }

        // Handle the okay button (shown after invite success).
        let okay_button = self.view.button(ids!(okay_button));
        if okay_button.clicked(actions) {
            cx.action(InviteModalAction::Close);
            return;
        }

        let confirm_button = self.view.button(ids!(confirm_button));
        let user_id_input = self.view.text_input(ids!(user_id_input));
        let status_label = self.view.label(ids!(status_label));

        // Handle return key or invite button click.
        if let Some(user_id_str) = confirm_button.clicked(actions)
            .then(|| user_id_input.text())
            .or_else(|| user_id_input.returned(actions).map(|(t, _)| t))
        {
            // Validate the user ID
            if user_id_str.is_empty() {
                status_label.apply_over(cx, live!{
                    text: "Please enter a user ID.",
                    draw_text: {
                        color: (COLOR_FG_DANGER_RED),
                    },
                });
                self.view.redraw(cx);
                return;
            }

            // Try to parse the user ID
            match ruma::UserId::parse(&user_id_str) {
                Ok(user_id) => {
                    if let Some(room_name_id) = &self.room_name_id {
                        submit_async_request(MatrixRequest::InviteUser {
                            room_id: room_name_id.room_id().clone(),
                            user_id: user_id.to_owned(),
                        });
                        self.state = InviteModalState::WaitingForInvite(user_id.to_owned());
                        status_label.apply_over(cx, live!(
                            text: "Sending invite...",
                            draw_text: {
                                color: (COLOR_ACTIVE_PRIMARY_DARKER),
                            },
                        ));
                        confirm_button.set_enabled(cx, false);
                        user_id_input.set_is_read_only(cx, true);
                    }
                }
                Err(_) => {
                    status_label.apply_over(cx, live!(
                        text: "Invalid User ID. Expected format: @user:server.xyz",
                        draw_text: {
                            color: (COLOR_FG_DANGER_RED),
                        },
                    ));
                    user_id_input.set_key_focus(cx);
                }
            }
            self.view.redraw(cx);
        }

        // Handle the result of a previously-sent invite.
        if let InviteModalState::WaitingForInvite(invited_user_id) = &self.state {
            for action in actions {
                let new_state = match action.downcast_ref() {
                    Some(InviteResultAction::Sent { room_id, user_id })
                        if self.room_name_id.as_ref().is_some_and(|rni| rni.room_id() == room_id)
                            && invited_user_id == user_id
                    => {
                        let status = format!("Successfully invited {user_id}!");
                        status_label.apply_over(cx, live!{
                            text: (status),
                            draw_text: {
                                color: (COLOR_FG_ACCEPT_GREEN)
                            }
                        });
                        confirm_button.set_visible(cx, false);
                        cancel_button.set_visible(cx, false);
                        okay_button.set_visible(cx, true);
                        Some(InviteModalState::InviteSuccess)
                    }
                    Some(InviteResultAction::Failed { room_id, user_id, error })
                        if self.room_name_id.as_ref().is_some_and(|rni| rni.room_id() == room_id)
                            && invited_user_id == user_id
                    => {
                        let status = format!("Failed to send invite: {error}");
                        status_label.apply_over(cx, live!{
                            text: (status),
                            draw_text: {
                                color: (COLOR_FG_DANGER_RED),
                            }
                        });
                        confirm_button.set_enabled(cx, true);
                        user_id_input.set_is_read_only(cx, false);
                        user_id_input.set_key_focus(cx);
                        Some(InviteModalState::InviteError)
                    }
                    _ => None,
                };
                if let Some(new_state) = new_state {
                    self.state = new_state;
                    self.view.redraw(cx);
                    break;
                }
            }
        }
    }
}

impl InviteModal {
    pub fn show(&mut self, cx: &mut Cx, room_name_id: RoomNameId) {
        self.view.label(ids!(title)).set_text(
            cx,
            &format!("Invite to {room_name_id}"),
        );
        self.state = InviteModalState::WaitingForUserInput;
        self.room_name_id = Some(room_name_id);

        // Reset the UI state
        let confirm_button = self.view.button(ids!(confirm_button));
        let cancel_button = self.view.button(ids!(cancel_button));
        let okay_button = self.view.button(ids!(okay_button));
        let user_id_input = self.view.text_input(ids!(user_id_input));
        let status_label = self.view.label(ids!(status_label));
        confirm_button.set_visible(cx, true);
        confirm_button.set_enabled(cx, true);
        confirm_button.reset_hover(cx);
        cancel_button.set_visible(cx, true);
        cancel_button.set_enabled(cx, true);
        cancel_button.reset_hover(cx);
        okay_button.set_visible(cx, false);
        okay_button.reset_hover(cx);
        user_id_input.set_is_read_only(cx, false);
        user_id_input.set_text(cx, "");
        status_label.set_text(cx, "");
        self.view.redraw(cx);
    }
}

impl InviteModalRef {
    pub fn show(&self, cx: &mut Cx, room_name_id: RoomNameId) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.show(cx, room_name_id);
    }
}
