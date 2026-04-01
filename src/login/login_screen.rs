use std::ops::Not;

use makepad_widgets::*;
use url::Url;

use crate::sliding_sync::{submit_async_request, AccountSwitchAction, LoginByPassword, LoginRequest, MatrixRequest, RegisterAccount};

use super::login_status_modal::{LoginStatusModalAction, LoginStatusModalWidgetExt};

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*


    mod.widgets.IMG_APP_LOGO = crate_resource("self://resources/robrix_logo_alpha.png")

    mod.widgets.SsoButton = RoundedView {
        width: Fit,
        height: Fit,
        cursor: MouseCursor.Hand,
        visible: true,
        padding: 10,
        margin: Inset{ left: 16.6, right: 16.6, top: 10, bottom: 10}
        draw_bg +: {
            border_size: 0.5
            border_color: #6c6c6c
            color: (COLOR_PRIMARY)
        }
    }

    mod.widgets.SsoImage = Image {
        width: 30, height: 30,
        draw_bg +: {
            mask: instance(0.0)
            pixel: fn() {
                let color = self.get_color();
                let gray = dot(color.rgb, vec3(0.299, 0.587, 0.114));
                let grayed = mix(color, vec4(gray, gray, gray, color.a), self.mask);
                return grayed;
            }
        }
    }


    mod.widgets.LoginScreen = set_type_default() do #(LoginScreen::register_widget(vm)) {
        ..mod.widgets.SolidView

        width: Fill, height: Fill,
        align: Align{x: 0.5, y: 0.5}
        show_bg: true,
        draw_bg +: {
            color: COLOR_SECONDARY
            // color: COLOR_PRIMARY // TODO: once Makepad supports `Fill {max: 375}`, change this back to COLOR_PRIMARY
        }

        ScrollYView {
            width: Fill, height: Fill,
            // Note: *do NOT* vertically center this, it will break scrolling.
            align: Align{x: 0.5}
            show_bg: true,
            draw_bg.color: (COLOR_SECONDARY)
            // draw_bg.color: (COLOR_PRIMARY) // TODO: once Makepad supports `Fill {max: 375}`, change this back to COLOR_PRIMARY
   
            // allow the view to be scrollable but hide the actual scroll bar
            scroll_bars: {
                scroll_bar_y: {
                    bar_size: 0.0
                    min_handle_size: 0.0
                }
            }

            View {
                margin: Inset{top: 40, bottom: 40}
                width: Fill // TODO: once Makepad supports it, use `Fill {max: 375}`
                height: Fit
                align: Align{x: 0.5, y: 0.5}
                flow: Overlay,

                View {
                    width: Fill // TODO: once Makepad supports it, use `Fill {max: 375}`
                    height: Fit
                    flow: Down
                    align: Align{x: 0.5, y: 0.5}
                    padding: Inset{top: 30, bottom: 30}
                    margin: Inset{top: 40, bottom: 40}
                    spacing: 15.0

                    logo_image := Image {
                        fit: ImageFit.Smallest,
                        width: 80
                        src: (mod.widgets.IMG_APP_LOGO),
                    }

                    title := Label {
                        width: Fit, height: Fit
                        margin: Inset{ bottom: 5 }
                        padding: 0,
                        draw_text +: {
                            color: (COLOR_TEXT)
                            text_style: TITLE_TEXT {font_size: 16.0}
                        }
                        text: "Login to Robrix"
                    }

                    user_id_input := RobrixTextInput {
                        width: 275, height: Fit
                        flow: Right, // do not wrap
                        padding: 10,
                        empty_text: "User ID"
                    }

                    password_input := RobrixTextInput {
                        width: 275, height: Fit
                        flow: Right, // do not wrap
                        padding: 10,
                        empty_text: "Password"
                        is_password: true,
                    }

                    confirm_password_wrapper := View {
                        width: 275, height: Fit,
                        visible: false,

                        confirm_password_input := RobrixTextInput {
                            width: 275, height: Fit
                            flow: Right, // do not wrap
                            padding: 10,
                            empty_text: "Confirm password"
                            is_password: true,
                        }
                    }

                    View {
                        width: 275, height: Fit,
                        flow: Down,

                        homeserver_input := RobrixTextInput {
                            width: 275, height: Fit,
                            flow: Right, // do not wrap
                            padding: Inset{top: 5, bottom: 5, left: 10, right: 10}
                            empty_text: "matrix.org"
                            draw_text +: {
                                text_style: TITLE_TEXT {font_size: 10.0}
                            }
                        }

                        View {
                            width: 275,
                            height: Fit,
                            flow: Right,
                            padding: Inset{top: 3, left: 2, right: 2}
                            spacing: 0.0,
                            align: Align{x: 0.5, y: 0.5} // center horizontally and vertically

                            LineH { draw_bg.color: #C8C8C8 }

                            Label {
                                width: Fit, height: Fit
                                padding: 0
                                draw_text +: {
                                    color: #8C8C8C
                                    text_style: REGULAR_TEXT {font_size: 9}
                                }
                                text: "Homeserver URL (optional)"
                            }

                            LineH { draw_bg.color: #C8C8C8 }
                        }
                    }
                    

                    login_button := RobrixIconButton {
                        width: 275,
                        height: 40
                        padding: 10
                        margin: Inset{top: 5, bottom: 10}
                        align: Align{x: 0.5, y: 0.5}
                        text: "Login"
                    }

                    login_only_view := View {
                        width: Fit, height: Fit,
                        flow: Down,
                        align: Align{x: 0.5, y: 0.5}
                        spacing: 15.0

                        LineH {
                            width: 275
                            margin: Inset{bottom: -5}
                            draw_bg.color: #C8C8C8
                        }

                        Label {
                            width: Fit, height: Fit
                            padding: 0,
                            draw_text +: {
                                color: (COLOR_TEXT)
                                text_style: TITLE_TEXT {font_size: 11.0}
                            }
                            text: "Or, login with an SSO provider:"
                        }

                        sso_view := View {
                            width: 275, height: Fit,
                            margin: Inset{left: 30, right: 5} // make the inner view 240 pixels wide
                            flow: Flow.Right{wrap: true},
                            apple_button := mod.widgets.SsoButton {
                                image := mod.widgets.SsoImage {
                                    src: crate_resource("self://resources/img/apple.png")
                                }
                            }
                            facebook_button := mod.widgets.SsoButton {
                                image := mod.widgets.SsoImage {
                                    src: crate_resource("self://resources/img/facebook.png")
                                }
                            }
                            github_button := mod.widgets.SsoButton {
                                image := mod.widgets.SsoImage {
                                    src: crate_resource("self://resources/img/github.png")
                                }
                            }
                            gitlab_button := mod.widgets.SsoButton {
                                image := mod.widgets.SsoImage {
                                    src: crate_resource("self://resources/img/gitlab.png")
                                }
                            }
                            google_button := mod.widgets.SsoButton {
                                image := mod.widgets.SsoImage {
                                    src: crate_resource("self://resources/img/google.png")
                                }
                            }
                            twitter_button := mod.widgets.SsoButton {
                                image := mod.widgets.SsoImage {
                                    src: crate_resource("self://resources/img/x.png")
                                }
                            }
                        }
                    }

                    View {
                        width: 275,
                        height: Fit,
                        flow: Right,
                        // padding: 3,
                        spacing: 0.0,
                        align: Align{x: 0.5, y: 0.5} // center horizontally and vertically

                        LineH { draw_bg.color: #C8C8C8 }

                        account_prompt_label := Label {
                            width: Fit, height: Fit
                            padding: Inset{left: 1, right: 1, top: 0, bottom: 0}
                            draw_text +: {
                                color: #x6c6c6c
                                text_style: REGULAR_TEXT {}
                            }
                            text: "Don't have an account?"
                        }

                        LineH { draw_bg.color: #C8C8C8 }
                    }
                    
                    mode_toggle_button := RobrixIconButton {
                        width: Fit, height: Fit
                        padding: Inset{left: 15, right: 15, top: 10, bottom: 10}
                        margin: Inset{bottom: 5}
                        align: Align{x: 0.5, y: 0.5}
                        text: "Sign up here"
                    }

                    // Cancel button for add-account mode (hidden by default)
                    cancel_button := RobrixIconButton {
                        width: Fit, height: Fit
                        padding: Inset{left: 15, right: 15, top: 10, bottom: 10}
                        margin: Inset{top: 10, bottom: 5}
                        align: Align{x: 0.5, y: 0.5}
                        text: "Cancel"
                        visible: false
                    }
                }

                // The modal that pops up to display login status messages,
                // such as when the user is logging in or when there is an error.
                login_status_modal := Modal {
                    // width: Fit, height: Fit,
                    // align: Align{x: 0.5, y: 0.5},
                    can_dismiss: false,
                    content +: {
                        login_status_modal_inner := mod.widgets.LoginStatusModal {}
                    }
                }
            }
        }
    }
}

#[derive(Script, ScriptHook, Widget)]
pub struct LoginScreen {
    #[source] source: ScriptObjectRef,
    #[deref] view: View,
    /// Whether the screen is showing the in-app sign-up flow.
    #[rust] signup_mode: bool,
    /// Boolean to indicate if the SSO login process is still in flight
    #[rust] sso_pending: bool,
    /// The URL to redirect to after logging in with SSO.
    #[rust] sso_redirect_url: Option<String>,
    /// The most recent login failure message shown to the user.
    #[rust] last_failure_message_shown: Option<String>,
    /// Boolean to indicate if we're in "add account" mode (adding another Matrix account).
    #[rust] adding_account: bool,
}

impl LoginScreen {
    fn set_signup_mode(&mut self, cx: &mut Cx, signup_mode: bool) {
        self.signup_mode = signup_mode;
        self.view.view(cx, ids!(confirm_password_wrapper)).set_visible(cx, signup_mode);
        self.view.view(cx, ids!(login_only_view)).set_visible(cx, !signup_mode);
        self.view.label(cx, ids!(title)).set_text(cx,
            if signup_mode { "Create your Robrix account" } else { "Login to Robrix" }
        );
        self.view.button(cx, ids!(login_button)).set_text(cx,
            if signup_mode { "Create account" } else { "Login" }
        );
        self.view.label(cx, ids!(account_prompt_label)).set_text(cx,
            if signup_mode { "Already have an account?" } else { "Don't have an account?" }
        );
        self.view.button(cx, ids!(mode_toggle_button)).set_text(cx,
            if signup_mode { "Back to login" } else { "Sign up here" }
        );

        if !signup_mode {
            self.view.text_input(cx, ids!(confirm_password_input)).set_text(cx, "");
        }

        self.redraw(cx);
    }
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
        let login_button = self.view.button(cx, ids!(login_button));
        let mode_toggle_button = self.view.button(cx, ids!(mode_toggle_button));
        let cancel_button = self.view.button(cx, ids!(cancel_button));
        let user_id_input = self.view.text_input(cx, ids!(user_id_input));
        let password_input = self.view.text_input(cx, ids!(password_input));
        let confirm_password_input = self.view.text_input(cx, ids!(confirm_password_input));
        let homeserver_input = self.view.text_input(cx, ids!(homeserver_input));

        let login_status_modal = self.view.modal(cx, ids!(login_status_modal));
        let login_status_modal_inner = self.view.login_status_modal(cx, ids!(login_status_modal_inner));

        // Handle cancel button for add-account mode
        if cancel_button.clicked(actions) {
            self.adding_account = false;
            // Reset the UI back to normal login mode
            self.view.label(cx, ids!(title)).set_text(cx, "Login to Robrix");
            cancel_button.set_visible(cx, false);
            self.view.view(cx, ids!(sso_view)).set_visible(cx, true);
            mode_toggle_button.set_visible(cx, true);
            cx.action(LoginAction::CancelAddAccount);
            self.redraw(cx);
        }

        if mode_toggle_button.clicked(actions) {
            self.set_signup_mode(cx, !self.signup_mode);
        }

        if login_button.clicked(actions)
            || user_id_input.returned(actions).is_some()
            || password_input.returned(actions).is_some()
            || (self.signup_mode && confirm_password_input.returned(actions).is_some())
            || homeserver_input.returned(actions).is_some()
        {
            let user_id = user_id_input.text().trim().to_owned();
            let password = password_input.text();
            let confirm_password = confirm_password_input.text();
            let homeserver = homeserver_input.text().trim().to_owned();
            if user_id.is_empty() {
                login_status_modal_inner.set_title(cx, "Missing User ID");
                login_status_modal_inner.set_status(cx, "Please enter a valid User ID.");
                login_status_modal_inner.button_ref(cx).set_text(cx, "Okay");
            } else if password.is_empty() {
                login_status_modal_inner.set_title(cx, "Missing Password");
                login_status_modal_inner.set_status(cx, "Please enter a valid password.");
                login_status_modal_inner.button_ref(cx).set_text(cx, "Okay");
            } else if self.signup_mode && password != confirm_password {
                login_status_modal_inner.set_title(cx, "Passwords do not match");
                login_status_modal_inner.set_status(cx, "Please enter the same password in both password fields.");
                login_status_modal_inner.button_ref(cx).set_text(cx, "Okay");
            } else {
                self.last_failure_message_shown = None;
                login_status_modal_inner.set_title(cx, if self.signup_mode {
                    "Creating account..."
                } else {
                    "Logging in..."
                });
                login_status_modal_inner.set_status(
                    cx,
                    if self.signup_mode {
                        "Waiting for the homeserver to create your account..."
                    } else {
                        "Waiting for a login response..."
                    },
                );
                login_status_modal_inner.button_ref(cx).set_text(cx, "Cancel");
                submit_async_request(MatrixRequest::Login(if self.signup_mode {
                    LoginRequest::Register(RegisterAccount {
                        user_id,
                        password,
                        homeserver: homeserver.is_empty().not().then_some(homeserver),
                    })
                } else {
                    LoginRequest::LoginByPassword(LoginByPassword {
                        user_id,
                        password,
                        homeserver: homeserver.is_empty().not().then_some(homeserver),
                        is_add_account: self.adding_account,
                    })
                }));
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
                    self.last_failure_message_shown = None;
                    user_id_input.set_text(cx, user_id);
                    password_input.set_text(cx, "");
                    homeserver_input.set_text(cx, homeserver.as_deref().unwrap_or_default());
                    login_status_modal_inner.set_title(cx, "Logging in via CLI...");
                    login_status_modal_inner.set_status(
                        cx,
                        &format!("Auto-logging in as user {user_id}...")
                    );
                    let login_status_modal_button = login_status_modal_inner.button_ref(cx);
                    login_status_modal_button.set_text(cx, "Cancel");
                    login_status_modal_button.set_enabled(cx, false); // Login cancel not yet supported
                    login_status_modal.open(cx);
                }
                Some(LoginAction::Status { title, status }) => {
                    self.last_failure_message_shown = None;
                    login_status_modal_inner.set_title(cx, title);
                    login_status_modal_inner.set_status(cx, status);
                    let login_status_modal_button = login_status_modal_inner.button_ref(cx);
                    login_status_modal_button.set_text(cx, "Cancel");
                    login_status_modal_button.set_enabled(cx, true);
                    login_status_modal.open(cx);
                    self.redraw(cx);
                }
                Some(LoginAction::LoginSuccess) => {
                    // The main `App` component handles showing the main screen
                    // and hiding the login screen & login status modal.
                    self.last_failure_message_shown = None;
                    self.set_signup_mode(cx, false);
                    self.adding_account = false;
                    user_id_input.set_text(cx, "");
                    password_input.set_text(cx, "");
                    confirm_password_input.set_text(cx, "");
                    homeserver_input.set_text(cx, "");
                    // Reset title and buttons in case we were in add-account mode
                    self.view.label(cx, ids!(title)).set_text(cx, "Login to Robrix");
                    cancel_button.set_visible(cx, false);
                    mode_toggle_button.set_visible(cx, true);
                    login_status_modal.close(cx);
                    self.redraw(cx);
                }
                Some(LoginAction::LoginFailure(error)) => {
                    if self.last_failure_message_shown.as_deref() == Some(error.as_str()) {
                        continue;
                    }
                    self.last_failure_message_shown = Some(error.clone());
                    login_status_modal_inner.set_title(cx, if self.signup_mode {
                        "Account Creation Failed."
                    } else {
                        "Login Failed."
                    });
                    login_status_modal_inner.set_status(cx, error);
                    let login_status_modal_button = login_status_modal_inner.button_ref(cx);
                    login_status_modal_button.set_text(cx, "Okay");
                    login_status_modal_button.set_enabled(cx, true);
                    login_status_modal.open(cx);
                    self.redraw(cx);
                }
                Some(LoginAction::SsoPending(pending)) => {
                    let mask = if *pending { 1.0 } else { 0.0 };
                    let cursor = if *pending { MouseCursor::NotAllowed } else { MouseCursor::Hand };
                    for view_ref in self.view_set(cx, button_set).iter() {
                        let Some(mut view_mut) = view_ref.borrow_mut() else { continue };
                        let mut image = view_mut.image(cx, ids!(image));
                        script_apply_eval!(cx, image, {
                            draw_bg.mask: #(mask)
                        });
                        view_mut.cursor = Some(cursor);
                    }
                    self.sso_pending = *pending;
                    self.redraw(cx);
                }
                Some(LoginAction::SsoSetRedirectUrl(url)) => {
                    self.sso_redirect_url = Some(url.to_string());
                }
                Some(LoginAction::ShowAddAccountScreen) => {
                    self.adding_account = true;
                    // Update UI to "add account" mode
                    self.view.label(cx, ids!(title)).set_text(cx, "Add Another Account");
                    cancel_button.set_visible(cx, true);
                    // Hide signup button in add-account mode (user already has an account)
                    mode_toggle_button.set_visible(cx, false);
                    self.redraw(cx);
                }
                Some(LoginAction::AddAccountSuccess) => {
                    // Reset the login screen state
                    self.adding_account = false;
                    user_id_input.set_text(cx, "");
                    password_input.set_text(cx, "");
                    homeserver_input.set_text(cx, "");
                    // Reset title and buttons
                    self.view.label(cx, ids!(title)).set_text(cx, "Login to Robrix");
                    cancel_button.set_visible(cx, false);
                    mode_toggle_button.set_visible(cx, true);
                    login_status_modal.close(cx);
                    self.redraw(cx);
                }
                _ => { }
            }

            // Handle account switch actions - close modal when switch completes or fails
            match action.downcast_ref() {
                Some(AccountSwitchAction::Switched(_)) => {
                    login_status_modal.close(cx);
                    self.redraw(cx);
                }
                Some(AccountSwitchAction::Failed(error)) => {
                    login_status_modal_inner.set_title(cx, "Account Switch Failed");
                    login_status_modal_inner.set_status(cx, error);
                    let login_status_modal_button = login_status_modal_inner.button_ref(cx);
                    login_status_modal_button.set_text(cx, "Okay");
                    login_status_modal_button.set_enabled(cx, true);
                    self.redraw(cx);
                }
                _ => { }
            }
        }

        // If the Login SSO screen's "cancel" button was clicked, send a http request to gracefully shutdown the SSO server
        if let Some(sso_redirect_url) = &self.sso_redirect_url {
            let login_status_modal_button = login_status_modal_inner.button_ref(cx);
            if login_status_modal_button.clicked(actions) {
                let request_id = id!(SSO_CANCEL_BUTTON);
                let request = HttpRequest::new(format!("{}/?login_token=",sso_redirect_url), HttpMethod::GET);
                cx.http_request(request_id, request);
                self.sso_redirect_url = None;
            }
        }

        // Handle any of the SSO login buttons being clicked
        for (view_ref, brand) in self.view_set(cx, button_set).iter().zip(&provider_brands) {
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
#[derive(Clone, Default, Debug)]
pub enum LoginAction {
    /// A positive response from the backend Matrix task to the login screen.
    LoginSuccess,
    /// A positive response when adding an additional account (multi-account mode).
    /// The login was successful but we should add this as a new account, not replace the existing one.
    AddAccountSuccess,
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
    /// Request to show the login screen in "add account" mode.
    /// This is used when the user wants to add another Matrix account.
    ShowAddAccountScreen,
    /// Request to cancel adding an account and return to the previous screen.
    CancelAddAccount,
    #[default]
    None,
}
