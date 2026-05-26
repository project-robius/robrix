#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
use std::cell::RefCell;

use makepad_widgets::{text::selection::Cursor, *};
#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
use rfd::FileDialog;
use matrix_sdk::{encryption::VerificationState, ruma::OwnedUserId};

use crate::{account_manager, app::AppState, avatar_cache::{self}, home::navigation_tab_bar::get_own_profile, i18n::{AppLanguage, tr_fmt, tr_key}, login::login_screen::LoginAction, logout::logout_confirm_modal::{LogoutAction, LogoutConfirmModalAction}, profile::{user_profile::UserProfile, user_profile_cache}, shared::{avatar::{AvatarState, AvatarWidgetExt}, popup_list::{PopupKind, enqueue_popup_notification}, styles::*}, sliding_sync::{get_client, AccessTokenCopyAction, AccessTokenCopyError, AccountDataAction, AccountSwitchAction, MatrixRequest, OwnDeviceInfo, submit_async_request}, utils, verification::VerificationStateAction};
#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
use crate::{app::ConfirmDeleteAction, shared::confirmation_modal::ConfirmationModalContent};

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*


    // The view containing all user account-related settings.
    mod.widgets.AccountSettings = #(AccountSettings::register_widget(vm)) {
        width: Fill, height: Fit
        flow: Down

        account_settings_title := TitleLabel {
            text: "Account Settings"
        }

        verification_banner_verified := RoundedView {
            visible: false
            width: Fill
            height: Fit
            flow: Right
            align: Align{y: 0.5}
            margin: Inset{top: (SPACE_SM)}
            padding: Inset{top: 10, bottom: 9, left: 12, right: 12}
            show_bg: true
            draw_bg +: {
                color: (COLOR_BG_ACCEPT_GREEN)
                border_color: (COLOR_FG_ACCEPT_GREEN)
                border_size: 1.0
                border_radius: 4.0
            }
            Label {
                width: Fill, height: Fit
                flow: Flow.Right{wrap: true}
                draw_text +: {
                    color: (COLOR_FG_ACCEPT_GREEN),
                    text_style: theme.font_bold { font_size: 11.5 },
                }
                text: "This device is verified and can access encrypted messages."
            }
        }

        verification_banner_unverified := RoundedView {
            visible: false
            width: Fill
            height: Fit
            flow: Down,
            align: Align{y: 0.5}
            spacing: 0,
            margin: Inset{top: (SPACE_SM)}
            padding: Inset{top: 10, bottom: 13, left: 12, right: 12}
            show_bg: true
            draw_bg +: {
                color: (COLOR_BG_DANGER_RED)
                border_color: (COLOR_FG_DANGER_RED)
                border_size: 1.0
                border_radius: 4.0
            }
            Label {
                width: Fill, height: Fit
                flow: Flow.Right{wrap: true}
                draw_text +: {
                    color: (COLOR_FG_DANGER_RED),
                    text_style: theme.font_bold { font_size: 11.5 },
                }
                text: "This device is not verified and can't view encrypted messages."
            }
            Label {
                width: Fill, height: Fit
                flow: Flow.Right{wrap: true}
                margin: Inset{top: 4, bottom: 1}
                draw_text +: {
                    color: (MESSAGE_TEXT_COLOR),
                    text_style: theme.font_regular { font_size: 11.5 },
                }
                text: "Verify it from another client using this info:"
            }
            unverified_device_info_label := Label {
                width: Fill, height: Fit
                padding: Inset{left: 8}
                flow: Flow.Right{wrap: true}
                draw_text +: {
                    color: (MESSAGE_TEXT_COLOR),
                    text_style: theme.font_regular { font_size: 11.5 },
                }
                text: ""
            }
        }

        // --- Avatar card ---
        RoundedView {
            width: Fill, height: Fit
            flow: Down
            padding: Inset{left: (SPACE_MD), right: (SPACE_MD), top: (SPACE_SM), bottom: (SPACE_MD)}
            margin: Inset{top: (SPACE_SM)}
            show_bg: true
            draw_bg +: {
                color: #F8F8FA
                border_radius: (RADIUS_LG)
            }

            avatar_section_label := SubsectionLabel {
                margin: Inset{top: 0, bottom: (SPACE_XS)}
                text: "Your Avatar:"
            }

            View {
                width: Fill, height: Fit
                flow: Right { wrap: true },
                align: Align{y: 0.5}

                our_own_avatar := Avatar {
                    width: 100,
                    height: 100,
                    margin: (SPACE_SM),
                    text_view +: {
                        text +: {
                            draw_text +: {
                                text_style: theme.font_regular { font_size: 35.0 }
                            }
                        }
                    }
                }

                View {
                    width: Fit, height: Fit
                    flow: Down,
                    align: Align{y: 0.5}
                    padding: Inset{ left: (SPACE_SM), right: (SPACE_SM) }
                    spacing: (SPACE_SM)

                    View {
                        width: Fit, height: Fit
                        flow: Right,
                        align: Align{y: 0.5}
                        spacing: (SPACE_SM)

                        upload_avatar_button := RobrixIconButton {
                            width: 140,
                            height: mod.widgets.SETTINGS_BUTTON_HEIGHT,
                            padding: Inset{top: (SPACE_SM), bottom: (SPACE_SM), left: (SPACE_MD), right: (SPACE_LG)}
                            margin: 0,
                            draw_bg +: { border_radius: (RADIUS_MD) }
                            draw_icon.svg: (ICON_UPLOAD)
                            icon_walk: Walk{width: 16, height: 16}
                            text: "Upload Avatar"
                        }

                        upload_avatar_spinner := LoadingSpinner {
                            width: 16, height: 16
                            visible: false
                            draw_bg.color: (COLOR_ACTIVE_PRIMARY)
                        }
                    }

                    View {
                        width: Fit, height: Fit
                        flow: Right,
                        align: Align{y: 0.5}
                        spacing: (SPACE_SM)

                        delete_avatar_button := RobrixNegativeIconButton {
                            width: 140,
                            height: mod.widgets.SETTINGS_BUTTON_HEIGHT,
                            padding: Inset{top: (SPACE_SM), bottom: (SPACE_SM), left: (SPACE_MD), right: (SPACE_LG)}
                            margin: 0,
                            draw_bg +: { border_radius: (RADIUS_MD) }
                            draw_icon.svg: (ICON_TRASH)
                            icon_walk: Walk{ width: 16, height: 16 }
                            text: "Delete Avatar"
                        }

                        delete_avatar_spinner := LoadingSpinner {
                            width: 16, height: 16
                            visible: false
                            draw_bg.color: (COLOR_ACTIVE_PRIMARY)
                        }
                    }
                }
            }
        }

        // --- Display Name card ---
        RoundedView {
            width: Fill, height: Fit
            flow: Down
            padding: Inset{left: (SPACE_MD), right: (SPACE_MD), top: (SPACE_SM), bottom: (SPACE_MD)}
            margin: Inset{top: (SPACE_SM)}
            show_bg: true
            draw_bg +: {
                color: #F8F8FA
                border_radius: (RADIUS_LG)
            }

            display_name_section_label := SubsectionLabel {
                margin: Inset{top: 0, bottom: (SPACE_XS)}
                text: "Your Display Name:"
            }

            display_name_input := RobrixTextInput {
                margin: Inset{top: 3, left: (SPACE_XS), right: (SPACE_XS), bottom: (SPACE_SM)},
                width: Fill { max: 226}, // to match the button width
                height: Fit
                empty_text: "Add a display name..."
            }

            View {
                width: Fill, height: Fit
                flow: Flow.Right{wrap: true},
                align: Align{y: 0.5},
                spacing: (SPACE_SM)

                // These buttons are disabled by default, and enabled when the user
                // changes the `display_name_input` text.
                // These buttons start disabled; Rust code enables them and swaps
                // their styles to RobrixNeutralIconButton / RobrixPositiveIconButton.
                cancel_display_name_button := RobrixNeutralIconButton {
                    enabled: false,
                    width: Fit, height: Fit,
                    padding: (SPACE_SM),
                    margin: Inset{left: (SPACE_XS)},
                    draw_icon.svg: (ICON_FORBIDDEN)
                    icon_walk: Walk{width: 16, height: 16, margin: 0}
                    text: "Cancel"
                }

                accept_display_name_button := RobrixPositiveIconButton {
                    enabled: false,
                    width: Fit, height: Fit,
                    padding: (SPACE_SM),
                    margin: Inset{left: (SPACE_XS)},
                    draw_bg.border_radius: (RADIUS_MD)
                    draw_icon.svg: (ICON_CHECKMARK)
                    icon_walk: Walk{width: 16, height: 16, margin: 0}
                    text: "Save Name"
                }

                save_name_spinner := LoadingSpinner {
                    width: 16, height: 16
                    margin: Inset{left: (SPACE_XS), top: 13} // vertically center with buttons
                    visible: false
                    draw_bg.color: (COLOR_ACTIVE_PRIMARY)
                }
            }
        }

        // --- User ID card ---
        RoundedView {
            width: Fill, height: Fit
            flow: Down
            padding: Inset{left: (SPACE_MD), right: (SPACE_MD), top: (SPACE_SM), bottom: (SPACE_MD)}
            margin: Inset{top: (SPACE_SM)}
            show_bg: true
            draw_bg +: {
                color: #F8F8FA
                border_radius: (RADIUS_LG)
            }

            user_id_section_label := SubsectionLabel {
                margin: Inset{top: 0, bottom: (SPACE_XS)}
                text: "Your User ID:"
            }

            View {
                width: Fill, height: Fit
                flow: Right,
                align: Align{y: 0.5}
                spacing: (SPACE_SM)

                copy_user_id_button := RobrixNeutralIconButton {
                    enable_long_press: true,
                    padding: (SPACE_MD),
                    spacing: 0,
                    draw_icon.svg: (ICON_COPY)
                    icon_walk: Walk{width: 16, height: 16, margin: Inset{right: -2} }
                }

                user_id := Label {
                    width: Fill, height: Fit
                    flow: Flow.Right{wrap: true},
                    margin: Inset{top: 9}
                    draw_text +: {
                        color: (MESSAGE_TEXT_COLOR),
                        text_style: MESSAGE_TEXT_STYLE { font_size: 11.5 },
                    }
                    text: "You are not logged in."
                }
            }
        }

        // --- Multiple Accounts card ---
        RoundedView {
            width: Fill, height: Fit
            flow: Down
            padding: Inset{left: (SPACE_MD), right: (SPACE_MD), top: (SPACE_SM), bottom: (SPACE_MD)}
            margin: Inset{top: (SPACE_SM)}
            show_bg: true
            draw_bg +: {
                color: #F8F8FA
                border_radius: (RADIUS_LG)
            }

            multiple_accounts_section_label := SubsectionLabel {
                margin: Inset{top: 0, bottom: (SPACE_XS)}
                text: "Multiple Accounts:"
            }

            View {
                width: Fill, height: Fit
                flow: Down,
                spacing: (SPACE_SM),

            // Account entries will be shown here
            // Active account (current)
            active_account_view := RoundedView {
                width: Fill, height: Fit
                flow: Right,
                align: Align{y: 0.5}
                padding: Inset{left: (SPACE_MD), right: (SPACE_LG), top: (SPACE_SM), bottom: (SPACE_SM)}
                spacing: (SPACE_SM)
                show_bg: true
                draw_bg +: {
                    color: (COLOR_ACCOUNT_ACTIVE_BG)
                    border_radius: (RADIUS_LG)
                }

                View {
                    width: Fill, height: Fit
                    flow: Down,
                    spacing: 2

                    active_account_label := Label {
                        width: Fill, height: Fit
                        draw_text +: {
                            color: (COLOR_PRIMARY),
                            text_style: MESSAGE_TEXT_STYLE { font_size: 11 },
                        }
                        text: "@user:server"
                    }

                    active_account_status_label := Label {
                        width: Fit, height: Fit
                        draw_text +: {
                            color: (COLOR_PRIMARY),
                            text_style: MESSAGE_TEXT_STYLE { font_size: 9 },
                        }
                        text: "Active"
                    }
                }
            }

            // Other accounts section (populated dynamically)
            other_accounts_label := Label {
                width: Fill, height: Fit
                margin: Inset{top: (SPACE_XS), left: (SPACE_XS)}
                visible: false
                draw_text +: {
                    color: (MESSAGE_TEXT_COLOR),
                    text_style: MESSAGE_TEXT_STYLE { font_size: 10 },
                }
                text: "Other accounts:"
            }

            // Container for other account entries (simplified: show one other account)
            other_account_entry := RoundedView {
                width: Fill, height: Fit
                flow: Right,
                align: Align{y: 0.5}
                padding: Inset{left: (SPACE_MD), right: (SPACE_MD), top: (SPACE_SM), bottom: (SPACE_SM)}
                spacing: (SPACE_SM)
                visible: false
                show_bg: true
                draw_bg +: {
                    color: (COLOR_SECONDARY)
                    border_radius: (RADIUS_LG)
                    border_size: 1.0
                    border_color: (COLOR_INACTIVE_BORDER)
                }

                View {
                    width: Fill, height: Fit
                    flow: Down,
                    spacing: 2

                    other_account_label := Label {
                        width: Fill, height: Fit
                        draw_text +: {
                            color: (COLOR_TEXT),
                            text_style: MESSAGE_TEXT_STYLE { font_size: 11 },
                        }
                        text: "@other:server"
                    }
                }

                switch_account_button := RobrixIconButton {
                    width: Fit, height: Fit
                    padding: Inset{top: (SPACE_SM), bottom: (SPACE_SM), left: (SPACE_SM), right: (SPACE_SM)}
                    draw_icon.svg: (ICON_JUMP)
                    icon_walk: Walk{width: 14, height: 14}
                    text: "Switch"
                }
            }

            account_count_label := Label {
                width: Fill, height: Fit
                margin: Inset{top: (SPACE_XS), bottom: (SPACE_XS), left: (SPACE_XS)}
                draw_text +: {
                    color: (MESSAGE_TEXT_COLOR),
                    text_style: MESSAGE_TEXT_STYLE { font_size: 10 },
                }
                text: "1 account logged in"
            }

            add_account_button := RobrixIconButton {
                width: Fit,
                padding: Inset{top: (SPACE_SM), bottom: (SPACE_SM), left: (SPACE_MD), right: (SPACE_LG)}
                margin: Inset{top: (SPACE_XS)}
                draw_bg +: { border_radius: (RADIUS_MD) }
                draw_icon.svg: (ICON_ADD)
                icon_walk: Walk{width: 16, height: 16}
                text: "Add Another Account"
            }
            }
        } // end Multiple Accounts card

        // --- Other actions card ---
        RoundedView {
            width: Fill, height: Fit
            flow: Down
            padding: Inset{left: (SPACE_MD), right: (SPACE_MD), top: (SPACE_SM), bottom: (SPACE_MD)}
            margin: Inset{top: (SPACE_SM), bottom: (SPACE_LG)}
            show_bg: true
            draw_bg +: {
                color: #F8F8FA
                border_radius: (RADIUS_LG)
            }

            other_actions_section_label := SubsectionLabel {
                margin: Inset{top: 0, bottom: (SPACE_XS)}
                text: "Other actions:"
            }

            View {
                width: Fill, height: Fit
                flow: Flow.Right{wrap: true},
                align: Align{y: 0.5},
                spacing: (SPACE_SM)

                manage_account_button := RobrixIconButton {
                    padding: Inset{top: (SPACE_SM), bottom: (SPACE_SM), left: (SPACE_MD), right: (SPACE_LG)}
                    draw_bg +: { border_radius: (RADIUS_MD) }
                    draw_icon.svg: (ICON_EXTERNAL_LINK)
                    icon_walk: Walk{width: 16, height: 16}
                    text: "Manage Account"
                }

                copy_access_token_button := RobrixNeutralIconButton {
                    padding: Inset{top: (SPACE_SM), bottom: (SPACE_SM), left: (SPACE_MD), right: (SPACE_LG)}
                    draw_bg +: { border_radius: (RADIUS_MD) }
                    draw_icon.svg: (ICON_COPY)
                    icon_walk: Walk{width: 16, height: 16}
                    text: "Copy Access Token"
                }

                logout_button := RobrixNegativeIconButton {
                    padding: Inset{top: (SPACE_SM), bottom: (SPACE_SM), left: (SPACE_MD), right: (SPACE_LG)}
                    draw_bg +: { border_radius: (RADIUS_MD) }
                    draw_icon.svg: (ICON_LOGOUT)
                    icon_walk: Walk{ width: 16, height: 16, margin: Inset{right: -2} }
                    text: "Log out"
                }
            }
        }
    }
}

/// The view containing all user account-related settings.
#[derive(Script, ScriptHook, Widget)]
pub struct AccountSettings {
    #[deref] view: View,

    #[rust] own_profile: Option<UserProfile>,
    #[rust(VerificationState::Unknown)] verification_state: VerificationState,
    #[rust] own_device: Option<OwnDeviceInfo>,
    #[rust] app_language: AppLanguage,
    /// List of other account user IDs (not the currently active one)
    #[rust] other_accounts: Vec<OwnedUserId>,
}

impl Widget for AccountSettings {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        let app_language = scope.data.get::<AppState>()
            .map(|app_state| app_state.app_language)
            .unwrap_or_default();
        if self.app_language != app_language {
            self.set_app_language(cx, app_language);
        }
        self.match_event(cx, event);

        let copy_user_id_button = self.view.button(cx, ids!(copy_user_id_button));
        let copy_user_id_button_area = copy_user_id_button.area();
        match event.hits(cx, copy_user_id_button_area) {
            Hit::FingerHoverIn(_) | Hit::FingerLongPress(_) => {
                cx.widget_action(
                    copy_user_id_button.widget_uid(), 
                    TooltipAction::HoverIn {
                        text: tr_key(self.app_language, "settings.account.tooltip.copy_user_id").to_string(),
                        widget_rect: copy_user_id_button_area.rect(cx),
                        options: CalloutTooltipOptions {
                            position: TooltipPosition::Top,
                            ..Default::default()
                        },
                    },
                );
            }
            Hit::FingerHoverOut(_) => {
                cx.widget_action(
                    copy_user_id_button.widget_uid(), 
                    TooltipAction::HoverOut,
                );
            }
            _ => {}
        }

        self.view.handle_event(cx, event, scope);
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

impl MatchEvent for AccountSettings {
    fn handle_signal(&mut self, cx: &mut Cx) {
        // If we don't have a profile yet, try to get it
        if self.own_profile.is_none() {
            user_profile_cache::process_user_profile_updates(cx);
            if let Some(new_profile) = get_own_profile(cx) {
                self.own_profile = Some(new_profile.clone());
                self.own_device = None;
                self.view.label(cx, ids!(user_id))
                    .set_text(cx, new_profile.user_id.as_str());
                self.view.text_input(cx, ids!(display_name_input))
                    .set_text(cx, new_profile.username.as_deref().unwrap_or_default());
                self.populate_avatar_views(cx);
                self.populate_account_list(cx);
                self.refresh_verification_state(cx);
                self.view.redraw(cx);
            }
            return;
        }
        // Process avatar updates from the cache
        avatar_cache::process_avatar_updates(cx);

        // Update avatar from cache if we have a profile
        if let Some(profile) = self.own_profile.as_mut() {
            if profile.avatar_state.uri().is_some() {
                let new_data = profile.avatar_state.update_from_cache(cx);
                if new_data.is_some() {
                    self.populate_avatar_views(cx);
                    self.view.redraw(cx);
                }
            }
        }
    }

    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        let accept_display_name_button = self.view.button(cx, ids!(accept_display_name_button));
        let cancel_display_name_button = self.view.button(cx, ids!(cancel_display_name_button));
        let display_name_input = self.view.text_input(cx, ids!(display_name_input));
        let delete_avatar_button = self.view.button(cx, ids!(delete_avatar_button));
        let upload_avatar_button = self.view.button(cx, ids!(upload_avatar_button));

        for action in actions {
            if let Some(VerificationStateAction::Update(state)) = action.downcast_ref() {
                self.verification_state = *state;
                self.update_verification_banner(cx);
                continue;
            }

            // Handle LogoutAction::InProgress to update button state
            if let Some(LogoutAction::InProgress(is_in_progress)) = action.downcast_ref() {
                let logout_button = self.view.button(cx, ids!(logout_button));
                logout_button.set_text(cx, if *is_in_progress {
                    tr_key(self.app_language, "settings.account.button.logging_out")
                } else {
                    tr_key(self.app_language, "settings.account.button.log_out")
                });
                logout_button.set_enabled(cx, !*is_in_progress);
                logout_button.reset_hover(cx);
                continue;
            }

            match action.downcast_ref() {
                Some(AccessTokenCopyAction::Ready { access_token }) => {
                    cx.copy_to_clipboard(access_token);
                    enqueue_popup_notification(
                        tr_key(self.app_language, "settings.account.popup.copied_access_token"),
                        PopupKind::Success,
                        Some(3.0),
                    );
                    continue;
                }
                Some(AccessTokenCopyAction::Failed { reason }) => {
                    let error_key = match reason {
                        AccessTokenCopyError::NoSession => "settings.account.popup.access_token_no_session",
                        AccessTokenCopyError::Unavailable => "settings.account.popup.access_token_unavailable",
                    };
                    enqueue_popup_notification(
                        tr_key(self.app_language, error_key),
                        PopupKind::Error,
                        Some(4.0),
                    );
                    continue;
                }
                _ => {}
            }

            // Handle account data changes.
            // Note: the NavigationTabBar handles removing stale data from the user_profile_cache,
            // so here, we only need to update this widget's local profile info.
            match action.downcast_ref() {
                Some(AccountDataAction::AvatarChanged(new_avatar_url)) => {
                    self.view.widget(cx, ids!(upload_avatar_spinner)).set_visible(cx, false);
                    self.view.widget(cx, ids!(delete_avatar_spinner)).set_visible(cx, false);
                    // Update our cached profile with the new avatar URL
                    if let Some(profile) = self.own_profile.as_mut() {
                        profile.avatar_state = AvatarState::Known(new_avatar_url.clone());
                        profile.avatar_state.update_from_cache(cx);
                        self.populate_avatar_views(cx);
                        enqueue_popup_notification(
                            if new_avatar_url.is_some() {
                                tr_key(self.app_language, "settings.account.popup.avatar_updated")
                            } else {
                                tr_key(self.app_language, "settings.account.popup.avatar_deleted")
                            },
                            PopupKind::Success,
                            Some(4.0),
                        );
                    }
                    continue;
                }
                Some(AccountDataAction::AvatarChangeFailed(err_msg)) => {
                    self.view.widget(cx, ids!(upload_avatar_spinner)).set_visible(cx, false);
                    self.view.widget(cx, ids!(delete_avatar_spinner)).set_visible(cx, false);
                    // Re-enable the avatar buttons so user can try again
                    Self::enable_upload_avatar_button(cx, true, &upload_avatar_button);
                    Self::enable_delete_avatar_button(
                        cx,
                        self.own_profile.as_ref().is_some_and(|p| p.avatar_state.has_avatar()),
                        &delete_avatar_button
                    );
                    enqueue_popup_notification(
                        err_msg.clone(),
                        PopupKind::Error,
                        Some(4.0),
                    );
                    continue;
                }
                Some(AccountDataAction::DisplayNameChanged(new_name)) => {
                    self.view.widget(cx, ids!(save_name_spinner)).set_visible(cx, false);
                    // Update our cached profile with the new display name
                    if let Some(profile) = self.own_profile.as_mut() {
                        profile.username = new_name.clone();
                    }
                    // Update the display name text input and disable buttons
                    let (text, len) = new_name.as_deref().map(|s| (s, s.len())).unwrap_or_default();
                    display_name_input.set_text(cx, text);
                    display_name_input.set_cursor(cx, Cursor { index: len, prefer_next_row: false }, false);
                    display_name_input.set_is_read_only(cx, false);
                    display_name_input.set_disabled(cx, false);
                    Self::enable_display_name_buttons(cx, false, &accept_display_name_button, &cancel_display_name_button);
                    enqueue_popup_notification(
                        if new_name.is_some() {
                            tr_key(self.app_language, "settings.account.popup.display_name_updated")
                        } else {
                            tr_key(self.app_language, "settings.account.popup.display_name_removed")
                        },
                        PopupKind::Success,
                        Some(4.0),
                    );
                    continue;
                }
                Some(AccountDataAction::DisplayNameChangeFailed(err_msg)) => {
                    self.view.widget(cx, ids!(save_name_spinner)).set_visible(cx, false);
                    // Re-enable the buttons and text input so that the user can try again
                    display_name_input.set_is_read_only(cx, false);
                    display_name_input.set_disabled(cx, false);
                    Self::enable_display_name_buttons(cx, true, &accept_display_name_button, &cancel_display_name_button);
                    enqueue_popup_notification(
                        err_msg.clone(),
                        PopupKind::Error,
                        Some(4.0),
                    );
                    continue;
                }
                Some(AccountDataAction::OwnDeviceFetched(device)) => {
                    self.own_device = device.clone();
                    self.update_verification_banner(cx);
                    continue;
                }
                _ => {}
            }

            match action.downcast_ref() {
                Some(AccountSettingsAction::AvatarDeleteStarted) => {
                    self.view.widget(cx, ids!(delete_avatar_spinner)).set_visible(cx, true);
                    Self::enable_upload_avatar_button(cx, false, &upload_avatar_button);
                    Self::enable_delete_avatar_button(cx, false, &delete_avatar_button);
                    continue;
                }
                Some(AccountSettingsAction::AvatarUploadStarted) => {
                    self.view.widget(cx, ids!(upload_avatar_spinner)).set_visible(cx, true);
                    Self::enable_upload_avatar_button(cx, false, &upload_avatar_button);
                    Self::enable_delete_avatar_button(cx, false, &delete_avatar_button);
                    continue;
                }
                _ => {}
            }
        }

        if self.view.button(cx, ids!(logout_button)).clicked(actions) {
            cx.action(LogoutConfirmModalAction::Open);
            return;
        }

        if self.view.button(cx, ids!(copy_access_token_button)).clicked(actions) {
            submit_async_request(MatrixRequest::GetAccessTokenForCopy);
        }

        let Some(own_profile) = &self.own_profile else { return };

        if upload_avatar_button.clicked(actions) {
            #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
            {
                if let Some(avatar_path) = FileDialog::new()
                    .add_filter("Image", &["png", "jpg", "jpeg"])
                    .pick_file()
                {
                    submit_async_request(MatrixRequest::UploadAvatar { avatar_path });
                    cx.action(AccountSettingsAction::AvatarUploadStarted);
                    enqueue_popup_notification(
                        tr_key(self.app_language, "settings.account.popup.uploading_avatar"),
                        PopupKind::Info,
                        Some(5.0),
                    );
                }
            }
            #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
            {
                enqueue_popup_notification(
                    tr_key(self.app_language, "settings.account.popup.avatar_upload_not_implemented"),
                    PopupKind::Warning,
                    Some(4.0),
                );
            }
        }

        if delete_avatar_button.clicked(actions) {
            #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
            {
            // Don't immediately disable the buttons. Instead, we wait for the user
            // to confirm the action in the confirmation modal,
            // and then we disable the buttons in the AvatarDeleteStarted action handler.
            let app_language = self.app_language;
            let content = ConfirmationModalContent {
                title_text: tr_key(app_language, "settings.account.modal.delete_avatar.title").into(),
                body_text: tr_key(app_language, "settings.account.modal.delete_avatar.body").into(),
                accept_button_text: Some(tr_key(app_language, "settings.account.modal.delete_avatar.accept").into()),
                on_accept_clicked: Some(Box::new(move |cx| {
                    submit_async_request(MatrixRequest::SetAvatar { avatar_url: None });
                    cx.action(AccountSettingsAction::AvatarDeleteStarted);
                    enqueue_popup_notification(
                        tr_key(app_language, "settings.account.popup.deleting_avatar"),
                        PopupKind::Info,
                        Some(5.0),
                    );
                })),
                ..Default::default()
            };
            cx.action(ConfirmDeleteAction::Show(RefCell::new(Some(content))));
            }
            #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
            {
                enqueue_popup_notification(
                    "Deleting avatar is not yet supported on this platform.",
                    PopupKind::Warning,
                    Some(4.0),
                );
            }
        }

        // Enable the name change buttons if the user modified the display name to be different.
        if let Some(new_name) = display_name_input.changed(actions) {
            let trimmed = new_name.trim();
            let current_name = own_profile.username.as_deref().unwrap_or("");
            let enable = trimmed != current_name;
            Self::enable_display_name_buttons(cx, enable, &accept_display_name_button, &cancel_display_name_button);
        }

        if cancel_display_name_button.clicked(actions) {
            // Reset the display name input and disable the name change buttons.
            let new_text = own_profile.username.as_deref().unwrap_or("");
            display_name_input.set_text(cx, new_text);
            display_name_input.set_cursor(cx, Cursor { index: new_text.len(), prefer_next_row: false }, false);
            Self::enable_display_name_buttons(cx, false, &accept_display_name_button, &cancel_display_name_button);
        }

        if accept_display_name_button.clicked(actions) {
            let new_display_name = match display_name_input.text().trim() {
                "" => None,
                name => Some(name.to_string()),
            };
            // While the request is in flight, show the loading spinner and disable the buttons & text input
            submit_async_request(MatrixRequest::SetDisplayName { new_display_name });
            self.view.widget(cx, ids!(save_name_spinner)).set_visible(cx, true);
            display_name_input.set_disabled(cx, true);
            display_name_input.set_is_read_only(cx, true);
            Self::enable_display_name_buttons(cx, false, &accept_display_name_button, &cancel_display_name_button);
            enqueue_popup_notification(
                tr_key(self.app_language, "settings.account.popup.uploading_display_name"),
                PopupKind::Info,
                Some(5.0),
            );
        }

        if self.view.button(cx, ids!(copy_user_id_button)).clicked(actions) {
            cx.copy_to_clipboard(own_profile.user_id.as_str());
            enqueue_popup_notification(
                tr_key(self.app_language, "settings.account.popup.copied_user_id"),
                PopupKind::Success,
                Some(3.0),
            );
        }

        if self.view.button(cx, ids!(manage_account_button)).clicked(actions) {
            // TODO: support opening the user's account management page in a browser,
            //       or perhaps in an in-app pane if that's what is needed for regular UN+PW login.
            enqueue_popup_notification(
                tr_key(self.app_language, "settings.account.popup.account_management_not_implemented"),
                PopupKind::Warning,
                Some(4.0),
            );
        }

        if self.view.button(cx, ids!(logout_button)).clicked(actions) {
            cx.action(LogoutConfirmModalAction::Open);
        }

        // Handle "Switch Account" button click
        if self.view.button(cx, ids!(switch_account_button)).clicked(actions) {
            // Switch to the first other account
            if let Some(other_id) = self.other_accounts.first().cloned() {
                log!("Switching to account: {}", other_id);
                submit_async_request(MatrixRequest::SwitchAccount { user_id: other_id });
            }
        }

        // Handle "Add Account" button click
        if self.view.button(cx, ids!(add_account_button)).clicked(actions) {
            // Navigate to login screen in "add account" mode
            cx.action(LoginAction::ShowAddAccountScreen);
        }

        // Handle account switch result and new account added
        for action in actions {
            if let Some(AccountSwitchAction::Switched(new_user_id)) = action.downcast_ref() {
                log!("Account switched to: {}, refreshing profile and account list", new_user_id);
                self.own_device = None;
                self.refresh_verification_state(cx);
                // Refresh the profile with new account's data
                if let Some(new_profile) = get_own_profile(cx) {
                    self.own_profile = Some(new_profile.clone());
                    // Update the UI with new profile
                    self.view.label(cx, ids!(user_id))
                        .set_text(cx, new_profile.user_id.as_str());
                    self.view.text_input(cx, ids!(display_name_input))
                        .set_text(cx, new_profile.username.as_deref().unwrap_or_default());
                    self.populate_avatar_views(cx);
                } else {
                    // Profile not yet available, at least update the user_id label
                    self.view.label(cx, ids!(user_id))
                        .set_text(cx, new_user_id.as_str());
                    self.view.text_input(cx, ids!(display_name_input))
                        .set_text(cx, "");
                    // Clear the old avatar
                    self.own_profile = None;
                }
                // Refresh the account list to show new active account
                self.populate_account_list(cx);
                self.view.redraw(cx);
            }
            // Refresh account list when a new account is added
            if let Some(LoginAction::AddAccountSuccess) = action.downcast_ref() {
                log!("New account added, refreshing account list");
                self.populate_account_list(cx);
                self.view.redraw(cx);
            }
            // Refresh profile and account list after login success
            if let Some(LoginAction::LoginSuccess) = action.downcast_ref() {
                log!("Login success, refreshing profile and account list");
                self.own_device = None;
                self.refresh_verification_state(cx);
                if let Some(new_profile) = get_own_profile(cx) {
                    self.own_profile = Some(new_profile.clone());
                    self.view.label(cx, ids!(user_id))
                        .set_text(cx, new_profile.user_id.as_str());
                    self.view.text_input(cx, ids!(display_name_input))
                        .set_text(cx, new_profile.username.as_deref().unwrap_or_default());
                    self.populate_avatar_views(cx);
                }
                self.populate_account_list(cx);
                self.view.redraw(cx);
            }
        }
    }
}

impl AccountSettings {
    fn set_app_language(&mut self, cx: &mut Cx, app_language: AppLanguage) {
        self.app_language = app_language;
        self.sync_app_language(cx);
    }

    fn sync_app_language(&mut self, cx: &mut Cx) {
        self.view
            .label(cx, ids!(account_settings_title))
            .set_text(cx, tr_key(self.app_language, "settings.account.title"));
        self.view
            .label(cx, ids!(avatar_section_label))
            .set_text(cx, tr_key(self.app_language, "settings.account.section.your_avatar"));
        self.view
            .button(cx, ids!(upload_avatar_button))
            .set_text(cx, tr_key(self.app_language, "settings.account.button.upload_avatar"));
        self.view
            .button(cx, ids!(delete_avatar_button))
            .set_text(cx, tr_key(self.app_language, "settings.account.button.delete_avatar"));
        self.view
            .label(cx, ids!(display_name_section_label))
            .set_text(cx, tr_key(self.app_language, "settings.account.section.your_display_name"));
        self.view
            .text_input(cx, ids!(display_name_input))
            .set_empty_text(cx, tr_key(self.app_language, "settings.account.display_name.placeholder").to_string());
        self.view
            .button(cx, ids!(cancel_display_name_button))
            .set_text(cx, tr_key(self.app_language, "settings.account.button.cancel"));
        self.view
            .button(cx, ids!(accept_display_name_button))
            .set_text(cx, tr_key(self.app_language, "settings.account.button.save_name"));
        self.view
            .label(cx, ids!(user_id_section_label))
            .set_text(cx, tr_key(self.app_language, "settings.account.section.your_user_id"));
        if self.own_profile.is_none() {
            self.view
                .label(cx, ids!(user_id))
                .set_text(cx, tr_key(self.app_language, "settings.account.user_id.not_logged_in"));
        }
        self.view
            .label(cx, ids!(multiple_accounts_section_label))
            .set_text(cx, tr_key(self.app_language, "settings.account.section.multiple_accounts"));
        self.view
            .label(cx, ids!(active_account_status_label))
            .set_text(cx, tr_key(self.app_language, "settings.account.active_status"));
        self.view
            .label(cx, ids!(other_accounts_label))
            .set_text(cx, tr_key(self.app_language, "settings.account.other_accounts"));
        self.view
            .button(cx, ids!(switch_account_button))
            .set_text(cx, tr_key(self.app_language, "settings.account.button.switch"));
        self.view
            .button(cx, ids!(add_account_button))
            .set_text(cx, tr_key(self.app_language, "settings.account.button.add_another_account"));
        self.view
            .label(cx, ids!(other_actions_section_label))
            .set_text(cx, tr_key(self.app_language, "settings.account.section.other_actions"));
        self.view
            .button(cx, ids!(manage_account_button))
            .set_text(cx, tr_key(self.app_language, "settings.account.button.manage_account"));
        self.view
            .button(cx, ids!(copy_access_token_button))
            .set_text(cx, tr_key(self.app_language, "settings.account.button.copy_access_token"));
        self.view
            .button(cx, ids!(logout_button))
            .set_text(cx, tr_key(self.app_language, "settings.account.button.log_out"));
        self.populate_account_list(cx);
        self.view.redraw(cx);
    }

    fn refresh_verification_state(&mut self, cx: &mut Cx) {
        if let Some(client) = get_client() {
            self.verification_state = client.encryption().verification_state().get();
        } else {
            self.verification_state = VerificationState::Unknown;
        }
        submit_async_request(MatrixRequest::GetOwnDevice);
        self.update_verification_banner(cx);
    }

    fn update_verification_banner(&mut self, cx: &mut Cx) {
        let (verified, unverified) = match self.verification_state {
            VerificationState::Verified => (true, false),
            VerificationState::Unverified => (false, true),
            VerificationState::Unknown => (false, false),
        };
        self.view.view(cx, ids!(verification_banner_verified)).set_visible(cx, verified);
        self.view.view(cx, ids!(verification_banner_unverified)).set_visible(cx, unverified);

        let info_text = match self.own_device.as_ref() {
            Some(device) => match device.display_name.as_ref() {
                Some(name) => format!("Session: \"{name}\",  Device ID: {}", device.device_id),
                None => format!("Device ID: {}", device.device_id),
            },
            None => String::new(),
        };
        self.view.label(cx, ids!(unverified_device_info_label)).set_text(cx, &info_text);
        self.view.redraw(cx);
    }

    /// Populate avatar-related views with the user's profile data.
    ///
    /// This does nothing if `self.own_profile` is `None`.
    fn populate_avatar_views(&mut self, cx: &mut Cx) {
        let Some(own_profile) = &self.own_profile else {
            error!("BUG: AccountSettings::populate_avatar_views() called with no profile data.");
            return;
        };

        let our_own_avatar = self.view.avatar(cx, ids!(our_own_avatar));
        let mut drew_avatar = false;
        if let Some(avatar_img_data) = own_profile.avatar_state.data() {
            drew_avatar = our_own_avatar.show_image(
                cx,
                None, // don't make this avatar clickable; we handle clicks on this ProfileIcon widget directly.
                |cx, img| utils::load_png_or_jpg(&img, cx, avatar_img_data),
            ).is_ok();
        }
        if !drew_avatar {
            our_own_avatar.show_text(
                cx,
                Some(COLOR_ROBRIX_PURPLE),
                None, // don't make this avatar clickable; we handle clicks on this ProfileIcon widget directly.
                own_profile.displayable_name(),
            );
        }

        Self::enable_upload_avatar_button(
            cx,
            true,
            &self.view.button(cx, ids!(upload_avatar_button))
        );
        Self::enable_delete_avatar_button(
            cx,
            own_profile.avatar_state.has_avatar(),
            &self.view.button(cx, ids!(delete_avatar_button))
        );
    }

    /// Show and initializes the account settings within the SettingsScreen.
    pub fn populate(&mut self, cx: &mut Cx, own_profile: UserProfile) {
        self.view.label(cx, ids!(user_id))
            .set_text(cx, own_profile.user_id.as_str());
        self.view.text_input(cx, ids!(display_name_input))
            .set_text(cx, own_profile.username.as_deref().unwrap_or_default());
        Self::enable_display_name_buttons(
            cx,
            false,
            &self.view.button(cx, ids!(accept_display_name_button)),
            &self.view.button(cx, ids!(cancel_display_name_button)),
        );

        self.own_profile = Some(own_profile);
        self.populate_avatar_views(cx);
        self.refresh_verification_state(cx);
        self.sync_app_language(cx);

        self.view.button(cx, ids!(upload_avatar_button)).reset_hover(cx);
        self.view.button(cx, ids!(delete_avatar_button)).reset_hover(cx);
        self.view.button(cx, ids!(accept_display_name_button)).reset_hover(cx);
        self.view.button(cx, ids!(cancel_display_name_button)).reset_hover(cx);
        self.view.button(cx, ids!(copy_user_id_button)).reset_hover(cx);
        self.view.button(cx, ids!(manage_account_button)).reset_hover(cx);
        self.view.button(cx, ids!(logout_button)).reset_hover(cx);
        self.view.redraw(cx);
    }

    /// Populate the account list with logged-in accounts from the AccountManager.
    fn populate_account_list(&mut self, cx: &mut Cx) {
        let count = account_manager::account_count();
        let label_text = if count == 0 {
            tr_key(self.app_language, "settings.account.account_count.none").to_string()
        } else if count == 1 {
            tr_key(self.app_language, "settings.account.account_count.one").to_string()
        } else {
            tr_fmt(
                self.app_language,
                "settings.account.account_count.many",
                &[("count", &count.to_string())],
            )
        };
        self.view.label(cx, ids!(account_count_label)).set_text(cx, &label_text);

        // Get the active account
        let active_user_id = account_manager::get_active_user_id();

        // Show/hide active account view based on whether there's an active account
        let has_active = active_user_id.is_some();
        self.view.view(cx, ids!(active_account_view)).set_visible(cx, has_active);

        // Show the active account
        if let Some(ref active_id) = active_user_id {
            self.view.label(cx, ids!(active_account_label))
                .set_text(cx, active_id.as_str());
        }

        // Get other accounts (excluding active)
        let all_accounts = account_manager::get_all_user_ids();
        self.other_accounts = all_accounts
            .into_iter()
            .filter(|id| Some(id) != active_user_id.as_ref())
            .collect();

        // Show "Other accounts" label and entry only if there are other accounts
        let has_other_accounts = !self.other_accounts.is_empty();
        self.view.label(cx, ids!(other_accounts_label)).set_visible(cx, has_other_accounts);
        self.view.view(cx, ids!(other_account_entry)).set_visible(cx, has_other_accounts);

        // If there's at least one other account, show it
        if let Some(other_id) = self.other_accounts.first() {
            self.view.label(cx, ids!(other_account_label))
                .set_text(cx, other_id.as_str());
        }
    }

    /// Enable or disable the delete avatar button.
    fn enable_delete_avatar_button(
        cx: &mut Cx,
        enable: bool,
        delete_avatar_button: &ButtonRef,
    ) {
        let (delete_button_fg_color, delete_button_bg_color) = if enable {
            (COLOR_FG_DANGER_RED, COLOR_BG_DANGER_RED)
        } else {
            (COLOR_FG_DISABLED, COLOR_BG_DISABLED)
        };
        let mut delete_avatar_button = delete_avatar_button.clone();
        script_apply_eval!(cx, delete_avatar_button, {
            enabled: #(enable),
            draw_bg +: {
                color: #(delete_button_bg_color),
                border_color: #(delete_button_fg_color),
            }
            draw_icon +: {
                color: #(delete_button_fg_color),
            }
            draw_text +: {
                color: #(delete_button_fg_color),
            }
        });
    }

    /// Enable or disable the upload avatar button.
    fn enable_upload_avatar_button(
        cx: &mut Cx,
        enable: bool,
        upload_avatar_button: &ButtonRef,
    ) {
        let (upload_button_fg_color, upload_button_bg_color) = if enable {
            (COLOR_PRIMARY, COLOR_ACTIVE_PRIMARY)
        } else {
            (COLOR_FG_DISABLED, COLOR_BG_DISABLED)
        };
        let mut upload_avatar_button = upload_avatar_button.clone();
        script_apply_eval!(cx, upload_avatar_button, {
            enabled: #(enable),
            draw_bg +: {
                color: #(upload_button_bg_color),
                border_color: #(upload_button_fg_color),
            }
            draw_icon +: {
                color: #(upload_button_fg_color),
            }
            draw_text +: {
                color: #(upload_button_fg_color),
            }
        });
    }

    /// Enable or disable the display name accept and cancel buttons.
    fn enable_display_name_buttons(
        cx: &mut Cx,
        enable: bool,
        accept_display_name_button: &ButtonRef,
        cancel_display_name_button: &ButtonRef,
    ) {
        let (accept_button_fg_color, accept_button_bg_color) = if enable {
            (COLOR_FG_ACCEPT_GREEN, COLOR_BG_ACCEPT_GREEN)
        } else {
            (COLOR_FG_DISABLED, COLOR_BG_DISABLED)
        };
        let (cancel_button_fg_color, cancel_button_bg_color) = if enable {
            (COLOR_FG_DANGER_RED, COLOR_BG_DANGER_RED)
        } else {
            (COLOR_FG_DISABLED, COLOR_BG_DISABLED)
        };

        let mut accept_display_name_button = accept_display_name_button.clone();
        script_apply_eval!(cx, accept_display_name_button, {
            enabled: #(enable),
            draw_bg +: {
                color: #(accept_button_bg_color),
                border_color: #(accept_button_fg_color),
            },
            draw_text +: {
                color: #(accept_button_fg_color),
            },
            draw_icon +: {
                color: #(accept_button_fg_color),
            }
        });
        let mut cancel_display_name_button = cancel_display_name_button.clone();
        script_apply_eval!(cx, cancel_display_name_button, {
            enabled: #(enable),
            draw_bg +: {
                color: #(cancel_button_bg_color),
                border_color: #(cancel_button_fg_color),
            },
            draw_text +: {
                color: #(cancel_button_fg_color),
            },
            draw_icon +: {
                color: #(cancel_button_fg_color),
            }
        });
    }
}

impl AccountSettingsRef {
    /// See [`AccountSettings::show()`].
    pub fn populate(&self, cx: &mut Cx, own_profile: UserProfile) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.populate(cx, own_profile);
    }

    pub fn set_app_language(&self, cx: &mut Cx, app_language: AppLanguage) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.set_app_language(cx, app_language);
    }
}

/// Actions that are handled by the AccountSettings widget.
#[derive(Debug)]
pub enum AccountSettingsAction {
    /// The avatar delete operation was started (e.g., confirmed in a modal).
    AvatarDeleteStarted,
    /// The avatar upload operation was started (e.g., confirmed in a modal).
    AvatarUploadStarted,
}
