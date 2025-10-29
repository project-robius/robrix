//! A `TombstoneFooter` widget shows information about a tombstoned room.
//!
//! This screen is displayed when a user tries to access a room that has been
//! tombstoned (shut down and replaced with a successor room), offering them
//! the option to join the successor room or stay in the current tombstoned room.

use makepad_widgets::*;
use matrix_sdk::{
    ruma::OwnedRoomId,
    RoomDisplayName,
    SuccessorRoom
};

use crate::{
    app::AppStateAction, 
    home::rooms_list::RoomsListRef,
    room::{BasicRoomDetails, RoomPreviewAvatar}, 
    shared::avatar::AvatarWidgetExt, 
    utils::{self, room_name_or_id}
};

const DEFAULT_TOMBSTONE_REASON: &str = "This room has been replaced and is no longer active";

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::helpers::*;
    use crate::shared::styles::*;
    use crate::shared::avatar::*;
    use crate::shared::icon_button::*;
    use crate::home::invite_screen::*;

    pub TombstoneFooter = {{TombstoneFooter}} {
        visible: false,
        width: Fill, height: Fit
        flow: Down,
        align: {x: 0.5}
        padding: 20,
        spacing: 8

        show_bg: false // don't cover up the RoomInputBar

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
            text: "Go to the replacement room"
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


/// A view that shows information about a tombstoned room and its successor.
#[derive(Live, LiveHook, Widget)]
pub struct TombstoneFooter {
    #[deref] view: View,
    /// The ID of the current tombstoned room.
    #[rust] room_id: Option<OwnedRoomId>,
    /// The details of the successor room.
    #[rust] successor_info: Option<BasicRoomDetails>,
}

impl Widget for TombstoneFooter {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if let Event::Actions(actions) = event {
            if self.view.button(id!(join_successor_button)).clicked(actions) {
                self.navigate_to_successor_room(cx, scope);
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
    pub fn show(&mut self, cx: &mut Cx, room_id: &OwnedRoomId, successor_room: &SuccessorRoom) {
        self.set_visible(cx, true);
        self.room_id = Some(room_id.clone());
        self.view.label(id!(replacement_reason)).set_text(
            cx,
            successor_room.reason.as_deref().unwrap_or(DEFAULT_TOMBSTONE_REASON),
        );
        let rooms_list_ref = cx.get_global::<RoomsListRef>();
        let (successor_avatar_preview, successor_room_name, is_joined) = match rooms_list_ref
            .get_room_avatar_and_name(&successor_room.room_id)
        {
            Some((avatar, name)) => (avatar, name, true),
            None => (RoomPreviewAvatar::default(), RoomDisplayName::Empty, false),
        };

        match &successor_avatar_preview {
            RoomPreviewAvatar::Text(text) => {
                self.view
                    .avatar(id!(successor_room_avatar))
                    .show_text(cx, None, None, text);
            }
            RoomPreviewAvatar::Image(image_data) => {
                let _ = self.view.avatar(id!(successor_room_avatar)).show_image(
                    cx,
                    None,
                    |cx, img_ref| utils::load_png_or_jpg(&img_ref, cx, image_data),
                );
            }
        }
        let display_name = room_name_or_id(&successor_room_name, &successor_room.room_id);
        let successor_info = Some(BasicRoomDetails {
            room_id: successor_room.room_id.clone(),
            room_name: successor_room_name.clone(),
            room_avatar: successor_avatar_preview,
        });

        self.view
            .label(id!(successor_room_name))
            .set_text(cx, &display_name);

        let join_successor_button = self.view.button(id!(join_successor_button));
        join_successor_button.reset_hover(cx);
        join_successor_button.set_text(
            cx,
            if is_joined { "Go to the replacement room" } else { "Join the replacement room" },
        );

        self.successor_info = successor_info;
    }

    /// Navigate to the successor room or show join room modal if not loaded.
    ///
    /// If the successor room is not loaded, show a join room modal. Otherwise,
    /// close the tombstone room and show the successor room in the room list.
    ///
    fn navigate_to_successor_room(&mut self, cx: &mut Cx, _scope: &mut Scope) {
        let Some(successor_room_detail) = self.successor_info.as_ref() else {
            error!("BUG: cannot navigate to replacement room: no successor room information.");
            return;
        };

        cx.action(AppStateAction::NavigateToRoom {
            room_to_close: self.room_id.clone(),
            destination_room: successor_room_detail.clone(),
        });
    }

    /// Hides the tombstone footer and clears any successor room information.
    fn hide(&mut self, cx: &mut Cx) {
        self.set_visible(cx, false);
        self.successor_info = None;
    }
}

impl TombstoneFooterRef {
    /// See [`TombstoneFooter::show()`].
    pub fn show(&self, cx: &mut Cx, room_id: &OwnedRoomId, successor_room: &SuccessorRoom) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.show(cx, room_id, successor_room);
    }

    /// See [`TombstoneFooter::hide()`].
    pub fn hide(&self, cx: &mut Cx) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.hide(cx);
    }
}
