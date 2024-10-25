use makepad_widgets::*;

live_design! {
    import makepad_draw::shader::std::*;
    import makepad_widgets::base::*;
    import makepad_widgets::theme_desktop_dark::*;

    import crate::shared::helpers::*;
    import crate::shared::styles::*;
    import crate::shared::avatar::*;
    import crate::shared::icon_button::*;
    import crate::shared::search_bar::SearchBar;

    RoomInfoPane = <ScrollXYView> {
        width: Fill,
        height: Fill,
        align: {x: 0.5, y: 0},
        padding: {left: 15., right: 15., top: 15.}
        spacing: 20,
        flow: Down,

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

    RoomMember = <View> {
        height: 48, width: Fill,
        show_bg: true,
        flow: Right,
        align: {y: 0.5},
        padding: {left: 5, right: 5},

        // Avatar
        avatar = <View> {
            width: 40, height: 40,
            show_bg: true,
            draw_bg: {
                color: #0
            }
        }

        name = <Label> {
            margin: {left: 5.0},
            text: "Name"
            draw_text: {
                color: #0
            }
        }

        <Filler> {}
        // Power levels

        member_room_power_level = <Label> {
            text: "Admin",
            draw_text: {
                color: #0
            }
        }
    }

    RoomMembersPane = <View> {
        width: Fill,
        height: Fill,
        align: {x: 0.5, y: 0},
        padding: {left: 10., right: 10.}
        spacing: 10,
        flow: Down,

        show_bg: true,
        draw_bg: {
            color: #f
        }

        <SearchBar> { }

        room_members_list = <ScrollXYView> {
            width: Fill,height: Fill,
            flow: Down,
            spacing: 1,
            <RoomMember> {}
            <RoomMember> {}
            <RoomMember> {}
            <RoomMember> {}
            <RoomMember> {}
            <RoomMember> {}
            <RoomMember> {}
        }

        invite_button = <RobrixIconButton> {
            width: Fill, height: 32,
            margin: { bottom: 10 },
            draw_icon: {
                svg_file: dep("crate://self/resources/icon_members.svg")
                color: #000
            }
            icon_walk: { width: 12, height: Fit },
            text: "Invite to this room",
            draw_text: {
                fn get_color(self) -> vec4 {
                    return #000
                }
            }
            draw_bg: {
                fn pixel(self) -> vec4 {
                    return (THEME_COLOR_MAKEPAD) + self.pressed * vec4(1., 1., 1., 1.)
                }
            }
        }

    }

    // Copied from Moxin
    FadeView = <CachedView> {
        draw_bg: {
            instance opacity: 1.0

            fn pixel(self) -> vec4 {
                let color = sample2d_rt(self.image, self.pos * self.scale + self.shift) + vec4(self.marked, 0.0, 0.0, 0.0);
                return Pal::premul(vec4(color.xyz, color.w * self.opacity))
            }
        }
    }

    RoomDetailsSlidingPane = {{RoomDetailsSlidingPane}} {
        flow: Overlay,
        width: Fill, height: Fill,
        align: { x: 1.0, y: 0 }

        bg_view = <View> {
            width: Fill
            height: Fill
            visible: false,
            show_bg: true
            draw_bg: {
                fn pixel(self) -> vec4 {
                    return vec4(0., 0., 0., 0.7)
                }
            }
        }

        main_content = <FadeView> {
            width: 360, height: Fill,
            flow: Overlay,

            <View> {
                width: Fill, height: Fill,
                show_bg: true,
                draw_bg: {
                    color: #fff
                }
                flow: Down,
                room_info_pane = <RoomInfoPane> { }
                room_members_pane = <RoomMembersPane> {
                    visible: false
                }

            }

            // The "X" close button on the top left
            close_button = <RobrixIconButton> {
                width: Fit,
                height: Fit,
                align: {x: 0.0, y: 0.0},
                margin: 7,
                padding: 15,

                draw_icon: {
                    svg_file: (ICON_CLOSE),
                    fn get_color(self) -> vec4 {
                        return #x0;
                    }
                }
                icon_walk: {width: 14, height: 14}
            }
        }
        animator: {
            panel = {
                default: hide,
                show = {
                    redraw: true,
                    from: {all: Forward {duration: 0.4}}
                    ease: ExpDecay {d1: 0.80, d2: 0.97}
                    apply: {main_content = { width: 300, draw_bg: {opacity: 1.0} }}
                }
                hide = {
                    redraw: true,
                    from: {all: Forward {duration: 0.5}}
                    ease: ExpDecay {d1: 0.80, d2: 0.97}
                    apply: {main_content = { width: 0, draw_bg: {opacity: 0.0} }}
                }
            }
        }
    }

}

#[derive(Clone, Debug, Default)]
pub enum RoomDetailsSlidingPaneType {
    #[default]
    Info,
    Members,
}

#[derive(Live, LiveHook, Widget)]
pub struct RoomDetailsSlidingPane {
    #[deref] view: View,
    #[animator] animator: Animator,
}

impl Widget for RoomDetailsSlidingPane {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        if self.animator_handle_event(cx, event).must_redraw() {
            self.redraw(cx);
        }

        let close_pane = match event {
            Event::Actions(actions) => self.button(id!(close_button)).clicked(actions),
            Event::MouseUp(mouse) => mouse.button == 3, // the "back" button on the mouse
            Event::KeyUp(key) => key.key_code == KeyCode::Escape,
            Event::BackPressed => true,
            _ => false,
        };
        if close_pane {
            self.animator_play(cx, id!(panel.hide));
            self.view(id!(bg_view)).set_visible(false);
            return;
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}


impl RoomDetailsSlidingPane {

    pub fn show(&mut self, cx: &mut Cx, pane_type: RoomDetailsSlidingPaneType) {

        let room_info_pane_ref = self.view(id!(room_info_pane));
        let room_members_pane_ref = self.view(id!(room_members_pane));

        self.visible = true;
        self.animator_play(cx, id!(panel.show));
        self.view(id!(bg_view)).set_visible(true);

        match pane_type {
            RoomDetailsSlidingPaneType::Info => {
                // Show the info pane
                if room_members_pane_ref.visible() {
                    room_members_pane_ref.set_visible(false);
                }
                room_info_pane_ref.set_visible(true);
            }
            RoomDetailsSlidingPaneType::Members => {
                // Show the members pane
                if room_info_pane_ref.visible() {
                    room_info_pane_ref.set_visible(false);
                }
                room_members_pane_ref.set_visible(true);
            }
        }

        self.redraw(cx);
    }
}

impl RoomDetailsSlidingPaneRef {

    pub fn show(&self, cx: &mut Cx, pane_type: RoomDetailsSlidingPaneType) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.show(cx, pane_type);
    }
}
