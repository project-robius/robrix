use std::cell::RefCell;

use makepad_widgets::{text::selection::Cursor, *};

use matrix_sdk::ruma::OwnedUserId;
use crate::{account_manager, app::ConfirmDeleteAction, avatar_cache::{self}, home::navigation_tab_bar::get_own_profile, login::login_screen::LoginAction, logout::logout_confirm_modal::{LogoutAction, LogoutConfirmModalAction}, profile::{user_profile::UserProfile, user_profile_cache}, shared::{avatar::{AvatarState, AvatarWidgetExt}, confirmation_modal::ConfirmationModalContent, popup_list::{PopupKind, enqueue_popup_notification}, styles::*}, sliding_sync::{AccountDataAction, AccountSwitchAction, MatrixRequest, submit_async_request}, utils};

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*


    // The view containing all user account-related settings.
    mod.widgets.AccountSettings = #(AccountSettings::register_widget(vm)) {
        width: Fill, height: Fit
        flow: Down

        TitleLabel {
            text: "Account Settings"
        }

        SubsectionLabel {
            text: "Your Avatar:"
        }

        View {
            width: Fill, height: Fit
            // TODO: I'd like to use RightWrap here, but Makepad doesn't yet
            //       support RightWrap with align: Align{y: 0.5}.
            flow: Right,
            align: Align{y: 0.5}

            our_own_avatar := Avatar {
                width: 100,
                height: 100,
                margin: 10,
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
                padding: Inset{ left: 10, right: 10 }
                spacing: 10

                View {
                    width: Fit, height: Fit
                    flow: Right,
                    align: Align{y: 0.5}
                    spacing: 10

                    upload_avatar_button := RobrixIconButton {
                        width: 140,
                        padding: Inset{top: 10, bottom: 10, left: 12, right: 15}
                        margin: 0,
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
                    spacing: 10

                    delete_avatar_button := RobrixNegativeIconButton {
                        width: 140,
                        padding: Inset{top: 10, bottom: 10, left: 12, right: 15}
                        margin: 0,
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

        SubsectionLabel {
            text: "Your Display Name:"
        }

        display_name_input := RobrixTextInput {
            margin: Inset{top: 3, left: 5, right: 5, bottom: 8},
            width: 216, height: Fit
            empty_text: "Add a display name..."
        }

        View {
            width: Fill, height: Fit
            flow: Flow.Right{wrap: true},
            align: Align{y: 0.5},
            spacing: 10

            // These buttons are disabled by default, and enabled when the user
            // changes the `display_name_input` text.
            // These buttons start disabled; Rust code enables them and swaps
            // their styles to RobrixNeutralIconButton / RobrixPositiveIconButton.
            cancel_display_name_button := RobrixNeutralIconButton {
                enabled: false,
                width: Fit, height: Fit,
                padding: 10,
                margin: Inset{left: 5},
                draw_icon.svg: (ICON_FORBIDDEN)
                icon_walk: Walk{width: 16, height: 16, margin: 0}
                text: "Cancel"
            }

            accept_display_name_button := RobrixPositiveIconButton {
                enabled: false,
                width: Fit, height: Fit,
                padding: 10,
                margin: Inset{left: 5},
                draw_bg.border_radius: 5.0
                draw_icon.svg: (ICON_CHECKMARK)
                icon_walk: Walk{width: 16, height: 16, margin: 0}
                text: "Save Name"
            }

            save_name_spinner := LoadingSpinner {
                width: 16, height: 16
                margin: Inset{left: 5, top: 13} // vertically center with buttons
                visible: false
                draw_bg.color: (COLOR_ACTIVE_PRIMARY)
            }
        }

        SubsectionLabel {
            text: "Your User ID:"
        }

        View {
            width: Fill, height: Fit
            flow: Right,
            spacing: 10

            copy_user_id_button := RobrixNeutralIconButton {
                enable_long_press: true,
                margin: Inset{left: 5}
                padding: 12,
                spacing: 0,
                draw_icon.svg: (ICON_COPY)
                icon_walk: Walk{width: 16, height: 16, margin: Inset{right: -2} }
            }

            user_id := Label {
                width: Fill, height: Fit
                flow: Flow.Right{wrap: true},
                margin: Inset{top: 10}
                draw_text +: {
                    color: (MESSAGE_TEXT_COLOR),
                    text_style: MESSAGE_TEXT_STYLE { font_size: 11 },
                }
                text: "You are not logged in."
            }
        }

        SubsectionLabel {
            text: "Multiple Accounts:"
        }

        View {
            width: Fill, height: Fit
            flow: Down,
            spacing: 8,
            margin: Inset{left: 5, right: 5, bottom: 10}

            // Account entries will be shown here
            // Active account (current)
            active_account_view := RoundedView {
                width: Fill, height: Fit
                flow: Right,
                align: Align{y: 0.5}
                padding: Inset{left: 10, right: 10, top: 8, bottom: 8}
                spacing: 10
                show_bg: true
                draw_bg +: {
                    color: (COLOR_ACTIVE_PRIMARY)
                    border_radius: 4.0
                }

                View {
                    width: Fill, height: Fit
                    flow: Down,
                    spacing: 2

                    active_account_label := Label {
                        width: Fill, height: Fit
                        draw_text +: {
                            color: (COLOR_TEXT),
                            text_style: MESSAGE_TEXT_STYLE { font_size: 11 },
                        }
                        text: "@user:server"
                    }

                    Label {
                        width: Fit, height: Fit
                        draw_text +: {
                            color: (COLOR_FG_ACCEPT_GREEN),
                            text_style: MESSAGE_TEXT_STYLE { font_size: 9 },
                        }
                        text: "Active"
                    }
                }
            }

            // Other accounts section (populated dynamically)
            other_accounts_label := Label {
                width: Fill, height: Fit
                margin: Inset{top: 5, left: 2}
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
                padding: Inset{left: 10, right: 10, top: 8, bottom: 8}
                spacing: 10
                visible: false
                show_bg: true
                draw_bg +: {
                    color: (COLOR_SECONDARY)
                    border_radius: 4.0
                    border_size: 1.0
                    border_color: #555
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
                    padding: Inset{top: 6, bottom: 6, left: 10, right: 10}
                    draw_icon.svg: (ICON_JUMP)
                    icon_walk: Walk{width: 14, height: 14}
                    text: "Switch"
                }
            }

            account_count_label := Label {
                width: Fill, height: Fit
                margin: Inset{top: 5, bottom: 5, left: 5}
                draw_text +: {
                    color: (MESSAGE_TEXT_COLOR),
                    text_style: MESSAGE_TEXT_STYLE { font_size: 10 },
                }
                text: "1 account logged in"
            }

            add_account_button := RobrixIconButton {
                width: Fit,
                padding: Inset{top: 10, bottom: 10, left: 12, right: 15}
                margin: Inset{top: 5}
                draw_icon.svg: (ICON_ADD)
                icon_walk: Walk{width: 16, height: 16}
                text: "Add Another Account"
            }
        }

        SubsectionLabel {
            text: "Other actions:"
        }

        View {
            // margin: Inset{top: 20},
            width: Fill, height: Fit
            flow: Flow.Right{wrap: true},
            align: Align{y: 0.5},
            spacing: 10

            manage_account_button := RobrixIconButton {
                padding: Inset{top: 10, bottom: 10, left: 12, right: 15}
                margin: Inset{left: 5}
                draw_icon.svg: (ICON_EXTERNAL_LINK)
                icon_walk: Walk{width: 16, height: 16}
                text: "Manage Account"
            }

            logout_button := RobrixNegativeIconButton {
                padding: Inset{top: 10, bottom: 10, left: 12, right: 15}
                margin: Inset{left: 5}
                draw_icon.svg: (ICON_LOGOUT)
                icon_walk: Walk{ width: 16, height: 16, margin: Inset{right: -2} }
                text: "Log out"
            }
        }
    }
}

/// The view containing all user account-related settings.
#[derive(Script, ScriptHook, Widget)]
pub struct AccountSettings {
    #[deref] view: View,

    #[rust] own_profile: Option<UserProfile>,
    /// List of other account user IDs (not the currently active one)
    #[rust] other_accounts: Vec<OwnedUserId>,
}

impl Widget for AccountSettings {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.match_event(cx, event);

        let copy_user_id_button = self.view.button(cx, ids!(copy_user_id_button));
        let copy_user_id_button_area = copy_user_id_button.area();
        match event.hits(cx, copy_user_id_button_area) {
            Hit::FingerHoverIn(_) | Hit::FingerLongPress(_) => {
                cx.widget_action(
                    copy_user_id_button.widget_uid(),
                    TooltipAction::HoverIn {
                        text: "Copy User ID".to_string(),
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
        self.view.draw_walk(cx, scope, walk)
    }
}

impl MatchEvent for AccountSettings {
    fn handle_signal(&mut self, cx: &mut Cx) {
        // Process avatar updates from the cache
        avatar_cache::process_avatar_updates(cx);

        // If we don't have a profile yet, try to get it
        if self.own_profile.is_none() {
            user_profile_cache::process_user_profile_updates(cx);
            if let Some(new_profile) = get_own_profile(cx) {
                self.own_profile = Some(new_profile.clone());
                self.view.label(cx, ids!(user_id))
                    .set_text(cx, new_profile.user_id.as_str());
                self.view.text_input(cx, ids!(display_name_input))
                    .set_text(cx, new_profile.username.as_deref().unwrap_or_default());
                self.populate_avatar_views(cx);
                self.populate_account_list(cx);
                self.view.redraw(cx);
            }
            return;
        }

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
            // Handle LogoutAction::InProgress to update button state
            if let Some(LogoutAction::InProgress(is_in_progress)) = action.downcast_ref() {
                let logout_button = self.view.button(cx, ids!(logout_button));
                logout_button.set_text(cx, if *is_in_progress { "Logging out..." } else { "Log out" });
                logout_button.set_enabled(cx, !*is_in_progress);
                logout_button.reset_hover(cx);
                continue;
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
                            format!("Successfully {} avatar.", if new_avatar_url.is_some() { "updated" } else { "deleted" }),
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
                        format!("Successfully {} display name.", if new_name.is_some() { "updated" } else { "removed" }),
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

        let Some(own_profile) = &self.own_profile else { return };

        if upload_avatar_button.clicked(actions) {
            enqueue_popup_notification(
                "Avatar upload is not yet implemented.",
                PopupKind::Info,
                Some(3.0),
            );
        }

        if delete_avatar_button.clicked(actions) {
            // Don't immediately disable the buttons. Instead, we wait for the user
            // to confirm the action in the confirmation modal,
            // and then we disable the buttons in the AvatarDeleteStarted action handler.
            let content = ConfirmationModalContent {
                title_text: "Delete Avatar".into(),
                body_text: "Are you sure you want to delete your avatar?".into(),
                accept_button_text: Some("Delete".into()),
                on_accept_clicked: Some(Box::new(|cx| {
                    submit_async_request(MatrixRequest::SetAvatar { avatar_url: None });
                    cx.action(AccountSettingsAction::AvatarDeleteStarted);
                    enqueue_popup_notification(
                        "Deleting your avatar...",
                        PopupKind::Info,
                        Some(5.0),
                    );
                })),
                ..Default::default()
            };
            cx.action(ConfirmDeleteAction::Show(RefCell::new(Some(content))));
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
                "Uploading new display name...",
                PopupKind::Info,
                Some(5.0),
            );
        }

        if self.view.button(cx, ids!(copy_user_id_button)).clicked(actions) {
            cx.copy_to_clipboard(own_profile.user_id.as_str());
            enqueue_popup_notification(
                "Copied your User ID to the clipboard.",
                PopupKind::Success,
                Some(3.0),
            );
        }

        if self.view.button(cx, ids!(manage_account_button)).clicked(actions) {
            // TODO: support opening the user's account management page in a browser,
            //       or perhaps in an in-app pane if that's what is needed for regular UN+PW login.
            enqueue_popup_notification(
                "Account management is not yet implemented.",
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
        self.populate_account_list(cx);

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
            "No accounts logged in".to_string()
        } else if count == 1 {
            "1 account logged in".to_string()
        } else {
            format!("{} accounts logged in", count)
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
}

/// Actions that are handled by the AccountSettings widget.
#[derive(Debug)]
pub enum AccountSettingsAction {
    /// The avatar delete operation was started (e.g., confirmed in a modal).
    AvatarDeleteStarted,
    /// The avatar upload operation was started (e.g., confirmed in a modal).
    AvatarUploadStarted,
}
