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

    // Room info View
    RoomInfoView = <ScrollXYView> {
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

    // Members View
    RoomMembersView = <View> {
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

        <SearchBar> {
            input = {
                empty_message: "Filter members",
            }
        }

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

        invite_button = <ButtonFlat> {
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

    MolyRadioButtonTab = <RadioButtonTab> {
        // padding: 10,

        draw_radio: {
            uniform radius: 3.0
            uniform border_width: 0.0
            instance color_unselected: #202425
            instance color_unselected_hover: #03C078
            instance color_selected: #03C078
            instance border_color: #03C078

            fn get_color(self) -> vec4 {
                return mix(
                    mix(
                        self.color_unselected,
                        self.color_unselected_hover,
                        self.hover
                    ),
                    self.color_selected,
                    self.selected
                )
            }

            fn get_border_color(self) -> vec4 {
                return self.border_color;
            }

            fn pixel(self) -> vec4 {
                let sdf = Sdf2d::viewport(self.pos * self.rect_size)
                match self.radio_type {
                    RadioType::Tab => {
                        sdf.box(
                            self.border_width,
                            self.border_width,
                            self.rect_size.x - (self.border_width * 2.0),
                            self.rect_size.y - (self.border_width * 2.0),
                            max(1.0, self.radius)
                        )
                        sdf.fill_keep(self.get_color())
                        if self.border_width > 0.0 {
                            sdf.stroke(self.get_border_color(), self.border_width)
                        }
                    }
                }
                return sdf.result
            }
        }

        draw_text: {
            fn get_color(self) -> vec4 {
                return mix(
                    mix(
                        self.color_unselected,
                        self.color_unselected_hover,
                        self.hover
                    ),
                    self.color_selected,
                    self.selected
                )
            }
        }
    }

    ActionToggleButton = <MolyRadioButtonTab> {
        width: Fit,
        height: 32,
        padding: { left: 20, top: 10, bottom: 10, right: 20 },
        label_walk: { margin: 0 }
        draw_text: {
            text_style: <THEME_FONT_BOLD>{font_size: 9},
            color_selected: #475467;
            color_unselected: #173437;
            color_unselected_hover: #0;
        }
        draw_radio: {
            color_unselected: #fff,
            color_selected: #fff,
            color_unselected_hover: #fff,
        }
    }


    RoomDetailsActions = <View> {
        width: Fill, height: Fit,
        tab_buttons = <View> {
            width: Fill,
            height: Fit,
            padding: 5,
            // align: { x: 0.5 },
            spacing: 5,

            show_bg: true,
            draw_bg: {
                color: #f,
            }

            info_button =  <ActionToggleButton> {
                text: "Info",
            }
            members_button = <ActionToggleButton> {
                text: "Members",
            }
        }

    }

    RoomDetailsSlidingPane = {{RoomDetailsSlidingPane}} {
        flow: Overlay,
        width: Fill,
        height: Fill,
        align: {
            x: 1.0,
            y: 0
        }

        bg_view = <View> {
            width: Fill
            height: Fill
            // visible: false,
            show_bg: true
            draw_bg: {
                fn pixel(self) -> vec4 {
                    return vec4(0., 0., 0., 0.7)
                }
            }
        }

        main_content = <FadeView> {
            width: 360,
            height: Fill
            flow: Overlay,

            <View> {
                width: Fill, height: Fill,
                show_bg: true,
                draw_bg: {
                    color: #fff
                }
                flow: Down,
                room_details_actions = <RoomDetailsActions> {}
                <RoomMembersView> {}
            }
        }
    }
}

#[derive(Live, LiveHook, Widget)]
pub struct RoomDetailsSlidingPane {
    #[deref] view: View,
}

impl Widget for RoomDetailsSlidingPane {
   fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
      self.view.handle_event(cx, event, scope);
   }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl RoomDetailsSlidingPane {
    pub fn show(&mut self, cx: &mut Cx) {
        self.visible = true;
        self.view(id!(bg_view)).set_visible(true);
        self.redraw(cx);
    }
}

impl RoomDetailsSlidingPaneRef {
    pub fn show(&self, cx: &mut Cx) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.show(cx);
    }
}
