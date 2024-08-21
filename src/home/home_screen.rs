use makepad_widgets::*;

live_design! {
    import makepad_widgets::base::*;
    import makepad_widgets::theme_desktop_dark::*;
    import makepad_draw::shader::std::*;

    import crate::home::rooms_list::RoomsList;
    import crate::home::main_content::MainContent;
    import crate::home::spaces_dock::SpacesDock;
    import crate::shared::styles::*;

    ICON_COLLAPSE = dep("crate://self/resources/icons/collapse.svg")

    SideBar = <View> {
        padding: {top: 40., left: 20., right: 20.}
        width: 400, height: Fill
        flow: Down, spacing: 20.
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
        <View> {
            width: Fill, height: Fit
            flow: Right, spacing: 15.
            align: {x: 0.0, y: 0.5}
            <Icon> {
                draw_icon: {
                    svg_file: (ICON_COLLAPSE),
                    fn get_color(self) -> vec4 {
                        return #666;
                    }
                }
                icon_walk: {width: 12, height: Fit}
            }

            label = <Label> {
                text: "Rooms",
                draw_text: {
                    color: #x0,
                    text_style: {
                        font_size: 13.0
                    }
                }
            }
        }
        <RoomsList> {}
    }

    HomeScreen = <View> {
        flow: Right
        <SpacesDock> {}
        <SideBar> {}
        <MainContent> {}
    }
}
