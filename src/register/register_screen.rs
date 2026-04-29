//! RegisterScreen widget: homeserver picker + capability display.
//!
//! Phase 1 renders:
//!   - Back button (returns to login)
//!   - Screen title
//!   - Homeserver URL input
//!   - Next button (triggers capability discovery)
//!   - Three-state status area (MAS / UIAA / Disabled / errors)
//!
//! Phases 2-5 fill in OIDC launch / UIAA form / SSO buttons.

use makepad_widgets::*;

use crate::login::login_screen::LoginAction;
use crate::register::{HsCapabilities, RegisterAction, RegisterMode};
use crate::register::validation::{normalize_homeserver_url, HomeserverUrlError};
use crate::sliding_sync::{submit_async_request, MatrixRequest};

fn can_start_capability_discovery(registration_pending: bool, awaiting_sync_startup: bool) -> bool {
    !registration_pending && !awaiting_sync_startup
}

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.RegisterScreen = set_type_default() do #(RegisterScreen::register_widget(vm)) {
        ..mod.widgets.SolidView

        width: Fill, height: Fill,
        flow: Overlay
        align: Align{x: 0.5, y: 0.5}
        show_bg: true,
        draw_bg +: {
            color: COLOR_SECONDARY
        }

        ScrollYView {
            width: Fill,
            height: Fill,
            flow: Down,
            align: Align{x: 0.5, y: 0.5}
            show_bg: true,
            draw_bg.color: (COLOR_SECONDARY)

            scroll_bars: {
                show_scroll_x: false,
                show_scroll_y: true,
                scroll_bar_y: {
                    bar_size: 0.0
                    min_handle_size: 0.0
                    drag_scrolling: true
                }
            }

            RoundedView {
                margin: Inset{top: 50, bottom: 50}
                width: Fill,
                height: Fit,
                align: Align{x: 0.5, y: 0.5}
                flow: Overlay

                View {
                    width: Fill,
                    height: Fit,
                    flow: Down,
                    align: Align{x: 0.5, y: 0.5}
                    spacing: 15.0

                    logo_image := Image {
                        fit: ImageFit.Smallest,
                        width: 80
                        src: (mod.widgets.IMG_APP_LOGO),
                    }

                    title := Label {
                        width: Fit,
                        height: Fit,
                        margin: Inset{bottom: 5}
                        padding: 0,
                        draw_text +: {
                            color: (COLOR_TEXT)
                            text_style: TITLE_TEXT {font_size: 16.0}
                        }
                        text: "Create Account"
                    }

                    View {
                        width: 275,
                        height: Fit,
                        flow: Down,

                        homeserver_input := RobrixTextInput {
                            width: 275,
                            height: Fit,
                            flow: Right,
                            padding: Inset{top: 10, bottom: 10, left: 10, right: 10}
                            empty_text: "matrix.org"
                        }

                        View {
                            width: 275,
                            height: Fit,
                            flow: Right,
                            padding: Inset{top: 3, left: 2, right: 2}
                            spacing: 0.0,
                            align: Align{x: 0.5, y: 0.5}

                            LineH { draw_bg.color: #C8C8C8 }

                            homeserver_hint_label := Label {
                                width: Fit,
                                height: Fit,
                                padding: 0,
                                draw_text +: {
                                    color: #8C8C8C
                                    text_style: REGULAR_TEXT {font_size: 9}
                                }
                                text: "Homeserver URL"
                            }

                            LineH { draw_bg.color: #C8C8C8 }
                        }
                    }

                    next_button := RobrixIconButton {
                        width: 275,
                        height: 40
                        padding: 10
                        margin: Inset{top: 5, bottom: 10}
                        align: Align{x: 0.5, y: 0.5}
                        text: "Next"
                    }

                    status_area := View {
                        width: 275,
                        height: Fit,
                        flow: Down,
                        visible: false
                        padding: Inset{top: 2, bottom: 2, left: 4, right: 4}

                        status_label := Label {
                            width: Fill,
                            height: Fit,
                            draw_text +: {
                                color: (COLOR_TEXT)
                                text_style: REGULAR_TEXT {font_size: 10.5}
                            }
                            text: ""
                        }
                    }

                    registration_form := View {
                        width: 275,
                        height: Fit,
                        flow: Down,
                        spacing: 10,
                        visible: false

                        username_input := RobrixTextInput {
                            width: 275, height: Fit,
                            flow: Right,
                            padding: Inset{top: 10, bottom: 10, left: 10, right: 10}
                            empty_text: "Username"
                        }

                        password_input := RobrixTextInput {
                            width: 275, height: Fit,
                            flow: Right,
                            padding: Inset{top: 10, bottom: 10, left: 10, right: 10}
                            empty_text: "Password"
                            is_password: true,
                        }

                        confirm_password_input := RobrixTextInput {
                            width: 275, height: Fit,
                            flow: Right,
                            padding: Inset{top: 10, bottom: 10, left: 10, right: 10}
                            empty_text: "Confirm password"
                            is_password: true,
                        }

                        form_error_label := Label {
                            width: Fill, height: Fit,
                            visible: false
                            draw_text +: {
                                color: (COLOR_FG_DANGER_RED)
                                text_style: REGULAR_TEXT {font_size: 10.5}
                            }
                            text: ""
                        }

                        submit_button := RobrixIconButton {
                            width: 275, height: 40
                            padding: 10
                            margin: Inset{top: 5}
                            align: Align{x: 0.5, y: 0.5}
                            text: "Create Account"
                        }
                    }

                    LineH {
                        width: 275
                        margin: Inset{bottom: -5}
                        draw_bg.color: #C8C8C8
                    }

                    View {
                        width: 275,
                        height: Fit,
                        flow: Right,
                        spacing: 0.0,
                        align: Align{x: 0.5, y: 0.5}

                        LineH { draw_bg.color: #C8C8C8 }

                        account_prompt_label := Label {
                            width: Fit,
                            height: Fit,
                            padding: Inset{left: 1, right: 1, top: 0, bottom: 0}
                            draw_text +: {
                                color: #x6c6c6c
                                text_style: REGULAR_TEXT {}
                            }
                            text: "Already have an account?"
                        }

                        LineH { draw_bg.color: #C8C8C8 }
                    }

                    back_button := RobrixIconButton {
                        width: Fit,
                        height: Fit,
                        padding: Inset{left: 15, right: 15, top: 10, bottom: 10}
                        margin: Inset{bottom: 5}
                        align: Align{x: 0.5, y: 0.5}
                        text: "← Back to Login"
                    }
                }
            }
        }
    }
}

#[derive(Script, ScriptHook, Widget)]
pub struct RegisterScreen {
    #[deref] view: View,
    #[rust] last_discovery: Option<HsCapabilities>,
    /// Normalized user-typed URL that produced `last_discovery`. Kept
    /// separate from `caps.base_url` because `.well-known` may rewrite the
    /// host; comparing current input against `base_url` causes false mismatches.
    #[rust] last_discovery_input_url: Option<String>,
    /// Gates duplicate submits; mirrors `sso_pending`.
    #[rust] registration_pending: bool,
    /// Drives next_button "Checking..." feedback during slow `.well-known` probes.
    #[rust] discovery_pending: bool,
    /// Gates the `LoginFailure` arm: true only during the post-register
    /// `SyncService::build()` window, which `app.rs` can't recover from
    /// because state is still `LoggedOut`.
    #[rust] awaiting_sync_startup: bool,
}

impl Widget for RegisterScreen {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        self.widget_match_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl WidgetMatchEvent for RegisterScreen {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions, _scope: &mut Scope) {
        let back = self.view.button(cx, ids!(back_button));
        let next = self.view.button(cx, ids!(next_button));
        let submit = self.view.button(cx, ids!(submit_button));

        if back.clicked(actions) {
            // In-flight request can't be cancelled; leaving now would
            // auto-log the user into the account on its eventual success.
            if self.registration_pending {
                return;
            }
            self.discovery_pending = false;
            self.view.button(cx, ids!(next_button)).set_text(cx, "Next");
            Cx::post_action(RegisterAction::NavigateToLogin);
            return;
        }

        if next.clicked(actions) {
            if !can_start_capability_discovery(self.registration_pending, self.awaiting_sync_startup) {
                return;
            }
            let raw = self.view.text_input(cx, ids!(homeserver_input)).text();
            match normalize_homeserver_url(&raw) {
                Ok(url) => {
                    // Prevent submit-against-stale-server in the Next→response window.
                    self.last_discovery = None;
                    self.view.view(cx, ids!(registration_form)).set_visible(cx, false);
                    self.clear_form_error(cx);

                    self.show_status(cx, "Checking server capabilities...");
                    self.discovery_pending = true;
                    self.view.button(cx, ids!(next_button)).set_text(cx, "Checking...");
                    self.last_discovery_input_url = Some(url.clone());
                    submit_async_request(MatrixRequest::DiscoverHomeserverCapabilities { url });
                }
                Err(HomeserverUrlError::Empty) => {
                    self.show_status(cx, "Please enter a homeserver URL (e.g. matrix.org).");
                }
                Err(HomeserverUrlError::UnsupportedScheme(s)) => {
                    self.show_status(cx, &format!("Unsupported scheme: {s}. Only http(s) is allowed."));
                }
                Err(HomeserverUrlError::Invalid) => {
                    self.show_status(cx, "That URL looks invalid. Please check and try again.");
                }
            }
        }

        let username_input = self.view.text_input(cx, ids!(username_input));
        let password_input = self.view.text_input(cx, ids!(password_input));
        let confirm_password_input = self.view.text_input(cx, ids!(confirm_password_input));

        let submit_triggered = submit.clicked(actions)
            || username_input.returned(actions).is_some()
            || password_input.returned(actions).is_some()
            || confirm_password_input.returned(actions).is_some();

        if submit_triggered {
            if self.registration_pending {
                return;
            }
            use crate::register::validation::{
                validate_localpart, validate_passwords_match, LocalpartError, PasswordError,
            };

            let username = username_input.text();
            let password = password_input.text();
            let confirm = confirm_password_input.text();

            let localpart = match validate_localpart(&username) {
                Ok(l) => l,
                Err(LocalpartError::Empty) => {
                    self.show_form_error(cx, "Please enter a username.");
                    return;
                }
                Err(LocalpartError::TooLong) => {
                    self.show_form_error(cx, "Username is too long (max 255 characters).");
                    return;
                }
                Err(LocalpartError::InvalidChars) => {
                    self.show_form_error(
                        cx,
                        "Username can contain only lowercase letters, digits, and . _ = - /",
                    );
                    return;
                }
            };

            if let Err(e) = validate_passwords_match(&password, &confirm) {
                match e {
                    PasswordError::Empty => {
                        self.show_form_error(cx, "Please enter and confirm a password.");
                    }
                    PasswordError::Mismatch => {
                        self.show_form_error(cx, "Passwords don't match. Please re-enter.");
                    }
                }
                return;
            }

            let Some(caps) = self.last_discovery.as_ref() else {
                self.show_form_error(cx, "Please check the homeserver first (click Next).");
                return;
            };

            // Stale-cache check: compare current input to the input that PRODUCED
            // the cache, not `caps.base_url` (which `.well-known` may rewrite).
            let current_raw = self.view.text_input(cx, ids!(homeserver_input)).text();
            let current_url = match normalize_homeserver_url(&current_raw) {
                Ok(u) => u,
                Err(_) => {
                    self.last_discovery = None;
                    self.last_discovery_input_url = None;
                    self.show_form_error(
                        cx,
                        "The homeserver URL looks invalid. Please fix it and click Next again.",
                    );
                    return;
                }
            };
            let probed_input = self.last_discovery_input_url.as_deref().unwrap_or("");
            if current_url != probed_input {
                self.last_discovery = None;
                self.last_discovery_input_url = None;
                self.show_form_error(
                    cx,
                    "The homeserver changed since the last check. Click Next to verify this server before creating an account.",
                );
                return;
            }

            let homeserver_url = caps.base_url.clone();

            self.clear_form_error(cx);
            self.show_status(cx, "Creating your account...");
            self.registration_pending = true;
            submit.set_text(cx, "Creating...");
            self.view.redraw(cx);
            submit_async_request(MatrixRequest::RegisterViaUiaa {
                username: localpart,
                password,
                homeserver_url,
            });
            return;
        }

        for action in actions {
            match action.downcast_ref::<LoginAction>() {
                Some(LoginAction::LoginSuccess) => {
                    self.awaiting_sync_startup = false;
                    self.view.view(cx, ids!(status_area)).set_visible(cx, false);
                    self.view.label(cx, ids!(status_label)).set_text(cx, "");
                }
                Some(LoginAction::LoginFailure(msg)) if self.awaiting_sync_startup => {
                    // Account already exists on the server; don't frame as registration failure.
                    self.awaiting_sync_startup = false;
                    Cx::post_action(LoginAction::ClearFailureState);
                    self.show_status(
                        cx,
                        &format!(
                            "Your account was created, but we couldn't start a session:\n{msg}\n\n\
                             Please click ← Back to Login and sign in with your new account."
                        ),
                    );
                }
                _ => {}
            }
        }

        for action in actions {
            match action.downcast_ref::<RegisterAction>() {
                Some(RegisterAction::CapabilitiesDiscovered { requested_url, caps }) => {
                    // Drop out-of-order response from a superseded Next click.
                    if self.last_discovery_input_url.as_deref() != Some(requested_url.as_str()) {
                        continue;
                    }
                    self.discovery_pending = false;
                    self.view.button(cx, ids!(next_button)).set_text(cx, "Next");
                    let caps = caps.as_ref();
                    match caps.mode() {
                        RegisterMode::MasWebOnly => {
                            self.view.view(cx, ids!(registration_form)).set_visible(cx, false);
                            self.clear_form_error(cx);
                            match caps.mas_signup_url.as_deref() {
                                Some(url) => match robius_open::Uri::new(url).open() {
                                    Ok(()) => {
                                        self.show_status(
                                            cx,
                                            "Browser opened. Complete registration in your web browser, \
                                             then click ← Back to Login and sign in with your new account.",
                                        );
                                    }
                                    Err(e) => {
                                        log!("robius_open failed for MAS signup url {url}: {e:?}");
                                        self.show_status(
                                            cx,
                                            &format!(
                                                "Could not open the browser automatically. Please visit this URL manually:\n{url}"
                                            ),
                                        );
                                    }
                                },
                                None => {
                                    self.show_status(
                                        cx,
                                        "This server advertises browser-based registration but no signup URL was found.",
                                    );
                                }
                            }
                        }
                        RegisterMode::Uiaa => {
                            self.view.view(cx, ids!(registration_form)).set_visible(cx, true);
                            self.clear_form_error(cx);
                            self.show_status(
                                cx,
                                "This homeserver allows direct registration. Fill in your details below to create an account.",
                            );
                        }
                        RegisterMode::Disabled => {
                            self.view.view(cx, ids!(registration_form)).set_visible(cx, false);
                            self.clear_form_error(cx);
                            self.show_status(
                                cx,
                                "This server does not allow registration. Please choose a different homeserver \
                                 or sign in with an existing account.",
                            );
                        }
                    }
                    self.last_discovery = Some(caps.clone());
                }
                Some(RegisterAction::DiscoveryFailed { requested_url, error }) => {
                    if self.last_discovery_input_url.as_deref() != Some(requested_url.as_str()) {
                        continue;
                    }
                    self.discovery_pending = false;
                    self.view.button(cx, ids!(next_button)).set_text(cx, "Next");
                    self.view.view(cx, ids!(registration_form)).set_visible(cx, false);
                    self.clear_form_error(cx);
                    self.show_status(cx, &format!("Could not reach that server: {error}"));
                    self.last_discovery = None;
                    self.last_discovery_input_url = None;
                }
                Some(RegisterAction::RegistrationSubmitted) => {}
                Some(RegisterAction::RegistrationSuccess) => {
                    // Full reset: the same widget instance is reused on re-entry
                    // (logout → "Sign up here"), so password fields must not linger.
                    self.registration_pending = false;
                    self.discovery_pending = false;
                    self.view.button(cx, ids!(submit_button)).set_text(cx, "Create Account");
                    self.view.button(cx, ids!(next_button)).set_text(cx, "Next");

                    self.view.text_input(cx, ids!(password_input)).set_text(cx, "");
                    self.view.text_input(cx, ids!(confirm_password_input)).set_text(cx, "");
                    self.view.text_input(cx, ids!(username_input)).set_text(cx, "");
                    self.view.text_input(cx, ids!(homeserver_input)).set_text(cx, "");

                    self.last_discovery = None;
                    self.last_discovery_input_url = None;
                    self.view.view(cx, ids!(registration_form)).set_visible(cx, false);
                    self.clear_form_error(cx);

                    // Bridging feedback during the ~100-200ms SyncService::build window.
                    self.show_status(cx, "Account created! Loading your account...");
                    self.awaiting_sync_startup = true;
                }
                Some(RegisterAction::RegistrationFailed(err)) => {
                    self.registration_pending = false;
                    self.awaiting_sync_startup = false;
                    self.view.button(cx, ids!(submit_button)).set_text(cx, "Create Account");
                    self.show_form_error(cx, err);
                    self.show_status(
                        cx,
                        "Registration didn't go through. Please check the error above and retry.",
                    );
                }
                _ => {}
            }
        }
    }
}

impl RegisterScreen {
    fn show_status(&mut self, cx: &mut Cx, message: &str) {
        self.view.view(cx, ids!(status_area)).set_visible(cx, true);
        self.view.label(cx, ids!(status_label)).set_text(cx, message);
        self.view.redraw(cx);
    }

    fn show_form_error(&mut self, cx: &mut Cx, message: &str) {
        let label = self.view.label(cx, ids!(form_error_label));
        label.set_text(cx, message);
        label.set_visible(cx, true);
        self.view.redraw(cx);
    }

    fn clear_form_error(&mut self, cx: &mut Cx) {
        let label = self.view.label(cx, ids!(form_error_label));
        label.set_text(cx, "");
        label.set_visible(cx, false);
    }
}

#[cfg(test)]
mod tests {
    use super::can_start_capability_discovery;

    #[test]
    fn capability_discovery_blocks_while_registration_request_is_in_flight() {
        assert!(!can_start_capability_discovery(true, false));
    }

    #[test]
    fn capability_discovery_blocks_while_waiting_for_post_register_sync_startup() {
        assert!(!can_start_capability_discovery(false, true));
    }

    #[test]
    fn capability_discovery_allows_idle_register_screen() {
        assert!(can_start_capability_discovery(false, false));
    }
}
