use std::ops::Not;

use makepad_widgets::*;
use matrix_sdk::ruma::api::client::session::get_login_types::v3::IdentityProvider;

use crate::sliding_sync::{submit_async_request, LoginByPassword, LoginRequest, MatrixRequest};

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::helpers::*;
    use crate::shared::styles::*;
    use crate::shared::icon_button::*;

    IMG_APP_LOGO = dep("crate://self/resources/robrix_logo_alpha.png")
    ICON_SEARCH = dep("crate://self/resources/icons/search.svg")

    SsoButton = <RoundedView> {
        width: Fit,
        height: Fit,
        cursor: Hand,
        visible: true,
        padding: 10,
        // margin: 10,
        margin: { left: 15.5, right: 15.5, top: 10, bottom: 10}
        draw_bg: {
            border_width: 0.5,
            border_color: (#6c6c6c),
            color: (COLOR_PRIMARY)
        }
    }
    SsoImage = <Image> {
        width: 30, height: 30,
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

    pub LoginScreen = {{LoginScreen}}<ScrollXYView> {
        width: Fill, height: Fill
        show_bg: true,
        draw_bg: {
            color: (COLOR_PRIMARY)
        }
        // Note: *do NOT* vertically center this, it will break scrolling.
        align: {x: 0.5}

        <RoundedView> {
            width: Fit, height: Fit
            flow: Down
            align: {x: 0.5, y: 0.5}
            padding: 30
            margin: 40
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

            user_id_input = <RobrixTextInput> {
                width: 250, height: 40
                empty_message: "User ID"
            }

            password_input = <RobrixTextInput> {
                width: 250, height: 40
                empty_message: "Password"
                draw_text: { text_style: { is_secret: true } }
            }

            <View> {
                width: 250, height: Fit,
                align: {x: 0.5}
                flow: Right,
                <View> {
                    width: 215, height: Fit,
                    flow: Down,

                    homeserver_input = <RobrixTextInput> {
                        width: 215, height: 30,
                        empty_message: "matrix.org"
                        draw_text: {
                            text_style: <TITLE_TEXT>{font_size: 10.0}
                        }
                    }

                    <View> {
                        width: 215,
                        height: Fit,
                        flow: Right,
                        padding: {top: 3, left: 2, right: 2}
                        spacing: 0.0,
                        align: {x: 0.5, y: 0.5} // center horizontally and vertically

                        left_line = <LineH> {
                            draw_bg: { color: #C8C8C8 }
                        }

                        <Label> {
                            width: Fit, height: Fit
                            draw_text: {
                                color: #8C8C8C
                                text_style: <REGULAR_TEXT>{font_size: 9}
                            }
                            text: "Homeserver URL (optional)"
                        }

                        right_line = <LineH> {
                            draw_bg: { color: #C8C8C8 }
                        }
                    }

                }
                sso_search_button = <RobrixIconButton> {
                    width: 28, height: 28,
                    margin: { left: 5, top: 1}
                    draw_icon: {
                        svg_file: (ICON_SEARCH)
                    }
                    icon_walk: {width: 16, height: 16, margin: {left: -2, right: -1} }
                }
            }

            login_button = <RobrixIconButton> {
                width: 250, height: 40
                margin: {top: 5, bottom: 10}
                draw_bg: {
                    color: (COLOR_SELECTED_PRIMARY)
                }
                draw_text: {
                    color: (COLOR_PRIMARY)
                    text_style: <REGULAR_TEXT> {}
                }
                text: "Login"
            }

            left_line = <LineH> {
                margin: {bottom: -5}
                draw_bg: { color: #C8C8C8 }
            }
            <Label> {
                width: Fit, height: Fit
                draw_text: {
                    color: (COLOR_TEXT)
                    text_style: <TITLE_TEXT>{font_size: 11.0}
                }
                text: "Or, login with an SSO provider:"
            }

            sso_view = <View> {
                align: {x: 0.5}
                width: 250, height: Fit,
                margin: {left: 5, right: 5} // make the inner view 240 pixels wide
                flow: RightWrap,
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
                width: 250, height: Fit,
                padding: {left: 5, right: 5, bottom: 10}
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
                margin: {top: -10}
                padding: {left: 15, right: 15, top: 10, bottom: 10}
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

        if login_button.clicked(actions)
            || user_id_input.returned(actions).is_some()
            || password_input.returned(actions).is_some()
            || homeserver_input.returned(actions).is_some()
        {
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
