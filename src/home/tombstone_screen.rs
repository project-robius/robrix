//! A `TombstoneScreen` widget shows information about a tombstoned room.
//!
//! This screen is displayed when a user tries to access a room that has been
//! tombstoned (shut down and replaced with a successor room), offering them
//! the option to join the successor room or stay in the current tombstoned room.

use makepad_widgets::*;
use matrix_sdk::ruma::OwnedRoomId;
use std::ops::Deref;

use crate::{
    app::{RoomsPanelRestoreAction, SelectedRoom},
    home::{
        invite_screen::InviteScreenWidgetExt, room_screen::RoomScreenWidgetExt, rooms_list::RoomsListRef,
    },
    join_leave_room_modal::{JoinLeaveModalKind, JoinLeaveRoomModalAction},
    room::{room_input_bar::RoomInputBarWidgetRefExt, BasicRoomDetails, RoomPreviewAvatar},
    shared::{
        avatar::AvatarWidgetExt,
        popup_list::{enqueue_popup_notification, PopupItem},
    },
    sliding_sync::avatar_from_room_name,
    utils::{self, OwnedRoomIdRon},
};

use super::{invite_screen::JoinRoomAction, rooms_list::RoomsListAction};

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::helpers::*;
    use crate::shared::styles::*;
    use crate::shared::avatar::*;
    use crate::shared::icon_button::*;
    use crate::home::room_screen::*;
    use crate::home::invite_screen::*;

    pub TombstoneScreen = {{TombstoneScreen}}<ScrollXYView> {
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
        <View> {
            width: Fill,
            height: Fill,
            room_screen = <RoomScreen> {}
        }
        bottom_space = <View> {
            width: Fill,
            height: 100,
            flow: Down,
            show_bg: true,
            draw_bg: {
                color: (COLOR_PRIMARY),
            }
            <Label> {
                width: Fill, height: Fit,
                align: {x: 0.0, y: 0},
                padding: {left: 5.0, right: 0.0}
                flow: RightWrap,
                margin: 0,
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
                    align: {x: 0.0, y: 0.5}
                    padding: 15,
                    draw_icon: {
                        svg_file: (ICON_TOMBSTONE)
                        color: (COLOR_ACCEPT_GREEN),
                    }
                    icon_walk: {width: 16, height: 16, margin: {left: -2, right: -1} }

                    draw_bg: {
                        border_color: (COLOR_ACCEPT_GREEN),
                        color: #f0fff0 // light green
                    }
                    text: "The conversation continues here."
                    draw_text:{
                        color: (COLOR_ACCEPT_GREEN),
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
                    margin: {top: 5}
                    flow: RightWrap,
                    text: ""
                    draw_text: {
                        text_style: <TITLE_TEXT>{
                            font_size: 12,
                        },
                        color: #000
                        wrap: Word,
                    }
                }
            }
        }
        restore_status_label = <Label> {
            width: Fill, height: Fit,
            align: {x: 0.5, y: 0},
            padding: {left: 5.0, right: 0.0}
            flow: RightWrap,
            margin: 0,
            draw_text: {
                color: (TYPING_NOTICE_TEXT_COLOR),
                text_style: <REGULAR_TEXT>{font_size: 11}
                wrap: Word,
            }
            text: ""
        }
    }
}

#[derive(Clone, Debug)]
pub struct TombstoneDetails {
    pub current_room_info: BasicRoomDetails,
    pub successor_room_info: Option<BasicRoomDetails>,
    pub tombstone_message: Option<String>,
}

impl Deref for TombstoneDetails {
    type Target = BasicRoomDetails;
    fn deref(&self) -> &Self::Target {
        &self.current_room_info
    }
}

/// A view that shows information about a tombstoned room and its successor.
#[derive(Live, LiveHook, Widget)]
pub struct TombstoneScreen {
    #[deref]
    view: View,
    #[rust]
    info: Option<TombstoneDetails>,
    /// Whether a JoinLeaveRoomModal dialog has been displayed
    #[rust]
    has_shown_confirmation: bool,
    /// The ID of the current tombstoned room
    #[rust]
    room_id: Option<OwnedRoomId>,
    /// The ID of the successor room
    #[rust]
    successor_room_id: Option<OwnedRoomId>,
    #[rust]
    room_name: Option<String>,
    #[rust]
    is_loaded: bool,
    #[rust]
    all_rooms_loaded: bool,
}

impl Widget for TombstoneScreen {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);

        // Let the embedded RoomScreen handle events as well
        if let Some(mut room_screen) = self.view.room_screen(id!(room_screen)).borrow_mut() {
            room_screen.handle_event(cx, event, scope);
        }
        // Currently, a Signal event is only used to tell this widget
        // to check if the room has been loaded from the homeserver yet.
        if let Event::Signal = event {
            if let (false, Some(room_id), true) = (
                self.is_loaded,
                &self.room_id,
                cx.has_global::<RoomsListRef>(),
            ) {
                let rooms_list_ref = cx.get_global::<RoomsListRef>();
                let restore_status_label = self.view.label(id!(restore_status_label));
                if !rooms_list_ref.is_room_loaded(room_id) {
                    let status_text = if rooms_list_ref.all_known_rooms_loaded() {
                        self.all_rooms_loaded = true;
                        format!(
                            "This tombstone room \"{}\" was not found in the homeserver's list of all rooms.\n\n\
                             You may close this screen.",
                            self.room_name.as_deref().unwrap_or_else(|| room_id.as_str())
                        )
                    } else {
                        String::from(
                            "[Placeholder for Loading Spinner]\n\
                         Waiting for this room to be loaded from the homeserver",
                        )
                    };
                    restore_status_label.set_text(cx, &status_text);
                    return;
                } else {
                    self.set_displayed_tombstone(cx, room_id.clone(), self.room_name.clone());
                }
            }
        }
        // Handle button clicks to join successor room or stay in current room
        if let Event::Actions(actions) = event {
            if self
                .view
                .button(id!(join_successor_button))
                .clicked(actions)
            {
                self.navigate_to_successor_room(cx, scope);
            }

            for action in actions {
                if let Some(RoomsPanelRestoreAction::Success(room_id)) = action.downcast_ref() {
                    if self.room_id.as_ref().is_some_and(|r| r == room_id) {
                        self.set_displayed_tombstone(cx, room_id.clone(), self.room_name.clone());
                        return;
                    }
                }

                // Handle modal close actions
                if let Some(JoinLeaveRoomModalAction::Close {
                    successful,
                    was_internal,
                }) = action.downcast_ref()
                {
                    if *was_internal && *successful {
                        self.navigate_to_successor_room(cx, scope);
                    } else if *was_internal && !*successful {
                        // Modal was closed after failed join or cancellation
                        self.has_shown_confirmation = false;
                    }
                    return;
                }
                match action.downcast_ref() {
                    Some(JoinRoomAction::Joined { room_id })
                        if Some(room_id) == self.successor_room_id.as_ref() =>
                    {
                        if !self.has_shown_confirmation {
                            enqueue_popup_notification(PopupItem {
                                message: "Successfully joined successor room.".into(),
                                auto_dismissal_duration: None,
                            });
                        }
                        // Redirect to the successor room
                        let Some(info) = self.info.as_ref() else {
                            return;
                        };
                        if let Some(successor_info) = info.successor_room_info.as_ref() {
                            let selected_room = SelectedRoom::JoinedRoom {
                                room_id: OwnedRoomIdRon(successor_info.room_id.clone()),
                                room_name: successor_info.room_name.clone(),
                            };
                            cx.widget_action(
                                self.widget_uid(),
                                &scope.path,
                                RoomsListAction::Close(SelectedRoom::JoinedRoom {
                                    room_id: OwnedRoomIdRon(room_id.clone()),
                                    room_name: self.room_name.clone(),
                                }),
                            );
                            cx.widget_action(
                                self.widget_uid(),
                                &scope.path,
                                RoomsListAction::Selected(selected_room),
                            );
                        }
                        continue;
                    }
                    Some(JoinRoomAction::Failed { room_id, error })
                        if Some(room_id) == self.successor_room_id.as_ref() =>
                    {
                        let Some(info) = self.info.as_ref() else {
                            return;
                        };
                        if !self.has_shown_confirmation {
                            let msg = utils::stringify_join_leave_error(
                                error,
                                info.successor_room_info
                                    .as_ref()
                                    .and_then(|s| s.room_name.as_deref()),
                                true,
                                true,
                            );
                            enqueue_popup_notification(PopupItem {
                                message: msg,
                                auto_dismissal_duration: None,
                            });
                        }
                        continue;
                    }
                    _ => {}
                }
            }
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        if !self.is_loaded {
            // only draw the loading status label if the room is not loaded yet.
            return self.view.label(id!(restore_status_label)).draw(cx, scope);
        }
        self.view.draw_walk(cx, scope, walk)
    }
}

impl TombstoneScreen {
    /// Sets the tombstone details to be displayed by this screen.
    pub fn set_displayed_tombstone(
        &mut self,
        cx: &mut Cx,
        room_id: OwnedRoomId,
        room_name: Option<String>,
    ) {
        self.room_id = Some(room_id.clone());
        self.room_name = room_name.clone();
        let mut replacement_room_id: Option<OwnedRoomId> = None;
        let mut replacement_room_name = None;
        let rooms_list_ref = cx.get_global::<RoomsListRef>();
        let Some(avatar_preview) = rooms_list_ref.get_room_avatar(&room_id) else {
            return;
        };
        if let Ok(guard) = crate::sliding_sync::ALL_JOINED_ROOMS.lock() {
            for (inner_room_id, room_info) in (*guard).iter() {
                room_info
                    .replaces_tombstoned_room
                    .clone()
                    .is_some_and(|replaces| replaces == *room_id)
                    .then(|| {
                        replacement_room_id = Some(inner_room_id.clone());
                        replacement_room_name = room_info.room_name.clone();
                    });
            }
            // Search for a replacement room ID in the TOBTOMBSTONED_ROOMS map if not found in ALL_JOINED_ROOMS.
            // This happens when the replacement room id is not one of the user's joined rooms.
            if replacement_room_id.is_none() {
                if let Ok(guard) = crate::sliding_sync::TOMBSTONED_ROOMS.lock() {
                    for (new_room_id, old_room_id) in (*guard).iter() {
                        if old_room_id == &room_id {
                            replacement_room_id = Some(new_room_id.clone());
                            break; // Stop searching once we find a replacement
                        }
                    }
                }
                if replacement_room_id.is_none() {
                    return;
                }
            }
        }
        let replacement_avatar_preview = replacement_room_id
            .as_ref()
            .map(|room_id| {
                rooms_list_ref
                    .get_room_avatar(room_id)
                    .unwrap_or_else(|| avatar_from_room_name(None))
            })
            .unwrap_or_else(|| avatar_from_room_name(None));
        // TODO: Get successor room info from the backend
        let current_room_info = crate::room::BasicRoomDetails {
            room_id: room_id.clone(),
            room_name,
            room_avatar: avatar_preview,
        };
        let mut replacement_room_replacement_room_name = None;
        let successor_room_info = replacement_room_id.as_ref().map(|successor_id| {
            if let Ok(guard) = crate::sliding_sync::ALL_JOINED_ROOMS.lock() {
                if let Some(replacement_room) = guard.get(successor_id) {
                    replacement_room_replacement_room_name = replacement_room.room_name.clone();
                }
            }
            crate::room::BasicRoomDetails {
                room_id: successor_id.clone(),
                room_name: replacement_room_name.clone(),
                room_avatar: replacement_avatar_preview.clone(),
            }
        });
        self.successor_room_id = replacement_room_id;
        self.info = Some(TombstoneDetails {
            current_room_info,
            successor_room_info,
            tombstone_message: Some("This room has been tombstoned and replaced.".to_string()),
        });
        self.has_shown_confirmation = false;
        self.is_loaded = true;
        self.view.label(id!(restore_status_label)).set_text(cx, "");
        // Set the successor room avatar
        match &replacement_avatar_preview {
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
        if let Some(replace_room_name) = replacement_room_replacement_room_name {
            self.view
                .label(id!(successor_room_name))
                .set_text(cx, &replace_room_name);
        }

        // Initialize the embedded RoomScreen to show the tombstone room's timeline
        let room_screen = self.view.room_screen(id!(room_screen));
        if let Some(room_id) = &self.room_id {
            room_screen.set_displayed_room(cx, room_id.clone(), self.room_name.clone());
            room_screen
                .room_input_bar(id!(input_bar))
                .set_visible(cx, false);
        }
        self.view
            .invite_screen(id!(invite_screen))
            .set_displayed_invite(cx, room_id.clone(), self.room_name.clone());
    }

    /// Navigate to the successor room or show a join room modal if not loaded.
    ///
    /// If the successor room is not loaded, show a join room modal. Otherwise,
    /// close the tombstone room and show the successor room in the room list.
    ///
    fn navigate_to_successor_room(&mut self, cx: &mut Cx, scope: &mut Scope) {
        let Some(info) = self.info.as_ref() else {
            return;
        };
        if let Some(successor_info) = info.successor_room_info.as_ref() {
            let Some(room_id) = self.room_id.as_ref() else {
                return;
            };
            let new_selected_room = SelectedRoom::JoinedRoom {
                room_id: OwnedRoomIdRon(successor_info.room_id.clone()),
                room_name: successor_info.room_name.clone(),
            };

            // Check if successor room is loaded, if not show join modal
            let rooms_list_ref = cx.get_global::<RoomsListRef>();
            if !rooms_list_ref.is_room_loaded(&successor_info.room_id) {
                // Show join room modal for the successor room
                cx.action(JoinLeaveRoomModalAction::Open(
                    JoinLeaveModalKind::JoinRoom(successor_info.clone()),
                ));
                self.has_shown_confirmation = true;
                return;
            }

            cx.widget_action(
                self.widget_uid(),
                &scope.path,
                RoomsListAction::Close(SelectedRoom::TombstoneRoom {
                    room_id: OwnedRoomIdRon(room_id.clone()),
                    room_name: self.room_name.clone(),
                }),
            );
            cx.widget_action(
                self.widget_uid(),
                &scope.path,
                RoomsListAction::Selected(new_selected_room),
            );
        }
    }
}

impl TombstoneScreenRef {
    /// See [`TombstoneScreen::set_displayed_tombstone()`].
    pub fn set_displayed_tombstone(
        &self,
        cx: &mut Cx,
        room_id: OwnedRoomId,
        room_name: Option<String>,
    ) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_displayed_tombstone(cx, room_id, room_name);
        }
    }
}
