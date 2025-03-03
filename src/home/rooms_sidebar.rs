use makepad_widgets::*;

use crate::shared::search_bar::SearchBarAction;

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::styles::*;
    use crate::shared::helpers::*;
    use crate::shared::search_bar::SearchBar;

    use crate::home::rooms_list::RoomsList;

    RoomsView = {{RoomsView}} {
        show_bg: true,
        draw_bg: {
            instance bg_color: (COLOR_PRIMARY)
            instance border_color: #f2f2f2
            instance border_width: 0.003

            // Draws a right-side border
            fn pixel(self) -> vec4 {
                if self.pos.x > 1.0 - self.border_width {
                    return self.border_color;
                } else {
                    return self.bg_color;
                }
            }
        }
        <Label> {
            text: "Rooms"
            draw_text: {
                color: #x0
                text_style: <TITLE_TEXT>{}
            }
        }
        search_bar = <SearchBar> {
            input = {
                empty_message: "Search rooms..."
            }
        }
        <CachedWidget> {
            rooms_list = <RoomsList> {}
        }
    }

    pub RoomsSideBar = <AdaptiveView> {
        Desktop = <RoomsView> {
            padding: {top: 20., left: 10., right: 10.}
            flow: Down, spacing: 10
            width: Fill, height: Fill
        },
        Mobile = <RoomsView> {
            padding: {top: 17., left: 17., right: 17.}
            flow: Down, spacing: 7
            width: Fill, height: Fill
        }        
    }
}

#[derive(Clone, Debug, DefaultNone)]
pub enum RoomsViewAction {
    /// Search for rooms
    Search(String),
    None,
}

#[derive(Widget, Live, LiveHook)]
pub struct RoomsView {
    #[deref]
    view: View,
}

impl Widget for RoomsView {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        self.widget_match_event(cx, event, scope);
    }
    
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl WidgetMatchEvent for RoomsView {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions, scope: &mut Scope) {
        let widget_uid = self.widget_uid();
        for action in actions {
            match action.as_widget_action().cast() {
                SearchBarAction::Search(keywords) => {
                    cx.widget_action(widget_uid, &scope.path, RoomsViewAction::Search(keywords.clone()));
                }
                SearchBarAction::ResetSearch => {
                    cx.widget_action(widget_uid, &scope.path, RoomsViewAction::Search("".to_string()));
                }
                _ => {}
            }
        }
    }
}