//! A `TombstoneFooter` widget shows information about a tombstoned room.
//!
//! This screen is displayed when a user tries to access a room that has been
//! tombstoned (shut down and replaced with a successor room), offering them
//! the option to join the successor room or stay in the current tombstoned room.


use makepad_widgets::*;
use matrix_sdk::ruma::OwnedRoomId;

use crate::{
    app::SelectedRoom,
    home::
        rooms_list::RoomsListRef
    ,
    join_leave_room_modal::{JoinLeaveModalKind, JoinLeaveRoomModalAction},
    room::{BasicRoomDetails, RoomPreviewAvatar},
    shared::
        avatar::AvatarWidgetExt
    ,
    sliding_sync::avatar_from_room_name,
    utils::{self, OwnedRoomIdRon},
};

use super::rooms_list::RoomsListAction;

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::helpers::*;
    use crate::shared::styles::*;
    use crate::shared::avatar::*;
    use crate::shared::icon_button::*;
    use crate::home::invite_screen::*;

    pub TombstoneFooter = {{TombstoneFooter}}{
        visible: false,
        width: Fill,
        height: Fit,
        flow: Overlay
        <View> {
            height: Fit
            flow: Down,
            replacement_reason = <Label> {
                width: Fill, height: Fit,
                padding: 5
                flow: RightWrap,
                draw_text: {
                    color: (TYPING_NOTICE_TEXT_COLOR),
                    text_style: <REGULAR_TEXT>{font_size: 11}
                    wrap: Word,
                }
                text: "This room has been replaced and is no longer active."
            }
            <View> {
                width: Fill, height: Fit,
                align: {y: 0.5}
                join_successor_button = <RobrixIconButton> {
                    align: {y: 0.5}
                    padding: 15,
                    draw_icon: {
                        svg_file: (ICON_TOMBSTONE)
                        color: (COLOR_FG_ACCEPT_GREEN),
                    }
                    icon_walk: {width: 16, height: 16, margin: {left: -2, right: -1} }

                    draw_bg: {
                        border_color: (COLOR_FG_ACCEPT_GREEN),
                        color: #f0fff0 // light green
                    }
                    text: "The conversation continues here."
                    draw_text:{
                        color: (COLOR_FG_ACCEPT_GREEN),
                    }
                }
                successor_room_avatar = <Avatar> {
                    width: 25,
                    height: 25,
                    text_view = { text = { draw_text: {
                        text_style: <TITLE_TEXT>{ font_size: 13.0 }
                    }}}
                }
                successor_room_name = <Label> {
                    width: Fill, height: Fit,
                    flow: RightWrap,
                    draw_text: {
                        text_style: <TITLE_TEXT>{
                            font_size: 12,
                        },
                        color: (COLOR_TEXT)
                        wrap: Word,
                    }
                }
            }
        }
    }
}

/// The information about a tombstoned room.
#[derive(Clone, Debug)]
pub struct TimelineTombstone {
    pub current_room_id: OwnedRoomId,
    pub successor_room_id: Option<OwnedRoomId>,
    /// The reason why the room was tombstoned
    pub successor_reason: String,
}

/// A view that shows information about a tombstoned room and its successor.
#[derive(Live, LiveHook, Widget)]
pub struct TombstoneFooter {
    #[deref]
    view: View,
    #[live(false)] visible: bool,
    /// The details of the successor room
    #[rust]
    successor_info: Option<BasicRoomDetails>,
    /// The ID of the current tombstoned room
    #[rust]
    room_id: Option<OwnedRoomId>,
    #[rust]
    room_name: Option<String>,
}

impl Widget for TombstoneFooter {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if !self.visible {
            return;
        }
        if let Event::Actions(actions) = event {
            if self
                .view
                .button(id!(join_successor_button))
                .clicked(actions)
            {
                self.navigate_to_successor_room(cx, scope);
            }
        }
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        if !self.visible {
            return DrawStep::done();
        }
        self.view.draw_walk(cx, scope, walk)
    }
}

impl TombstoneFooter {
    /// Sets the tombstone details to be displayed by this screen.
    pub fn show(
        &mut self,
        cx: &mut Cx,
        room_id: OwnedRoomId,
    ) {
        self.room_id = Some(room_id.clone());
        let mut successor_id: Option<OwnedRoomId> = None;
        let mut successor_reason = None;
        let mut successor_room_name = None;
        // This is required as tombstoned room may not have successor room. Hence not added to TOMBSTONED_ROOMS.
        if let Ok(all_joined_rooms_guard) = crate::sliding_sync::ALL_JOINED_ROOMS.lock() {
            for (inner_room_id, room_info) in (*all_joined_rooms_guard).iter() {
                if let Some((_, reason)) = room_info
                    .replaces_tombstoned_room
                    .as_ref()
                    .filter(|(replaces, _)| replaces == &room_id)
                {
                    successor_reason = reason.clone();
                    successor_id = Some(inner_room_id.clone());
                    successor_room_name = room_info.room_name.clone();
                    break;
                }
            }
        }
        if successor_id.is_none() {
            if let Ok(tombstoned_rooms_guard) = crate::sliding_sync::TOMBSTONED_ROOMS.lock() {
                for (new_room_id, (old_room_id, reason)) in (*tombstoned_rooms_guard).iter() {
                    if old_room_id == &room_id {
                        successor_id = Some(new_room_id.clone());
                        successor_reason = reason.clone();
                        break; // Stop searching once we find a replacement
                    }
                }
            }
        }

        let successor_avatar_preview = successor_id
            .as_ref()
            .map(|room_id| {
                let rooms_list_ref = cx.get_global::<RoomsListRef>();
                rooms_list_ref
                    .get_joined_room_info(room_id)
                    .map(|room_info| room_info.avatar)
                    .unwrap_or_else(|| avatar_from_room_name(None))
            })
            .unwrap_or_else(|| avatar_from_room_name(None));
         self.view.label(id!(restore_status_label)).set_text(cx, "");
        // Set the successor room avatar
        match &successor_avatar_preview {
            RoomPreviewAvatar::Text(text) => {
                self.view
                    .avatar(id!(successor_room_avatar))
                    .show_text(cx, None, None, text);
            }
            RoomPreviewAvatar::Image(ref image_data) => {
                self.view
                    .avatar(id!(successor_room_avatar))
                    .show_image(cx, None, |cx, img_ref| {
                        utils::load_png_or_jpg(&img_ref, cx, image_data)
                    })
                    .ok();
            }
        }
        if let Some(successor_reason) = &successor_reason {
            self.view
                .label(id!(replacement_reason))
                .set_text(cx, successor_reason);
        }
        if let Some(successor_room_name) = &successor_room_name {
            self.view
                .label(id!(successor_room_name))
                .set_text(cx, successor_room_name);
        }
        let successor_room_info = successor_id.as_ref().map(|successor_id| {
            crate::room::BasicRoomDetails {
                room_id: successor_id.clone(),
                room_name: successor_room_name,
                room_avatar: successor_avatar_preview,
            }
        });
        self.successor_info = successor_room_info;

    }

    /// Navigate to the successor room or show a join room modal if not loaded.
    ///
    /// If the successor room is not loaded, show a join room modal. Otherwise,
    /// close the tombstone room and show the successor room in the room list.
    ///
    fn navigate_to_successor_room(&mut self, cx: &mut Cx, scope: &mut Scope) {
        let Some(success_room_detail) = self.successor_info.as_ref() else {
            return;
        };
        
        let Some(room_id) = self.room_id.as_ref() else {
            return;
        };
        // Check if successor room is loaded, if not show join modal
        let rooms_list_ref = cx.get_global::<RoomsListRef>();
        if !rooms_list_ref.is_room_loaded(&success_room_detail.room_id) {
            // Show join room modal for the successor room
            cx.action(JoinLeaveRoomModalAction::Open(
                JoinLeaveModalKind::JoinRoom(success_room_detail.clone()),
            ));
            return;
        }
        
        let new_selected_room = SelectedRoom::JoinedRoom {
            room_id: OwnedRoomIdRon(success_room_detail.room_id.clone()),
            room_name: success_room_detail.room_name.clone(),
        };
        
        // Close the current tombstoned room and navigate to the successor room
        cx.widget_action(
            self.widget_uid(),
            &scope.path,
            RoomsListAction::Close(SelectedRoom::JoinedRoom {
                room_id: OwnedRoomIdRon(room_id.clone()),
                room_name: self.room_name.clone(),
            }),
        );
        // BUG: This opens the correct tab, but it does not select the room preview in the room list.
        cx.widget_action(
            self.widget_uid(),
            &scope.path,
            RoomsListAction::Selected(new_selected_room),
        );
    }
    /// Returns `true` if the room with the given `room_id` is tombstoned (shut down and replaced with a successor room).
    /// Returns `false` if the room is not tombstoned.
    pub fn is_tombstoned(&self, cx: &mut Cx, room_id: &OwnedRoomId) -> bool {
        let rooms_list_ref = cx.get_global::<RoomsListRef>();
        let Some(is_tombstoned) = rooms_list_ref.get_joined_room_info(room_id).map(|room_info| room_info.is_tombstoned) else {
            return false;
        };
        is_tombstoned
    }
}

impl TombstoneFooterRef {
    /// See [`TombstoneFooter::show()`].
    pub fn show(
        &mut self,
        cx: &mut Cx,
        room_id: OwnedRoomId,
    ) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.visible = true;
            inner.show(cx, room_id);
        }
    }
    /// See [`TombstoneFooter::hide()`].
    pub fn hide(&mut self, _cx: &mut Cx) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.visible = false;
        }
    }
    /// See [`TombstoneFooter::is_tombstoned()`].
    pub fn is_tombstoned(&self, cx: &mut Cx, room_id: &OwnedRoomId) -> bool {
        if let Some(inner) = self.borrow() {
            return inner.is_tombstoned(cx, room_id)
        }
        false
    } 
}
