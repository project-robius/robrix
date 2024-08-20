use makepad_widgets::*;
use matrix_sdk::ruma::OwnedRoomId;

use crate::home::room_screen::RoomScreenWidgetExt;

use super::rooms_list::RoomListAction;

live_design! {
    import makepad_widgets::base::*;
    import makepad_widgets::theme_desktop_dark::*;
    import makepad_draw::shader::std::*;
    import crate::home::room_screen::RoomScreen;

    MainContent = {{MainContent}} {
        padding: {top: 40.}
        width: Fill, height: Fill
        flow: Down
        show_bg: true
        draw_bg: {
            color: #f
        }
        align: {x: 0.5, y: 0.5}
        
        welcome = <View> {
            align: {x: 0.5, y: 0.5}
            welcome_message = <RoundedView> {
                padding: 40.
                show_bg: true,
                width: Fit, height: Fit
                draw_bg: {
                    radius: 4.0
                    color: #f2
                }

                <Label> {
                    text: "Welcome to Robrix",
                    draw_text: {
                        color: #x4,
                        text_style: {
                            font_size: 20.0
                        }
                    }
                }
            }
        }

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
    Welcome,
    Rooms(Vec<OwnedRoomId>), // TODO: decide on how to represent the list of tabs within the panel
}

impl Widget for MainContent {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.match_event(cx, event);
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        if let PanelStatus::Welcome = self.panel_status {
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
                RoomListAction::Selected { room_id, room_index: _, room_name } => {
                    log!("Room selected: {}", room_id);
                    self.panel_status = PanelStatus::Rooms(vec![room_id.clone()]);

                    // Set the title of the RoomScreen's header to the room name.
                    let displayed_room_name = room_name.unwrap_or_else(|| format!("Room ID {}", &room_id));
                    // stack_navigation.set_title(live_id!(rooms_stack_view), &displayed_room_name);
                    // Get a reference to the `RoomScreen` widget and tell it which room's data to show.
                    self.view
                        .room_screen(id!(room_screen))
                        .set_displayed_room(displayed_room_name, room_id);
                    self.redraw(cx);
                },
                _ => ()
            }
        }
    }
}
