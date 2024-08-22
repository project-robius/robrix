use makepad_widgets::*;

use crate::home::room_screen::RoomScreenWidgetExt;

use super::rooms_list::RoomListAction;

live_design! {
    import makepad_widgets::base::*;
    import makepad_widgets::theme_desktop_dark::*;
    import makepad_draw::shader::std::*;

    import crate::shared::styles::*;
    import crate::home::room_screen::RoomScreen;
    import crate::home::welcome_screen::WelcomeScreen;
    import crate::shared::search_bar::SearchBar;

    MainContent = {{MainContent}} {
        width: Fill, height: Fill
        flow: Down,
        show_bg: true
        draw_bg: {
            color: (COLOR_PRIMARY_DARKER)
        }
        align: {x: 0.5, y: 0.5}

        <SearchBar> {}

        welcome = <WelcomeScreen> {}
        rooms = <View> {
            align: {x: 0.5, y: 0.5}
            width: Fill, height: Fill
            room_screen = <RoomScreen> {}
        }
    }
}

#[derive(Live, LiveHook, Widget)]
pub struct MainContent {
    #[deref]
    view: View,

    #[rust]
    panel_status: PanelStatus,
}

#[derive(Default)]
enum PanelStatus {
    #[default]
    DisplayingWelcome,
    DisplayingRooms,
}

impl Widget for MainContent {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.match_event(cx, event);
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        if let PanelStatus::DisplayingWelcome = self.panel_status {
            self.view.view(id!(welcome)).set_visible(true);
            self.view.view(id!(rooms)).set_visible(false);
            return self.view.draw_walk(cx, scope, walk);
        }
        self.view.view(id!(welcome)).set_visible(false);
        self.view.view(id!(rooms)).set_visible(true);

        self.view.draw_walk(cx, scope, walk)
    }
}

impl MatchEvent for MainContent {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        for action in actions.iter() {
            match action.as_widget_action().cast() {
                // Whenever a room is selected, we hide the welcome message and switch the to the rooms screen.
                RoomListAction::Selected {
                    room_id,
                    room_index: _,
                    room_name,
                } => {
                    self.panel_status = PanelStatus::DisplayingRooms;

                    let displayed_room_name =
                        room_name.unwrap_or_else(|| format!("Room ID {}", &room_id));
                    // Get a reference to the `RoomScreen` widget and tell it which room's data to show.
                    self.view
                        .room_screen(id!(room_screen))
                        .set_displayed_room(displayed_room_name, room_id);
                    self.redraw(cx);
                }
                _ => (),
            }
        }
    }
}
