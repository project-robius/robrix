use makepad_widgets::*;
use matrix_sdk::encryption::VerificationState;

use crate::shared::adaptive_view::DisplayContext;
use crate::sliding_sync::{get_client, submit_async_request, MatrixRequest};
use crate::verification::VerificationStateAction;

live_design! {
    import makepad_widgets::base::*;
    import makepad_widgets::theme_desktop_dark::*;
    import makepad_draw::shader::std::*;

    import crate::shared::styles::*;
    import crate::shared::helpers::*;
    import crate::shared::adaptive_view::AdaptiveView;
    import crate::shared::verification_badge::*;

    ICON_HOME = dep("crate://self/resources/icons/home.svg")
    ICON_SETTINGS = dep("crate://self/resources/icons/settings.svg")
    ICON_LOGOUT = dep("crate://self/resources/icons/logout.svg")

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

            verification_icon = <View> {
                flow: Overlay
                align:{ x: 0.5, y: 0.5 }
                width: 31, height: 31

                icon_yes = <IconYes> {}
                icon_no = <IconNo> {}
                icon_unk = <IconUnk> {}
            }
        }
        verification_notice = <VerificationNotice> { }
    }

    Separator = <LineH> {
        margin: {left: 15, right: 15}
    }

    Home = <RoundedView> {
        width: Fit, height: Fit
        // FIXME: the extra padding on the right is because the icon is not correctly centered
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

    Logout = {{Logout}} {
        width: Fit, height: Fit
        padding: {top: 8, left: 8, right: 12, bottom: 8}
        align: {x: 0.5, y: 0.5}
        logout_button = <Button> {
            draw_bg: {
                fn pixel(self) -> vec4 {
                    let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                    return sdf.result
                }
            }

            draw_icon: {
                svg_file: (ICON_LOGOUT),
                fn get_color(self) -> vec4 {
                    return (COLOR_DANGER_RED);
                    // return #x566287; // grayed-out #1C274C until enabled
                }
            }

            icon_walk: {width: 25, height: Fit}
        }
    }

    Settings = <View> {
        width: Fit, height: Fit
        // FIXME: the extra padding on the right is because the icon is not correctly centered
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

    SpacesDock = <AdaptiveView> {
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

            <Home> {}

            <Filler> {}

            <Logout> {}

            <Settings> {}
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

            <Home> {}

            <Filler> {}

            <Settings> {}

            <Filler> {}
        }
    }
}
struct VerificationNoticeText {
    yes: &'static str,
    no: &'static str,
    unk: &'static str,
}

impl Default for VerificationNoticeText{
    fn default() -> Self {
        Self {
            yes: "This device is fully verified.",
            no: "This device is unverified. To view your encrypted message history, please verify it from another client.",
            unk: "Verification state is unknown.",
        }
    }
}

#[derive(Live, Widget)]
pub struct Profile {
    #[deref]
    view: View,
    #[rust(VerificationState::Unknown)]
    verification_state: VerificationState,
    #[rust]
    verification_notice_text: VerificationNoticeText,
}

impl Profile {
    fn set_verification_icon_visibility(&self) {
        let (yes_visible, no_visible, unk_visible) = match self.verification_state {
            VerificationState::Unknown => (false, false, true),
            VerificationState::Unverified => (false, true, false),
            VerificationState::Verified => (true, false, false),
        };

        self.view(id!(icon_yes)).set_visible(yes_visible);
        self.view(id!(icon_no)).set_visible(no_visible);
        self.view(id!(icon_unk)).set_visible(unk_visible);
    }
}

impl Widget for Profile {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if let Event::MouseMove(e) = event {
            let mut verification_notice = self.tooltip(id!(verification_notice));

            if self.view(id!(verification_icon)).area().rect(cx).contains(e.abs) {
                let text = match self.verification_state {
                    VerificationState::Unknown => self.verification_notice_text.unk,
                    VerificationState::Unverified => self.verification_notice_text.no,
                    VerificationState::Verified => self.verification_notice_text.yes
                };

                //Determine if it's a desktop or mobile layout,
                //then we set the relative position so that the tooltip looks like following the cursor.
                if cx.get_global::<DisplayContext>().is_desktop() {
                    verification_notice.show_with_options(cx, DVec2 {x: 65., y: 23.}, text);
                }
                else {
                    verification_notice.apply_over(cx, live!{
                        content: {

                            // Via setting suitable align & padding,
                            // we can simulate a relative position to make the tootip follow widget `Profile (U)`,
                            // this is not a perfect solution.
                            // TODO: Find a way to follow widget `Profile (U)` more precisely.
                            align: { x: 0.43, y: 1. }
                            padding: { left: 30., bottom: 31. }
                        }
                    });
                    verification_notice.show_with_options(cx, DVec2 {x: 0., y: 0.}, text);
                }
            }
            //Hide it if cursor is not hovering.
            else {
                verification_notice.hide(cx);
            }
        }

        self.widget_match_event(cx, event, scope);
        self.view.handle_event(cx, event, scope)
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl WidgetMatchEvent for Profile {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions, _scope: &mut Scope) {

        for action in actions {
            if let Some(VerificationStateAction::Update(state)) = action.downcast_ref() {
                if self.verification_state != *state {
                    self.verification_state = *state;
    
                    self.set_verification_icon_visibility();
                    self.redraw(cx);
                }
            }
        }
    }
}

impl LiveHook for Profile {
    fn after_new_from_doc(&mut self, cx:&mut Cx) {
        if let Some(client) = get_client() {
            let current_verification_state = client.encryption().verification_state().get();
            self.verification_state = current_verification_state;

            self.set_verification_icon_visibility();
            self.redraw(cx);
        }
    }
}

#[derive(Live, LiveHook, Widget)]
pub struct Logout {
    #[deref] view: View
}

impl Widget for Logout {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.widget_match_event(cx, event, scope);
        self.view.handle_event(cx, event, scope)
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl WidgetMatchEvent for Logout {
    fn handle_actions(&mut self, _cx: &mut Cx, actions:&Actions, _scope: &mut Scope) {
        if self.button(id!(logout_button)).clicked(actions) {
            submit_async_request(MatrixRequest::Logout);
        }
    }
}