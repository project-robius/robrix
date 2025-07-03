//! A `TombstoneScreen` widget shows information about a tombstoned room.
//!
//! This screen is displayed when a user tries to access a room that has been
//! tombstoned (shut down and replaced with a successor room), offering them
//! the option to join the successor room or stay in the current tombstoned room.

use std::ops::Deref;
use makepad_widgets::*;
use matrix_sdk::ruma::OwnedRoomId;

use crate::{
    join_leave_room_modal::{JoinLeaveModalKind, JoinLeaveRoomModalAction},
    room::{BasicRoomDetails, RoomPreviewAvatar},
    shared::{
        avatar::AvatarWidgetRefExt,
        popup_list::{enqueue_popup_notification, PopupItem}
    },
    sliding_sync::{submit_async_request, MatrixRequest},
    utils
};

use super::invite_screen::JoinRoomAction;

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::helpers::*;
    use crate::shared::styles::*;
    use crate::shared::avatar::*;
    use crate::shared::icon_button::*;

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

        // Tombstone icon and header
        tombstone_header = <View> {
            width: Fill, height: Fit
            align: {x: 0.5, y: 0}
            spacing: 15,
            flow: Down,
            margin: {bottom: 20}

            tombstone_icon = <Icon> {
                width: 48, height: 48
                draw_icon: {
                    svg_file: (ICON_TOMBSTONE)
                    fn get_color(self) -> vec4 {
                        return #999;
                    }
                }
            }

            tombstone_title = <Label> {
                width: Fill, height: Fit,
                align: {x: 0.5, y: 0},
                flow: RightWrap,
                text: "Room Tombstoned"
                draw_text: {
                    text_style: <TITLE_TEXT>{
                        font_size: 20,
                    },
                    color: #000
                    wrap: Word,
                }
            }
        }

        // Current (tombstoned) room info
        current_room_view = <View> {
            width: Fill, height: Fit
            align: {x: 0.5, y: 0}
            spacing: 10,
            flow: Down,
            margin: {bottom: 15}

            current_room_label = <Label> {
                width: Fill, height: Fit,
                align: {x: 0.5, y: 0},
                flow: RightWrap,
                text: "Current Room:"
                draw_text: {
                    text_style: <REGULAR_TEXT>{
                        font_size: 12,
                    },
                    color: #666
                    wrap: Word,
                }
            }

            current_room_avatar = <Avatar> {
                width: 32,
                height: 32,
                text_view = { text = { draw_text: {
                    text_style: <TITLE_TEXT>{ font_size: 11.0 }
                }}}
            }

            current_room_name = <Label> {
                width: Fill, height: Fit,
                align: {x: 0.5, y: 0},
                margin: {top: 5}
                flow: RightWrap,
                text: ""
                draw_text: {
                    text_style: <TITLE_TEXT>{
                        font_size: 16,
                    },
                    color: #000
                    wrap: Word,
                }
            }
        }

        tombstone_message = <Label> {
            margin: {top: 10, bottom: 20},
            width: Fill, height: Fit,
            align: {x: 0.5, y: 0},
            flow: RightWrap,
            text: "This room has been tombstoned and is no longer active.",
            draw_text: {
                text_style: <REGULAR_TEXT>{
                    font_size: 15,
                },
                color: #000
                wrap: Word
            }
        }

        // Successor room info
        successor_room_view = <View> {
            width: Fill, height: Fit
            align: {x: 0.5, y: 0}
            spacing: 10,
            flow: Down,
            margin: {bottom: 20}

            successor_room_label = <Label> {
                width: Fill, height: Fit,
                align: {x: 0.5, y: 0},
                flow: RightWrap,
                text: "Successor Room:"
                draw_text: {
                    text_style: <REGULAR_TEXT>{
                        font_size: 12,
                    },
                    color: #666
                    wrap: Word,
                }
            }

            successor_room_avatar = <Avatar> {
                width: 40,
                height: 40,
                text_view = { text = { draw_text: {
                    text_style: <TITLE_TEXT>{ font_size: 13.0 }
                }}}
            }

            successor_room_name = <Label> {
                width: Fill, height: Fit,
                align: {x: 0.5, y: 0},
                margin: {top: 5}
                flow: RightWrap,
                text: ""
                draw_text: {
                    text_style: <TITLE_TEXT>{
                        font_size: 18,
                    },
                    color: #000
                    wrap: Word,
                }
            }
        }

        buttons = <View> {
            width: Fill, height: Fit
            flow: Right,
            align: {x: 0.5, y: 0.5}
            margin: {top: 20}
            spacing: 40

            stay_button = <RobrixIconButton> {
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
                text: "Stay Here"
                draw_text:{
                    color: (COLOR_DANGER_RED),
                }
            }

            join_successor_button = <RobrixIconButton> {
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
                text: "Join Successor Room"
                draw_text:{
                    color: (COLOR_ACCEPT_GREEN),
                }
            }
        }

        completion_label = <Label> {
            width: Fill, height: Fit,
            align: {x: 0.5, y: 0},
            margin: {top: 10, bottom: 10},
            flow: RightWrap,
            draw_text: {
                color: (COLOR_ACCEPT_GREEN),
                text_style: <THEME_FONT_BOLD>{font_size: 12}
                wrap: Word,
            }
            text: ""
        }

        filler = <View> {
            width: Fill, height: 30,
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

#[derive(Clone, Debug, Default, PartialEq)]
pub enum TombstoneState {
    #[default]
    WaitingOnUserInput,
    WaitingForJoinResult,
    WaitingForJoinedRoom,
    JoinedSuccessor,
}

/// A view that shows information about a tombstoned room and its successor.
#[derive(Live, LiveHook, Widget)]
pub struct TombstoneScreen {
    #[deref] view: View,

    #[rust] tombstone_state: TombstoneState,
    #[rust] info: Option<TombstoneDetails>,
    /// Whether a JoinLeaveRoomModal dialog has been displayed
    #[rust] has_shown_confirmation: bool,
    /// The ID of the current tombstoned room
    #[rust] room_id: Option<OwnedRoomId>,
    /// The ID of the successor room
    #[rust] successor_room_id: Option<OwnedRoomId>,
    #[rust] room_name: Option<String>,
    #[rust] is_loaded: bool,
}

impl Widget for TombstoneScreen {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);

        let orig_state = self.tombstone_state.clone();

        // Handle button clicks to join successor room or stay in current room
        if let Event::Actions(actions) = event {
            let Some(info) = self.info.as_ref() else { return; };
            
            if let Some(modifiers) = self.view.button(id!(stay_button)).clicked_modifiers(actions) {
                // User chooses to stay in the tombstoned room
                self.tombstone_state = TombstoneState::WaitingOnUserInput;
                if !modifiers.shift {
                    // Just show a message that they're staying in the tombstoned room
                    self.view.label(id!(completion_label)).set_text(
                        cx,
                        "You are staying in the tombstoned room. Note that this room is no longer active.",
                    );
                }
            }
            
            if let Some(modifiers) = self.view.button(id!(join_successor_button)).clicked_modifiers(actions) {
                if let Some(successor_info) = info.successor_room_info.as_ref() {
                    self.tombstone_state = TombstoneState::WaitingForJoinResult;
                    if modifiers.shift {
                        submit_async_request(MatrixRequest::JoinRoom {
                            room_id: successor_info.room_id.clone(),
                        });
                        self.has_shown_confirmation = false;
                    } else {
                        cx.action(JoinLeaveRoomModalAction::Open(
                            JoinLeaveModalKind::JoinRoom(successor_info.clone())
                        ));
                        self.has_shown_confirmation = true;
                    }
                }
            }

            for action in actions {
                match action.downcast_ref() {
                    Some(JoinRoomAction::Joined { room_id }) if Some(room_id) == self.successor_room_id.as_ref() => {
                        self.tombstone_state = TombstoneState::JoinedSuccessor;
                        if !self.has_shown_confirmation {
                            enqueue_popup_notification(PopupItem{ 
                                message: "Successfully joined successor room.".into(), 
                                auto_dismissal_duration: None 
                            });
                        }
                        continue;
                    }
                    Some(JoinRoomAction::Failed { room_id, error }) if Some(room_id) == self.successor_room_id.as_ref() => {
                        self.tombstone_state = TombstoneState::WaitingOnUserInput;
                        if !self.has_shown_confirmation {
                            let msg = utils::stringify_join_leave_error(
                                error, 
                                info.successor_room_info.as_ref().and_then(|s| s.room_name.as_deref()), 
                                true, 
                                true
                            );
                            enqueue_popup_notification(PopupItem { message: msg, auto_dismissal_duration: None });
                        }
                        continue;
                    }
                    _ => {}
                }

                if let Some(JoinLeaveRoomModalAction::Close { successful, .. }) = action.downcast_ref() {
                    if !*successful {
                        self.tombstone_state = TombstoneState::WaitingOnUserInput;
                    }
                    continue;
                }
            }
        }

        if self.tombstone_state != orig_state {
            self.redraw(cx);
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        let Some(info) = self.info.as_ref() else {
            return self.view.draw_walk(cx, scope, walk);
        };

        // Populate current room info
        let current_room_view = self.view.view(id!(current_room_view));
        let current_room_avatar = current_room_view.avatar(id!(current_room_avatar));
        match &info.current_room_info.room_avatar {
            RoomPreviewAvatar::Text(text) => {
                current_room_avatar.show_text(cx, None, text);
            }
            RoomPreviewAvatar::Image(avatar_bytes) => {
                let _ = current_room_avatar.show_image(
                    cx,
                    None,
                    |cx, img| utils::load_png_or_jpg(&img, cx, avatar_bytes),
                );
            }
        }
        current_room_view.label(id!(current_room_name)).set_text(
            cx,
            info.current_room_info.room_name.as_deref()
                .unwrap_or_else(|| info.current_room_info.room_id.as_str()),
        );

        // Populate successor room info if available
        let successor_room_view = self.view.view(id!(successor_room_view));
        if let Some(successor_info) = info.successor_room_info.as_ref() {
            successor_room_view.set_visible(cx, true);
            let successor_room_avatar = successor_room_view.avatar(id!(successor_room_avatar));
            match &successor_info.room_avatar {
                RoomPreviewAvatar::Text(text) => {
                    successor_room_avatar.show_text(cx, None, text);
                }
                RoomPreviewAvatar::Image(avatar_bytes) => {
                    let _ = successor_room_avatar.show_image(
                        cx,
                        None,
                        |cx, img| utils::load_png_or_jpg(&img, cx, avatar_bytes),
                    );
                }
            }
            successor_room_view.label(id!(successor_room_name)).set_text(
                cx,
                successor_info.room_name.as_deref()
                    .unwrap_or_else(|| successor_info.room_id.as_str()),
            );
        } else {
            successor_room_view.set_visible(cx, false);
        }

        // Set custom tombstone message if available
        if let Some(message) = &info.tombstone_message {
            self.view.label(id!(tombstone_message)).set_text(cx, message);
        }

        // Set button states based on tombstone state
        let stay_button = self.view.button(id!(stay_button));
        let join_successor_button = self.view.button(id!(join_successor_button));
        
        match self.tombstone_state {
            TombstoneState::WaitingOnUserInput => {
                join_successor_button.set_enabled(cx, true);
                stay_button.set_enabled(cx, true);
                join_successor_button.set_enabled(cx, info.successor_room_info.is_some());
                stay_button.set_text(cx, "Stay Here");
                join_successor_button.set_text(cx, "Join Successor Room");
            }
            TombstoneState::WaitingForJoinResult => {
                stay_button.set_enabled(cx, false);
                //join_successor_button.set_enabled(cx, false);
                stay_button.set_text(cx, "Stay Here");
                join_successor_button.set_text(cx, "Joining...");
            }
            TombstoneState::WaitingForJoinedRoom => {
                stay_button.set_enabled(cx, false);
                //join_successor_button.set_enabled(cx, false);
                stay_button.set_text(cx, "Stay Here");
                join_successor_button.set_text(cx, "Joined!");
            }
            TombstoneState::JoinedSuccessor => {
                stay_button.set_visible(cx, false);
                join_successor_button.set_visible(cx, false);
                self.view.label(id!(completion_label)).set_text(
                    cx,
                    "Successfully joined successor room!",
                );
            }
        }

        self.view.draw_walk(cx, scope, walk)
    }
}

impl TombstoneScreen {
    /// Sets the tombstone details to be displayed by this screen.
    pub fn set_displayed_tombstone(
        &mut self, 
        cx: &mut Cx, 
        current_room_info: BasicRoomDetails,
        successor_room_info: Option<BasicRoomDetails>,
        tombstone_message: Option<String>,
    ) {
        self.room_id = Some(current_room_info.room_id.clone());
        self.successor_room_id = successor_room_info.as_ref().map(|s| s.room_id.clone());
        self.room_name = current_room_info.room_name.clone();
        
        self.info = Some(TombstoneDetails {
            current_room_info,
            successor_room_info,
            tombstone_message,
        });
        
        self.tombstone_state = TombstoneState::WaitingOnUserInput;
        self.has_shown_confirmation = false;
        self.is_loaded = true;
        self.redraw(cx);
    }
}

impl TombstoneScreenRef {
    /// See [`TombstoneScreen::set_displayed_tombstone()`].
    pub fn set_displayed_tombstone(
        &self, 
        cx: &mut Cx, 
        current_room_info: BasicRoomDetails,
        successor_room_info: Option<BasicRoomDetails>,
        tombstone_message: Option<String>,
    ) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_displayed_tombstone(cx, current_room_info, successor_room_info, tombstone_message);
        }
    }
}