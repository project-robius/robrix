use std::ops::Not;

use makepad_widgets::*;
use url::Url;

use crate::sliding_sync::{submit_async_request, LoginByPassword, LoginRequest, MatrixRequest};

use super::login_status_modal::{LoginStatusModalAction, LoginStatusModalWidgetExt};

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::helpers::*;
    use crate::shared::styles::*;
    use crate::shared::icon_button::*;
    use crate::login::login_status_modal::*;

    IMG_APP_LOGO = dep("crate://self/resources/robrix_logo_alpha.png")
    ICON_SEARCH = dep("crate://self/resources/icons/search.svg")

    SsoButton = <RoundedView> {
        width: Fit,
        height: Fit,
        cursor: Hand,
        visible: true,
        padding: 10,
        margin: { left: 16.6, right: 16.6, top: 10, bottom: 10}
        draw_bg: {
            border_size: 0.5,
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


    pub LoginScreen = {{LoginScreen}} {
        width: Fill, height: Fill,
        align: {x: 0.5, y: 0.5}
        show_bg: true,
        draw_bg: {
            color: #FFF
        }

        <ScrollXYView> {
            width: Fit, height: Fill,
            // Note: *do NOT* vertically center this, it will break scrolling.
            align: {x: 0.5}
            show_bg: true,
            draw_bg: {
                color: (COLOR_PRIMARY)
            }
        
            <RoundedView> {
                margin: 40
                width: Fit, height: Fit
                align: {x: 0.5, y: 0.5}
                flow: Overlay,

                show_bg: true,
                draw_bg: {
                    color: (COLOR_SECONDARY)
                    border_radius: 6.0
                }

                <View> {
                    width: Fit, height: Fit
                    flow: Down
                    align: {x: 0.5, y: 0.5}
                    padding: 30
                    margin: 40
                    spacing: 15.0

                    logo_image = <Image> {
                        fit: Smallest,
                        width: 80
                        source: (IMG_APP_LOGO),
                    }

                    title = <Label> {
                        width: Fit, height: Fit
                        margin: { bottom: 5 }
                        padding: 0,
                        draw_text: {
                            color: (COLOR_TEXT)
                            text_style: <TITLE_TEXT>{font_size: 16.0}
                        }
                        text: "Login to Robrix"
                    }

                    user_id_input = <RobrixTextInput> {
                        width: 250, height: Fit
                        flow: Right, // do not wrap
                        padding: 10,
                        empty_text: "User ID"
                    }

                    password_input = <RobrixTextInput> {
                        width: 250, height: Fit
                        flow: Right, // do not wrap
                        padding: 10,
                        empty_text: "Password"
                        is_password: true,
                    }

                    <View> {
                        width: 250, height: Fit,
                        flow: Down,

                        homeserver_input = <RobrixTextInput> {
                            width: Fill, height: Fit,
                            flow: Right, // do not wrap
                            padding: {top: 3, bottom: 3}
                            empty_text: "matrix.org"
                            draw_text: {
                                text_style: <TITLE_TEXT>{font_size: 10.0}
                            }
                        }

                        <View> {
                            width: 250,
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
                                padding: 0
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
                    

                    login_button = <RobrixIconButton> {
                        width: 250,
                        height: 40
                        padding: 10
                        margin: {top: 5, bottom: 10}
                        align: {x: 0.5, y: 0.5}
                        draw_bg: {
                            color: (COLOR_ACTIVE_PRIMARY)
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
                        padding: 0,
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
                        twitter_button = <SsoButton> {
                            image = <SsoImage> {
                                source: dep("crate://self/resources/img/x.png")
                            }
                        }
                    }

                    <View> {
                        width: 250,
                        height: Fit,
                        flow: Right,
                        // padding: 3,
                        spacing: 0.0,
                        align: {x: 0.5, y: 0.5} // center horizontally and vertically

                        left_line = <LineH> {
                            draw_bg: { color: #C8C8C8 }
                        }

                        <Label> {
                            width: Fit, height: Fit
                            padding: {left: 1, right: 1, top: 0, bottom: 0}
                            draw_text: {
                                color: #x6c6c6c
                                text_style: <REGULAR_TEXT>{}
                            }
                            text: "Don't have an account?"
                        }

                        right_line = <LineH> {
                            draw_bg: { color: #C8C8C8 }
                        }
                    }
                    
                    signup_button = <RobrixIconButton> {
                        width: Fit, height: Fit
                        padding: {left: 15, right: 15, top: 10, bottom: 10}
                        margin: {bottom: 5}
                        align: {x: 0.5, y: 0.5}
                        draw_bg: {
                            color: (COLOR_ACTIVE_PRIMARY)
                        }
                        draw_text: {
                            color: (COLOR_PRIMARY)
                            text_style: <REGULAR_TEXT> {}
                        }

                        text: "Sign up here"
                    }
                }

                // The modal that pops up to display login status messages,
                // such as when the user is logging in or when there is an error.
                login_status_modal = <Modal> {
                    // width: Fit, height: Fit,
                    // align: {x: 0.5, y: 0.5},
                    can_dismiss: false,
                    content: {
                        login_status_modal_inner = <LoginStatusModal> {}
                    }
                }
            }
        }
    }
}

static MATRIX_SIGN_UP_URL: &str = "https://matrix.org/docs/chat_basics/matrix-for-im/#creating-a-matrix-account";

#[derive(Live, LiveHook, Widget)]
pub struct LoginScreen {
    #[deref] view: View,
    /// Boolean to indicate if the SSO login process is still in flight
    #[rust] sso_pending: bool,
    /// The URL to redirect to after logging in with SSO.
    #[rust] sso_redirect_url: Option<String>,
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
        let login_button = self.view.button(ids!(login_button));
        let signup_button = self.view.button(ids!(signup_button));
        let user_id_input = self.view.text_input(ids!(user_id_input));
        let password_input = self.view.text_input(ids!(password_input));
        let homeserver_input = self.view.text_input(ids!(homeserver_input));

        let login_status_modal = self.view.modal(ids!(login_status_modal));
        let login_status_modal_inner = self.view.login_status_modal(ids!(login_status_modal_inner));

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
            if user_id.is_empty() {
                login_status_modal_inner.set_title(cx, "Missing User ID");
                login_status_modal_inner.set_status(cx, "Please enter a valid User ID.");
                login_status_modal_inner.button_ref().set_text(cx, "Okay");
            } else if password.is_empty() {
                login_status_modal_inner.set_title(cx, "Missing Password");
                login_status_modal_inner.set_status(cx, "Please enter a valid password.");
                login_status_modal_inner.button_ref().set_text(cx, "Okay");
            } else {
                login_status_modal_inner.set_title(cx, "Logging in...");
                login_status_modal_inner.set_status(cx, "Waiting for a login response...");
                login_status_modal_inner.button_ref().set_text(cx, "Cancel");
                submit_async_request(MatrixRequest::Login(LoginRequest::LoginByPassword(LoginByPassword {
                    user_id,
                    password,
                    homeserver: homeserver.is_empty().not().then_some(homeserver),
                })));
            }
            login_status_modal.open(cx);
            self.redraw(cx);
        }
        
        let provider_brands = ["apple", "facebook", "github", "gitlab", "google", "twitter"];
        let button_set: &[&[LiveId]] = ids_array!(
            apple_button, 
            facebook_button, 
            github_button, 
            gitlab_button, 
            google_button, 
            twitter_button
        );
        for action in actions {
            if let LoginStatusModalAction::Close = action.as_widget_action().cast() {
                login_status_modal.close(cx);
            }

            // Handle login-related actions received from background async tasks.
            match action.downcast_ref() {
                Some(LoginAction::CliAutoLogin { user_id, homeserver }) => {
                    user_id_input.set_text(cx, user_id);
                    password_input.set_text(cx, "");
                    homeserver_input.set_text(cx, homeserver.as_deref().unwrap_or_default());
                    login_status_modal_inner.set_title(cx, "Logging in via CLI...");
                    login_status_modal_inner.set_status(
                        cx,
                        &format!("Auto-logging in as user {user_id}...")
                    );
                    let login_status_modal_button = login_status_modal_inner.button_ref();
                    login_status_modal_button.set_text(cx, "Cancel");
                    login_status_modal_button.set_enabled(cx, false); // Login cancel not yet supported
                    login_status_modal.open(cx);
                }
                Some(LoginAction::Status { title, status }) => {
                    login_status_modal_inner.set_title(cx, title);
                    login_status_modal_inner.set_status(cx, status);
                    let login_status_modal_button = login_status_modal_inner.button_ref();
                    login_status_modal_button.set_text(cx, "Cancel");
                    login_status_modal_button.set_enabled(cx, true);
                    login_status_modal.open(cx);
                    self.redraw(cx);
                }
                Some(LoginAction::LoginSuccess) => {
                    // The main `App` component handles showing the main screen
                    // and hiding the login screen & login status modal.
                    user_id_input.set_text(cx, "");
                    password_input.set_text(cx, "");
                    homeserver_input.set_text(cx, "");
                    login_status_modal.close(cx);
                    self.redraw(cx);
                }
                Some(LoginAction::LoginFailure(error)) => {
                    login_status_modal_inner.set_title(cx, "Login Failed.");
                    login_status_modal_inner.set_status(cx, error);
                    let login_status_modal_button = login_status_modal_inner.button_ref();
                    login_status_modal_button.set_text(cx, "Okay");
                    login_status_modal_button.set_enabled(cx, true);
                    login_status_modal.open(cx);
                    self.redraw(cx);
                }
                Some(LoginAction::SsoPending(pending)) => {
                    for view_ref in self.view_set(button_set).iter() {
                        let Some(mut view_mut) = view_ref.borrow_mut() else { continue };
                        if *pending {
                            view_mut.apply_over(cx, live! {
                                cursor: NotAllowed,
                                image = { draw_bg: { mask: 1.0 } }
                            });
                        } else {
                            view_mut.apply_over(cx, live! {
                                cursor: Hand,
                                image = { draw_bg: { mask: 0.0 } }
                            });
                        }
                    }
                    self.sso_pending = *pending;
                    self.redraw(cx);
                }
                Some(LoginAction::SsoSetRedirectUrl(url)) => {
                    self.sso_redirect_url = Some(url.to_string());
                }
                _ => { }
            }
        }

        // If the Login SSO screen's "cancel" button was clicked, send a http request to gracefully shutdown the SSO server
        if let Some(sso_redirect_url) = &self.sso_redirect_url {
            let login_status_modal_button = login_status_modal_inner.button_ref();
            if login_status_modal_button.clicked(actions) {
                let request_id = id!(SSO_CANCEL_BUTTON);
                let request = HttpRequest::new(format!("{}/?login_token=",sso_redirect_url), HttpMethod::GET);
                cx.http_request(request_id, request);
                self.sso_redirect_url = None;
            }
        }

        // Handle any of the SSO login buttons being clicked
        for (view_ref, brand) in self.view_set(button_set).iter().zip(&provider_brands) {
            if view_ref.finger_up(actions).is_some() && !self.sso_pending {
                submit_async_request(MatrixRequest::SpawnSSOServer{
                    identity_provider_id: format!("oidc-{}",brand),
                    brand: brand.to_string(),
                    homeserver_url: homeserver_input.text()
                });
            }
        }
    }

}

/// Actions sent to or from the login screen.
#[derive(Clone, DefaultNone, Debug)]
pub enum LoginAction {
    /// A positive response from the backend Matrix task to the login screen.
    LoginSuccess,
    /// A negative response from the backend Matrix task to the login screen.
    LoginFailure(String),
    /// A login-related status message to display to the user.
    Status {
        title: String,
        status: String,
    },
    /// The given login info was specified on the command line (CLI),
    /// and the login process is underway.
    CliAutoLogin {
        user_id: String,
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
    /// Set the SSO redirect URL in the LoginScreen.
    ///
    /// When an SSO-based login is pendng, pressing the cancel button will send
    /// an HTTP request to this SSO server URL to gracefully shut it down.
    SsoSetRedirectUrl(Url),
    None,
}
