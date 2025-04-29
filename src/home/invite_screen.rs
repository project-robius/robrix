//! An `InviteScreen` widget shows information about a room invite.
//!
//! This is similar to how a `RoomScreen` shows the full timeline of a joined room,
//! but it only shows a simple summary of a room the current user has been invited to,
//! with buttons to accept or decline the invitation.

use makepad_widgets::*;
use matrix_sdk::ruma::OwnedRoomId;

use crate::{shared::avatar::AvatarWidgetRefExt, utils};

use super::rooms_list::{InviterInfo, RoomPreviewAvatar};


live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::helpers::*;
    use crate::shared::styles::*;
    use crate::shared::avatar::*;
    use crate::shared::icon_button::*;

    pub InviteScreen = {{InviteScreen}}<ScrollXYView> {
        width: Fill,
        height: Fill,
        flow: Down,
        align: {x: 0.5, y: 0}
        padding: {left: 20, right: 20, top: 50}
        spacing: 30,

        inviter_view = <View> {
            width: Fill, height: Fit
            align: {x: 0.5, y: 0}
            spacing: 15,
            flow: Down,

            <View> {
                width: Fill, height: Fit
                align: {x: 0.5, y: 0}
                spacing: 10
                inviter_avatar = <Avatar> {
                    width: 30,
                    height: 30,
        
                    text_view = { text = { draw_text: {
                        text_style: <TITLE_TEXT>{ font_size: 10.0 }
                    }}}
                }

                inviter_name = <Label> {
                    margin: {top: 2}
                    text: ""
                    draw_text: {
                        text_style: <TITLE_TEXT>{
                            font_size: 15,
                        },
                        color: #000
                    }
                }
            }

            inviter_user_id = <Label> {
                text: ""
                draw_text: {
                    text_style: <TITLE_TEXT>{
                        font_size: 10,
                    },
                    color: #888
                }
            }
        }

        invite_message = <Label> {
            text: "",
            draw_text: {
                text_style: <REGULAR_TEXT>{
                    font_size: 15,
                },
                color: #000
                wrap: Word
            }
        }

        room_view = <View> {
            width: Fill, height: Fit
            align: {x: 0.5, y: 0}
            spacing: 10,
            flow: Right,

            room_avatar = <Avatar> {
                width: 40.,
                height: 40.,

                text_view = { text = { draw_text: {
                    text_style: <TITLE_TEXT>{ font_size: 13.0 }
                }}}
            }

            room_name = <Label> {
                margin: {top: 3.5}
                text: ""
                draw_text: {
                    text_style: <TITLE_TEXT>{
                        font_size: 18,
                    },
                    color: #000
                }
            }
        }

        buttons = <View> {
            width: Fill, height: Fit
            // We'd like to use RightWrap, but it doesn't work with x-centered alignment
            // flow: RightWrap,
            flow: Right,
            align: {x: 0.5, y: 0.5}
            margin: {top: 20}
            spacing: 40

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
                text: "Reject Invite"
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
                text: "Join Room"
                draw_text:{
                    color: (COLOR_ACCEPT_GREEN),
                }
            }
        }

        filler = <View> {
            width: Fill, height: 30,
        }
    }
}

struct InviteScreenInfo {
    pub room_id: OwnedRoomId,
    pub room_name: Option<String>,
    pub room_avatar: RoomPreviewAvatar,
    pub inviter: Option<InviterInfo>,
}

/// A view that shows information about a room that the user has been invited to.
#[derive(Live, LiveHook, Widget)]
pub struct InviteScreen {
    #[deref] view: View,

    #[rust] info: Option<InviteScreenInfo>,
}

impl Widget for InviteScreen {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);

        // TODO: handle button clicks to accept or decline the invite

    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        let Some(info) = self.info.as_ref() else {
            // If we don't have any info, just return.
            return self.view.draw_walk(cx, scope, walk);
        };
        
        // First, populate the inviter info, if we have it.
        let inviter_view = self.view.view(id!(inviter_view));
        let (is_visible, invite_text) = if let Some(inviter) = info.inviter.as_ref() {
            let inviter_avatar = inviter_view.avatar(id!(inviter_avatar));
            let mut drew_avatar = false;
            if let Some(avatar_bytes) = inviter.avatar.as_ref() {
                drew_avatar = inviter_avatar.show_image(
                    cx,
                    None, // don't make this avatar clickable.
                    |cx, img| utils::load_png_or_jpg(&img, cx, avatar_bytes),
                ).is_ok();
            }
            if !drew_avatar {
                inviter_avatar.show_text(
                    cx,
                    None, // don't make this avatar clickable.
                    inviter.display_name.as_deref().unwrap_or_else(|| inviter.user_id.as_str()),
                );
            }
            let inviter_name = inviter_view.label(id!(inviter_name));
            let inviter_user_id = inviter_view.label(id!(inviter_user_id));
            if let Some(inviter_user_name) = inviter.display_name.as_deref() {
                // If we have an inviter display name, show that *and* the user ID.
                inviter_name.set_text(cx, inviter_user_name);
                inviter_user_id.set_visible(cx, true);
                inviter_user_id.set_text(cx, inviter.user_id.as_str());
            }
            else {
                // If we only have a user ID, show it in the user_name field,
                // and hide the user ID field. 
                inviter_name.set_text(cx, inviter.user_id.as_str());
                inviter_user_id.set_visible(cx, false);
            }
            (true, "has invited you to join:")
        }
        else {
            (false, "You have been invited to join:")
        };
        inviter_view.set_visible(cx, is_visible);        
        self.view.label(id!(invite_message)).set_text(cx, invite_text);

        // Second, populate the room info, if we have it.
        let room_view = self.view.view(id!(room_view));
        let room_avatar = room_view.avatar(id!(room_avatar));
        match &info.room_avatar {
            RoomPreviewAvatar::Text(text) => {
                room_avatar.show_text(
                    cx,
                    None, // don't make this avatar clickable.
                    text,
                );
            }
            RoomPreviewAvatar::Image(avatar_bytes) => {
                let _ = room_avatar.show_image(
                    cx,
                    None, // don't make this avatar clickable.
                    |cx, img| utils::load_png_or_jpg(&img, cx, avatar_bytes),
                );
            }
        }
        room_view.label(id!(room_name)).set_text(
            cx,
            info.room_name.as_deref().unwrap_or_else(|| info.room_id.as_str()),
        );

        // The buttons don't need to be manually populated, as their content is static.

        self.view.draw_walk(cx, scope, walk)
    }
}

impl InviteScreen {
    /// Sets the ID of the invited room that will be displayed by this screen.
    pub fn set_displayed_invite(&mut self, cx: &mut Cx, room_id: OwnedRoomId) {
        if let Some(invite) = super::rooms_list::get_invited_rooms(cx)
            .borrow()
            .get(&room_id)
        {
            self.info = Some(InviteScreenInfo {
                room_id,
                room_name: invite.room_name.clone(),
                room_avatar: invite.room_avatar.clone(),
                inviter: invite.inviter_info.clone(),
            });
            self.redraw(cx);
        }
    }
}

impl InviteScreenRef {
    /// See [`InviteScreen::set_displayed_invite()`].
    pub fn set_displayed_invite(&self, cx: &mut Cx, room_id: OwnedRoomId) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_displayed_invite(cx, room_id);
        }
    }
}
