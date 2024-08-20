use makepad_widgets::*;

live_design! {
    import makepad_widgets::base::*;
    import makepad_widgets::theme_desktop_dark::*;
    import makepad_draw::shader::std::*;

    import crate::home::rooms_list::RoomsList;
    import crate::home::main_content::MainContent;

    ICON_COLLAPSE = dep("crate://self/resources/icons/collapse.svg")

    Spaces = <View> {c
        padding: {top: 40.}
        width: 60.
        flow: Down
        show_bg: true
        draw_bg: {
            color: #E8
        }
        filler_y = <View> {
            height: Fill,
            width: Fill,
        }
        profile = <View> {
            width: Fill, height: 70.
            align: { x: 0.5, y: 0.5 }

            text_view = <View> {
                width: 40., height: 40.,
                align: { x: 0.5, y: 0.5 }
                show_bg: true,

                draw_bg: {
                    instance background_color: #x7,
                    fn pixel(self) -> vec4 {
                        let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                        let c = self.rect_size * 0.5;
                        sdf.circle(c.x, c.x, c.x)
                        sdf.fill_keep(self.background_color);
                        return sdf.result
                    }
                }
                
                text = <Label> {
                    width: Fit, height: Fit,
                    padding: { top: 1.0 } // for better vertical alignment
                    draw_text: {
                        text_style: { font_size: 13. }
                        color: #f2,
                    }
                    text: "U"
                }
            }
        }
    }

    SideBar = <RoundedView> {
        padding: {top: 40., left: 20., right: 20.}
        width: 400, height: Fill
        flow: Down, spacing: 20.
        show_bg: true,
        draw_bg: {
            radius: 2.0
            border_color: #x0
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
        show_bg: true,
        draw_bg: {
            color: #EEEEEE,
        }
        <Spaces> {}
        <SideBar> {}
        <MainContent> {}
    }
}
