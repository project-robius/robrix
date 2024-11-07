use std::ops::Not;

use makepad_widgets::*;
use matrix_sdk::ruma::api::client::session::get_login_types::v3::IdentityProvider;

use crate::sliding_sync::{submit_async_request, LoginByPassword, LoginRequest, MatrixRequest};

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
    SsoButton = <RoundedView> {
        width: Fit,
        height: Fit,
        cursor: Hand,
        visible: false,
        padding: 10.0,
        draw_bg: {
            border_width: 1.0,
            border_color: (#6c6c6c),
            color: (COLOR_PRIMARY)
        }
    }
    SsoImage = <Image> {
        width: 21, height: 21
        draw_bg:{
            uniform mask: 0.0
            fn pixel(self) -> vec4 {
                let color = sample2d(self.image, self.pos).xyzw;
                if self.mask >= 0.5 {
                    let gray =  dot(color, vec4(0.299, 0.587, 0.114, 0.4));
                    let diff = pow(max(gray, 0), 3)
                    return vec4(diff);
                } else {
                    return color;
                }
                
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
                draw_text: { text_style: { is_secret: true } }
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
                apple_button = <SsoButton> {
                    image = <SsoImage> {
                        source: dep("crate://self/resources/img/apple.png")
                    }
                }
                facebook_button = <SsoButton> {
                    image = <SsoImage> {
                        source: dep("crate://self/resources/img/facebook.png")
                    }
                }
                github_button = <SsoButton> {
                    image = <SsoImage> {
                        source: dep("crate://self/resources/img/github.png")
                    }
                }
                github_button = <SsoButton> {
                    image = <SsoImage> {
                        source: dep("crate://self/resources/img/github.png")
                    }
                }
                gitlab_button = <SsoButton> {
                    image = <SsoImage> {
                        source: dep("crate://self/resources/img/gitlab.png")
                    }
                }
                google_button = <SsoButton> {
                    image = <SsoImage> {
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
        
        sso_button_pending_template: <RoundedView> {
            cursor: NotAllowed,
            draw_bg: {
                border_width: 1.0,
                border_color: (#6c6c6c),
                color: (#6c6c6c)
            }
        }
        sso_image_pending_template: <Image> {
            width: 21, height: 21
            draw_bg:{
                uniform mask: 1.0
            }
        }
        sso_button_ok_template: <RoundedView> {
            cursor: Hand,
            draw_bg: {
                border_width: 1.0,
                border_color: (#6c6c6c),
                color: (COLOR_PRIMARY)
            }
        }
        sso_image_ok_template: <Image> {
            width: 21, height: 21
            draw_bg:{
                uniform mask: 0.0
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
    #[rust]
    identity_providers: Vec<IdentityProvider>,
    #[live]
    sso_button_pending_template: Option<LivePtr>,
    #[live]
    sso_button_ok_template: Option<LivePtr>,
    #[live]
    sso_image_pending_template: Option<LivePtr>,
    #[live]
    sso_image_ok_template: Option<LivePtr>,
    #[rust]
    sso_pending:bool
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
                submit_async_request(MatrixRequest::Login(LoginRequest::LoginByPassword(LoginByPassword {
                    user_id,
                    password,
                    homeserver: homeserver.is_empty().not().then(|| homeserver),
                })));
            }
            self.redraw(cx);
        }
        let button_vec = vec!["apple","facebook","github","gitlab","google"];
        let button_set: &[&[LiveId]] = ids!(apple_button,facebook_button,github_button,gitlab_button,google_button);
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
                        self.view_set(button_set).iter().for_each(|view_ref| {
                            let Some(mut view_ref) = view_ref.borrow_mut() else {
                                return;
                            };
                            view_ref.apply_from_ptr(cx, self.sso_button_pending_template);
                            view_ref.image(id!(image)).apply_from_ptr(cx, self.sso_image_pending_template);
                        });
                    } else {
                        self.view_set(button_set).iter().for_each(|view_ref| {
                            let Some(mut view_ref) = view_ref.borrow_mut() else {
                                return;
                            };
                            view_ref.apply_from_ptr(cx, self.sso_button_ok_template);
                            view_ref.image(id!(image)).apply_from_ptr(cx, self.sso_image_ok_template);
                        });
                    }
                    self.sso_pending = *pending;
                    self.redraw(cx);
                }
                Some(LoginAction::IdentityProvider(identity_providers)) => {
                    let mut button_iter = button_vec.iter();
                    for view_ref in self.view_set(button_set).iter() {
                        if let Some(brand) = button_iter.next() {
                            for ip in identity_providers.iter() {
                                if ip.id.contains(brand) {
                                    view_ref.set_visible(true);
                                    break;
                                }
                            }
                        }
                    }
                    self.identity_providers = identity_providers.clone();
                    self.redraw(cx);
                }
                _ => {

                }
            }
        }
        let mut button_iter = button_vec.iter();
        for v in self.view_set(button_set).iter() {
            if let Some(brand) = button_iter.next() {
                for ip in self.identity_providers.iter() {
                    if ip.id.contains(brand) {
                        if let Some(_) = v.finger_up(&actions) {
                            if !self.sso_pending {
                                let matrix_req = MatrixRequest::Login(LoginRequest::LoginBySSO(ip.id.clone()));
                                crate::sliding_sync::submit_async_request(matrix_req);
                            }
                        }
                        break;
                    }
                }
            }
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
    /// An acknowledgment that is sent from the backend Matrix task to the login screen
    /// informing it that the SSO login process is either still in flight (`true`) or has finished (`false`).
    ///
    /// Note that an inner value of `false` does *not* imply that the login request has
    /// successfully finished. 
    /// The login screen can use this to prevent the user from submitting
    /// additional SSO login requests while a previous request is in flight. 
    SsoPending(bool),
    /// A list of SSO identity providers supported by the homeserver.
    ///
    /// This is sent from the backend async task to the login screen in order to
    /// inform the login screen which SSO identity providers it should display to the user.
    IdentityProvider(Vec<IdentityProvider>),
    None,
}
