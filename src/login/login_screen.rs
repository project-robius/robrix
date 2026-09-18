//! The `LoginScreen` queries and shows the homeserver's supported login methods.
//!
//! There are two main sections to it: the username/password form and the SSO/Oauth button.
//! They're each shown based on what the homeserver supports.

use std::{net::{Ipv4Addr, Ipv6Addr}, ops::Not};
use makepad_widgets::*;
use crate::{
    shared::{password_input::PasswordTextInputWidgetExt, styles::*},
    sliding_sync::{homeserver_of_user_id, submit_async_request, username_to_full_user_id, BrowserLoginKind, LoginByPassword, LoginMethods, LoginRequest, MatrixRequest},
    utils,
};
use super::login_status_modal::{LoginStatusModalAction, LoginStatusModalWidgetExt};

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    // A lighter version of the main Robrix purple color
    mod.widgets.COLOR_LOGIN_BG_TOP = #EEEAFA
    mod.widgets.COLOR_LOGIN_BG_BOTTOM = #FFFFFF

    mod.widgets.IMG_APP_LOGO = crate_resource("self://resources/robrix_logo_alpha.png")

    mod.widgets.LoginButton = mod.widgets.RobrixIconButton {
        width: Fill {max: 275}, height: Fit
        padding: Inset{top: 12.5, bottom: 12.5, left: 10, right: 10}
        flow: Flow.Right{wrap: true}
        align: Align{x: 0.5, y: 0.5}
        // Setting Fill for the label width lets it wrap on really narrow screens
        label_walk: Walk{width: Fill, height: Fit}
        label_align: Align{x: 0.5, y: 0.5}
        draw_text +: {
            text_style: REGULAR_TEXT {font_size: 10.5}
        }
    }

    // A small label with two horizontal lines on either side of it;
    // each use just sets `divider_label.text`.
    mod.widgets.LoginDivider = View {
        width: Fill {max: 275}, height: Fit
        flow: Right
        spacing: 0.0
        padding: Inset{top: 3, left: 2, right: 2}
        align: Align{x: 0.5, y: 0.5}

        LineH { draw_bg.color: #C8C8C8 }
        divider_label := Label {
            width: Fit, height: Fit
            padding: Inset{left: 4, right: 4, top: 0, bottom: 0}
            draw_text +: {
                color: #8C8C8C
                text_style: REGULAR_TEXT {font_size: 9}
            }
        }
        LineH { draw_bg.color: #C8C8C8 }
    }

    mod.widgets.LoginHint = Label {
        width: Fill {max: 275}, height: Fit
        flow: Flow.Right{wrap: true}
        align: Align{x: 0.5, y: 0.5}
        padding: 0
        draw_text +: {
            color: #8C8C8C
            text_style: REGULAR_TEXT {font_size: 9.5}
        }
    }

    // used for both SSO and Oauth since there's no real difference to the user
    mod.widgets.BrowserLoginButton = mod.widgets.LoginButton {
        text: "Login with your browser…"
    }

    mod.widgets.BrowserLoginHint = mod.widgets.LoginHint {
        text: "Robrix will open your homeserver's\nsingle sign-on (SSO) page."
    }

    mod.widgets.LoginScreen = set_type_default() do #(LoginScreen::register_widget(vm)) {
        ..mod.widgets.RectView

        width: Fill, height: Fill,
        align: Align{x: 0.5, y: 0.5}
        show_bg: true,
        // we do a neat lil gradient from top (light purple) to bottom (white)
        draw_bg +: {
            color: (mod.widgets.COLOR_LOGIN_BG_TOP)
            color_2: (mod.widgets.COLOR_LOGIN_BG_BOTTOM)
        }

        ScrollYView {
            width: Fill, height: Fill,
            flow: Down, // Required for vertical scrolling to work.
            align: Align{x: 0.5, y: 0.5}

            // allow the view to be scrollable but hide the actual scroll bar
            scroll_bars: {
                show_scroll_x: false, show_scroll_y: true,
                scroll_bar_y: {
                    bar_size: 0.0
                    min_handle_size: 0.0
                    drag_scrolling: true
                }
            }

            RoundedView {
                margin: Inset{top: 50, bottom: 50}
                width: Fill
                height: Fit
                align: Align{x: 0.5, y: 0.5}
                flow: Overlay,


                View {
                    width: Fill
                    height: Fit
                    flow: Down
                    align: Align{x: 0.5, y: 0.5}
                    spacing: 15.0
                    padding: Inset{left: 10, right: 10}

                    logo_image := Image {
                        fit: ImageFit.Smallest,
                        width: 80
                        src: (mod.widgets.IMG_APP_LOGO),
                    }

                    title := Label {
                        width: Fill {max: 275}, height: Fit
                        align: Align{x: 0.5, y: 0.5}
                        margin: Inset{ bottom: 5 }
                        padding: 0,
                        draw_text +: {
                            color: (COLOR_TEXT)
                            text_style: TITLE_TEXT {font_size: 16.0}
                        }
                        text: "Login to Robrix"
                    }

                    View {
                        width: Fill {max: 275}, height: Fit,
                        flow: Down,

                        View {
                            width: Fill {max: 275}, height: Fit
                            flow: Right
                            spacing: 5
                            align: Align{y: 0.5}

                            homeserver_input := RobrixTextInput {
                                width: Fill, height: Fit,
                                flow: Flow.Right { wrap: false },
                                padding: Inset{top: 6.5, bottom: 6.5, left: 10, right: 10}
                                empty_text: "matrix.org"
                                autocapitalize: None,
                                autocorrect: Disabled,
                                content_type: Url,
                                input_mode: Url,
                                draw_text +: {
                                    text_style: TITLE_TEXT {font_size: 10.0}
                                }
                            }

                            query_homeserver_button := RobrixIconButton {
                                width: 28, height: 28
                                align: Align{x: 0.5, y: 0.5}
                                padding: 0
                                spacing: 0
                                margin: 0
                                draw_icon.svg: (ICON_SEARCH)
                                icon_walk: Walk{width: 13, height: 13}
                                text: ""
                            }
                        }

                        mod.widgets.LoginDivider { divider_label.text: "Homeserver" }
                    }

                    // THis is shown while waiting for the homeserver to respond.
                    // see `show_login_methods()` for how the widgets in this view get populated.
                    View {
                        width: Fill {max: 275}, height: Fit
                        flow: Right
                        spacing: 6
                        align: Align{x: 0.5, y: 0.5}

                        query_loading_spinner := LoadingSpinner {
                            width: 13, height: 13
                            draw_bg.color: #8C8C8C
                        }

                        query_status_label := mod.widgets.LoginHint {
                            width: Fit
                            text: "Querying homeserver login options..."
                        }
                    }

                    retry_button := mod.widgets.LoginButton {
                        visible: false
                        text: "Try again"
                    }

                    password_view := View {
                        width: Fill {max: 275}, height: Fit
                        flow: Down
                        spacing: 15.0
                        align: Align{x: 0.5, y: 0.5}

                        user_id_input := RobrixTextInput {
                            width: Fill {max: 275}, height: Fit
                            flow: Flow.Right { wrap: false },
                            padding: 10,
                            empty_text: "User ID"
                            autocapitalize: None,
                            autocorrect: Disabled,
                            content_type: Username,
                        }

                        password_input := mod.widgets.PasswordTextInput {
                            width: Fill {max: 275}
                        }

                        login_button := mod.widgets.LoginButton {
                            text: "Login"
                            enabled: false
                            draw_bg +: {
                                color: (COLOR_BG_DISABLED)
                                border_color: (COLOR_FG_DISABLED)
                            }
                            draw_text +: {
                                color: (COLOR_FG_DISABLED)
                            }
                        }
                    }

                    oauth_view := View {
                        width: Fill {max: 275}, height: Fit
                        flow: Down
                        spacing: 15.0
                        align: Align{x: 0.5, y: 0.5}

                        oauth_divider := mod.widgets.LoginDivider { divider_label.text: "or" }

                        oauth_button := mod.widgets.BrowserLoginButton {}

                        mod.widgets.BrowserLoginHint {}

                        create_account_view := View {
                            visible: false
                            width: Fill, height: Fit
                            flow: Down
                            spacing: 15.0
                            align: Align{x: 0.5, y: 0.5}

                            mod.widgets.LoginDivider { divider_label.text: "Don't have an account?" }

                            create_account_button := mod.widgets.LoginButton {
                                text: "Create an account"
                            }
                        }
                    }

                    sso_view := View {
                        visible: false
                        width: Fill {max: 275}, height: Fit
                        flow: Down
                        spacing: 15.0
                        align: Align{x: 0.5, y: 0.5}

                        sso_divider := mod.widgets.LoginDivider { divider_label.text: "or" }

                        sso_button := mod.widgets.BrowserLoginButton {}

                        mod.widgets.BrowserLoginHint {}
                    }

                    signup_view := View {
                        visible: false
                        width: Fill {max: 275}, height: Fit
                        flow: Down
                        spacing: 15.0
                        align: Align{x: 0.5, y: 0.5}

                        mod.widgets.LoginDivider { divider_label.text: "Don't have an account?" }

                        signup_button := RobrixIconButton {
                            width: Fit, height: Fit
                            padding: Inset{left: 15, right: 15, top: 10, bottom: 10}
                            margin: Inset{bottom: 5}
                            align: Align{x: 0.5, y: 0.5}
                            text: "Sign up here"
                        }
                    }
                }

                // The modal that pops up to display login status messages,
                // such as when the user is logging in or when there is an error.
                login_status_modal := Modal {
                    can_dismiss: false,
                    content := mod.widgets.LoginStatusModal {}
                }
            }
        }
    }
}

/// Delay after the last keystroke before we send off a homeserver query.
const HOMESERVER_QUERY_DELAY: f64 = 0.5;

static MATRIX_SIGN_UP_URL: &str = "https://matrix.org/docs/chat_basics/matrix-for-im/#creating-a-matrix-account";

#[derive(Script, ScriptHook, Widget)]
pub struct LoginScreen {
    #[source] source: ScriptObjectRef,
    #[deref] view: View,

    /// While a browser-based login is in flight, the login buttons stay disabled.
    #[rust] is_login_pending: bool,
    /// The homeserver we last queried info for, or `None` if we haven't done any queries yet.
    #[rust] queried_homeserver: Option<String>,
    /// The login methods that are currently shown.
    ///
    /// Note that by default, we show everything, so if a homeserver is broken and doesn't properly
    /// answer our query or advertise its login methods, we still allow the user to log in with any method.
    #[rust(EVERY_LOGIN_METHOD)] shown_methods: LoginMethods,
    /// Whether we auto-populated the homeserver based on the entered user ID.
    #[rust] is_homeserver_from_user_id: bool,
    /// Whether the user ID is the field being typed in; only then do we put
    /// the homeserver it points at into the homeserver field.
    #[rust] fill_homeserver_from_user_id: bool,
    #[rust] homeserver_query_timer: Timer,
}

const EVERY_LOGIN_METHOD: LoginMethods = LoginMethods {
    has_oauth: true,
    supports_create_account: false,
    has_password: true,
    has_sso: false,
};

/// What the login screen knows about the homeserver text currently entered in the text input.
enum LoginMethodsState<'a> {
    /// The homeserver text changed, so the user has to submit/re-submit the query.
    NotQueried,
    Querying,
    Known(&'a Result<LoginMethods, String>),
}


impl Widget for LoginScreen {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        if self.homeserver_query_timer.is_event(event).is_some() {
            self.homeserver_query_timer = Timer::empty();
            self.query_login_methods(cx);
        }
        self.match_event(cx, event);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl MatchEvent for LoginScreen {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        let homeserver_input = self.view.text_input(cx, ids!(homeserver_input));
        let user_id_input = self.view.text_input(cx, ids!(user_id_input));
        let password_input = self.view.password_text_input(cx, ids!(password_input));

        let login_status_modal = self.view.modal(cx, ids!(login_status_modal));
        let login_status_modal_content = self.view.login_status_modal(cx, ids!(login_status_modal.content));

        if user_id_input.changed(actions).is_some() || password_input.changed(actions).is_some() {
            self.enable_login_button(cx);
        }
        // Wait a moment after the user finishes typing before we query the homeserver.
        if user_id_input.changed(actions).is_some() && self.homeserver_from_user_id(cx).is_some() {
            self.fill_homeserver_from_user_id = true;
            cx.stop_timer(self.homeserver_query_timer);
            self.homeserver_query_timer = cx.start_timeout(HOMESERVER_QUERY_DELAY);
        }

        // Automatically query a homeserver address that looks valid.
        // If it doesn't look valid, don't query it but also don't show an error message
        // because we wanna wait til the user stops typing or actively submits the query.
        if homeserver_input.changed(actions).is_some() {
            self.is_homeserver_from_user_id = false;
            self.fill_homeserver_from_user_id = false;
            self.queried_homeserver = None;
            self.show_login_methods(cx, LoginMethodsState::NotQueried);
            cx.stop_timer(self.homeserver_query_timer);
            let homeserver = self.homeserver_to_use(cx);
            if homeserver.is_empty() || is_homeserver_address(&homeserver) {
                self.homeserver_query_timer = cx.start_timeout(HOMESERVER_QUERY_DELAY);
            }
        }
        if homeserver_input.returned(actions).is_some()
            || self.view.button(cx, ids!(query_homeserver_button)).clicked(actions)
            || self.view.button(cx, ids!(retry_button)).clicked(actions)
        {
            cx.stop_timer(self.homeserver_query_timer);
            self.homeserver_query_timer = Timer::empty();
            self.query_login_methods(cx);
        }

        if self.view.button(cx, ids!(signup_button)).clicked(actions) {
            utils::open_url(MATRIX_SIGN_UP_URL);
        }

        let create_account = self.view.button(cx, ids!(create_account_button)).clicked(actions);
        let browser_login_kind = if self.view.button(cx, ids!(sso_button)).clicked(actions) {
            Some(BrowserLoginKind::LegacySso)
        } else if create_account || self.view.button(cx, ids!(oauth_button)).clicked(actions) {
            Some(BrowserLoginKind::OAuth { create_account })
        } else {
            None
        };
        if let Some(kind) = browser_login_kind && !self.is_login_pending {
            login_status_modal_content.set_title(cx, "Logging in...");
            login_status_modal_content.set_status(cx, "Connecting to the homeserver...");
            login_status_modal_content.button_ref(cx).set_text(cx, "Cancel");
            login_status_modal.open(cx);
            submit_async_request(MatrixRequest::LoginViaBrowser {
                homeserver: self.homeserver_to_use(cx),
                kind,
            });
            self.redraw(cx);
        }

        if self.view.button(cx, ids!(login_button)).clicked(actions)
            || user_id_input.returned(actions).is_some()
            || password_input.returned(actions).is_some()
        {
            self.show_full_user_id(cx);
            let user_id = user_id_input.text();
            let password = password_input.text();
            let homeserver = homeserver_input.text();
            if user_id.is_empty() {
                login_status_modal_content.set_title(cx, "Missing User ID");
                login_status_modal_content.set_status(cx, "Please enter a valid User ID.");
                login_status_modal_content.button_ref(cx).set_text(cx, "Okay");
            } else if password.is_empty() {
                login_status_modal_content.set_title(cx, "Missing Password");
                login_status_modal_content.set_status(cx, "Please enter a valid password.");
                login_status_modal_content.button_ref(cx).set_text(cx, "Okay");
            } else {
                login_status_modal_content.set_title(cx, "Logging in...");
                login_status_modal_content.set_status(cx, "Waiting for a login response...");
                login_status_modal_content.button_ref(cx).set_text(cx, "Cancel");
                submit_async_request(MatrixRequest::Login(LoginRequest::LoginByPassword(LoginByPassword {
                    user_id,
                    password,
                    homeserver: homeserver.is_empty().not().then_some(homeserver),
                })));
            }
            login_status_modal.open(cx);
            self.redraw(cx);
        }

        for action in actions {
            if let LoginStatusModalAction::Close = action.as_widget_action().cast() {
                login_status_modal.close(cx);
                if self.is_login_pending {
                    submit_async_request(MatrixRequest::CancelBrowserLogin);
                }
            }

            // Handle login-related actions received from background async tasks.
            match action.downcast_ref() {
                Some(LoginAction::CliAutoLogin { user_id, homeserver }) => {
                    user_id_input.set_text(cx, user_id);
                    password_input.set_text(cx, "");
                    homeserver_input.set_text(cx, homeserver.as_deref().unwrap_or_default());
                    self.enable_login_button(cx);
                    self.query_login_methods(cx);
                    login_status_modal_content.set_title(cx, "Logging in via CLI...");
                    login_status_modal_content.set_status(
                        cx,
                        &format!("Auto-logging in as user {user_id}...")
                    );
                    let login_status_modal_button = login_status_modal_content.button_ref(cx);
                    login_status_modal_button.set_text(cx, "Cancel");
                    login_status_modal_button.set_enabled(cx, false); // Login cancel not yet supported
                    login_status_modal.open(cx);
                }
                Some(LoginAction::Status { title, status }) => {
                    login_status_modal_content.set_title(cx, title);
                    login_status_modal_content.set_status(cx, status);
                    let login_status_modal_button = login_status_modal_content.button_ref(cx);
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
                    self.enable_login_button(cx);
                    self.is_homeserver_from_user_id = false;
                    self.fill_homeserver_from_user_id = false;
                    self.queried_homeserver = None;
                    self.shown_methods = EVERY_LOGIN_METHOD;
                    self.show_login_methods(cx, LoginMethodsState::Querying);
                    self.set_login_pending(cx, false);
                    login_status_modal.close(cx);
                }
                Some(LoginAction::LoginFailure(error)) => {
                    login_status_modal_content.set_title(cx, "Login Failed.");
                    login_status_modal_content.set_status(cx, error);
                    let login_status_modal_button = login_status_modal_content.button_ref(cx);
                    login_status_modal_button.set_text(cx, "Okay");
                    login_status_modal_button.set_enabled(cx, true);
                    login_status_modal.open(cx);
                    self.set_login_pending(cx, false);
                }
                Some(LoginAction::BrowserLoginStarted) => {
                    self.set_login_pending(cx, true);
                }
                Some(LoginAction::LoginCancelled) => {
                    login_status_modal.close(cx);
                    self.set_login_pending(cx, false);
                }
                // Ignore responses related to a homeserver that the user is no longer trying to query.
                Some(LoginAction::LoginMethods { homeserver, result })
                    if self.queried_homeserver.as_deref().unwrap_or_default() == homeserver =>
                {
                    self.show_login_methods(cx, LoginMethodsState::Known(result));
                }
                _ => { }
            }
        }
    }
}

impl LoginScreen {
    /// Only one login can run at a time, so we disable the buttons until it succeeds, fails, or cancels.
    fn set_login_pending(&mut self, cx: &mut Cx, is_pending: bool) {
        self.is_login_pending = is_pending;
        let buttons: &[&[LiveId]] = ids_array!(oauth_button, create_account_button, sso_button);
        for button in self.view.button_set(cx, buttons).iter() {
            button.set_enabled(cx, !is_pending);
        }
        self.redraw(cx);
    }

    /// For password login, only enable the login button if both username and password are non-empty.
    fn enable_login_button(&mut self, cx: &mut Cx) {
        let is_ready = !self.view.text_input(cx, ids!(user_id_input)).text().trim().is_empty()
            && !self.view.password_text_input(cx, ids!(password_input)).text().is_empty();
        let (fg_color, bg_color) = if is_ready {
            (COLOR_PRIMARY, COLOR_ACTIVE_PRIMARY)
        } else {
            (COLOR_FG_DISABLED, COLOR_BG_DISABLED)
        };
        let mut login_button = self.view.button(cx, ids!(login_button));
        script_apply_eval!(cx, login_button, {
            enabled: #(is_ready),
            draw_bg +: {
                color: #(bg_color),
                border_color: #(fg_color),
            }
            draw_text +: {
                color: #(fg_color),
            }
        });
    }

    /// Returns the homeserver to use for login.
    ///
    /// This returns what the user typed into the homeserver input, or if it's blank,
    /// the homeserver extracted from the user ID, or if that's empty, `None`.
    fn homeserver_from_user_id(&self, cx: &mut Cx) -> Option<String> {
        let shown = self.view.text_input(cx, ids!(homeserver_input)).text();
        if !shown.is_empty() && !self.is_homeserver_from_user_id {
            return None;
        }
        let from_user_id = homeserver_of_user_id(&self.view.text_input(cx, ids!(user_id_input)).text())
            .filter(|homeserver| is_homeserver_address(homeserver))
            .unwrap_or_default();
        (from_user_id != shown).then_some(from_user_id)
    }

    fn homeserver_to_use(&self, cx: &mut Cx) -> String {
        let homeserver = self.view.text_input(cx, ids!(homeserver_input)).text();
        if !homeserver.is_empty() {
            return homeserver;
        }
        homeserver_of_user_id(&self.view.text_input(cx, ids!(user_id_input)).text()).unwrap_or_default()
    }

    /// Rewrites a partial user ID into the full one we'd log in with.
    ///
    /// This ensures that the user knows what's going on.
    /// If it can't parse the user ID input into a proper user ID format, this does nothing.
    fn show_full_user_id(&mut self, cx: &mut Cx) {
        let user_id_input = self.view.text_input(cx, ids!(user_id_input));
        let user_id = user_id_input.text();
        if user_id.trim().is_empty() {
            return;
        }
        let homeserver = self.homeserver_to_use(cx);
        if let Some(full_user_id) = username_to_full_user_id(
            user_id.trim(),
            (!homeserver.is_empty()).then_some(homeserver.as_str()),
        ) && full_user_id.as_str() != user_id {
            user_id_input.set_text(cx, full_user_id.as_str());
        }
    }

    fn query_login_methods(&mut self, cx: &mut Cx) {
        self.show_full_user_id(cx);
        // Fill in the homeserver from the user ID, so it's obvious which homeserver we're querying.
        if self.fill_homeserver_from_user_id && let Some(from_user_id) = self.homeserver_from_user_id(cx) {
            self.view.text_input(cx, ids!(homeserver_input)).set_text(cx, &from_user_id);
            self.is_homeserver_from_user_id = !from_user_id.is_empty();
        }
        let homeserver = self.homeserver_to_use(cx);
        if !homeserver.is_empty() && !is_homeserver_address(&homeserver) {
            let invalid = Err(String::from("That's not a valid homeserver address."));
            self.queried_homeserver = None;
            self.show_login_methods(cx, LoginMethodsState::Known(&invalid));
            return;
        }
        self.queried_homeserver = Some(homeserver.clone());
        self.show_login_methods(cx, LoginMethodsState::Querying);
        submit_async_request(MatrixRequest::QueryLoginMethods { homeserver });
    }

    /// Updates the login screen to show the various sections that represent supported login methods.
    fn show_login_methods(&mut self, cx: &mut Cx, state: LoginMethodsState) {
        let status = match state {
            LoginMethodsState::NotQueried => None,
            LoginMethodsState::Querying => Some("Querying homeserver login options..."),
            LoginMethodsState::Known(Ok(methods)) if methods.has_oauth || methods.has_password || methods.has_sso => {
                self.shown_methods = methods.clone();
                // Say so when the homeserver leaves the user no choice of how to log in.
                match (methods.has_password, methods.has_oauth || methods.has_sso) {
                    (true, false) => Some("This homeserver only supports username + password login."),
                    (false, true) => Some("This homeserver only supports browser-based login."),
                    _ => None,
                }
            }
            LoginMethodsState::Known(Ok(_)) => {
                self.shown_methods = LoginMethods::default();
                Some("This homeserver doesn't offer any login methods that Robrix supports.")
            }
            LoginMethodsState::Known(Err(error)) => {
                self.shown_methods = EVERY_LOGIN_METHOD;
                Some(error.as_str())
            }
        };
        let shown = self.shown_methods.clone();
        self.view.view(cx, ids!(password_view)).set_visible(cx, shown.has_password);
        self.view.view(cx, ids!(oauth_view)).set_visible(cx, shown.has_oauth);
        self.view.view(cx, ids!(oauth_divider)).set_visible(cx, shown.has_password);
        self.view.view(cx, ids!(create_account_view)).set_visible(cx, shown.supports_create_account);
        self.view.view(cx, ids!(sso_view)).set_visible(cx, shown.has_sso);
        self.view.view(cx, ids!(sso_divider)).set_visible(cx, shown.has_password && shown.has_sso);

        // OAuth homeservers offer their own "Create an account" page, not a general sign-up link.
        self.view.view(cx, ids!(signup_view)).set_visible(cx, !shown.has_oauth && (shown.has_password || shown.has_sso));
        self.view.button(cx, ids!(retry_button)).set_visible(cx, matches!(state, LoginMethodsState::Known(Err(_))));
        let is_querying = matches!(state, LoginMethodsState::Querying);
        self.view.widget(cx, ids!(query_loading_spinner)).set_visible(cx, is_querying);
        let status_label = self.view.label(cx, ids!(query_status_label));
        status_label.set_visible(cx, status.is_some());
        status_label.set_text(cx, status.unwrap_or_default());

        if let Some(mut label) = status_label.borrow_mut() {
            label.walk.width = if is_querying { Size::fit() } else { Size::fill() };
        }
        self.redraw(cx);
    }
}

/// Returns `true` if the given text can be treated as a homeserver.
fn is_homeserver_address(text: &str) -> bool {
    let after_scheme = text.split_once("://").map_or(text, |(_, rest)| rest);
    let Some(host) = after_scheme.split(['/', '?', '#']).next().filter(|host| !host.is_empty()) else {
        return false;
    };
    // An IPv6 address might be surrounded by square brackets (`http://[::1]:8008`).
    if let Some(ipv6) = host.strip_prefix('[') {
        return ipv6.split_once(']').is_some_and(|(ipv6, _)| ipv6.parse::<Ipv6Addr>().is_ok());
    }
    let host = host.split(':').next().unwrap_or_default();
    host.parse::<Ipv4Addr>().is_ok()
        || host == "localhost"
        || (host.contains('.')
            && !host.starts_with('.')
            && !host.ends_with('.')
            && host.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-'))
}

/// Actions sent to or from the login screen.
#[derive(Clone, Debug)]
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
    /// A browser-based login is now in progress.
    ///
    /// This will end in either `LoginSuccess`, `LoginFailure`, or `LoginCancelled`.
    BrowserLoginStarted,
    /// The browser login was cancelled by the user.
    LoginCancelled,
    /// The response from the homeserver containing its supported login methods.
    LoginMethods {
        /// The homeserver text that was actually queried (so we can ignore old queries).
        homeserver: String,
        result: Result<LoginMethods, String>,
    },
}
