use makepad_widgets::*;
use matrix_sdk::ruma::{OwnedRoomId, OwnedUserId};

live_design! {
    import makepad_draw::shader::std::*;
    import makepad_widgets::base::*;
    import makepad_widgets::theme_desktop_dark::*;

    import crate::shared::helpers::*;
    import crate::shared::styles::*;
    import crate::shared::avatar::*;

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

    // Copied from Moxin
    //
    // Customized button widget, based on the RoundedView shaders with some modifications
    // which is a better fit with our application UI design
    MoxinButton = <Button> {
        width: Fit, 
        height: Fit,
        spacing: 10,
        padding: {top: 10, bottom: 10, left: 15, right: 15}

        draw_bg: {
            instance color: #EDFCF2
            instance color_hover: #fff
            instance border_width: 1.2
            instance border_color: #D0D5DD
            instance border_color_hover: #fff
            instance radius: 3.0

            fn get_color(self) -> vec4 {
                return mix(self.color, mix(self.color, self.color_hover, 0.2), self.hover)
            }

            fn get_border_color(self) -> vec4 {
                return mix(self.border_color, mix(self.border_color, self.border_color_hover, 0.2), self.hover)
            }

            fn pixel(self) -> vec4 {
                let sdf = Sdf2d::viewport(self.pos * self.rect_size)
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
                return sdf.result;
            }
        }

        draw_icon: {
            instance color: #000
            instance color_hover: #000
            uniform rotation_angle: 0.0,

            fn get_color(self) -> vec4 {
                return mix(self.color, mix(self.color, self.color_hover, 0.2), self.hover)
            }

            // Support rotation of the icon
            fn clip_and_transform_vertex(self, rect_pos: vec2, rect_size: vec2) -> vec4 {
                let clipped: vec2 = clamp(
                    self.geom_pos * rect_size + rect_pos,
                    self.draw_clip.xy,
                    self.draw_clip.zw
                )
                self.pos = (clipped - rect_pos) / rect_size

                // Calculate the texture coordinates based on the rotation angle
                let angle_rad = self.rotation_angle * 3.14159265359 / 180.0;
                let cos_angle = cos(angle_rad);
                let sin_angle = sin(angle_rad);
                let rot_matrix = mat2(
                    cos_angle, -sin_angle,
                    sin_angle, cos_angle
                );
                self.tex_coord1 = mix(
                    self.icon_t1.xy,
                    self.icon_t2.xy,
                    (rot_matrix * (self.pos.xy - vec2(0.5))) + vec2(0.5)
                );

                return self.camera_projection * (self.camera_view * (self.view_transform * vec4(
                    clipped.x,
                    clipped.y,
                    self.draw_depth + self.draw_zbias,
                    1.
                )))
            }
        }
        icon_walk: {width: 16, height: 16}

        draw_text: {
            text_style: <REGULAR_TEXT>{font_size: 10},
            color: #000
            fn get_color(self) -> vec4 {
                return self.color;
            }
        }
    }


    ICON_BLOCK_USER  = dep("crate://self/resources/icons/forbidden.svg")
    ICON_CLOSE       = dep("crate://self/resources/icons/close.svg")
    ICON_DOUBLE_CHAT = dep("crate://self/resources/icons/double_chat.svg")
    ICON_COPY        = dep("crate://self/resources/icons/copy.svg")
    ICON_JUMP        = dep("crate://self/resources/icons/go_back.svg")


    UserProfileView = <ScrollXYView> {
        width: Fill,
        height: Fill,
        align: {x: 0.5, y: 0},
        spacing: 15,
        flow: Down,

        show_bg: true,
        draw_bg: {
            color: #f3f3fa
            // 241, 244, 251
        }

        avatar = <Avatar> {
            width: 150,
            height: 150,
            margin: 10.0,
        }

        user_name = <Label> {
            width: Fill, height: Fit
            draw_text: {
                wrap: Line,
                color: #000,
                text_style: <USERNAME_TEXT_STYLE>{ },
            }
            text: "User Name"
        }

        user_id = <Label> {
            width: Fill, height: Fit
            draw_text: {
                wrap: Line,
                color: (MESSAGE_TEXT_COLOR),
                text_style: <MESSAGE_TEXT_STYLE>{ font_size: 10 },
            }
            text: "User ID"
        }

        <LineH> { padding: 15 }

        <View> {
            width: Fill,
            height: Fit,
            flow: Down,
            spacing: 15,
            align: {x: 0.0, y: 0.0}

            role_info_section_label = <Label> {
                width: Fill, height: Fit
                padding: {left: 15}
                draw_text: {
                    wrap: Line,
                    text_style: <USERNAME_TEXT_STYLE>{},
                    color: #000
                }
                text: "Role in this room"
            }

            role_info_label = <Label> {
                width: Fill, height: Fit
                padding: {left: 30}
                draw_text: {
                    wrap: Line,
                    color: (MESSAGE_TEXT_COLOR),
                    text_style: <MESSAGE_TEXT_STYLE>{ font_size: 11.5},
                }
                text: "Default"
            }

            <LineH> { padding: 15 }

            <Label> {
                width: Fill, height: Fit
                padding: {left: 15}
                draw_text: {
                    wrap: Line,
                    text_style: <USERNAME_TEXT_STYLE>{},
                    color: #000
                }
                text: "Actions"
            }
        }

        actions = <View> {
            width: Fill, height: Fit
            flow: Down,
            spacing: 10
            padding: {left: 25, bottom: 50 }


            direct_message_button = <MoxinButton> {
                draw_icon: {
                    svg_file: (ICON_DOUBLE_CHAT)
                }
                icon_walk: {width: 22, height: 16, margin: {left: -4, right: -4, top: 1, bottom: -1} }
                text: "Direct Message"
            }

            copy_link_to_user_button = <MoxinButton> {
                draw_icon: {
                    svg_file: (ICON_COPY)
                }
                icon_walk: {width: 16, height: 16, margin: {right: -2} }
                text: "Copy Link to User"
            }

            jump_to_read_receipt_button = <MoxinButton> {
                draw_icon: {
                    svg_file: (ICON_JUMP)
                }
                icon_walk: {width: 14, height: 16, margin: {left: -1, right: 2}}
                text: "Jump to Read Receipt"
            }

            block_user_button = <MoxinButton> {
                draw_icon: {
                    svg_file: (ICON_BLOCK_USER)
                    color: (COLOR_DANGER_RED),
                }
                icon_walk: {width: 16, height: 16, margin: {left: -2, right: -1} }

                draw_bg: {
                    border_color: (COLOR_DANGER_RED),
                    color: #fff0f0
                }
                text: "Ignore/Block User"
                draw_text:{
                    color: (COLOR_DANGER_RED),
                }
            }
        }
    }


    UserProfileSlidingPane = {{UserProfileSlidingPane}} {
        flow: Overlay,
        width: Fit,
        height: Fill,

        // TODO FIXME: I would like to wrap the entire content in a RoundedView,
        //             but it breaks everything when I try.

        // wrapper = <RoundedView> {
        //     width: Fit
        //     height: Fit
        //     flow: Down
        //     padding: 10
        //     spacing: 10

        //     show_bg: true
        //     draw_bg: {
        //         color: #fff
        //         radius: 3
        //     }


            main_content = <FadeView> {
                width: Fill,
                height: Fill
                flow: Overlay,

                // The "X" close button on the top left
                close_button = <MoxinButton> {
                    padding: 10,
                    width: Fit,
                    height: Fit,
                    align: {x: 0.0, y: 0.0},

                    margin: -15,

                    draw_icon: {
                        svg_file: (ICON_CLOSE),
                        fn get_color(self) -> vec4 {
                            return #000;
                        }
                    }
                    icon_walk: {width: 12, height: 12}
                }

                user_profile_view = <UserProfileView> { }
            }

        // }

        animator: {
            panel = {
                default: show,
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
                    apply: {main_content = { width: 110, draw_bg: {opacity: 0.0} }}
                }
            }
        }
    }
}


#[derive(Clone, DefaultNone, Debug)]
pub enum ShowUserProfileAction {
    ShowUserProfile(OwnedRoomId, OwnedUserId),
    None,
}


#[derive(Live, LiveHook, Widget)]
pub struct UserProfileSlidingPane {
    #[deref]
    view: View,

    #[animator]
    animator: Animator,

    #[rust] info: Option<(OwnedRoomId, OwnedUserId)>,
}

impl Widget for UserProfileSlidingPane {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        if self.animator_handle_event(cx, event).must_redraw() {
            self.redraw(cx);
        }
        // if !self.visible { return; }

        // Determine whether we should close the pane.
        let close_pane = match event {
            Event::Actions(actions) => self.button(id!(close_button)).clicked(actions),
            Event::MouseUp(mouse) => mouse.button == 3, // the "back" button on the mouse
            Event::KeyUp(key) => key.key_code == KeyCode::Escape,
            Event::BackPressed => true,
            _ => false,
        };
        if close_pane {
            self.animator_play(cx, id!(panel.hide));
        }

        // TODO: handle button clicks for things like:
        // * direct_message_button
        // * copy_link_to_user_button
        // * jump_to_read_receipt_button
        // * block_user_button

    }


    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        let Some((room_id, user_id)) = self.info.clone() else {
            self.visible = false;
            return self.view.draw_walk(cx, scope, walk);
        };
        self.visible = true;
        self.label(id!(user_id)).set_text(user_id.as_str());
        self.label(id!(role_info)).set_text(&format!("Role in {}", room_id.as_str()));

        // TODO: use room_id & user_id to retrieve and set other fields:
        // * avatar
        // * user_name
        // * role_info_section_label
        // * role_info_label

        self.view.draw_walk(cx, scope, walk)
    }
}

impl UserProfileSlidingPaneRef {
    pub fn set_info(&self, room_id: OwnedRoomId, user_id: OwnedUserId) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.info = Some((room_id, user_id));
        }
    }

    pub fn show(&self, cx: &mut Cx) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.visible = true;
            inner.animator_play(cx, id!(panel.show));
            inner.redraw(cx);
        }
    }
}
