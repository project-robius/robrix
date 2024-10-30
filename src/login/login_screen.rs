use std::ops::Not;

use makepad_widgets::*;

use crate::sliding_sync::{submit_async_request, LoginRequest, MatrixRequest};

live_design! {
    import makepad_widgets::base::*;
    import makepad_widgets::theme_desktop_dark::*;
    import makepad_draw::shader::std::*;

    import crate::shared::styles::*;
    import crate::shared::icon_button::*;

    IMG_APP_LOGO = dep("crate://self/resources/robrix_logo_alpha.png")

    LoginTextInput = <TextInput> {
        width: Fill, height: Fit, margin: 0
        align: {y: 0.5}
        draw_bg: {
            color: (COLOR_PRIMARY)
            instance radius: 2.0
            instance border_width: 0.8
            instance border_color: #D0D5DD
            instance inset: vec4(0.0, 0.0, 0.0, 0.0)

            fn get_color(self) -> vec4 {
                return self.color
            }

            fn get_border_color(self) -> vec4 {
                return self.border_color
            }

            fn pixel(self) -> vec4 {
                let sdf = Sdf2d::viewport(self.pos * self.rect_size)
                sdf.box(
                    self.inset.x + self.border_width,
                    self.inset.y + self.border_width,
                    self.rect_size.x - (self.inset.x + self.inset.z + self.border_width * 2.0),
                    self.rect_size.y - (self.inset.y + self.inset.w + self.border_width * 2.0),
                    max(1.0, self.radius)
                )
                sdf.fill_keep(self.get_color())
                if self.border_width > 0.0 {
                    sdf.stroke(self.get_border_color(), self.border_width)
                }
                return sdf.result;
            }
        }

        draw_text: {
            color: (MESSAGE_TEXT_COLOR),
            text_style: <MESSAGE_TEXT_STYLE>{},

            fn get_color(self) -> vec4 {
                return mix(
                    self.color,
                    #B,
                    self.is_empty
                )
            }
        }


        // TODO find a way to override colors
        draw_cursor: {
            instance focus: 0.0
            uniform border_radius: 0.5
            fn pixel(self) -> vec4 {
                let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                sdf.box(
                    0.,
                    0.,
                    self.rect_size.x,
                    self.rect_size.y,
                    self.border_radius
                )
                sdf.fill(mix(#fff, #bbb, self.focus));
                return sdf.result
            }
        }

        // TODO find a way to override colors
        draw_selection: {
            instance hover: 0.0
            instance focus: 0.0
            uniform border_radius: 2.0
            fn pixel(self) -> vec4 {
                let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                sdf.box(
                    0.,
                    0.,
                    self.rect_size.x,
                    self.rect_size.y,
                    self.border_radius
                )
                sdf.fill(mix(#eee, #ddd, self.focus)); // Pad color
                return sdf.result
            }
        }
    }

    LoginScreen = {{LoginScreen}} {
        width: Fill, height: Fill
        show_bg: true,
        draw_bg: {
            color: (COLOR_PRIMARY)
        }
        align: {x: 0.5, y: 0.5}

        <RoundedView> {
            width: Fit, height: Fit
            flow: Down
            align: {x: 0.5, y: 0.5}
            padding: 30
            spacing: 15.0

            show_bg: true,
            draw_bg: {
                color: (COLOR_SECONDARY)
            }

            logo_image = <Image> {
                fit: Smallest,
                width: 80
                source: (IMG_APP_LOGO),
            }

            title = <Label> {
                width: Fit, height: Fit
                margin: { bottom: 10 }
                draw_text: {
                    color: (COLOR_TEXT)
                    text_style: <TITLE_TEXT>{font_size: 16.0}
                }
                text: "Login to Robrix"
            }

            user_id_input = <LoginTextInput> {
                width: 250, height: 40
                empty_message: "User ID"
            }

            password_input = <LoginTextInput> {
                width: 250, height: 40
                empty_message: "Password"
                // password: true
            }

            homeserver_input = <LoginTextInput> {
                width: 250, height: 40
                margin: {bottom: -10}
                empty_message: "matrix.org"
            }
            <Label> {
                width: Fit, height: Fit
                draw_text: {
                    color: #x8c8c8c
                    text_style: <REGULAR_TEXT>{font_size: 9}
                }
                text: "Homeserver (optional)"
            }

            login_button = <RobrixIconButton> {
                width: 250, height: 40
                margin: {top: 15}
                draw_bg: {
                    color: (COLOR_SELECTED_PRIMARY)
                }
                draw_text: {
                    color: (COLOR_PRIMARY)
                    text_style: <REGULAR_TEXT> {}
                }
                text: "Login"
            }
           sso_view = <View> {
                spacing: 20,
                width: Fit, height: Fit,
                flow: Right,
                apple_button = <RoundedView> {
                    width: Fit,
                    height: Fit,
                    cursor: Hand,
                    <Image> {
                        width: 21, height: 21, margin: { left: 10.0  }
                        source: dep("crate://self/resources/img/apple.png")
                    }
                }
                facebook_button = <RoundedView> {
                    width: Fit,
                    height: Fit,
                    cursor: Hand,
                    <Image> {
                        width: 21, height: 21, margin: { left: 10.0  }
                        source: dep("crate://self/resources/img/facebook.png")
                    }
                }
                github_button = <RoundedView> {
                    width: Fit,
                    height: Fit,
                    cursor: Hand,
                    <Image> {
                        width: 21, height: 21, margin: { left: 10.0  }
                        source: dep("crate://self/resources/img/github.png")
                    }
                }
                gitlab_button = <RoundedView> {
                    width: Fit,
                    height: Fit,
                    cursor: Hand,
                    <Image> {
                        width: 21, height: 21, margin: { left: 10.0  }
                        source: dep("crate://self/resources/img/gitlab.png")
                    }
                }
                google_button = <RoundedView> {
                    width: Fit,
                    height: Fit,
                    cursor: Hand,
                    <Image> {
                        width: 21, height: 21, margin: { left: 10.0  }
                        source: dep("crate://self/resources/img/google.png")
                    }
                }
            }
            status_label = <Label> {
                width: 250, height: Fit
                padding: {left: 5, right: 5, top: 10, bottom: 10}
                draw_text: {
                    color: (MESSAGE_TEXT_COLOR)
                    text_style: <REGULAR_TEXT> {}
                }
                text: ""
            }

            <Label> {
                width: Fit, height: Fit
                draw_text: {
                    color: #x6c6c6c
                    text_style: <REGULAR_TEXT>{}
                }
                text: "Don't have an account?"
            }
            signup_button = <Button> {
                width: Fit, height: Fit
                margin: {top: -5}
                draw_text: {
                    // color: (MESSAGE_TEXT_COLOR)
                    fn get_color(self) -> vec4 {
                        return MESSAGE_TEXT_COLOR
                    }
                    text_style: <REGULAR_TEXT>{}
                }
                draw_bg: {
                    bodybottom: #DDDDDD
                }

                text: "Sign up here"
            }
        }
    }
}

static MATRIX_SIGN_UP_URL: &str = "https://matrix.org/docs/chat_basics/matrix-for-im/#creating-a-matrix-account";

// An unfortunate hack we must do to get the colors to work in Rust code.
const COLOR_DANGER_RED: Vec4 = Vec4 { x: 220f32/255f32, y: 0f32, z: 5f32/255f32, w: 1f32 };
const COLOR_ACCEPT_GREEN: Vec4 = Vec4 { x: 19f32/255f32, y: 136f32/255f32, z: 8f32/255f32, w: 1f32 };
const MESSAGE_TEXT_COLOR: Vec4 = Vec4 { x: 68f32/255f32, y: 68f32/255f32, z: 68f32/255f32, w: 1f32 };

#[derive(Live, LiveHook, Widget)]
pub struct LoginScreen {
    #[deref] view: View,
}


impl Widget for LoginScreen {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        self.match_event(cx, event);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl MatchEvent for LoginScreen {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        let status_label = self.view.label(id!(status_label));
        let login_button = self.view.button(id!(login_button));
        let signup_button = self.view.button(id!(signup_button));
        let user_id_input = self.view.text_input(id!(user_id_input));
        let password_input = self.view.text_input(id!(password_input));
        let homeserver_input = self.view.text_input(id!(homeserver_input));

        if signup_button.clicked(actions) {
            let _ = robius_open::Uri::new(MATRIX_SIGN_UP_URL).open();
        }

        if login_button.clicked(actions) || user_id_input.returned(actions).is_some() || password_input.returned(actions).is_some() || homeserver_input.returned(actions).is_some(){
            let user_id = user_id_input.text();
            let password = password_input.text();
            let homeserver = homeserver_input.text();
            if user_id.is_empty() || password.is_empty() {
                status_label.apply_over(cx, live!{
                    draw_text: { color: (COLOR_DANGER_RED) }
                });
                status_label.set_text("Please enter both User ID and Password.");
            } else {
                status_label.apply_over(cx, live!{
                    draw_text: { color: (MESSAGE_TEXT_COLOR) }
                });
                status_label.set_text("Waiting for login response...");
                submit_async_request(MatrixRequest::Login(LoginRequest {
                    user_id,
                    password,
                    homeserver: homeserver.is_empty().not().then(|| homeserver),
                }));
            }
            self.redraw(cx);
        }

        for action in actions {
            match action.downcast_ref() {
                Some(LoginAction::AutofillInfo { .. }) => {
                    todo!("set user_id, password, and homeserver inputs");
                }
                Some(LoginAction::Status(status)) => {
                    status_label.set_text(status);
                    status_label.apply_over(cx, live!{
                        draw_text: { color: (MESSAGE_TEXT_COLOR) }
                    });
                    self.redraw(cx);
                }
                Some(LoginAction::LoginSuccess) => {
                    // The other real action of showing the main screen
                    // is handled by the main app, not by this login screen.
                    user_id_input.set_text("");
                    password_input.set_text("");
                    homeserver_input.set_text("");
                    status_label.set_text("Login successful!");
                    status_label.apply_over(cx, live!{
                        draw_text: { color: (COLOR_ACCEPT_GREEN) }
                    });
                    self.redraw(cx);
                }
                Some(LoginAction::LoginFailure(error)) => {
                    status_label.set_text(error);
                    status_label.apply_over(cx, live!{
                        draw_text: { color: (COLOR_DANGER_RED) }
                    });
                    self.redraw(cx);
                }
                Some(LoginAction::SsoPending(ref pending)) => {
                    if *pending {
                        self.view.view(id!(sso_view)).set_visible(false);
                    } else {
                        self.view.view(id!(sso_view)).set_visible(true);
                    }
                    self.redraw(cx);
                }
                _ => {

                }
            }
        }
        if let Some(_) = self.view.view(id!(apple_button)).finger_down(&actions) {
            let matrix_req = MatrixRequest::SSO { id: String::from("oidc-apple") };
            crate::sliding_sync::submit_async_request(matrix_req);
        }
        if let Some(_) = self.view.view(id!(facebook_button)).finger_down(&actions) {
            let matrix_req = MatrixRequest::SSO { id: String::from("oidc-facebook") };
            crate::sliding_sync::submit_async_request(matrix_req);
        }
        if let Some(_) = self.view.view(id!(github_button)).finger_down(&actions) {
            let matrix_req = MatrixRequest::SSO { id: String::from("oidc-github") };
            crate::sliding_sync::submit_async_request(matrix_req);
        }
        if let Some(_) = self.view.view(id!(gitlab_button)).finger_down(&actions) {
            let matrix_req = MatrixRequest::SSO { id: String::from("oidc-gitlab") };
            crate::sliding_sync::submit_async_request(matrix_req);
        }
        if let Some(_) = self.view.view(id!(google_button)).finger_down(&actions) {
            let matrix_req = MatrixRequest::SSO { id: String::from("oidc-google") };
            crate::sliding_sync::submit_async_request(matrix_req);
        }
    }

}

/// Actions sent to or from the login screen.
#[derive(Clone, DefaultNone, Debug)]
pub enum LoginAction {
    /// A positive response from the backend Matrix task to the login screen.
    ///
    /// This is not handled by the login screen itself, but by the main app.
    LoginSuccess,
    /// A negative response from the backend Matrix task to the login screen.
    LoginFailure(String),
    /// A status message to display to the user.
    Status(String),
    /// Login credentials that should be filled into the login screen,
    /// which get sent from the main function that parses CLI arguments.
    AutofillInfo {
        user_id: String,
        password: String,
        homeserver: Option<String>,
    },
    SsoPending(bool),
    None,
}
