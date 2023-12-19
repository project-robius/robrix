use crate::shared::clickable_view::*;
use crate::shared::stack_view_action::StackViewAction;
use makepad_widgets::widget::WidgetCache;
use makepad_widgets::*;

live_design! {
    import makepad_draw::shader::std::*;
    import makepad_widgets::base::*;
    import makepad_widgets::theme_desktop_dark::*;

    import crate::shared::helpers::FillerX;
    import crate::shared::helpers::Divider;
    import crate::shared::styles::*;
    import crate::shared::clickable_view::ClickableView

    IMG_DEFAULT_AVATAR = dep("crate://self/resources/img/default_avatar.png")
    IMG_FAVORITES = dep("crate://self/resources/img/favorites.png")
    IMG_MY_POSTS = dep("crate://self/resources/img/my-posts.png")
    IMG_STICKER_GALLERY = dep("crate://self/resources/img/sticker-gallery.png")
    IMG_SETTINGS = dep("crate://self/resources/img/settings.png")
    IMG_QR = dep("crate://self/resources/img/qr_icon.png")

    ActionIcon = <Label> {
        width: Fit, height: Fit
        text: ">"
        draw_text:{
            color: #b4,
            text_style: <REGULAR_TEXT>{font_size: 16},
        }
    }

    OptionsItem = <View> {
        width: Fill, height: Fit
        padding: {left: 10., top: 10., right: 10. bottom: 2.}, spacing: 8., flow: Down
        show_bg: true
        draw_bg: {
            color: #fff
        }

        content = <View> {
            width: Fill, height: Fit
            padding: 0, align: {x: 0.0, y: 0.5}, spacing: 10., flow: Right

            icon = <Image> {
                width: 36., height: 36.
            }

            label = <Label> {
                width: Fit, height: Fit
                draw_text:{
                    color: #000,
                    text_style: <REGULAR_TEXT>{},
                },
            }

            <FillerX> {}

            action_icon = <ActionIcon> {}
        }

        divider = <Divider> {
            margin: {left: 42.0}
        }
    }

    Options = <View> {
        width: Fill, height: Fit
        padding: 0, spacing: 0., flow: Down
    }

    Profile = {{Profile}} {
        view: {
            width: Fill, height: Fill
            flow: Down, spacing: 10.

            show_bg: true,
            draw_bg: {
                color: #ddd
            }

            ProfileInfo = <View> {
                width: Fill, height: Fit
                flow: Right, spacing: 10., padding: {top: 100., bottom: 30., right: 10., left: 20.}

                show_bg: true
                draw_bg: {
                    color: #fff
                }

                <View> {
                    width: Fit, height: Fit
                    flow: Down, align: {y: 0}
                    avatar = <Image> {
                        source: (IMG_DEFAULT_AVATAR),
                        width: 80., height: 80.
                    }
                }

                <View> {
                    width: Fill, height: Fit
                    flow: Down, align: {x: 0, y: 0.5}, padding: {top: 5., left: 10.}, spacing: 20.

                    username = <Label> {
                        draw_text:{
                            color: #000,
                            text_style: <TEXT_SUB>{font_size: 20.},
                        }
                        text:"facu"
                    }

                    <View> {
                        width: Fill, height: Fit
                        flow: Right, spacing: 5., align: {y: 0.5}

                        <View> {
                            width: Fill, height: Fit
                            flow: Down, spacing: 5.

                            wechat_id_prefix = <Label> {
                                draw_text: {
                                    color: #6a6a6a,
                                    text_style: <REGULAR_TEXT>{font_size: 11.},
                                }
                                text: "WeChat ID:"
                            }

                            wechat_id = <Label> {
                                draw_text: {
                                    color: #6a6a6a,
                                    text_style: <REGULAR_TEXT>{font_size: 11.},
                                }
                                text: "wxid_123n43kjl123hjg"
                            }
                        }

                        my_profile_frame = <ClickableView> {
                            width: Fit, height: Fit
                            align: {y: 0.5}, spacing: 15
                            qr_icon = <Image> {
                                source: (IMG_QR),
                                width: 20., height: 20.
                            }

                            <Label> {
                                width: Fit, height: Fit
                                text: ">"
                                draw_text: {
                                    text_style: <REGULAR_TEXT>{font_size: 16},
                                    fn get_color(self) -> vec4 {
                                        return #b4
                                    }
                                }
                            }
                        }
                    }

                    <View> {
                        width: Fit, height: Fit
                        flow: Right

                        status_button = <Button> {
                            width: Fit, height: 34.
                            text: "+ Status"
                            draw_text: {
                                text_style: <REGULAR_TEXT>{font_size: 12.},
                                fn get_color(self) -> vec4 {
                                    return #6a
                                }
                            }
                            draw_bg: {
                                border_radius: 8.
                                fn pixel(self) -> vec4 {
                                    let border_color = #b4b4b4;
                                    let border_width = 0.5;
                                    let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                                    let body = mix(mix(#f, #f0, self.hover), #d1, self.pressed);

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
                                    let border_color = #b4b4b4;
                                    let border_width = 0.5;
                                    let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                                    let body = mix(mix(#f, #f0, self.hover), #d1, self.pressed);

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

                <OptionsItem> {
                    content = {
                        icon = {
                            source: (IMG_STICKER_GALLERY)
                        }

                        label = {
                            text: "Stickers"
                        }

                    }

                    divider = <View> {}
                }
            }

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

                    divider = <View> {}
                }
            }
        }
    }

    ProfileScreen = <View> {
        width: Fill, height: Fill
        <Profile> {}
    }
}

#[derive(Live)]
pub struct Profile {
    #[live]
    view:View,
}

impl LiveHook for Profile {
    fn before_live_design(cx: &mut Cx) {
        register_widget!(cx, Profile);
    }
}

impl Widget for Profile {
    fn handle_widget_event_with(
        &mut self,
        cx: &mut Cx,
        event: &Event,
        dispatch_action: &mut dyn FnMut(&mut Cx, WidgetActionItem),
    ) {
        let uid = self.widget_uid();
        self.handle_event_with(cx, event, &mut |cx, action: StackViewAction| {
            dispatch_action(cx, WidgetActionItem::new(action.into(), uid));
        });
    }

    fn redraw(&mut self, cx: &mut Cx) {
        self.view.redraw(cx);
    }

    fn find_widgets(&mut self, path: &[LiveId], cached: WidgetCache, results: &mut WidgetSet) {
        self.view.find_widgets(path, cached, results);
    }

    fn draw_walk_widget(&mut self, cx: &mut Cx2d, walk: Walk) -> WidgetDraw {
        self.view.draw_walk_widget(cx, walk)
    }
}

impl Profile {
    fn handle_event_with(
        &mut self,
        cx: &mut Cx,
        event: &Event,
        dispatch_action: &mut dyn FnMut(&mut Cx, StackViewAction),
    ) {
        let actions = self.view.handle_widget_event(cx, event);

        if self
            .view
            .clickable_view(id!(my_profile_frame))
            .clicked(&actions)
        {
            dispatch_action(cx, StackViewAction::ShowMyProfile);
        }
    }
}
