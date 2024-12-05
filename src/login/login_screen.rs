use std::ops::Not;

use makepad_widgets::*;
use matrix_sdk::ruma::{api::client::session::get_login_types::v3::IdentityProvider, UserId};

use crate::sliding_sync::{submit_async_request, LoginByPassword, LoginRequest, MatrixRequest};

live_design! {
    import makepad_widgets::base::*;
    import makepad_widgets::theme_desktop_dark::*;
    import makepad_draw::shader::std::*;

    import crate::shared::styles::*;
    import crate::shared::icon_button::*;

    IMG_APP_LOGO = dep("crate://self/resources/robrix_logo_alpha.png")
    ICON_SEARCH = dep("crate://self/resources/icons/search.svg")

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
                let gray = dot(color.rgb, vec3(0.299, 0.587, 0.114));
                let grayed = mix(color, vec4(gray, gray, gray, color.a), self.mask);
                return grayed;
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
            width: Fit, height: Fill
            flow: Down
            align: {x: 0.5, y: 0.5}

            show_bg: true,
            draw_bg: {
                color: (COLOR_SECONDARY)
            }

            logo_image = <Image> {
                fit: Smallest,
                width: 80
                source: (IMG_APP_LOGO),
            }

            components_required_for_login = <View> {
                visible: false,
                width: Fit, height: Fit
                flow: Down
                align: {x: 0.5, y: 0.5}
                padding: 30
                spacing: 15.0

                show_bg: true,
                draw_bg: {
                    color: (COLOR_SECONDARY)
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

                <View> {
                    width: Fit, height: Fit,
                    flow: Right,
                    homeserver_input = <LoginTextInput> {
                        width: 220, height: 40
                        margin: {bottom: -10}
                        empty_message: "matrix.org"
                        draw_text: {
                            text_style: <TITLE_TEXT>{font_size: 9.0}
                        }
                    }
                    sso_search_button = <RobrixIconButton> {
                        width: 25, height: 25,
                        margin: {top: 5, left: 5 }
                        draw_icon: {
                            svg_file: (ICON_SEARCH)
                        }
                        icon_walk: {width: 16, height: 16, margin: {left: -2, right: -1} }
                    }
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

            status_label = <Label> {
                width: 250, height: Fit
                padding: {left: 5, right: 5, top: 10, bottom: 10}
                draw_text: {
                    color: (MESSAGE_TEXT_COLOR)
                    text_style: <REGULAR_TEXT> {}
                }
                text: ""
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
    #[rust]
    sso_pending: bool,
    #[rust]
    prev_homeserver_url: Option<String>,
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
        let components_required_for_login = self.view(id!(components_required_for_login));
        let status_label = self.view.label(id!(status_label));
        let login_button = self.view.button(id!(login_button));
        let signup_button = self.view.button(id!(signup_button));
        let user_id_input = self.view.text_input(id!(user_id_input));
        let password_input = self.view.text_input(id!(password_input));
        let homeserver_input = self.view.text_input(id!(homeserver_input));
        let sso_search_button = self.view.button(id!(sso_search_button));

        if signup_button.clicked(actions) {
            log!("Opening URL \"{}\"", MATRIX_SIGN_UP_URL);
            let _ = robius_open::Uri::new(MATRIX_SIGN_UP_URL).open();
        }

        if login_button.clicked(actions) || user_id_input.returned(actions).is_some() || password_input.returned(actions).is_some() || homeserver_input.returned(actions).is_some(){
            let user_id = user_id_input.text();
            let password = password_input.text();
            let homeserver = homeserver_input.text();
            let is_valid_user_id = UserId::parse(&user_id).is_ok();
            if user_id.is_empty() || password.is_empty() {
                status_label.apply_over(cx, live!{
                    draw_text: { color: (COLOR_DANGER_RED) }
                });
                status_label.set_text("Please enter both User ID and Password.");
            } else if !is_valid_user_id {
                status_label.apply_over(cx, live!{
                    draw_text: { color: (COLOR_DANGER_RED) }
                });
                status_label.set_text("User ID is invalid");
            } else {
                status_label.apply_over(cx, live!{
                    draw_text: { color: (MESSAGE_TEXT_COLOR) }
                });
                status_label.set_text("Waiting for login response...");
                submit_async_request(MatrixRequest::Login(LoginRequest::LoginByPassword(LoginByPassword {
                    user_id,
                    password,
                    homeserver: homeserver.is_empty().not().then_some(homeserver),
                })));
            }
            sso_search_button.set_enabled(self.prev_homeserver_url == Some(homeserver_input.text()));
            self.redraw(cx);
        }

        let provider_brands = ["apple", "facebook", "github", "gitlab", "google"];
        let button_set: &[&[LiveId]] = ids!(apple_button, facebook_button, github_button, gitlab_button, google_button);
        for action in actions {
            match action.downcast_ref() {
                Some(LoginAction::SessionFileExists) => {
                    components_required_for_login.set_visible_and_redraw(cx, false)
                }
                Some(LoginAction::ProcessSessionFailure) => {
                    components_required_for_login.set_visible_and_redraw(cx, true)
                }
                Some(LoginAction::AutofillInfo { .. }) => {
                    todo!("set user_id, password, and homeserver inputs");
                }
                Some(LoginAction::Status(status)) => {
                    status_label.set_text(status);
                    status_label.apply_over(cx, live!{
                        draw_text: { color: (MESSAGE_TEXT_COLOR) }
                    });
                    sso_search_button.set_enabled(true);
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
                    let truncated_error = error.chars().take(200).collect::<String>();
                    status_label.set_text(&truncated_error);
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
                            view_ref.apply_over(cx,
                                live! {
                                    cursor: NotAllowed,
                                    image = {
                                        draw_bg: {
                                            mask: (1.0)
                                        }
                                    }
                                }
                            );
                        });
                    } else {
                        self.view_set(button_set).iter().for_each(|view_ref| {
                            let Some(mut view_ref) = view_ref.borrow_mut() else {
                                return;
                            };
                            view_ref.apply_over(cx,
                                live! {
                                    cursor: Hand,
                                    image = {
                                        draw_bg: {
                                            mask: (0.0)
                                        }
                                    }
                                },
                            );
                        });
                    }
                    self.sso_pending = *pending;
                    self.redraw(cx);
                }
                Some(LoginAction::IdentityProvider(identity_providers)) => {
                    for (view_ref, brand) in self.view_set(button_set).iter().zip(&provider_brands) {
                        for ip in identity_providers.iter() {
                            if ip.id.contains(brand) {
                                view_ref.set_visible(true);
                                break;
                            }
                        }
                    }
                    self.identity_providers = identity_providers.clone();
                    sso_search_button.set_enabled(true);
                    status_label.set_text("");
                    self.redraw(cx);
                }
                _ => {

                }
            }
        }
        if sso_search_button.clicked(actions) && self.prev_homeserver_url != Some(homeserver_input.text()) {
            self.prev_homeserver_url = Some(homeserver_input.text());
            status_label.set_text("Fetching support login types from the homeserver...");
            submit_async_request(MatrixRequest::Login(LoginRequest::HomeserverLoginTypesQuery(homeserver_input.text())));
            sso_search_button.set_enabled(false);
            for view_ref in self.view_set(button_set).iter() {
                view_ref.set_visible(false);
            }
            self.redraw(cx);
        }
        for (view_ref, brand) in self.view_set(button_set).iter().zip(&provider_brands) {
            for ip in self.identity_providers.iter() {
                if ip.id.contains(brand) {
                    if view_ref.finger_up(actions).is_some() && !self.sso_pending {
                        submit_async_request(MatrixRequest::SpawnSSOServer{
                            identity_provider_id: ip.id.clone(),
                            brand: brand.to_string(),
                            homeserver_url: homeserver_input.text()
                        });
                    }
                    break;
                }
            }
        }
    }

}

/// Actions sent to or from the login screen.
#[derive(Clone, DefaultNone, Debug)]
pub enum LoginAction {
    /// Reshow the components required for login via it.
    ProcessSessionFailure,
    /// Hide the components required for login via it.
    SessionFileExists,
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
