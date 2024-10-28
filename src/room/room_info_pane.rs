use makepad_widgets::*;

live_design! {
    import makepad_draw::shader::std::*;
    import makepad_widgets::base::*;
    import makepad_widgets::theme_desktop_dark::*;

    import crate::shared::helpers::*;
    import crate::shared::styles::*;
    import crate::shared::avatar::*;
    import crate::shared::icon_button::*;

    RoomInfoPane = {{RoomInfoPane}}<ScrollXYView> {
        width: Fill,
        height: Fill,
        align: {x: 0.5, y: 0},
        padding: {left: 15., right: 15., top: 15.}
        spacing: 20,
        flow: Down,
        visible: false,
        show_bg: true,
        draw_bg: {
            color: #f
        }

        room_info = <View> {
            width: Fill, height: Fit
            align: {x: 0.5, y: 0.0}
            padding: {left: 10, right: 10}
            spacing: 10
            flow: Down

            room_avatar = <Avatar> {
                width: 150,
                height: 150,
                margin: 10.0,
                text_view = { text = { draw_text: {
                    text_style: { font_size: 40.0 }
                }}}
            }

            room_name = <Label> {
                width: Fit, height: Fit
                draw_text: {
                    wrap: Word,
                    color: #000,
                    text_style: { font_size: 12 },
                }
                text: "Room Name"
            }

            room_id = <Label> {
                width: Fit, height: Fit
                draw_text: {
                    wrap: Line,
                    color: #90A4AE,
                    text_style: { font_size: 11 },
                }
                text: "Room ID"
            }

        }
    }

}


#[derive(Live, LiveHook, Widget)]
pub struct RoomInfoPane {
    #[deref] view: View,
}

impl Widget for RoomInfoPane {
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}
