use crate::shared::clickable_view::*;
use makepad_widgets::widget::WidgetCache;
use makepad_widgets::*;

live_design! {
    import makepad_draw::shader::std::*;
    import makepad_widgets::base::*;
    import makepad_widgets::theme_desktop_dark::*;

    import crate::shared::helpers::Divider;
    import crate::shared::styles::*;
    import crate::shared::clickable_view::ClickableView

    OptionsItem = <View> {
        flow: Down,
        width: Fill, height: Fit
        padding: <MSPACE_2> {}
        show_bg: false

        content = <View> {
            flow: Right,
            width: Fill, height: Fit,
            spacing: (SPACE_1),
            align: { x: 0.5, y: 0.5},

            icon = <Image> {
                width: 32.5, height: 32.5
            }

            label = <H4> {}
            <Filler> {}
            action_icon = <ActionIcon> {}
        }
    }

    Options = <View> {
        width: Fill, height: Fit
        flow: Down
        padding: 0.,
        spacing: 0.,
    }

    Profile = {{Profile}} {
        width: Fill, height: Fill
        flow: Down, spacing: (SPACE_2)

        ProfileInfo = <View> {
            width: Fill, height: Fit,
            flow: Down,
            padding: {top: 100., bottom: 30., right: 10., left: 20.},
            spacing: (SPACE_2),
            show_bg: false,
            
            <View> {
                username = <H2> { text:"facu" }
                width: Fill, height: Fit
                flow: Down,
                align: {x: 0.0, y: 0.}
                avatar = <Image> {
                    source: (IMG_DEFAULT_AVATAR),
                    width: 80., height: 80.
                }
            }

            <View> {
                width: Fill, height: Fit,
                flow: Down,
                align: {x: 0, y: 0.5},
                margin: <MSPACE_0> {}, padding: <MSPACE_0> {},
                spacing: (SPACE_2),

                <View> {
                    width: Fill, height: Fit,
                    flow: Right,
                    align: {y: 0.5},
                    spacing: 5.,

                    <View> {
                        width: Fill, height: Fit
                        flow: Down,
                        spacing: 5.,

                        wechat_id_prefix = <Pbold> { text: "WeChat ID" }
                        wechat_id = <P> { text: "wxid_123n43kjl123hjg" }
                    }

                    my_profile_frame = <ClickableView> {
                        show_bg: false,
                        width: Fit, height: Fit
                        align: {y: 0.5},
                        spacing: 15.
                        qr_icon = <Image> {
                            source: (IMG_QR),
                            width: 20., height: 20.
                        }

                        <ActionIcon> {}

                    }
                }

                <View> {
                    width: Fit, height: Fit
                    flow: Right

                    status_button = <Button> {
                        width: Fit, height: 34.
                        text: "Status"
                        draw_text: {
                            text_style: <REGULAR_TEXT> {font_size: 10.},
                            fn get_color(self) -> vec4 {
                                return (COLOR_D_6)
                            }
                        }
                        draw_bg: {
                            border_radius: 8.
                            fn pixel(self) -> vec4 {
                                let border_color = (COLOR_D_3);
                                let border_width = 1.5;
                                let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                                let body = mix(mix((COLOR_U), (COLOR_U_0), self.hover), (COLOR_D_2), self.pressed);

                                sdf.box(
                                    1.,
                                    1.,
                                    self.rect_size.x - 2.0,
                                    self.rect_size.y - 2.0,
                                    self.border_radius
                                )
                                sdf.fill_keep(body)

                                sdf.stroke(
                                    border_color,
                                    border_width
                                )
                                return sdf.result
                            }
                        }
                    }

                    meatball_menu_button = <Button> {
                        width: Fit, height: 34
                        text: "..."
                        draw_text: {
                            text_style: <TEXT_SUB>{font_size: 14.},
                            fn get_color(self) -> vec4 {
                                return #6a
                            }
                        }
                        draw_bg: {
                            fn pixel(self) -> vec4 {
                                let border_color = (COLOR_D_3);
                                let border_width = 1.5;
                                let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                                let body = mix(mix((COLOR_U), (COLOR_U_0), self.hover), (COLOR_D_2), self.pressed);

                                let radius = min(self.rect_size.x, self.rect_size.y) / 2.0 - 1.;

                                sdf.circle(self.rect_size.x / 2.0, self.rect_size.y / 2.0, radius)

                                sdf.fill_keep(body)

                                sdf.stroke(
                                    border_color,
                                    border_width
                                )
                                return sdf.result
                            }
                        }
                    }
                }
            }
        }
        
        <Options> {
            <OptionsItem> {
                content = {
                    icon = {
                        source: (IMG_FAVORITES)
                    }

                    label = {
                        text: "Favorites"
                    }
                }
            }
            <DividerH> {}
            <OptionsItem> {
                content = {
                    icon = {
                        source: (IMG_MY_POSTS)
                    }

                    label = {
                        text: "My Posts"
                    }
                }
            }
            <DividerH> {}
            <OptionsItem> {
                content = {
                    icon = {
                        source: (IMG_STICKER_GALLERY)
                    }

                    label = {
                        text: "Stickers"
                    }

                }
            }
            <DividerH> {}
            <Options> {
                <OptionsItem> {
                    content = {
                        icon = {
                            source: (IMG_SETTINGS)
                        }

                        label = {
                            text: "Settings"
                        }
                    }
                }
            }
        }

    }

    ProfileScreen = <View> {
        width: Fill, height: Fill
        <Profile> {
            show_bg: true,
            draw_bg: {color: (COLOR_D_0) }
        }
    }
}

#[derive(Live, LiveHook, Widget)]
pub struct Profile {
    #[deref]
    view: View
}

impl Widget for Profile {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        let actions = cx.capture_actions(|cx| self.view.handle_event(cx, event, scope));
        if self
            .view
            .clickable_view(id!(my_profile_frame))
            .clicked(&actions)
        {
            let uid = self.widget_uid();
            cx.widget_action(uid, &scope.path, StackNavigationAction::NavigateTo(live_id!(my_profile_stack_view)));
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}
