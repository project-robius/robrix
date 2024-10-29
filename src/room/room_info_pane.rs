use makepad_widgets::*;

live_design! {
    import makepad_draw::shader::std::*;
    import makepad_widgets::base::*;
    import makepad_widgets::theme_desktop_dark::*;

    import crate::shared::helpers::*;
    import crate::shared::styles::*;
    import crate::shared::avatar::*;
    import crate::shared::icon_button::*;

    ICON_INVITE_PEOPLE = dep("crate://self/resources/icon_invite_people.svg");
    ICON_COPY        = dep("crate://self/resources/icons/copy.svg")
    ICON_LEAVE_ROOM = dep("crate://self/resources/icon_leave_room.svg");


    RoomAdditionalInfoGridItem = <RoundedView> {
        width: Fit, height: Fit,
        flow: Down,
        show_bg: true,
        draw_bg: {
            color: #D0D5DD
        }
        padding: { top: 5, bottom: 5, left: 10, right: 10 },
        align: {x: 0.5, y: 0},

        title = <Label> {
            text: "Title",
            draw_text: {
                wrap: Word,
                color: #fff,
                text_style: { font_size: 8 },
            }
        }

        value = <Label> {
            text: "Value",
            margin: { top: 2 },
            draw_text: {
                wrap: Word,
                color: #fff,
                text_style: { font_size: 12 },
            }
        }
    }


    RoomAdditionalInfoGrid = <View> {
        width: Fill, height: Fit
        flow: Right,
        spacing: 20,
        align: {x: 0.5, y: 0},

        room_type = <RoomAdditionalInfoGridItem> {
            title = {
                text: "Room Type"
            }
            value = {
                text: "Public"
            }
        }

        members_count = <RoomAdditionalInfoGridItem> {
            title = {
                text: "Members"
            }
            value = {
                text: "5"
            }
        }
    }


    RoomInfoPane = {{RoomInfoPane}} {
        width: Fill,
        height: Fill,
        align: {x: 0.5, y: 0},
        padding: {left: 15., right: 15., top: 15.}
        spacing: 20,
        flow: Down,

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

            room_additional_info = <RoomAdditionalInfoGrid> {}

        }

        <LineH> { padding: 15 }

        actions = <View> {
            width: Fill, height: Fill
            flow: Down,
            spacing: 7

            <Label> {
                width: Fill, height: Fit
                draw_text: {
                    wrap: Line,
                    text_style: <USERNAME_TEXT_STYLE>{ font_size: 11.5 },
                    color: #000
                }
                text: "Actions"
            }

            <ScrollXYView> {
                width: Fill, height: Fill
                flow: Down,
                spacing: 7

                invite_button = <RobrixIconButton> {
                    width: Fill,
                    align: { x: 0 }
                    draw_icon: {
                        svg_file: (ICON_INVITE_PEOPLE)
                    }
                    text: "Invite"
                }

                copy_link_to_room_button = <RobrixIconButton> {
                    width: Fill,
                    align: { x: 0 }
                    draw_icon: {
                        svg_file: (ICON_COPY)
                    }
                    text: "Copy link"
                }
            }


            <LineH> { padding: 15 }

            leave_room_button = <RobrixIconButton> {
                width: Fill, height: Fit
                align: { y: 1 }
                margin: { bottom: 10}
                draw_icon: {
                    svg_file: (ICON_LEAVE_ROOM)
                    color: (COLOR_DANGER_RED)
                }
                draw_bg: {
                    border_color: (COLOR_DANGER_RED)
                    color: #fff0f0
                }
                text: "Leave room"
            }
        }
    }

}


#[derive(Live, LiveHook, Widget)]
pub struct RoomInfoPane {
    #[deref] view: View,
}

impl Widget for RoomInfoPane {

    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        self.widget_match_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl WidgetMatchEvent for RoomInfoPane {
    fn handle_actions(&mut self, _cx: &mut Cx, actions:&Actions, _scope: &mut Scope) {
        if self.button(id!(copy_link_to_room_button)).clicked(actions) {
            log!("Copy link to room button clicked");
        }

        if self.button(id!(invite_button)).clicked(actions) {
            log!("Invite button clicked");
        }

        if self.button(id!(leave_room_button)).clicked(actions) {
            log!("Leave room button clicked");
        }
    }
}