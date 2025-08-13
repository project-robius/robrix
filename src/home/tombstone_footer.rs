//! A `TombstoneFooter` widget shows information about a tombstoned room.
//!
//! This screen is displayed when a user tries to access a room that has been
//! tombstoned (shut down and replaced with a successor room), offering them
//! the option to join the successor room or stay in the current tombstoned room.

use makepad_widgets::*;
use matrix_sdk::ruma::OwnedRoomId;

use crate::{
    app::{AppStateAction, SelectedRoom},
    home::rooms_list::RoomsListRef,
    join_leave_room_modal::{JoinLeaveModalKind, JoinLeaveRoomModalAction},
    room::{BasicRoomDetails, RoomPreviewAvatar},
    shared::avatar::AvatarWidgetExt,
    utils::{self, avatar_from_room_name, OwnedRoomIdRon},
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
                    cursor: Default,
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
pub struct TombstoneDetail {
    /// The ID of the tombstoned room.
    pub tombstoned_room_id: OwnedRoomId,
    /// The ID of the successor room.
    pub successor_room_id: Option<OwnedRoomId>,
    /// The name of the successor room.
    pub successor_room_name: Option<String>,
    /// The reason why the room was tombstoned.
    pub replacement_reason: String,
}

/// A view that shows information about a tombstoned room and its successor.
#[derive(Live, LiveHook, Widget)]
pub struct TombstoneFooter {
    #[deref]
    view: View,
    #[live(false)]
    visible: bool,
    /// The details of the successor room.
    #[rust]
    successor_info: Option<BasicRoomDetails>,
    /// The ID of the current tombstoned room.
    #[rust]
    room_id: Option<OwnedRoomId>,
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
    pub fn show(&mut self, cx: &mut Cx, tombstone_detail: TombstoneDetail) {
        self.view
            .label_set(ids![replacement_reason, successor_room_name])
            .set_text(cx, "");
        self.visible = true;
        self.room_id = Some(tombstone_detail.tombstoned_room_id.clone());
        self.view
            .label(id!(replacement_reason))
            .set_text(cx, &tombstone_detail.replacement_reason);
        
        let successor_avatar_preview = tombstone_detail.successor_room_id
            .as_ref()
            .map(|room_id| {
                let rooms_list_ref = cx.get_global::<RoomsListRef>();
                rooms_list_ref
                    .get_joined_room_info(room_id)
                    .map(|room_info| room_info.avatar)
                    .unwrap_or_else(|| avatar_from_room_name(None))
            })
            .unwrap_or_else(|| avatar_from_room_name(None));

        // Set the successor room avatar
        match &successor_avatar_preview {
            RoomPreviewAvatar::Text(text) => {
                self.view
                    .avatar(id!(successor_room_avatar))
                    .show_text(cx, None, None, text);
            }
            RoomPreviewAvatar::Image(ref image_data) => {
                let _ = self.view.avatar(id!(successor_room_avatar)).show_image(
                    cx,
                    None,
                    |cx, img_ref| utils::load_png_or_jpg(&img_ref, cx, image_data),
                );
            }
        }
        let successor_info = if let Some(successor_room_id) = tombstone_detail.successor_room_id.as_ref() {
            Some(BasicRoomDetails {
                room_id: successor_room_id.clone(),
                room_name: tombstone_detail.successor_room_name.clone(),
                room_avatar: successor_avatar_preview,
            })
        } else {
            None
        };

        if let Some(successor_room_name) = successor_info.as_ref().and_then(|f| f.room_name.as_ref()) {
            self.view
                .label(id!(successor_room_name))
                .set_text(cx, successor_room_name);
        }
        self.successor_info = successor_info;
    }

    /// Navigate to the successor room or show join room modal if not loaded.
    ///
    /// If the successor room is not loaded, show a join room modal. Otherwise,
    /// close the tombstone room and show the successor room in the room list.
    ///
    fn navigate_to_successor_room(&mut self, cx: &mut Cx, scope: &mut Scope) {
        let Some(successor_room_detail) = self.successor_info.as_ref() else {
            log!("Cannot navigate to successor room: no successor room information available");
            return;
        };

        let Some(room_id) = self.room_id.as_ref() else {
            error!("Cannot navigate to successor room: current room ID is not set");
            return;
        };
        // Check if successor room is loaded, if not show join modal
        let rooms_list_ref = cx.get_global::<RoomsListRef>();
        if !rooms_list_ref.is_room_loaded(&successor_room_detail.room_id) {
            log!(
                "Successor room {} not loaded, showing join modal",
                successor_room_detail.room_id
            );
            // Show join room modal for the successor room
            cx.action(JoinLeaveRoomModalAction::Open(
                JoinLeaveModalKind::JoinRoom(successor_room_detail.clone()),
            ));
            return;
        }

        let new_selected_room = SelectedRoom::JoinedRoom {
            room_id: OwnedRoomIdRon(successor_room_detail.room_id.clone()),
            room_name: successor_room_detail.room_name.clone(),
        };

        log!(
            "Navigating from tombstoned room {} to successor room {}",
            room_id,
            successor_room_detail.room_id
        );

        cx.widget_action(
            self.widget_uid(),
            &scope.path,
            AppStateAction::RoomFocusLost(room_id.clone())
        );
        // RoomsListAction is used instead of RoomPreviewAction::Clicked because RoomsListAction::Selected being called much later. 
        cx.widget_action(
            self.widget_uid(),
            &scope.path,
            RoomsListAction::Selected(new_selected_room),
        );
    }

    /// Hides the tombstone footer, making it invisible and clearing any successor room information.
    fn hide(&mut self, cx: &mut Cx) {
        self.visible = false;
        self.successor_info = None;
        self.view
            .label_set(ids![replacement_reason, successor_room_name])
            .set_text(cx, "");
    }
}

impl TombstoneFooterRef {
    /// See [`TombstoneFooter::show()`].
    pub fn show(&mut self, cx: &mut Cx, tombstone_detail: TombstoneDetail) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.show(cx, tombstone_detail);
        }
    }
    /// See [`TombstoneFooter::hide()`].
    pub fn hide(&mut self, cx: &mut Cx) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.hide(cx);
        }
    }
}
