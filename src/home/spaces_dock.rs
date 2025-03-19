use makepad_widgets::*;

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::styles::*;
    use crate::shared::helpers::*;
    use crate::shared::verification_badge::*;

    ICON_HOME = dep("crate://self/resources/icons/home.svg")
    ICON_SETTINGS = dep("crate://self/resources/icons/settings.svg")

    Filler = <View> {
        height: Fill, width: Fill
    }

    Profile = {{Profile}} {
        flow: Overlay
        width: Fit, height: Fit
        align: { x: 0.5, y: 0.5 }

        text_view = <View> {
            flow: Overlay
            width: 60, height: 60,
            align: { x: 0.5, y: 0.5 }
            show_bg: true,
            draw_bg: {
                instance background_color: (COLOR_AVATAR_BG_IDLE),
                fn pixel(self) -> vec4 {
                    let sdf = Sdf2d::viewport(self.pos * self.rect_size);

                    let c = self.rect_size * 0.5;

                    let r = self.rect_size * 0.38;

                    sdf.circle(c.x, c.x, r.x);
                    sdf.fill_keep(self.background_color);
                    return sdf.result
                }
            }

            text = <Label> {
                width: Fit, height: Fit,
                padding: { top: 1.0 } // for better vertical alignment
                draw_text: {
                    text_style: { font_size: 13. }
                    color: #f,
                }
                text: "U"
            }
        }

        <View> {
            align: { x: 1.0, y: 0.0 }
            verification_badge = <VerificationBadge> {}
        }

    }

    Separator = <LineH> {
        margin: {left: 15, right: 15}
    }

    HomeButton = {{HomeButton}} {
        width: Fit, height: Fit
        align: {x: 0.5, y: 0.5}
        select_btn = <RoundedView> {
            width: Fit, height: Fit
            show_bg: true
            draw_bg: {
                color: (COLOR_PRIMARY_DARKER)
                radius: 4.0
                border_color: (COLOR_SELECTED_PRIMARY)
                border_width: 1.5
            }
    
            align: {x: 0.5, y: 0.5}
            home_button = <Button> {
                draw_icon: {
                    svg_file: (ICON_HOME),
                    fn get_color(self) -> vec4 {
                        return #1C274C;
                    }
                }
                icon_walk: {width: 25, height: Fit}
            }
        }
    }

    SettingButton = {{SettingButton}} {
        width: Fit, height: Fit
        align: {x: 0.5, y: 0.5}
        setting_button = <Button> {
            draw_bg: {
                fn pixel(self) -> vec4 {
                    let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                    return sdf.result
                }
            }
            draw_icon: {
                svg_file: (ICON_SETTINGS),
                fn get_color(self) -> vec4 {
                    return #1C274C;
                }
            }
            icon_walk: {width: 25, height: Fit}
        }
    }

    pub SpacesDock = <AdaptiveView> {
        Desktop = {
            flow: Down, spacing: 15
            align: {x: 0.5}
            padding: {top: 40., bottom: 20.}
            width: 68., height: Fill
            show_bg: true
            draw_bg: {
                color: (COLOR_SECONDARY)
            }

            <Profile> {}

            <Separator> {}

            <HomeButton> {}

            <Filler> {}

            <SettingButton> {}
        }

        Mobile = {
            flow: Right
            align: {x: 0.5, y: 0.5}
            padding: {top: 10, right: 10, bottom: 10, left: 10}
            width: Fill, height: Fit
            show_bg: true
            draw_bg: {
                color: (COLOR_SECONDARY)
            }

            <Filler> {}

            <Profile> {}

            <Filler> {}

            <HomeButton> {}

            <Filler> {}

            <SettingButton> {}

            <Filler> {}
        }
    }
}

#[derive(Live, LiveHook, Widget)]
pub struct Profile {
    #[deref] view: View,
}

impl Widget for Profile {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

#[derive(Live, LiveHook, Widget)]
pub struct SettingButton {
    #[deref] view: View,
}

impl Widget for SettingButton {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        self.widget_match_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl WidgetMatchEvent for SettingButton {
    fn handle_actions(&mut self, cx: &mut Cx, actions :&Actions, _scope: &mut Scope) {
        if self.button(id!(setting_button)).clicked(actions) {
            cx.action(PageSwitchAction::SwitchToSetting);
        }
    }
}

#[derive(Live, LiveHook, Widget)]
pub struct HomeButton {
    #[deref] view: View,
}

impl Widget for HomeButton {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        self.widget_match_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl WidgetMatchEvent for HomeButton {
    fn handle_actions(&mut self, cx: &mut Cx, actions :&Actions, _scope: &mut Scope) {
        if self.button(id!(home_button)).clicked(actions) {
            cx.action(PageSwitchAction::SwitchToHome);
        }
    }
}


#[derive(Clone, DefaultNone, Debug)]
pub enum PageSwitchAction {
    None,
    SwitchToSetting,
    SwitchToHome
}
