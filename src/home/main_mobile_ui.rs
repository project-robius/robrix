use makepad_widgets::*;

use crate::{
    app::{AppState, SelectedRoom}, home::room_screen::RoomScreenWidgetExt
};

use super::invite_screen::InviteScreenWidgetExt;

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::styles::*;
    use crate::home::welcome_screen::WelcomeScreen;
    use crate::home::room_screen::RoomScreen;
    use crate::home::invite_screen::InviteScreen;

    pub MainMobileUI = {{MainMobileUI}} {
        width: Fill, height: Fill
        flow: Down,
        show_bg: true
        draw_bg: {
            color: (COLOR_PRIMARY_DARKER)
        }
        align: {x: 0.0, y: 0.5}

        welcome = <WelcomeScreen> {}
        // TODO: see if we can remove these wrappers
        room_view = <View> {
            align: {x: 0.5, y: 0.5}
            width: Fill, height: Fill
            room_screen = <RoomScreen> {}
        }
        invite_view = <View> {
            align: {x: 0.5, y: 0.5}
            width: Fill, height: Fill
            invite_screen = <InviteScreen> {}
        }
    }
}

#[derive(Live, LiveHook, Widget)]
pub struct MainMobileUI {
    #[deref]
    view: View,
}

impl Widget for MainMobileUI {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        let app_state = scope.data.get::<AppState>().unwrap();
        let show_welcome: bool;
        let show_room: bool;
        let show_invite: bool;

        match app_state.rooms_panel.selected_room.as_ref() {
            Some(SelectedRoom::JoinedRoom { room_id, room_name }) => {
                show_welcome = false;
                show_room = true;
                show_invite = false;
                // Get a reference to the `RoomScreen` widget and tell it which room's data to show.
                self.view
                    .room_screen(id!(room_screen))
                    .set_displayed_room(cx, room_id.clone(), room_name.clone());
            }
            Some(SelectedRoom::InvitedRoom { room_id, room_name: _ }) => {
                show_welcome = false;
                show_room = false;
                show_invite = true;
                self.view
                    .invite_screen(id!(invite_screen))
                    .set_displayed_invite(cx, room_id.clone());
            }
            None => {
                show_welcome = true;
                show_room = false;
                show_invite = false;
            }
        }

        self.view.view(id!(welcome)).set_visible(cx, show_welcome);
        self.view.view(id!(room_view)).set_visible(cx, show_room);
        self.view.view(id!(invite_view)).set_visible(cx, show_invite);
        self.view.draw_walk(cx, scope, walk)
    }
}
