use makepad_widgets::*;

use crate::{
    app::{AppState, AppStateAction, SelectedRoom}, home::{room_screen::RoomScreenWidgetExt, rooms_list::RoomsListAction, space_lobby::SpaceLobbyScreenWidgetExt}
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
    use crate::home::space_lobby::SpaceLobbyScreen;

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
        space_lobby_view = <View> {
            align: {x: 0.5, y: 0.5}
            width: Fill, height: Fill
            space_lobby_screen = <SpaceLobbyScreen> {}
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

        if let Event::Actions(actions) = event {
            for action in actions {
                match action.as_widget_action().cast() {
                    // This is currently handled in the top-level App.
                    RoomsListAction::Selected(_selected_room) => {}
                    // Because the MainMobileUI is drawn based on the AppState only,
                    // all we need to do is update the AppState here.
                    RoomsListAction::InviteAccepted { room_name_id: room_name } => {
                        cx.action(AppStateAction::UpgradedInviteToJoinedRoom(room_name.room_id().clone()));
                    }
                    RoomsListAction::None => {}
                }
            }
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        let app_state = scope.data.get::<AppState>().unwrap();
        let show_welcome: bool;
        let show_room: bool;
        let show_invite: bool;
        let show_space_lobby: bool;

        match app_state.selected_room.as_ref() {
            Some(SelectedRoom::JoinedRoom { room_name_id }) => {
                show_welcome = false;
                show_room = true;
                show_invite = false;
                show_space_lobby = false;
                // Get a reference to the `RoomScreen` widget and tell it which room's data to show.
                self.view
                    .room_screen(ids!(room_screen))
                    .set_displayed_room(cx, room_name_id);
            }
            Some(SelectedRoom::InvitedRoom { room_name_id }) => {
                show_welcome = false;
                show_room = false;
                show_invite = true;
                show_space_lobby = false;
                self.view
                    .invite_screen(ids!(invite_screen))
                    .set_displayed_invite(cx, room_name_id);
            }
            Some(SelectedRoom::Space { space_name_id }) => {
                show_welcome = false;
                show_room = false;
                show_invite = false;
                show_space_lobby = true;
                self.view
                    .space_lobby_screen(ids!(space_lobby_screen))
                    .set_displayed_space(cx, space_name_id);
            }
            None => {
                show_welcome = true;
                show_room = false;
                show_invite = false;
                show_space_lobby = false;
            }
        }

        self.view.view(ids!(welcome)).set_visible(cx, show_welcome);
        self.view.view(ids!(room_view)).set_visible(cx, show_room);
        self.view.view(ids!(invite_view)).set_visible(cx, show_invite);
        self.view.view(ids!(space_lobby_view)).set_visible(cx, show_space_lobby);
        self.view.draw_walk(cx, scope, walk)
    }
}
