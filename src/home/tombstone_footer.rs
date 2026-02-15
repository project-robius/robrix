//! A `TombstoneFooter` widget shows information about a tombstoned room.
//!
//! This screen is displayed when a user tries to access a room that has been
//! tombstoned (shut down and replaced with a successor room), offering them
//! the option to join the successor room or stay in the current tombstoned room.

use makepad_widgets::*;
use matrix_sdk::{ruma::OwnedRoomId, RoomState, SuccessorRoom};

use crate::{
    app::AppStateAction,
    room::{BasicRoomDetails, FetchedRoomAvatar, FetchedRoomPreview},
    shared::avatar::AvatarWidgetExt,
    utils,
};

const DEFAULT_TOMBSTONE_REASON: &str = "This room has been replaced and is no longer active.";
const DEFAULT_JOIN_BUTTON_TEXT: &str = "Go to the replacement room";

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::helpers::*;
    use crate::shared::styles::*;
    use crate::shared::avatar::*;
    use crate::shared::icon_button::*;
    use crate::home::invite_screen::*;

    pub TombstoneFooter = {{TombstoneFooter}}<ScrollYView> {
        visible: false,
        width: Fill,
        height: Fit { max: Rel { base: Full, factor: 0.625 } }
        flow: Down,
        align: {x: 0.5}
        padding: 20,
        spacing: 8

        show_bg: true
        draw_bg: {
            color: (COLOR_SECONDARY)
        }

        replacement_reason = <Label> {
            width: Fill, height: Fit,
            flow: RightWrap,
            align: {x: 0.5}
            draw_text: {
                color: (TYPING_NOTICE_TEXT_COLOR),
                text_style: <REGULAR_TEXT>{font_size: 11}
                wrap: Word,
            }
        }

        join_successor_button = <RobrixIconButton> {
            padding: 15,
            draw_icon: {
                svg_file: (ICON_JOIN_ROOM),
                color: (COLOR_FG_ACCEPT_GREEN),
            }
            icon_walk: {width: 17, height: 17, margin: {left: -2, right: -1} }

            draw_bg: {
                border_color: (COLOR_FG_ACCEPT_GREEN),
                color: #f0fff0 // light green
            }
            draw_text: {
                color: (COLOR_FG_ACCEPT_GREEN),
            }
        }

        successor_room_avatar = <Avatar> {
            width: 30, height: 30,
            cursor: Default,
            text_view = { text = { draw_text: {
                text_style: <TITLE_TEXT>{ font_size: 13.0 }
            }}}
        }

        successor_room_name = <Label> {
            width: Fill, height: Fit,
            flow: RightWrap,
            align: {x: 0.5}
            draw_text: {
                text_style: <TITLE_TEXT>{ font_size: 12 }
                color: (COLOR_TEXT)
                wrap: Word,
            }
        }
    }
}

/// Info about the successor room that replaces a tombstoned room.
#[derive(Default, Debug)]
#[allow(clippy::large_enum_variant)]
pub enum SuccessorRoomDetails {
    /// No information was known about the successor room.
    /// This should not happen because it violates the Matrix spec.
    #[default]
    None,
    /// Only basic information was known about the successor room.
    Basic(SuccessorRoom),
    /// All details about the successor room have been fetched.
    Full {
        room_preview: FetchedRoomPreview,
        reason: Option<String>,
    },
}

/// A view that shows information about a tombstoned room and its successor.
#[derive(Live, LiveHook, Widget)]
pub struct TombstoneFooter {
    #[deref]
    view: View,
    /// The ID of the current tombstoned room.
    #[rust]
    room_id: Option<OwnedRoomId>,
    /// The details of the successor room.
    #[rust]
    successor_info: Option<BasicRoomDetails>,
}

impl Widget for TombstoneFooter {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if let Event::Actions(actions) = event {
            if self
                .view
                .button(ids!(join_successor_button))
                .clicked(actions)
            {
                let Some(destination_room) = self.successor_info.clone() else {
                    error!(
                        "BUG: cannot navigate to replacement room: no successor room information."
                    );
                    return;
                };
                cx.action(AppStateAction::NavigateToRoom {
                    room_to_close: self.room_id.clone(),
                    destination_room,
                });
            }
        }
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl TombstoneFooter {
    /// Sets the tombstone information to be displayed by this screen.
    pub fn show(
        &mut self,
        cx: &mut Cx,
        tombstoned_room_id: &OwnedRoomId,
        successor_room_details: &SuccessorRoomDetails,
    ) {
        let replacement_reason = self.view.label(ids!(replacement_reason));
        let join_successor_button = self.view.button(ids!(join_successor_button));
        let successor_room_avatar = self.view.avatar(ids!(successor_room_avatar));
        let successor_room_name = self.view.label(ids!(successor_room_name));

        log!(
            "Showing TombstoneFooter for room {tombstoned_room_id}, Successor: {successor_room_details:?}"
        );
        match successor_room_details {
            SuccessorRoomDetails::None => {
                replacement_reason.set_text(cx, DEFAULT_TOMBSTONE_REASON);
                join_successor_button.set_text(cx, DEFAULT_JOIN_BUTTON_TEXT);
                successor_room_avatar.show_text(cx, None, None, "?");
                successor_room_name.set_text(cx, "(Unknown successor room");
                self.successor_info = None;
            }
            SuccessorRoomDetails::Basic(sr) => {
                replacement_reason
                    .set_text(cx, sr.reason.as_deref().unwrap_or(DEFAULT_TOMBSTONE_REASON));
                join_successor_button.set_text(cx, DEFAULT_JOIN_BUTTON_TEXT);
                successor_room_avatar.show_text(cx, None, None, "#");
                successor_room_name.set_text(cx, &format!("Room ID {}", sr.room_id));
                self.successor_info = Some(sr.into());
            }
            SuccessorRoomDetails::Full {
                room_preview,
                reason,
            } => {
                replacement_reason
                    .set_text(cx, reason.as_deref().unwrap_or(DEFAULT_TOMBSTONE_REASON));
                join_successor_button.set_text(
                    cx,
                    matches!(room_preview.state, Some(RoomState::Joined))
                        .then_some(DEFAULT_JOIN_BUTTON_TEXT)
                        .unwrap_or("Join the replacement room"),
                );
                match &room_preview.room_avatar {
                    FetchedRoomAvatar::Text(text) => {
                        successor_room_avatar.show_text(cx, None, None, text);
                    }
                    FetchedRoomAvatar::Image(image_data) => {
                        let res = successor_room_avatar.show_image(cx, None, |cx, img_ref| {
                            utils::load_png_or_jpg(&img_ref, cx, image_data)
                        });
                        if res.is_err() {
                            successor_room_avatar.show_text(
                                cx,
                                None,
                                None,
                                room_preview
                                    .room_name_id
                                    .name_for_avatar()
                                    .as_deref()
                                    .unwrap_or("?"),
                            );
                        }
                    }
                }
                match room_preview.room_name_id.name_for_avatar().as_deref() {
                    Some(n) => successor_room_name.set_text(cx, n),
                    _ => successor_room_name.set_text(
                        cx,
                        &format!("Unnamed Room, ID: {}", room_preview.room_name_id.room_id()),
                    ),
                }
                self.successor_info = Some(room_preview.clone().into());
            }
        }

        join_successor_button.reset_hover(cx);
        self.room_id = Some(tombstoned_room_id.clone());
        self.set_visible(cx, true);
    }

    /// Hides the tombstone footer and clears any successor room information.
    fn hide(&mut self, cx: &mut Cx) {
        self.set_visible(cx, false);
        self.successor_info = None;
    }
}

impl TombstoneFooterRef {
    /// See [`TombstoneFooter::show()`].
    pub fn show(
        &self,
        cx: &mut Cx,
        tombstoned_room_id: &OwnedRoomId,
        successor_room_details: &SuccessorRoomDetails,
    ) {
        let Some(mut inner) = self.borrow_mut() else {
            return;
        };
        inner.show(cx, tombstoned_room_id, successor_room_details);
    }

    /// See [`TombstoneFooter::hide()`].
    pub fn hide(&self, cx: &mut Cx) {
        let Some(mut inner) = self.borrow_mut() else {
            return;
        };
        inner.hide(cx);
    }
}
