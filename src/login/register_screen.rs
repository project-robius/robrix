// src/login/register_screen.rs

use std::ops::Not;

use makepad_widgets::*;

use crate::sliding_sync::{submit_async_request, AuthRequest, MatrixRequest, RegisterRequest};

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

    pub RegisterScreen = {{RegisterScreen}} {
        width: Fill,
        height: Fill,
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
                        margin: { bottom: 10 }
                        draw_text: {
                            color: (COLOR_TEXT)
                            text_style: <TITLE_TEXT>{font_size: 16.0}
                        }
                        text: "Register for Robrix"
                    }

                    username_input = <RobrixTextInput> {
                        width: 250, height: 40
                        empty_message: "User ID"

                    }

                    password_input = <RobrixTextInput> {
                        width: 250, height: 40
                        empty_message: "Password"
                        draw_text: { text_style: { is_secret: true } }
                    }

                    confirm_password_input = <RobrixTextInput> {
                        width: 250, height: 40
                        empty_message: "Confirm Password"
                        draw_text: { text_style: { is_secret: true } }
                    }

                    <View> {
                        width: 250, height: Fit,
                        flow: Down,

                        homeserver_input = <RobrixTextInput> {
                            width: Fill, height: 30,
                            empty_message: "matrix.org"
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

                    register_button = <RobrixIconButton> {
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
                        text: "Register"
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
                            draw_text: {
                                color: #8C8C8C
                                text_style: <REGULAR_TEXT>{font_size: 9}
                            }
                            text: "Already have an account?"
                        }

                        right_line = <LineH> {
                            draw_bg: { color: #C8C8C8 }
                        }
                    }

                    login_button = <RobrixIconButton> {
                        width: Fit, height: Fit
                        padding: {left: 15, right: 15, top: 10, bottom: 10}
                        margin: {bottom: 150}
                        align: {x: 0.5, y: 0.5}
                        draw_bg: {
                            color: (COLOR_ACTIVE_PRIMARY)
                        }
                        draw_text: {
                            color: (COLOR_PRIMARY)
                            text_style: <REGULAR_TEXT> {}
                        }

                        text: "Login here"
                    }
                }

                // The modal that pops up to display registration status messages
                register_status_modal = <Modal> {
                    content: {
                        register_status_modal_inner = <LoginStatusModal> {}
                    }
                }
            }
        }
    }
}

#[derive(Live, LiveHook, Widget)]
pub struct RegisterScreen {
    #[deref]
    view: View,
    #[rust]
    server_url: String,
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
        let register_button = self.view.button(id!(register_button));
        let login_button = self.view.button(id!(login_button));
        let username_input = self.view.text_input(id!(username_input));
        let password_input = self.view.text_input(id!(password_input));
        let confirm_password_input = self.view.text_input(id!(confirm_password_input));
        let homeserver_input = self.view.text_input(id!(homeserver_input));

        let register_status_modal = self.view.modal(id!(register_status_modal));
        let register_status_modal_inner = self
            .view
            .login_status_modal(id!(register_status_modal_inner));

        if login_button.clicked(actions) {
            log!("Back to login clicked");
            cx.action(RegisterAction::SwitchToLogin);
        }

        if register_button.clicked(actions)
            || username_input.returned(actions).is_some()
            || password_input.returned(actions).is_some()
            || confirm_password_input.returned(actions).is_some()
            || homeserver_input.returned(actions).is_some()
        {
            let username = username_input.text();
            let password = password_input.text();
            let confirm_password = confirm_password_input.text();
            let homeserver = homeserver_input.text();

            if username.is_empty() {
                register_status_modal_inner.set_title(cx, "Missing Username");
                register_status_modal_inner.set_status(cx, "Please enter a valid username.");
                register_status_modal_inner
                    .button_ref()
                    .set_text(cx, "Okay");
                register_status_modal.open(cx);
            } else if password.is_empty() {
                register_status_modal_inner.set_title(cx, "Missing Password");
                register_status_modal_inner.set_status(cx, "Please enter a valid password.");
                register_status_modal_inner
                    .button_ref()
                    .set_text(cx, "Okay");
                register_status_modal.open(cx);
            } else if password != confirm_password {
                register_status_modal_inner.set_title(cx, "Passwords Don't Match");
                register_status_modal_inner.set_status(
                    cx,
                    "The passwords you entered do not match. Please try again.",
                );
                register_status_modal_inner
                    .button_ref()
                    .set_text(cx, "Okay");
                register_status_modal.open(cx);
            } else {
                log!("Register attempt: {}", username);
                register_status_modal_inner.set_title(cx, "Registering...");
                register_status_modal_inner.set_status(cx, "Creating your account, please wait...");
                register_status_modal_inner
                    .button_ref()
                    .set_text(cx, "Cancel");
                register_status_modal.open(cx);

                submit_async_request(MatrixRequest::Auth(AuthRequest::RegisterRequest(RegisterRequest {
                    username,
                    password,
                    homeserver: homeserver.is_empty().not().then_some(homeserver),
                })));
            }
            self.redraw(cx);
        }

        for action in actions {
            if let LoginStatusModalAction::Close = action.as_widget_action().cast() {
                register_status_modal.close(cx);
            }

            // Handle registration-related actions received from background async tasks
            match action.downcast_ref() {
                Some(RegisterAction::Status { title, status }) => {
                    register_status_modal_inner.set_title(cx, title);
                    register_status_modal_inner.set_status(cx, status);
                    let register_status_modal_button = register_status_modal_inner.button_ref();
                    register_status_modal_button.set_text(cx, "Cancel");
                    register_status_modal_button.set_enabled(cx, true);
                    register_status_modal.open(cx);
                    self.redraw(cx);
                }
                Some(RegisterAction::RegisterSuccess) => {
                    // The main `App` component handles showing the main screen
                    username_input.set_text(cx, "");
                    password_input.set_text(cx, "");
                    confirm_password_input.set_text(cx, "");
                    homeserver_input.set_text(cx, "");
                    register_status_modal.close(cx);
                    self.redraw(cx);
                }
                Some(RegisterAction::RegisterFailure(error)) => {
                    register_status_modal_inner.set_title(cx, "Registration Failed");
                    register_status_modal_inner.set_status(cx, error);
                    let register_status_modal_button = register_status_modal_inner.button_ref();
                    register_status_modal_button.set_text(cx, "Okay");
                    register_status_modal_button.set_enabled(cx, true);
                    register_status_modal.open(cx);
                    self.redraw(cx);
                }
                _ => {}
            }
        }
    }
}

impl RegisterScreen {
    pub fn set_server_url(&mut self, cx: &mut Cx, url: &str) {
        self.server_url = url.to_string();
        if !url.is_empty() {
            self.view
                .text_input(id!(homeserver_input))
                .set_text(cx, url);
        }
    }
}

/// Actions sent to or from the register screen.
#[derive(Clone, DefaultNone, Debug)]
pub enum RegisterAction {
    /// Switch back to the login screen
    SwitchToLogin,
    /// Registration was successful
    RegisterSuccess,
    /// Registration failed
    RegisterFailure(String),
    /// A registration-related status message to display to the user
    Status {
        title: String,
        status: String,
    },
    None,
}