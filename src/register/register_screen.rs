//! Registration screen implementation with support for both password-based and SSO registration.
//!
//! # Supported Registration Methods
//!
//! ## 1. Password-based Registration (Custom Homeservers)
//! For custom Matrix homeservers, users can register with username/password:
//! - Minimum password length: 8 characters
//! - Automatic UIA handling for `m.login.dummy` flow
//! - Basic URL validation for custom homeserver addresses
//!
//! ## 2. SSO Registration (matrix.org)
//! For matrix.org, registration uses Google SSO by default:
//! - **Why Google SSO?** Following Element's implementation, matrix.org primarily uses
//!   Google OAuth as the main SSO provider for public registrations
//! - The SSO flow is shared with login - Matrix server automatically determines whether
//!   to create a new account or login an existing user based on the OAuth identity
//! - UI provides clear feedback during SSO process (button disabled, status modal)
//!
//! # Registration Flow
//!
//! ```text
//! Password Registration:                    SSO Registration:
//! User → Username/Password → Server         User → Continue with SSO → Browser OAuth
//!        ↓                                          ↓
//!        UIA Challenge (if needed)                 Google Authentication
//!        ↓                                          ↓
//!        Auto-handle m.login.dummy                 OAuth Callback
//!        ↓                                          ↓
//!        Registration Success                      Auto Login/Register
//! ```
//!
//! # SSO Action Handling Design
//!
//! The register screen uses source-aware SSO handling:
//!
//! ## How It Works
//! 1. Register screen sends `SpawnSSOServer` with `is_registration: true`
//! 2. `sliding_sync.rs` sends appropriate actions based on this flag:
//!    - For registration: `RegisterAction::SsoRegistrationPending/Status/Success/Failure`
//!    - For login: `LoginAction::SsoPending/Status/LoginSuccess/LoginFailure`
//! 3. Each screen only receives and handles its own actions
//!
//! ## Benefits
//! - **Zero Coupling:** Login and register screens are completely independent
//! - **Clear Intent:** The SSO flow knows its purpose from the start
//! - **No Action Conversion:** No need to intercept and convert actions
//! - **Maintainable:** Each screen has its own clear action flow
//!
//! # Implementation Notes
//! - SSO at protocol level doesn't distinguish login/register - server decides based on account existence
//! - Registration token support has been intentionally omitted for simplicity
//! - Advanced UIA flows (captcha, email verification) are not supported

use makepad_widgets::*;
use crate::sliding_sync::{submit_async_request, MatrixRequest, RegisterRequest};
use crate::login::login_screen::LoginAction;
use super::register_status_modal::RegisterStatusModalAction;

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::helpers::*;
    use crate::shared::styles::*;
    use crate::shared::icon_button::*;
    use crate::register::register_status_modal::RegisterStatusModal;

    IMG_APP_LOGO = dep("crate://self/resources/robrix_logo_alpha.png")
    
    MaskableButton = <RobrixIconButton> {
        draw_bg: {
            instance mask: 0.0
            fn pixel(self) -> vec4 {
                let base_color = mix(self.color, mix(self.color, self.color_hover, 0.2), self.hover);
                let gray = dot(base_color.rgb, vec3(0.299, 0.587, 0.114));
                return mix(base_color, vec4(gray, gray, gray, base_color.a), self.mask);
            }
        }
    }

    pub RegisterScreen = {{RegisterScreen}} {
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
                        text: "Create account"
                    }

                    // Homeserver selection area
                    <View> {
                        width: 250, height: Fit
                        flow: Down
                        spacing: 5

                        <Label> {
                            width: Fit, height: Fit
                            draw_text: {
                                color: (COLOR_TEXT)
                                text_style: <REGULAR_TEXT>{font_size: 11}
                            }
                            text: "Host account on"
                        }

                        homeserver_selector = <View> {
                            width: Fill, height: Fit
                            flow: Right
                            align: {x: 0.0, y: 0.5}
                            padding: {left: 10, right: 10, top: 8, bottom: 8}
                            spacing: 5

                            show_bg: true
                            draw_bg: {
                                color: (COLOR_SECONDARY)
                            }

                            selected_homeserver = <Label> {
                                width: Fill, height: Fit
                                draw_text: {
                                    color: (COLOR_TEXT)
                                    text_style: <REGULAR_TEXT>{font_size: 12}
                                }
                                text: "matrix.org"
                            }

                            edit_button = <RobrixIconButton> {
                                width: Fit, height: Fit
                                padding: {left: 8, right: 8, top: 4, bottom: 4}
                                draw_bg: {
                                    color: (COLOR_ACTIVE_PRIMARY)
                                }
                                draw_text: {
                                    color: (COLOR_PRIMARY)
                                    text_style: <REGULAR_TEXT>{font_size: 10}
                                }
                                text: "Edit"
                            }
                        }

                        homeserver_description = <Label> {
                            width: Fill, height: Fit
                            draw_text: {
                                color: (COLOR_TEXT)
                                text_style: <REGULAR_TEXT>{font_size: 9}
                            }
                            text: "Join millions for free on the largest public server"
                        }
                    }

                    // Homeserver selection options (initially hidden)
                    homeserver_options = <View> {
                        width: 250, height: Fit
                        flow: Down
                        spacing: 10
                        visible: false

                        show_bg: true
                        draw_bg: {
                            color: (COLOR_SECONDARY)
                        }
                        padding: 10

                        <Label> {
                            width: Fill, height: Fit
                            draw_text: {
                                color: (COLOR_TEXT)
                                text_style: <REGULAR_TEXT>{font_size: 10}
                            }
                            text: "Select a homeserver:"
                        }

                        matrix_option = <RobrixIconButton> {
                            width: Fill, height: Fit
                            padding: {left: 10, right: 10, top: 8, bottom: 8}
                            draw_bg: {
                                color: (COLOR_BG_DISABLED)
                            }
                            draw_text: {
                                color: (COLOR_TEXT)
                                text_style: <REGULAR_TEXT>{font_size: 11}
                            }
                            text: "● matrix.org"
                        }

                        other_option = <RobrixIconButton> {
                            width: Fill, height: Fit
                            padding: {left: 10, right: 10, top: 8, bottom: 8}
                            draw_bg: {
                                color: (COLOR_SECONDARY)
                            }
                            draw_text: {
                                color: (COLOR_TEXT)
                                text_style: <REGULAR_TEXT>{font_size: 11}
                            }
                            text: "○ Other homeserver"
                        }

                        custom_homeserver = <View> {
                            width: Fill, height: Fit
                            visible: false
                            
                            custom_homeserver_input = <RobrixTextInput> {
                                width: Fill, height: Fit
                                padding: {top: 5, bottom: 5}
                                empty_text: "your-server.com"
                                draw_text: {
                                    text_style: <REGULAR_TEXT>{font_size: 10}
                                }
                            }
                        }
                    }

                    // Dynamic registration area
                    sso_area = <View> {
                        width: 250, height: Fit
                        flow: Down
                        spacing: 10
                        visible: true

                        sso_button = <MaskableButton> {
                            width: Fill, height: 40
                            padding: 10
                            margin: {top: 10}
                            align: {x: 0.5, y: 0.5}
                            draw_bg: {
                                color: (COLOR_ACTIVE_PRIMARY)
                                mask: 0.0
                            }
                            draw_text: {
                                color: (COLOR_PRIMARY)
                                text_style: <REGULAR_TEXT> {}
                            }
                            text: "Continue with SSO"
                        }
                    }

                    password_area = <View> {
                        width: 250, height: Fit
                        flow: Down
                        spacing: 10
                        visible: false

                        username_input = <RobrixTextInput> {
                            width: Fill, height: Fit
                            flow: Right,
                            padding: 10,
                            empty_text: "Username"
                        }

                        password_input = <RobrixTextInput> {
                            width: Fill, height: Fit
                            flow: Right,
                            padding: 10,
                            empty_text: "Password"
                            is_password: true,
                        }

                        confirm_password_input = <RobrixTextInput> {
                            width: Fill, height: Fit
                            flow: Right,
                            padding: 10,
                            empty_text: "Confirm Password"
                            is_password: true,
                        }

                        register_button = <MaskableButton> {
                            width: Fill, height: 40
                            padding: 10
                            margin: {top: 5, bottom: 10}
                            align: {x: 0.5, y: 0.5}
                            draw_bg: {
                                color: (COLOR_ACTIVE_PRIMARY)
                                mask: 0.0
                            }
                            draw_text: {
                                color: (COLOR_PRIMARY)
                                text_style: <REGULAR_TEXT> {}
                            }
                            text: "Register"
                        }
                    }

                    <View> {
                        width: 250,
                        height: Fit,
                        flow: Right,
                        spacing: 0.0,
                        align: {x: 0.5, y: 0.5}

                        left_line = <LineH> {
                            draw_bg: { color: (COLOR_DIVIDER) }
                        }

                        <Label> {
                            width: Fit, height: Fit
                            padding: {left: 1, right: 1, top: 0, bottom: 0}
                            draw_text: {
                                color: (COLOR_TEXT)
                                text_style: <REGULAR_TEXT>{}
                            }
                            text: "Already have an account?"
                        }

                        right_line = <LineH> {
                            draw_bg: { color: (COLOR_DIVIDER) }
                        }
                    }

                    login_button = <RobrixIconButton> {
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
                        text: "Back to Login"
                    }
                }

                // Modal for registration status (both password and SSO)
                status_modal = <Modal> {
                    content: {
                        status_modal_inner = <RegisterStatusModal> {}
                    }
                }
            }
        }
    }
}

#[derive(Live, LiveHook, Widget)]
pub struct RegisterScreen {
    #[deref] view: View,
    #[rust] is_homeserver_editing: bool,
    #[rust] selected_homeserver: String,
    #[rust] sso_pending: bool,
}

impl RegisterScreen {
    fn toggle_homeserver_options(&mut self, cx: &mut Cx) {
        self.is_homeserver_editing = !self.is_homeserver_editing;
        self.view.view(ids!(homeserver_options)).set_visible(cx, self.is_homeserver_editing);
        self.redraw(cx);
    }
    
    fn show_warning(&self, message: &str) {
        use crate::shared::popup_list::{enqueue_popup_notification, PopupItem, PopupKind};
        enqueue_popup_notification(PopupItem {
            message: message.to_string(),
            kind: PopupKind::Warning,
            auto_dismissal_duration: Some(3.0),
        });
    }
    
    fn update_button_mask(&self, button: &ButtonRef, cx: &mut Cx, mask: f32) {
        button.apply_over(cx, live! {
            draw_bg: { mask: (mask) }
        });
    }
    
    fn reset_modal_state(&mut self, cx: &mut Cx) {
        let register_button = self.view.button(ids!(register_button));
        register_button.set_enabled(cx, true);
        register_button.reset_hover(cx);
        self.update_button_mask(&register_button, cx, 0.0);
        self.redraw(cx);
    }

    fn update_registration_mode(&mut self, cx: &mut Cx) {
        let is_matrix_org = self.selected_homeserver == "matrix.org" || self.selected_homeserver.is_empty();

        // Update UI based on homeserver selection
        self.view.view(ids!(sso_area)).set_visible(cx, is_matrix_org);
        self.view.view(ids!(password_area)).set_visible(cx, !is_matrix_org);

        // Update description text
        let desc_label = self.view.label(ids!(homeserver_description));
        if is_matrix_org {
            desc_label.set_text(cx, "Join millions for free on the largest public server");
        } else {
            desc_label.set_text(cx, "Use your custom Matrix homeserver");
        }

        self.redraw(cx);
    }

    /// Reset the registration screen to its initial state
    /// This should be called when navigating away from the registration screen
    pub fn reset_screen_state(&mut self, cx: &mut Cx) {
        // Reset internal state
        self.is_homeserver_editing = false;
        self.selected_homeserver = "matrix.org".to_string();
        self.sso_pending = false;

        // Reset homeserver selection UI
        self.view.view(ids!(homeserver_options)).set_visible(cx, false);
        self.view.label(ids!(selected_homeserver)).set_text(cx, "matrix.org");
        self.view.view(ids!(custom_homeserver)).set_visible(cx, false);

        // Reset homeserver option buttons
        let matrix_option_button = self.view.button(ids!(matrix_option));
        let other_option_button = self.view.button(ids!(other_option));
        matrix_option_button.set_text(cx, "● matrix.org");
        other_option_button.set_text(cx, "○ Other homeserver");

        // Clear input fields
        self.view.text_input(ids!(username_input)).set_text(cx, "");
        self.view.text_input(ids!(password_input)).set_text(cx, "");
        self.view.text_input(ids!(confirm_password_input)).set_text(cx, "");
        self.view.text_input(ids!(custom_homeserver_input)).set_text(cx, "");

        // Reset button states
        let register_button = self.view.button(ids!(register_button));
        register_button.set_enabled(cx, true);
        register_button.reset_hover(cx);
        self.update_button_mask(&register_button, cx, 0.0);

        let sso_button = self.view.button(ids!(sso_button));
        sso_button.set_enabled(cx, true);
        sso_button.reset_hover(cx);
        self.update_button_mask(&sso_button, cx, 0.0);

        // Close any open modals
        self.view.modal(ids!(status_modal)).close(cx);

        // Update registration mode to show correct UI for matrix.org
        self.update_registration_mode(cx);

        self.redraw(cx);
    }
}

impl Widget for RegisterScreen {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        self.match_event(cx, event);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl MatchEvent for RegisterScreen {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        let login_button = self.view.button(ids!(login_button));
        let edit_button = self.view.button(ids!(edit_button));
        let sso_button = self.view.button(ids!(sso_button));
        
        // Initialize selected_homeserver if empty
        if self.selected_homeserver.is_empty() {
            self.selected_homeserver = "matrix.org".to_string();
        }

        if login_button.clicked(actions) {
            cx.action(RegisterAction::NavigateToLogin);
        }
        
        // Handle Edit button click
        if edit_button.clicked(actions) {
            self.toggle_homeserver_options(cx);
        }

        // Handle SSO button click for matrix.org
        if sso_button.clicked(actions) && !self.sso_pending {
            // Mark SSO as pending for this screen
            self.sso_pending = true;
            self.update_button_mask(&sso_button, cx, 1.0);

            // Show SSO registration modal immediately
            let status_label = self.view.label(ids!(status_modal_inner.status));
            status_label.set_text(cx, "Opening your browser...\n\nPlease complete registration in your browser, then return to Robrix.");
            let cancel_button = self.view.button(ids!(status_modal_inner.cancel_button));
            cancel_button.set_text(cx, "Cancel");
            self.view.modal(ids!(status_modal)).open(cx);
            self.redraw(cx);

            // Use the same SSO flow as login screen - spawn SSO server with Google provider
            // This follows Element's implementation where SSO login and registration share the same OAuth flow
            // The Matrix server will handle whether to create a new account or login existing user
            submit_async_request(MatrixRequest::SpawnSSOServer{
                identity_provider_id: "oidc-google".to_string(),
                brand: "google".to_string(),
                homeserver_url: String::new(), // Use default matrix.org
                is_registration: true,
            });
        }
        
        // Handle homeserver selection buttons
        let matrix_option_button = self.view.button(ids!(matrix_option));
        let other_option_button = self.view.button(ids!(other_option));
        
        if matrix_option_button.clicked(actions) {
            self.selected_homeserver = "matrix.org".to_string();
            self.view.label(ids!(selected_homeserver)).set_text(cx, "matrix.org");
            self.view.view(ids!(custom_homeserver)).set_visible(cx, false);
            
            // Update button styles to show selection
            matrix_option_button.set_text(cx, "● matrix.org");
            other_option_button.set_text(cx, "○ Other homeserver");
            
            self.is_homeserver_editing = false;
            self.view.view(ids!(homeserver_options)).set_visible(cx, false);
            self.update_registration_mode(cx);
        }
        
        if other_option_button.clicked(actions) {
            self.view.view(ids!(custom_homeserver)).set_visible(cx, true);
            
            // Update button styles to show selection
            matrix_option_button.set_text(cx, "○ matrix.org");
            other_option_button.set_text(cx, "● Other homeserver");
        }
        
        // Handle custom homeserver input
        if let Some(text_event) = self.view.text_input(ids!(custom_homeserver_input)).changed(actions) {
            if !text_event.is_empty() {
                // Basic URL validation - ensure it starts with http:// or https://
                let trimmed = text_event.trim();
                let is_valid_url = trimmed.starts_with("http://") || trimmed.starts_with("https://") 
                    || (!trimmed.contains("://") && !trimmed.is_empty()); // Allow domain-only input
                
                if is_valid_url {
                    self.selected_homeserver = text_event.clone();
                    self.view.label(ids!(selected_homeserver)).set_text(cx, &text_event);
                    self.update_registration_mode(cx);
                }
            }
        }
        
        // Handle password-based registration
        let register_button = self.view.button(ids!(register_button));
        let username_input = self.view.text_input(ids!(username_input));
        let password_input = self.view.text_input(ids!(password_input));
        let confirm_password_input = self.view.text_input(ids!(confirm_password_input));

        if register_button.clicked(actions)
            || username_input.returned(actions).is_some()
            || password_input.returned(actions).is_some()
            || confirm_password_input.returned(actions).is_some()
        {
            let username = username_input.text();
            let password = password_input.text();
            let confirm_password = confirm_password_input.text();

            if username.is_empty() {
                self.show_warning("Username is required");
                return;
            }

            if password.is_empty() {
                self.show_warning("Password is required");
                return;
            }

            // Check password strength - minimum 8 characters
            if password.len() < 8 {
                self.show_warning("Password must be at least 8 characters");
                return;
            }

            if password != confirm_password {
                self.show_warning("Passwords do not match");
                return;
            }

            let homeserver = if self.selected_homeserver == "matrix.org" {
                None
            } else {
                Some(self.selected_homeserver.clone())
            };

            // Disable register button to prevent duplicate submissions
            register_button.set_enabled(cx, false);
            self.update_button_mask(&register_button, cx, 1.0);

            // Show registration status modal with appropriate text for password registration
            let status_label = self.view.label(ids!(status_modal_inner.status));
            status_label.set_text(cx, "Registering account, please wait...");
            let title_label = self.view.label(ids!(status_modal_inner.title));
            title_label.set_text(cx, "Registration Status");
            let cancel_button = self.view.button(ids!(status_modal_inner.cancel_button));
            cancel_button.set_text(cx, "Cancel");
            self.view.modal(ids!(status_modal)).open(cx);
            self.redraw(cx);

            // Submit registration request
            submit_async_request(MatrixRequest::Register(RegisterRequest {
                username,
                password,
                homeserver,
                registration_token: None,
            }));

            cx.action(RegisterAction::RegistrationSubmitted);
        }

        // Handle modal closing for both success and failure in one place
        for action in actions {
            // Handle RegisterStatusModal close action
            if let Some(RegisterStatusModalAction::Close { was_internal }) = action.downcast_ref::<RegisterStatusModalAction>() {
                if *was_internal {
                    self.view.modal(ids!(status_modal)).close(cx);
                }
                // Reset appropriate button based on registration type
                if self.sso_pending {
                    self.sso_pending = false;
                    self.update_button_mask(&sso_button, cx, 0.0);
                    sso_button.set_enabled(cx, true);
                    sso_button.reset_hover(cx);
                } else {
                    // Password registration - reset register button
                    self.reset_modal_state(cx);
                }
                self.redraw(cx);
            }

            // Handle SSO completion from login flow
            // SSO success ultimately goes through the login flow, so we listen for LoginSuccess
            if self.sso_pending {
                if let Some(LoginAction::LoginSuccess) = action.downcast_ref::<LoginAction>() {
                    // SSO registration successful
                    self.view.modal(ids!(status_modal)).close(cx);
                    self.sso_pending = false;
                    self.update_button_mask(&sso_button, cx, 0.0);
                    cx.action(RegisterAction::RegistrationSuccess);
                    self.redraw(cx);
                }
            }
            
            // Handle RegisterAction for SSO (now directly sent from sliding_sync.rs)
            match action.downcast_ref::<RegisterAction>() {
                Some(RegisterAction::SsoRegistrationPending(pending)) => {
                    // Update pending state (modal already shown when button clicked)
                    if !*pending {
                        // SSO ended
                        self.sso_pending = false;
                        self.update_button_mask(&sso_button, cx, 0.0);
                        self.view.modal(ids!(status_modal)).close(cx);
                    }
                    self.redraw(cx);
                }
                Some(RegisterAction::SsoRegistrationStatus { status }) => {
                    // Update SSO status in modal (only if our modal is already open)
                    if self.sso_pending {
                        let status_label = self.view.label(ids!(status_modal_inner.status));
                        status_label.set_text(cx, status);
                        let cancel_button = self.view.button(ids!(status_modal_inner.cancel_button));
                        cancel_button.set_text(cx, "Cancel");
                        self.redraw(cx);
                    }
                }
                _ => {}
            }

            if let Some(reg_action) = action.downcast_ref::<RegisterAction>() {
                match reg_action {
                    RegisterAction::RegistrationSuccess => {
                        // Close modal and let app.rs handle screen transition
                        self.view.modal(ids!(status_modal)).close(cx);
                        if self.sso_pending {
                            self.sso_pending = false;
                            self.update_button_mask(&sso_button, cx, 0.0);
                        }
                        self.redraw(cx);
                    }
                    RegisterAction::RegistrationFailure(error) => {
                        // Show error and reset buttons
                        if self.sso_pending {
                            self.show_warning(error);
                            self.sso_pending = false;
                            self.update_button_mask(&sso_button, cx, 0.0);
                        }
                        self.view.modal(ids!(status_modal)).close(cx);
                        let register_button = self.view.button(ids!(register_button));
                        register_button.set_enabled(cx, true);
                        register_button.reset_hover(cx);
                        self.redraw(cx);
                    }
                    _ => {}
                }
            }
        }
    }
}

/// Actions for the registration screen.
/// 
/// These actions handle both password-based and SSO registration flows.
/// SSO actions are completely independent from LoginAction to ensure
/// no interference between login and register screens.
#[derive(Clone, DefaultNone, Debug)]
pub enum RegisterAction {
    /// User requested to go back to the login screen
    NavigateToLogin,
    /// Password registration was submitted (internal use)
    RegistrationSubmitted,
    /// Registration completed successfully (both password and SSO)
    RegistrationSuccess,
    /// Registration failed with error message (both password and SSO)
    RegistrationFailure(String),
    /// SSO registration state changed
    /// - `true`: SSO flow started, button should be disabled
    /// - `false`: SSO flow ended, button should be re-enabled
    SsoRegistrationPending(bool),
    /// SSO registration progress update (e.g., "Opening browser...")
    SsoRegistrationStatus { status: String },
    None,
}

impl RegisterScreenRef {
    pub fn reset_screen_state(&self, cx: &mut Cx) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.reset_screen_state(cx);
        }
    }
}