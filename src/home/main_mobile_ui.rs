use makepad_widgets::*;

use crate::{
    app::AppState, home::room_screen::RoomScreenWidgetExt
};

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::styles::*;
    use crate::home::room_screen::RoomScreen;
    use crate::home::welcome_screen::WelcomeScreen;

    pub MainMobileUI = {{MainMobileUI}} {
        width: Fill, height: Fill
        flow: Down,
        show_bg: true
        draw_bg: {
            color: (COLOR_PRIMARY_DARKER)
        }
        align: {x: 0.0, y: 0.5}


        welcome = <WelcomeScreen> {}
        rooms = <View> {
            align: {x: 0.5, y: 0.5}
            width: Fill, height: Fill
            room_screen = <RoomScreen> {}
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

        if let Some(room) = app_state.rooms_panel.selected_room.as_ref() {
            let displayed_room_name = room.room_name.clone().unwrap_or_else(|| format!("Room ID {}", &room.room_id));
            
            // Get a reference to the `RoomScreen` widget and tell it which room's data to show.
            self.view
                .room_screen(id!(room_screen))
                .set_displayed_room(cx, room.room_id.clone(), displayed_room_name);

            self.view.view(id!(welcome)).set_visible(cx, false);
            self.view.view(id!(rooms)).set_visible(cx, true);
        } else {
            self.view.view(id!(welcome)).set_visible(cx, true);
            self.view.view(id!(rooms)).set_visible(cx, false);
            return self.view.draw_walk(cx, scope, walk);
        }

        self.view.draw_walk(cx, scope, walk)
    }
}
