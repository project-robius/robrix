use makepad_widgets::*;

live_design! {
    import makepad_widgets::base::*;
    import makepad_widgets::theme_desktop_dark::*;
    import makepad_draw::shader::std::*;

    import crate::shared::styles::*;
    import crate::shared::helpers::*;
    import crate::shared::adaptive_view::AdaptiveView;

    import crate::home::rooms_list::RoomsList;
    import crate::shared::cached_widget::CachedWidget;

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
        <CachedWidget> {
            rooms_list = <RoomsList> {}
        }
    }

    RoomsSideBar = <AdaptiveView> {
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

#[derive(Widget, Live, LiveHook)]
pub struct RoomsView {
    #[deref]
    view: View,
}

impl Widget for RoomsView {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
    }
    
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}
