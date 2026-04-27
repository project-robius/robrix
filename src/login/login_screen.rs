use std::ops::Not;

use makepad_widgets::*;
use url::Url;

use crate::{app::AppState, i18n::{AppLanguage, tr_fmt, tr_key}, sliding_sync::{submit_async_request, AccountSwitchAction, LoginByPassword, LoginRequest, MatrixRequest, RegisterAccount}};

use super::login_status_modal::{LoginStatusModalAction, LoginStatusModalWidgetExt};

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.IMG_APP_LOGO = crate_resource("self://resources/robrix_logo_alpha.png")
    mod.widgets.ICON_EYE_OPEN   = crate_resource("self://resources/icons/eye_open.svg")
    mod.widgets.ICON_EYE_CLOSED = crate_resource("self://resources/icons/eye_closed.svg")

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
        flow: Overlay
        align: Align{x: 0.5, y: 0.5}
        show_bg: true,
        draw_bg +: {
            color: COLOR_SECONDARY
        }

        ScrollYView {
            width: Fill, height: Fill,
            flow: Down, // Required for vertical scrolling to work.
            align: Align{x: 0.5, y: 0.5}
            show_bg: true,
            draw_bg.color: (COLOR_SECONDARY)

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

                    View {
                        width: 275, height: Fit
                        flow: Overlay,

                        password_input := RobrixTextInput {
                            width: Fill, height: Fit
                            flow: Right, // do not wrap
                            padding: Inset{top: 10, bottom: 10, left: 10, right: 40}
                            empty_text: "Password"
                            is_password: true,
                        }

                        View {
                            width: Fill, height: Fill
                            align: Align{x: 1.0, y: 0.5}

                            show_password_button := Button {
                                width: 36, height: 36,
                                padding: 6,
                                draw_bg +: {
                                    color: #0000
                                    color_hover: #0000
                                    color_down: #0000
                                    border_size: 0.0
                                }
                                draw_icon +: {
                                    svg: (mod.widgets.ICON_EYE_CLOSED),
                                    color: #8C8C8C,
                                }
                                icon_walk: Walk{width: 20, height: 20}
                                text: ""
                            }

                            hide_password_button := Button {
                                visible: false,
                                width: 36, height: 36,
                                padding: 6,
                                draw_bg +: {
                                    color: #0000
                                    color_hover: #0000
                                    color_down: #0000
                                    border_size: 0.0
                                }
                                draw_icon +: {
                                    svg: (mod.widgets.ICON_EYE_OPEN),
                                    color: #8C8C8C,
                                }
                                icon_walk: Walk{width: 20, height: 20}
                                text: ""
                            }
                        }
                    }

                    confirm_password_wrapper := View {
                        width: 275, height: Fit,
                        visible: false,
                        flow: Overlay,

                        confirm_password_input := RobrixTextInput {
                            width: Fill, height: Fit
                            flow: Right, // do not wrap
                            padding: Inset{top: 10, bottom: 10, left: 10, right: 40}
                            empty_text: "Confirm password"
                            is_password: true,
                        }

                        View {
                            width: Fill, height: Fill
                            align: Align{x: 1.0, y: 0.5}

                            show_confirm_password_button := Button {
                                width: 36, height: 36,
                                padding: 6,
                                draw_bg +: {
                                    color: #0000
                                    color_hover: #0000
                                    color_down: #0000
                                    border_size: 0.0
                                }
                                draw_icon +: {
                                    svg: (mod.widgets.ICON_EYE_CLOSED),
                                    color: #8C8C8C,
                                }
                                icon_walk: Walk{width: 20, height: 20}
                                text: ""
                            }

                            hide_confirm_password_button := Button {
                                visible: false,
                                width: 36, height: 36,
                                padding: 6,
                                draw_bg +: {
                                    color: #0000
                                    color_hover: #0000
                                    color_down: #0000
                                    border_size: 0.0
                                }
                                draw_icon +: {
                                    svg: (mod.widgets.ICON_EYE_OPEN),
                                    color: #8C8C8C,
                                }
                                icon_walk: Walk{width: 20, height: 20}
                                text: ""
                            }
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

                            homeserver_hint_label := Label {
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

                        sso_prompt_label := Label {
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
                        width: Fit, height: Fit,
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
                    can_dismiss: false,
                    content +: {
                        login_status_modal_inner := mod.widgets.LoginStatusModal {}
                    }
                }

                proxy_settings_modal := Modal {
                    can_dismiss: true,
                    content +: {
                        proxy_settings_modal_inner := RoundedView {
                            width: 380, height: Fit,
                            flow: Down
                            spacing: 12.0
                            padding: Inset{top: 18, left: 16, right: 16, bottom: 16}
                            show_bg: true
                            draw_bg +: {
                                color: (COLOR_PRIMARY)
                                border_radius: 10.0
                                border_size: 1.0
                                border_color: #D8D8D8
                            }

                            proxy_settings_header := View {
                                width: Fill, height: Fit,
                                flow: Right,
                                align: Align{x: 1.0, y: 0.5}
                                spacing: 8.0

                                proxy_settings_title := Label {
                                    width: Fill, height: Fit
                                    draw_text +: {
                                        color: (COLOR_ACTIVE_PRIMARY)
                                        text_style: TITLE_TEXT {font_size: 14}
                                    }
                                    text: "Network proxy settings"
                                }

                                proxy_settings_close_button := RobrixNeutralIconButton {
                                    width: Fit, height: Fit
                                    padding: Inset{left: 7, right: 4, top: 7, bottom: 7}
                                    text: ""
                                    icon_walk: Walk{width: 14, height: 14, margin: 0}
                                    draw_icon.svg: (ICON_CLOSE)
                                }
                            }

                            proxy_use_card := RoundedView {
                                width: Fill, height: Fit,
                                flow: Right,
                                align: Align{x: 1.0, y: 0.5}
                                show_bg: true
                                draw_bg +: {
                                    color: #F5F5F5
                                    border_radius: 8.0
                                    border_size: 1.0
                                    border_color: #DADADA
                                }
                                padding: Inset{top: 12, bottom: 12, left: 12, right: 12}

                                proxy_use_label := Label {
                                    width: Fill, height: Fit
                                    draw_text +: {
                                        color: (COLOR_TEXT)
                                        text_style: TITLE_TEXT {font_size: 12}
                                    }
                                    text: "Use proxy"
                                }

                                proxy_use_toggle := Toggle {
                                    width: 52, height: 28
                                    text: ""
                                    active: false
                                    icon_walk: Walk{width: 0, height: 0, margin: 0}
                                    label_walk: Walk{width: 0, height: 0, margin: 0}
                                    draw_bg +: {
                                        size: 18.0
                                        color: #E3E7EF
                                        color_hover: #E3E7EF
                                        color_down: #D5DBE6
                                        color_active: (COLOR_ACTIVE_PRIMARY)
                                        border_radius: 14.0
                                        border_size: 1.5
                                        border_color: #7E879A
                                        border_color_hover: #7E879A
                                        border_color_down: #6F788D
                                        border_color_active: (COLOR_ACTIVE_PRIMARY_DARKER)
                                        mark_color: #2D3A57
                                        mark_color_hover: #2D3A57
                                        mark_color_down: #2D3A57
                                        mark_color_active: #FFFFFF
                                        mark_color_active_hover: #FFFFFF
                                    }
                                }
                            }

                            proxy_fields_section := RoundedView {
                                visible: false
                                width: Fill, height: Fit,
                                flow: Down
                                spacing: 0
                                show_bg: true
                                draw_bg +: {
                                    color: #F5F5F5
                                    border_radius: 8.0
                                    border_size: 1.0
                                    border_color: #DADADA
                                }
                                padding: Inset{top: 4, left: 12, right: 12, bottom: 8}

                                proxy_address_row := View {
                                    width: Fill, height: Fit,
                                    flow: Right
                                    align: Align{y: 0.5}
                                    spacing: 8.0
                                    padding: Inset{top: 8, bottom: 8}

                                    proxy_address_label := Label {
                                        width: 90, height: Fit
                                        draw_text +: {
                                            color: (COLOR_TEXT)
                                            text_style: TITLE_TEXT {font_size: 12}
                                        }
                                        text: "Address"
                                    }

                                    proxy_address_input := RobrixTextInput {
                                        width: Fill, height: Fit,
                                        flow: Right,
                                        empty_text: "127.0.0.1"
                                        padding: Inset{top: 5, bottom: 5, left: 10, right: 10}
                                    }
                                }

                                LineH { draw_bg.color: #DDDDDD }

                                proxy_port_row := View {
                                    width: Fill, height: Fit,
                                    flow: Right
                                    align: Align{y: 0.5}
                                    spacing: 8.0
                                    padding: Inset{top: 8, bottom: 8}

                                    proxy_port_label := Label {
                                        width: 90, height: Fit
                                        draw_text +: {
                                            color: (COLOR_TEXT)
                                            text_style: TITLE_TEXT {font_size: 12}
                                        }
                                        text: "Port"
                                    }

                                    proxy_port_input := RobrixTextInput {
                                        width: Fill, height: Fit,
                                        flow: Right,
                                        empty_text: "7890"
                                        padding: Inset{top: 5, bottom: 5, left: 10, right: 10}
                                    }
                                }

                                LineH { draw_bg.color: #DDDDDD }

                                proxy_account_row := View {
                                    width: Fill, height: Fit,
                                    flow: Right
                                    align: Align{y: 0.5}
                                    spacing: 8.0
                                    padding: Inset{top: 8, bottom: 8}

                                    proxy_account_label := Label {
                                        width: 90, height: Fit
                                        draw_text +: {
                                            color: (COLOR_TEXT)
                                            text_style: TITLE_TEXT {font_size: 12}
                                        }
                                        text: "Account"
                                    }

                                    proxy_account_input := RobrixTextInput {
                                        width: Fill, height: Fit,
                                        flow: Right,
                                        empty_text: ""
                                        padding: Inset{top: 5, bottom: 5, left: 10, right: 10}
                                    }
                                }

                                LineH { draw_bg.color: #DDDDDD }

                                proxy_password_row := View {
                                    width: Fill, height: Fit,
                                    flow: Right
                                    align: Align{y: 0.5}
                                    spacing: 8.0
                                    padding: Inset{top: 8, bottom: 8}

                                    proxy_password_label := Label {
                                        width: 90, height: Fit
                                        draw_text +: {
                                            color: (COLOR_TEXT)
                                            text_style: TITLE_TEXT {font_size: 12}
                                        }
                                        text: "Password"
                                    }

                                    proxy_password_input := RobrixTextInput {
                                        width: Fill, height: Fit,
                                        flow: Right,
                                        empty_text: ""
                                        is_password: true,
                                        padding: Inset{top: 5, bottom: 5, left: 10, right: 10}
                                    }
                                }
                            }

                            proxy_settings_save_button := RobrixIconButton {
                                width: 120, height: 40
                                align: Align{x: 0.5, y: 0.5}
                                text: "Save"
                            }
                        }
                    }
                }
            }

        }

        proxy_settings_button_anchor := View {
            width: Fill, height: Fill
            flow: Down
            align: Align{x: 0.0, y: 0.0}

            View {
                width: Fill, height: Fit
                flow: Right
                padding: Inset{top: 10, right: 10}

                View {
                    width: Fill, height: Fit
                }

                proxy_settings_button := RobrixNeutralIconButton {
                    width: Fit, height: Fit
                    spacing: 0
                    padding: 8
                    text: ""
                    label_walk: Walk{width: 0, height: 0, margin: 0}
                    icon_walk: Walk{width: 14, height: 14, margin: 0}
                    draw_icon.svg: (ICON_SETTINGS)
                }
            }
        }
    }
}

#[derive(Script, Widget)]
pub struct LoginScreen {
    #[source] source: ScriptObjectRef,
    #[deref] view: View,
    /// Whether the screen is showing the in-app sign-up flow.
    #[rust] signup_mode: bool,
    /// Whether the password field is currently showing plaintext.
    #[rust] password_visible: bool,
    /// Whether the confirm password field is currently showing plaintext.
    #[rust] confirm_password_visible: bool,
    /// Boolean to indicate if the SSO login process is still in flight
    #[rust] sso_pending: bool,
    /// The URL to redirect to after logging in with SSO.
    #[rust] sso_redirect_url: Option<String>,
    /// The most recent login failure message shown to the user.
    #[rust] last_failure_message_shown: Option<String>,
    #[rust] app_language: AppLanguage,
    /// Boolean to indicate if we're in "add account" mode (adding another Matrix account).
    #[rust] adding_account: bool,
    #[rust] use_proxy_enabled: bool,
}

impl LoginScreen {
    fn sync_proxy_settings_modal_layout(&mut self, cx: &mut Cx) {
        let rect = self.view.area().rect(cx);
        let available_width = (rect.size.x - 24.0).max(260.0);
        let modal_width = available_width.min(380.0);
        let mut proxy_settings_modal_inner = self.view.view(cx, ids!(proxy_settings_modal_inner));
        script_apply_eval!(cx, proxy_settings_modal_inner, {
            width: #(modal_width)
        });
    }

    fn set_sso_pending_state(&mut self, cx: &mut Cx, pending: bool) {
        let mask = if pending { 1.0 } else { 0.0 };
        let cursor = if pending { MouseCursor::NotAllowed } else { MouseCursor::Hand };
        let button_set: &[&[LiveId]] = ids_array!(
            apple_button,
            facebook_button,
            github_button,
            gitlab_button,
            google_button,
            twitter_button
        );
        for view_ref in self.view_set(cx, button_set).iter() {
            let Some(mut view_mut) = view_ref.borrow_mut() else { continue };
            let mut image = view_mut.image(cx, ids!(image));
            script_apply_eval!(cx, image, {
                draw_bg.mask: #(mask)
            });
            view_mut.cursor = Some(cursor);
        }
        self.sso_pending = pending;
    }

    fn reset_sso_state(&mut self, cx: &mut Cx) {
        self.sso_redirect_url = None;
        self.set_sso_pending_state(cx, false);
    }

    fn sync_mode_texts(&mut self, cx: &mut Cx) {
        self.view.label(cx, ids!(title)).set_text(cx,
            if self.signup_mode {
                tr_key(self.app_language, "login.title.create_account")
            } else {
                tr_key(self.app_language, "login.title.login_to_robrix")
            }
        );
        self.view.button(cx, ids!(login_button)).set_text(cx,
            if self.signup_mode {
                tr_key(self.app_language, "login.button.create_account")
            } else {
                tr_key(self.app_language, "login.button.login")
            }
        );
        self.view.label(cx, ids!(account_prompt_label)).set_text(cx,
            if self.signup_mode {
                tr_key(self.app_language, "login.account_prompt.already_have")
            } else {
                tr_key(self.app_language, "login.account_prompt.no_account")
            }
        );
        self.view.button(cx, ids!(mode_toggle_button)).set_text(cx,
            if self.signup_mode {
                tr_key(self.app_language, "login.mode_toggle.back_to_login")
            } else {
                tr_key(self.app_language, "login.mode_toggle.sign_up_here")
            }
        );
    }

    fn set_app_language(&mut self, cx: &mut Cx, app_language: AppLanguage) {
        self.app_language = app_language;
        self.view.text_input(cx, ids!(user_id_input))
            .set_empty_text(cx, tr_key(self.app_language, "login.input.user_id").to_string());
        self.view.text_input(cx, ids!(password_input))
            .set_empty_text(cx, tr_key(self.app_language, "login.input.password").to_string());
        self.view.text_input(cx, ids!(confirm_password_input))
            .set_empty_text(cx, tr_key(self.app_language, "login.input.confirm_password").to_string());
        self.view.text_input(cx, ids!(homeserver_input))
            .set_empty_text(cx, tr_key(self.app_language, "login.input.homeserver").to_string());
        self.view.text_input(cx, ids!(proxy_address_input))
            .set_empty_text(cx, tr_key(self.app_language, "login.proxy_settings.input.address").to_string());
        self.view.text_input(cx, ids!(proxy_port_input))
            .set_empty_text(cx, tr_key(self.app_language, "login.proxy_settings.input.port").to_string());
        self.view.text_input(cx, ids!(proxy_account_input))
            .set_empty_text(cx, tr_key(self.app_language, "login.proxy_settings.input.account").to_string());
        self.view.text_input(cx, ids!(proxy_password_input))
            .set_empty_text(cx, tr_key(self.app_language, "login.proxy_settings.input.password").to_string());
        self.view.label(cx, ids!(homeserver_hint_label))
            .set_text(cx, tr_key(self.app_language, "login.label.homeserver_optional"));
        self.view.label(cx, ids!(proxy_settings_title))
            .set_text(cx, tr_key(self.app_language, "login.proxy_settings.title"));
        self.view.label(cx, ids!(proxy_use_label))
            .set_text(cx, tr_key(self.app_language, "login.proxy_settings.use_proxy"));
        self.view.label(cx, ids!(proxy_address_label))
            .set_text(cx, tr_key(self.app_language, "login.proxy_settings.address"));
        self.view.label(cx, ids!(proxy_port_label))
            .set_text(cx, tr_key(self.app_language, "login.proxy_settings.port"));
        self.view.label(cx, ids!(proxy_account_label))
            .set_text(cx, tr_key(self.app_language, "login.proxy_settings.account"));
        self.view.label(cx, ids!(proxy_password_label))
            .set_text(cx, tr_key(self.app_language, "login.proxy_settings.password"));
        self.view.button(cx, ids!(proxy_settings_save_button))
            .set_text(cx, tr_key(self.app_language, "login.proxy_settings.save"));
        self.view.label(cx, ids!(sso_prompt_label))
            .set_text(cx, tr_key(self.app_language, "login.sso.prompt"));
        let login_status_modal_inner = self.view.login_status_modal(cx, ids!(login_status_modal_inner));
        login_status_modal_inner.set_title(cx, tr_key(self.app_language, "login_status_modal.title"));
        login_status_modal_inner.button_ref(cx).set_text(cx, tr_key(self.app_language, "login_status_modal.button.cancel"));
        self.sync_mode_texts(cx);
    }

    fn set_use_proxy_enabled(&mut self, cx: &mut Cx, enabled: bool) {
        self.use_proxy_enabled = enabled;
        self.view
            .check_box(cx, ids!(proxy_use_toggle))
            .set_active(cx, enabled);
        self.view
            .view(cx, ids!(proxy_fields_section))
            .set_visible(cx, enabled);
        self.redraw(cx);
    }

    fn load_saved_proxy_to_form(&mut self, cx: &mut Cx) {
        let saved_proxy = crate::proxy_config::load_saved_proxy_url();
        let Some(saved_proxy) = saved_proxy else {
            self.set_use_proxy_enabled(cx, false);
            self.view.text_input(cx, ids!(proxy_address_input)).set_text(cx, "");
            self.view.text_input(cx, ids!(proxy_port_input)).set_text(cx, "");
            self.view.text_input(cx, ids!(proxy_account_input)).set_text(cx, "");
            self.view.text_input(cx, ids!(proxy_password_input)).set_text(cx, "");
            return;
        };

        let Ok(parsed_url) = Url::parse(&saved_proxy) else {
            self.set_use_proxy_enabled(cx, true);
            self.view.text_input(cx, ids!(proxy_address_input)).set_text(cx, &saved_proxy);
            self.view.text_input(cx, ids!(proxy_port_input)).set_text(cx, "");
            self.view.text_input(cx, ids!(proxy_account_input)).set_text(cx, "");
            self.view.text_input(cx, ids!(proxy_password_input)).set_text(cx, "");
            return;
        };

        self.set_use_proxy_enabled(cx, true);
        self.view
            .text_input(cx, ids!(proxy_address_input))
            .set_text(cx, parsed_url.host_str().unwrap_or_default());
        self.view
            .text_input(cx, ids!(proxy_port_input))
            .set_text(cx, &parsed_url.port().map(|p| p.to_string()).unwrap_or_default());
        self.view
            .text_input(cx, ids!(proxy_account_input))
            .set_text(cx, parsed_url.username());
        self.view
            .text_input(cx, ids!(proxy_password_input))
            .set_text(cx, parsed_url.password().unwrap_or_default());
    }

    fn build_proxy_url_from_form(&mut self, cx: &mut Cx) -> Result<Option<String>, String> {
        if !self.use_proxy_enabled {
            return Ok(None);
        }

        let address = self.view.text_input(cx, ids!(proxy_address_input)).text();
        let port_text = self.view.text_input(cx, ids!(proxy_port_input)).text();
        let account = self.view.text_input(cx, ids!(proxy_account_input)).text();
        let password = self.view.text_input(cx, ids!(proxy_password_input)).text();

        let address = address.trim().to_owned();
        let port_text = port_text.trim().to_owned();
        let account = account.trim().to_owned();
        let password = password.trim().to_owned();

        if address.is_empty() {
            return Err(tr_key(self.app_language, "login.proxy_settings.error.missing_address").to_string());
        }

        if port_text.is_empty() {
            return Err(tr_key(self.app_language, "login.proxy_settings.error.missing_port").to_string());
        }

        let port: u16 = port_text
            .parse()
            .map_err(|_| tr_key(self.app_language, "login.proxy_settings.error.invalid_port").to_string())?;

        let mut proxy_url = if address.contains("://") {
            Url::parse(&address)
                .map_err(|e| format!("Invalid proxy URL: {e}"))?
        } else {
            let mut url = Url::parse("http://127.0.0.1")
                .map_err(|e| format!("Failed to initialize proxy URL builder: {e}"))?;
            url.set_host(Some(&address))
                .map_err(|e| format!("Invalid proxy address `{address}`: {e}"))?;
            url
        };

        proxy_url
            .set_port(Some(port))
            .map_err(|()| format!("Invalid proxy port `{port}`"))?;

        if account.is_empty() {
            proxy_url
                .set_username("")
                .map_err(|()| String::from("Invalid proxy account value"))?;
            proxy_url
                .set_password(None)
                .map_err(|()| String::from("Invalid proxy password value"))?;
        } else {
            proxy_url
                .set_username(&account)
                .map_err(|()| String::from("Invalid proxy account value"))?;
            if password.is_empty() {
                proxy_url
                    .set_password(None)
                    .map_err(|()| String::from("Invalid proxy password value"))?;
            } else {
                proxy_url
                    .set_password(Some(&password))
                    .map_err(|()| String::from("Invalid proxy password value"))?;
            }
        }

        let proxy_url = proxy_url.to_string();
        crate::proxy_config::validate_proxy_url(&proxy_url)?;
        Ok(Some(proxy_url))
    }

    fn set_signup_mode(&mut self, cx: &mut Cx, signup_mode: bool) {
        self.signup_mode = signup_mode;
        self.view.view(cx, ids!(confirm_password_wrapper)).set_visible(cx, signup_mode);
        self.view.view(cx, ids!(login_only_view)).set_visible(cx, !signup_mode);
        self.sync_mode_texts(cx);

        if !signup_mode {
            self.view.text_input(cx, ids!(confirm_password_input)).set_text(cx, "");
        }

        self.redraw(cx);
    }
}

impl ScriptHook for LoginScreen {
    fn on_after_new(&mut self, vm: &mut ScriptVm) {
        vm.with_cx_mut(|cx| {
            self.load_saved_proxy_to_form(cx);
            self.set_app_language(cx, self.app_language);
            self.sync_proxy_settings_modal_layout(cx);
        });
    }
}


impl Widget for LoginScreen {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        let app_language = scope.data.get::<AppState>()
            .map(|app_state| app_state.app_language)
            .unwrap_or_default();
        if self.app_language != app_language {
            self.set_app_language(cx, app_language);
        }
        if matches!(event, Event::WindowGeomChange(_)) {
            self.sync_proxy_settings_modal_layout(cx);
        }
        self.view.handle_event(cx, event, scope);
        self.widget_match_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        let app_language = scope.data.get::<AppState>()
            .map(|app_state| app_state.app_language)
            .unwrap_or_default();
        if self.app_language != app_language {
            self.set_app_language(cx, app_language);
        }
        self.view.draw_walk(cx, scope, walk)
    }
}

impl WidgetMatchEvent for LoginScreen {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions, _scope: &mut Scope) {
        let login_button = self.view.button(cx, ids!(login_button));
        let mode_toggle_button = self.view.button(cx, ids!(mode_toggle_button));
        let cancel_button = self.view.button(cx, ids!(cancel_button));
        let user_id_input = self.view.text_input(cx, ids!(user_id_input));
        let password_input = self.view.text_input(cx, ids!(password_input));
        let confirm_password_input = self.view.text_input(cx, ids!(confirm_password_input));
        let homeserver_input = self.view.text_input(cx, ids!(homeserver_input));

        let login_status_modal = self.view.modal(cx, ids!(login_status_modal));
        let login_status_modal_inner = self.view.login_status_modal(cx, ids!(login_status_modal_inner));
        let proxy_settings_modal = self.view.modal(cx, ids!(proxy_settings_modal));

        if self.view.button(cx, ids!(proxy_settings_button)).clicked(actions) {
            self.sync_proxy_settings_modal_layout(cx);
            proxy_settings_modal.open(cx);
            self.redraw(cx);
        }

        if self.view.button(cx, ids!(proxy_settings_close_button)).clicked(actions) {
            proxy_settings_modal.close(cx);
            self.redraw(cx);
        }

        if let Some(enabled) = self.view.check_box(cx, ids!(proxy_use_toggle)).changed(actions) {
            self.set_use_proxy_enabled(cx, enabled);
        }

        if self.view.button(cx, ids!(proxy_settings_save_button)).clicked(actions) {
            match self.build_proxy_url_from_form(cx) {
                Ok(proxy_url) => {
                    if let Err(e) = crate::proxy_config::save_proxy_url(proxy_url.as_deref()) {
                        warning!("Failed to persist proxy configuration from proxy settings modal: {e}");
                        login_status_modal_inner.set_title(cx, tr_key(self.app_language, "login.status.invalid_proxy.title"));
                        let error_text = tr_fmt(self.app_language, "login.status.invalid_proxy.body", &[
                            ("error", e.as_str()),
                        ]);
                        login_status_modal_inner.set_status(cx, &error_text);
                        login_status_modal_inner.button_ref(cx).set_text(cx, tr_key(self.app_language, "login.status.okay"));
                        login_status_modal.open(cx);
                    } else {
                        proxy_settings_modal.close(cx);
                        login_status_modal_inner.set_title(cx, tr_key(self.app_language, "login.proxy_settings.saved.title"));
                        login_status_modal_inner.set_status(cx, tr_key(self.app_language, "login.proxy_settings.saved.body"));
                        login_status_modal_inner.button_ref(cx).set_text(cx, tr_key(self.app_language, "login.status.okay"));
                        login_status_modal.open(cx);
                    }
                    self.redraw(cx);
                }
                Err(proxy_validation_error) => {
                    login_status_modal_inner.set_title(cx, tr_key(self.app_language, "login.status.invalid_proxy.title"));
                    let error_text = tr_fmt(self.app_language, "login.status.invalid_proxy.body", &[
                        ("error", proxy_validation_error.as_str()),
                    ]);
                    login_status_modal_inner.set_status(cx, &error_text);
                    login_status_modal_inner.button_ref(cx).set_text(cx, tr_key(self.app_language, "login.status.okay"));
                    login_status_modal.open(cx);
                    self.redraw(cx);
                }
            }
        }

        // Handle cancel button for add-account mode
        if cancel_button.clicked(actions) {
            self.adding_account = false;
            self.reset_sso_state(cx);
            // Reset the UI back to normal login mode
            self.view.label(cx, ids!(title)).set_text(cx, tr_key(self.app_language, "login.title.login_to_robrix"));
            cancel_button.set_visible(cx, false);
            self.view.view(cx, ids!(sso_view)).set_visible(cx, true);
            mode_toggle_button.set_visible(cx, true);
            cx.action(LoginAction::CancelAddAccount);
            self.redraw(cx);
        }

        // Handle toggling password visibility
        let show_pw_button = self.view.button(cx, ids!(show_password_button));
        let hide_pw_button = self.view.button(cx, ids!(hide_password_button));
        if show_pw_button.clicked(actions) || hide_pw_button.clicked(actions) {
            self.password_visible = !self.password_visible;
            password_input.toggle_is_password(cx);
            show_pw_button.set_visible(cx, !self.password_visible);
            hide_pw_button.set_visible(cx, self.password_visible);
            self.redraw(cx);
        }

        // Handle toggling confirm password visibility
        let show_confirm_pw_button = self.view.button(cx, ids!(show_confirm_password_button));
        let hide_confirm_pw_button = self.view.button(cx, ids!(hide_confirm_password_button));
        if show_confirm_pw_button.clicked(actions) || hide_confirm_pw_button.clicked(actions) {
            self.confirm_password_visible = !self.confirm_password_visible;
            confirm_password_input.toggle_is_password(cx);
            show_confirm_pw_button.set_visible(cx, !self.confirm_password_visible);
            hide_confirm_pw_button.set_visible(cx, self.confirm_password_visible);
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
                login_status_modal_inner.set_title(cx, tr_key(self.app_language, "login.status.missing_user_id.title"));
                login_status_modal_inner.set_status(cx, tr_key(self.app_language, "login.status.missing_user_id.body"));
                login_status_modal_inner.button_ref(cx).set_text(cx, tr_key(self.app_language, "login.status.okay"));
            } else if password.is_empty() {
                login_status_modal_inner.set_title(cx, tr_key(self.app_language, "login.status.missing_password.title"));
                login_status_modal_inner.set_status(cx, tr_key(self.app_language, "login.status.missing_password.body"));
                login_status_modal_inner.button_ref(cx).set_text(cx, tr_key(self.app_language, "login.status.okay"));
            } else if self.signup_mode && password != confirm_password {
                login_status_modal_inner.set_title(cx, tr_key(self.app_language, "login.status.password_mismatch.title"));
                login_status_modal_inner.set_status(cx, tr_key(self.app_language, "login.status.password_mismatch.body"));
                login_status_modal_inner.button_ref(cx).set_text(cx, tr_key(self.app_language, "login.status.okay"));
            } else {
                let proxy = match self.build_proxy_url_from_form(cx) {
                    Ok(proxy) => proxy,
                    Err(proxy_validation_error) => {
                        login_status_modal_inner.set_title(cx, tr_key(self.app_language, "login.status.invalid_proxy.title"));
                        let error_text = tr_fmt(self.app_language, "login.status.invalid_proxy.body", &[
                            ("error", proxy_validation_error.as_str()),
                        ]);
                        login_status_modal_inner.set_status(cx, &error_text);
                        login_status_modal_inner.button_ref(cx).set_text(cx, tr_key(self.app_language, "login.status.okay"));
                        login_status_modal.open(cx);
                        self.redraw(cx);
                        return;
                    }
                };
                if let Err(e) = crate::proxy_config::save_proxy_url(proxy.as_deref()) {
                    warning!("Failed to persist proxy configuration from login screen: {e}");
                }
                self.last_failure_message_shown = None;
                login_status_modal_inner.set_title(cx, if self.signup_mode {
                    tr_key(self.app_language, "login.status.creating_account.title")
                } else {
                    tr_key(self.app_language, "login.status.logging_in.title")
                });
                login_status_modal_inner.set_status(
                    cx,
                    if self.signup_mode {
                        tr_key(self.app_language, "login.status.creating_account.body")
                    } else {
                        tr_key(self.app_language, "login.status.logging_in.body")
                    },
                );
                login_status_modal_inner.button_ref(cx).set_text(cx, tr_key(self.app_language, "login.status.cancel"));
                submit_async_request(MatrixRequest::Login(if self.signup_mode {
                    LoginRequest::Register(RegisterAccount {
                        user_id,
                        password,
                        homeserver: homeserver.is_empty().not().then_some(homeserver),
                        proxy: proxy.clone(),
                    })
                } else {
                    LoginRequest::LoginByPassword(LoginByPassword {
                        user_id,
                        password,
                        homeserver: homeserver.is_empty().not().then_some(homeserver),
                        proxy: proxy.clone(),
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
                    login_status_modal_inner.set_title(cx, tr_key(self.app_language, "login.status.logging_in_cli.title"));
                    login_status_modal_inner.set_status(
                        cx,
                        &tr_fmt(self.app_language, "login.status.auto_logging_in_as_user", &[
                            ("user_id", user_id.as_str()),
                        ])
                    );
                    let login_status_modal_button = login_status_modal_inner.button_ref(cx);
                    login_status_modal_button.set_text(cx, tr_key(self.app_language, "login.status.cancel"));
                    login_status_modal_button.set_enabled(cx, false); // Login cancel not yet supported
                    login_status_modal.open(cx);
                }
                Some(LoginAction::Status { title, status }) => {
                    self.last_failure_message_shown = None;
                    login_status_modal_inner.set_title(cx, title);
                    login_status_modal_inner.set_status(cx, status);
                    let login_status_modal_button = login_status_modal_inner.button_ref(cx);
                    login_status_modal_button.set_text(cx, tr_key(self.app_language, "login.status.cancel"));
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
                    self.view.label(cx, ids!(title)).set_text(cx, tr_key(self.app_language, "login.title.login_to_robrix"));
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
                        tr_key(self.app_language, "login.status.account_creation_failed")
                    } else {
                        tr_key(self.app_language, "login.status.login_failed")
                    });
                    login_status_modal_inner.set_status(cx, error);
                    let login_status_modal_button = login_status_modal_inner.button_ref(cx);
                    login_status_modal_button.set_text(cx, tr_key(self.app_language, "login.status.okay"));
                    login_status_modal_button.set_enabled(cx, true);
                    login_status_modal.open(cx);
                    self.redraw(cx);
                }
                Some(LoginAction::SsoPending(pending)) => {
                    self.set_sso_pending_state(cx, *pending);
                    self.redraw(cx);
                }
                Some(LoginAction::SsoSetRedirectUrl(url)) => {
                    self.sso_redirect_url = Some(url.to_string());
                }
                Some(LoginAction::ShowAddAccountScreen) => {
                    self.adding_account = true;
                    self.reset_sso_state(cx);
                    // Update UI to "add account" mode
                    self.view.label(cx, ids!(title)).set_text(cx, tr_key(self.app_language, "settings.account.button.add_another_account"));
                    cancel_button.set_visible(cx, true);
                    // Hide signup button in add-account mode (user already has an account)
                    mode_toggle_button.set_visible(cx, false);
                    self.redraw(cx);
                }
                Some(LoginAction::AddAccountSuccess) => {
                    // Reset the login screen state
                    self.adding_account = false;
                    self.reset_sso_state(cx);
                    user_id_input.set_text(cx, "");
                    password_input.set_text(cx, "");
                    homeserver_input.set_text(cx, "");
                    // Reset title and buttons
                    self.view.label(cx, ids!(title)).set_text(cx, tr_key(self.app_language, "login.title.login_to_robrix"));
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
                    login_status_modal_inner.set_title(cx, tr_key(self.app_language, "login.status.account_switch_failed"));
                    login_status_modal_inner.set_status(cx, error);
                    let login_status_modal_button = login_status_modal_inner.button_ref(cx);
                    login_status_modal_button.set_text(cx, tr_key(self.app_language, "login.status.okay"));
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
                self.reset_sso_state(cx);
                self.redraw(cx);
            }
        }

        // Handle any of the SSO login buttons being clicked
        for (view_ref, brand) in self.view_set(cx, button_set).iter().zip(&provider_brands) {
            if view_ref.finger_up(actions).is_some() && !self.sso_pending {
                let proxy = match self.build_proxy_url_from_form(cx) {
                    Ok(proxy) => proxy,
                    Err(proxy_validation_error) => {
                        login_status_modal_inner.set_title(cx, tr_key(self.app_language, "login.status.invalid_proxy.title"));
                        let error_text = tr_fmt(self.app_language, "login.status.invalid_proxy.body", &[
                            ("error", proxy_validation_error.as_str()),
                        ]);
                        login_status_modal_inner.set_status(cx, &error_text);
                        let login_status_modal_button = login_status_modal_inner.button_ref(cx);
                        login_status_modal_button.set_text(cx, tr_key(self.app_language, "login.status.okay"));
                        login_status_modal_button.set_enabled(cx, true);
                        login_status_modal.open(cx);
                        self.redraw(cx);
                        continue;
                    }
                };
                if let Err(e) = crate::proxy_config::save_proxy_url(proxy.as_deref()) {
                    warning!("Failed to persist proxy configuration from SSO login flow: {e}");
                }
                submit_async_request(MatrixRequest::SpawnSSOServer{
                    identity_provider_id: format!("oidc-{}",brand),
                    brand: brand.to_string(),
                    homeserver_url: homeserver_input.text(),
                    proxy,
                });
            }
        }
    }

}

/// Actions sent to or from the login screen.
#[derive(Clone, Default, Debug)]
pub enum LoginAction {
    /// Request to show the login screen because no reusable session is available.
    ShowLoginScreen,
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
