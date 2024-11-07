use makepad_widgets::*;
use matrix_sdk::encryption::VerificationState;
use crate::sliding_sync::get_client;

live_design! {
    import makepad_widgets::base::*;
    import makepad_widgets::theme_desktop_dark::*;
    import makepad_draw::shader::std::*;

    import crate::shared::styles::*;
    import crate::shared::helpers::*;
    import crate::shared::adaptive_view::AdaptiveView;

    ICON_HOME = dep("crate://self/resources/icons/home.svg")
    ICON_SETTINGS = dep("crate://self/resources/icons/settings.svg")

    Filler = <View> {
        height: Fill, width: Fill
    }

    Profile = <View> {
        width: Fit, height: Fit
        align: { x: 0.5, y: 0.5 }

        text_view = <View> {
            width: 45., height: 45.,
            align: { x: 0.5, y: 0.5 }
            show_bg: true,

            draw_bg: {
                instance background_color: (COLOR_AVATAR_BG_IDLE),
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
                    color: #f,
                }
                text: "U"
            }
        }
    }

    Separator = <LineH> {
        margin: {left: 15, right: 15}
    }

    Home = <RoundedView> {
        width: Fit, height: Fit
        // FIXME: the extra padding on the right is becase the icon is not correctly centered
        // within its parent
        padding: {top: 8, left: 8, right: 12, bottom: 8}
        show_bg: true
        draw_bg: {
            color: (COLOR_PRIMARY_DARKER)
            radius: 4.0
            border_color: (COLOR_SELECTED_PRIMARY)
            border_width: 1.5
        }

        align: {x: 0.5, y: 0.5}
        <Icon> {
            draw_icon: {
                svg_file: (ICON_HOME),
                fn get_color(self) -> vec4 {
                    return #1C274C;
                }
            }
            icon_walk: {width: 25, height: Fit}
        }
    }

    Settings = <View> {
        width: Fit, height: Fit
        // FIXME: the extra padding on the right is becase the icon is not correctly centered
        // within its parent
        padding: {top: 8, left: 8, right: 12, bottom: 8}
        align: {x: 0.5, y: 0.5}
        <Button> {
            enabled: false
            draw_bg: {
                fn pixel(self) -> vec4 {
                    let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                    return sdf.result
                }
            }
            draw_icon: {
                svg_file: (ICON_SETTINGS),
                fn get_color(self) -> vec4 {
                    return (COLOR_TEXT_IDLE);
                    // return #x566287; // grayed-out #1C274C until enabled
                }
            }
            icon_walk: {width: 25, height: Fit}
        }
    }

    SDock = {{SDock}} {}

    SpacesDock = <AdaptiveView> {
            Desktop =  <SDock> {
                    flow: Down, spacing: 15
                    align: {x: 0.5}
                    padding: {top: 40., bottom: 20.}
                    width: 68., height: Fill
                    show_bg: true
                    draw_bg: {
                        color: (COLOR_SECONDARY)
                    }

                    profile = <Profile> {}

                    <Separator> {}

                    <Home> {}

                    <Filler> {}

                    <Settings> {}
            }
            Mobile = <SDock> {
                    flow: Right
                    align: {x: 0.5, y: 0.5}
                    padding: {top: 10, right: 10, bottom: 10, left: 10}
                    width: Fill, height: Fit
                    show_bg: true
                    draw_bg: {
                        color: (COLOR_SECONDARY)
                    }

                    <Filler> {}

                    profile = <Profile> {}

                    <Filler> {}

                    <Home> {}

                    <Filler> {}

                    <Settings> {}

                    <Filler> {}
            }
    }
}

#[derive(Live, LiveHook, Widget)]
pub struct SDock {
    #[deref]
    view: View,
    #[rust(None)]
    verification_state: Option<matrix_sdk::encryption::VerificationState>,
}

impl Widget for SDock {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        let client = get_client().unwrap();
        let verficiation_state_subscriber = client.encryption().verification_state();
        self.verification_state = Some(verficiation_state_subscriber.get());

        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        if let Some(verification_state) = self.verification_state {
            let verification_text = match verification_state {
                VerificationState::Unknown => "U",
                VerificationState::Verified => "V",
                VerificationState::Unverified => "N",
            };
        self.label(id!(profile.text_view.text)).set_text(verification_text);
        };

        self.view.draw_walk(cx, scope, walk)
    }
}
