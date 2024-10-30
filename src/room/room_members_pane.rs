use makepad_widgets::*;
use matrix_sdk::{room::RoomMember, ruma::OwnedRoomId};

use crate::sliding_sync::{submit_async_request, MatrixRequest};

live_design! {
    import makepad_draw::shader::std::*;
    import makepad_widgets::base::*;
    import makepad_widgets::theme_desktop_dark::*;

    import crate::shared::helpers::*;
    import crate::shared::styles::*;
    import crate::shared::avatar::*;
    import crate::shared::icon_button::*;

    ICON_INVITE_PEOPLE = dep("crate://self/resources/icon_invite_people.svg");

    RoomMember = <View> {
        height: 48, width: Fill,
        show_bg: true,
        flow: Right,
        align: {y: 0.5},
        padding: {left: 5, right: 5},
        // Avatar
        member_avatar = <Avatar> {
            width: 40,
            height: 40,
            text_view = { text = { draw_text: {
                text_style: { font_size: 10.0 }
            }}}
        }

        member_name = <Label> {
            margin: {left: 5.0},
            text: "Name"
            draw_text: {
                color: #0
            }
        }

        <Filler> {}
        // Power levels

        member_room_power_level = <Label> {
            text: "Admin",
            draw_text: {
                color: #0
            }
        }
    }

    RoomMembersPane = {{RoomMembersPane}} {
        width: Fill,
        height: Fill,
        align: {x: 0.5, y: 0},
        padding: {left: 10., right: 10.}
        spacing: 10,
        flow: Down,
        show_bg: true,
        draw_bg: {
            color: #f
        }

        room_members_list = <ScrollXYView> {
            width: Fill,height: Fill,
            flow: Down,
            spacing: 1,
            <RoomMember> {}
        }

        invite_button = <RobrixIconButton> {
            width: Fill, height: Fit
            align: { y: 1 }
            margin: { bottom: 10 }
            draw_icon: {
                svg_file: (ICON_INVITE_PEOPLE)
                color: #f
            }
            draw_bg: {
                border_color: (COLOR_DANGER_RED)
                color: #0F88FE
            }
            draw_text: {
                color: #f
            }
            text: "Invite to this room",
        }

    }

}

#[derive(Clone, Debug)]
pub struct RoomMembersPaneInfo {
    pub room_id: OwnedRoomId,
    pub room_members: Vec<RoomMember>
}

#[derive(Live, LiveHook, Widget)]
pub struct RoomMembersPane {
    #[deref] view: View,
}

impl Widget for RoomMembersPane {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl RoomMembersPane {
    pub fn set_room_members_info(&mut self, _cx: &mut Cx, room_members_info: RoomMembersPaneInfo) {
        // TDDO: get room members from cache

        // Get the room members from server
        submit_async_request(MatrixRequest::FetchRoomMembers {
            room_id: room_members_info.room_id.clone()
        });
    }
}

impl RoomMembersPaneRef {
    pub fn set_room_members_info(self, _cx: &mut Cx, room_members_info: RoomMembersPaneInfo) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.set_room_members_info(_cx, room_members_info);
    }
}
