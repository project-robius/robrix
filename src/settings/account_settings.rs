use std::cell::RefCell;

use makepad_widgets::{text::selection::Cursor, *};

use crate::{
    app::ConfirmDeleteAction,
    avatar_cache::{self},
    logout::logout_confirm_modal::{LogoutAction, LogoutConfirmModalAction},
    profile::user_profile::UserProfile,
    shared::{
        avatar::{AvatarState, AvatarWidgetExt},
        callout_tooltip::{CalloutTooltipOptions, TooltipAction, TooltipPosition},
        confirmation_modal::ConfirmationModalContent,
        popup_list::{PopupKind, enqueue_popup_notification},
        styles::*,
    },
    sliding_sync::{AccountDataAction, MatrixRequest, submit_async_request},
    utils,
};

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::helpers::*;
    use crate::shared::styles::*;
    use crate::shared::avatar::*;
    use crate::shared::icon_button::*;

    // The view containing all user account-related settings.
    pub AccountSettings = {{AccountSettings}} {
        width: Fill, height: Fit
        flow: Down

        <TitleLabel> {
            text: "Account Settings"
        }

        <SubsectionLabel> {
            text: "Your Avatar:"
        }

        <View> {
            width: Fill, height: Fit
            // TODO: I'd like to use RightWrap here, but Makepad doesn't yet
            //       support RightWrap with align: {y: 0.5}.
            flow: Right,
            align: {y: 0.5}

            our_own_avatar = <Avatar> {
                width: 100,
                height: 100,
                margin: 10,
                text_view = { text = { draw_text: {
                    text_style: { font_size: 35.0 }
                }}}
            }

            <View> {
                width: Fit, height: Fit
                flow: Down,
                align: {y: 0.5}
                padding: { left: 10, right: 10 }
                spacing: 10

                <View> {
                    width: Fit, height: Fit
                    flow: Right,
                    align: {y: 0.5}
                    spacing: 10

                    upload_avatar_button = <RobrixIconButton> {
                        width: 140,
                        padding: {top: 10, bottom: 10, left: 12, right: 15}
                        margin: 0,
                        draw_bg: {
                            color: (COLOR_ACTIVE_PRIMARY)
                        }
                        draw_icon: {
                            svg_file: (ICON_UPLOAD)
                            color: (COLOR_PRIMARY)
                        }
                        draw_text: {
                            color: (COLOR_PRIMARY)
                            text_style: <REGULAR_TEXT> {}
                        }
                        icon_walk: {width: 16, height: 16}
                        text: "Upload Avatar"
                    }

                    upload_avatar_spinner = <LoadingSpinner> {
                        width: 16, height: 16
                        visible: false
                        draw_bg: {
                            color: (COLOR_ACTIVE_PRIMARY)
                        }
                    }
                }

                <View> {
                    width: Fit, height: Fit
                    flow: Right,
                    align: {y: 0.5}
                    spacing: 10

                    delete_avatar_button = <RobrixIconButton> {
                        width: 140,
                        padding: {top: 10, bottom: 10, left: 12, right: 15}
                        margin: 0,
                        draw_bg: {
                            color: (COLOR_BG_DANGER_RED)
                            border_color: (COLOR_FG_DANGER_RED)
                        }
                        draw_icon: {
                            svg_file: (ICON_TRASH),
                            color: (COLOR_FG_DANGER_RED),
                        }
                        draw_text: {
                            color: (COLOR_FG_DANGER_RED),
                        }
                        icon_walk: { width: 16, height: 16 }
                        text: "Delete Avatar"
                    }

                    delete_avatar_spinner = <LoadingSpinner> {
                        width: 16, height: 16
                        visible: false
                        draw_bg: {
                            color: (COLOR_ACTIVE_PRIMARY)
                        }
                    }
                }
            }
        }

        <SubsectionLabel> {
            text: "Your Display Name:"
        }

        display_name_input = <SimpleTextInput> {
            margin: {top: 3, left: 5, right: 5, bottom: 8},
            width: 216, height: Fit
            empty_text: "Add a display name..."
        }

        <View> {
            width: Fill, height: Fit
            flow: RightWrap,
            align: {y: 0.5},
            spacing: 10

            // These buttons are disabled by default, and enabled when the user
            // changes the `display_name_input` text.
            cancel_display_name_button = <RobrixIconButton> {
                enabled: false,
                width: Fit, height: Fit,
                padding: 10,
                margin: {left: 5},

                draw_bg: {
                    color: (COLOR_BG_DISABLED)
                }
                draw_icon: {
                    svg_file: (ICON_FORBIDDEN),
                    color: (COLOR_FG_DISABLED)
                }
                icon_walk: {width: 16, height: 16, margin: 0}
                draw_text: {
                    color: (COLOR_FG_DISABLED),
                }
                text: "Cancel"
            }

            accept_display_name_button = <RobrixIconButton> {
                enabled: false,
                width: Fit, height: Fit,
                padding: 10,
                margin: {left: 5},

                draw_bg: {
                    border_color: (COLOR_FG_DISABLED),
                    color: (COLOR_BG_DISABLED),
                    border_radius: 5
                }
                draw_icon: {
                    svg_file: (ICON_CHECKMARK)
                    color: (COLOR_FG_DISABLED),
                }
                icon_walk: {width: 16, height: 16, margin: 0}
                draw_text: {
                    color: (COLOR_FG_DISABLED),
                }
                text: "Save Name"
            }

            save_name_spinner = <LoadingSpinner> {
                width: 16, height: 16
                margin: {left: 5, top: 13} // vertically center with buttons
                visible: false
                draw_bg: {
                    color: (COLOR_ACTIVE_PRIMARY)
                }
            }
        }

        <SubsectionLabel> {
            text: "Your User ID:"
        }

        <View> {
            width: Fill, height: Fit
            flow: Right,
            spacing: 10

            copy_user_id_button = <RobrixIconButton> {
                enable_long_press: true,
                margin: {left: 5}
                padding: 12,
                spacing: 0,
                draw_bg: {
                    color: (COLOR_SECONDARY)
                }
                draw_icon: {
                    svg_file: (ICON_COPY)
                }
                icon_walk: {width: 16, height: 16, margin: {right: -2} }
            }

            user_id = <Label> {
                width: Fill, height: Fit
                flow: RightWrap,
                margin: {top: 10}
                draw_text: {
                    wrap: Line,
                    color: (MESSAGE_TEXT_COLOR),
                    text_style: <MESSAGE_TEXT_STYLE>{ font_size: 11 },
                }
                text: "You are not logged in."
            }
        }

        <SubsectionLabel> {
            text: "Other actions:"
        }

        <View> {
            // margin: {top: 20},
            width: Fill, height: Fit
            flow: RightWrap,
            align: {y: 0.5},
            spacing: 10

            manage_account_button = <RobrixIconButton> {

                padding: {top: 10, bottom: 10, left: 12, right: 15}
                margin: {left: 5}
                draw_bg: {
                    color: (COLOR_ACTIVE_PRIMARY)
                }
                draw_icon: {
                    svg_file: (ICON_EXTERNAL_LINK)
                    color: (COLOR_PRIMARY)
                }
                draw_text: {
                    color: (COLOR_PRIMARY)
                    text_style: <REGULAR_TEXT> {}
                }
                icon_walk: {width: 16, height: 16}
                text: "Manage Account"
            }

            logout_button = <RobrixIconButton> {
                padding: {top: 10, bottom: 10, left: 12, right: 15}
                margin: {left: 5}
                draw_bg: {
                    color: (COLOR_BG_DANGER_RED)
                    border_color: (COLOR_FG_DANGER_RED)
                }
                draw_icon: {
                    svg_file: (ICON_LOGOUT),
                    color: (COLOR_FG_DANGER_RED),
                }
                draw_text: {
                    color: (COLOR_FG_DANGER_RED),
                }
                icon_walk: { width: 16, height: 16, margin: {right: -2} }
                text: "Log out"
            }
        }
    }
}

/// The view containing all user account-related settings.
#[derive(Live, LiveHook, Widget)]
pub struct AccountSettings {
    #[deref]
    view: View,

    #[rust]
    own_profile: Option<UserProfile>,
}

impl Widget for AccountSettings {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.match_event(cx, event);

        let copy_user_id_button = self.view.button(ids!(copy_user_id_button));
        let copy_user_id_button_area = copy_user_id_button.area();
        match event.hits(cx, copy_user_id_button_area) {
            Hit::FingerHoverIn(_) | Hit::FingerLongPress(_) => {
                cx.widget_action(
                    copy_user_id_button.widget_uid(),
                    &scope.path,
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
                    &scope.path,
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
        if self.own_profile.is_none() {
            return;
        }
        avatar_cache::process_avatar_updates(cx);

        if let Some(profile) = self.own_profile.as_mut() {
            profile.avatar_state.update_from_cache(cx);
        }
    }

    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        let accept_display_name_button = self.view.button(ids!(accept_display_name_button));
        let cancel_display_name_button = self.view.button(ids!(cancel_display_name_button));
        let display_name_input = self.view.text_input(ids!(display_name_input));
        let delete_avatar_button = self.view.button(ids!(delete_avatar_button));
        let upload_avatar_button = self.view.button(ids!(upload_avatar_button));

        for action in actions {
            // Handle LogoutAction::InProgress to update button state
            if let Some(LogoutAction::InProgress(is_in_progress)) = action.downcast_ref() {
                let logout_button = self.view.button(ids!(logout_button));
                logout_button.set_text(
                    cx,
                    if *is_in_progress {
                        "Logging out..."
                    } else {
                        "Log out"
                    },
                );
                logout_button.set_enabled(cx, !*is_in_progress);
                logout_button.reset_hover(cx);
                continue;
            }

            // Handle account data changes.
            // Note: the NavigationTabBar handles removing stale data from the user_profile_cache,
            // so here, we only need to update this widget's local profile info.
            match action.downcast_ref() {
                Some(AccountDataAction::AvatarChanged(new_avatar_url)) => {
                    self.view
                        .widget(ids!(upload_avatar_spinner))
                        .set_visible(cx, false);
                    self.view
                        .widget(ids!(delete_avatar_spinner))
                        .set_visible(cx, false);
                    // Update our cached profile with the new avatar URL
                    if let Some(profile) = self.own_profile.as_mut() {
                        profile.avatar_state = AvatarState::Known(new_avatar_url.clone());
                        profile.avatar_state.update_from_cache(cx);
                        self.populate_avatar_views(cx);
                        enqueue_popup_notification(
                            format!(
                                "Successfully {} avatar.",
                                if new_avatar_url.is_some() {
                                    "updated"
                                } else {
                                    "deleted"
                                }
                            ),
                            PopupKind::Success,
                            Some(4.0),
                        );
                    }
                    continue;
                }
                Some(AccountDataAction::AvatarChangeFailed(err_msg)) => {
                    self.view
                        .widget(ids!(upload_avatar_spinner))
                        .set_visible(cx, false);
                    self.view
                        .widget(ids!(delete_avatar_spinner))
                        .set_visible(cx, false);
                    // Re-enable the avatar buttons so user can try again
                    Self::enable_upload_avatar_button(cx, true, &upload_avatar_button);
                    Self::enable_delete_avatar_button(
                        cx,
                        self.own_profile
                            .as_ref()
                            .is_some_and(|p| p.avatar_state.has_avatar()),
                        &delete_avatar_button,
                    );
                    enqueue_popup_notification(err_msg.clone(), PopupKind::Error, Some(4.0));
                    continue;
                }
                Some(AccountDataAction::DisplayNameChanged(new_name)) => {
                    self.view
                        .widget(ids!(save_name_spinner))
                        .set_visible(cx, false);
                    // Update our cached profile with the new display name
                    if let Some(profile) = self.own_profile.as_mut() {
                        profile.username = new_name.clone();
                    }
                    // Update the display name text input and disable buttons
                    let (text, len) = new_name
                        .as_deref()
                        .map(|s| (s, s.len()))
                        .unwrap_or_default();
                    display_name_input.set_text(cx, text);
                    display_name_input.set_cursor(
                        cx,
                        Cursor {
                            index: len,
                            prefer_next_row: false,
                        },
                        false,
                    );
                    display_name_input.set_is_read_only(cx, false);
                    display_name_input.set_disabled(cx, false);
                    Self::enable_display_name_buttons(
                        cx,
                        false,
                        &accept_display_name_button,
                        &cancel_display_name_button,
                    );
                    enqueue_popup_notification(
                        format!(
                            "Successfully {} display name.",
                            if new_name.is_some() {
                                "updated"
                            } else {
                                "removed"
                            }
                        ),
                        PopupKind::Success,
                        Some(4.0),
                    );
                    continue;
                }
                Some(AccountDataAction::DisplayNameChangeFailed(err_msg)) => {
                    self.view
                        .widget(ids!(save_name_spinner))
                        .set_visible(cx, false);
                    // Re-enable the buttons and text input so that the user can try again
                    display_name_input.set_is_read_only(cx, false);
                    display_name_input.set_disabled(cx, false);
                    Self::enable_display_name_buttons(
                        cx,
                        true,
                        &accept_display_name_button,
                        &cancel_display_name_button,
                    );
                    enqueue_popup_notification(err_msg.clone(), PopupKind::Error, Some(4.0));
                    continue;
                }
                _ => {}
            }

            match action.downcast_ref() {
                Some(AccountSettingsAction::AvatarDeleteStarted) => {
                    self.view
                        .widget(ids!(delete_avatar_spinner))
                        .set_visible(cx, true);
                    Self::enable_upload_avatar_button(cx, false, &upload_avatar_button);
                    Self::enable_delete_avatar_button(cx, false, &delete_avatar_button);
                    continue;
                }
                Some(AccountSettingsAction::AvatarUploadStarted) => {
                    self.view
                        .widget(ids!(upload_avatar_spinner))
                        .set_visible(cx, true);
                    Self::enable_upload_avatar_button(cx, false, &upload_avatar_button);
                    Self::enable_delete_avatar_button(cx, false, &delete_avatar_button);
                    continue;
                }
                _ => {}
            }
        }

        let Some(own_profile) = &self.own_profile else {
            return;
        };

        if upload_avatar_button.clicked(actions) {
            // TODO: uncomment the below once avatar uploading is implemented
            // Self::enable_upload_avatar_button(cx, false, &upload_avatar_button);
            // Self::enable_delete_avatar_button(cx, false, &delete_avatar_button);
            enqueue_popup_notification(
                "Avatar uploading is not yet implemented.",
                PopupKind::Warning,
                Some(4.0),
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
            Self::enable_display_name_buttons(
                cx,
                enable,
                &accept_display_name_button,
                &cancel_display_name_button,
            );
        }

        if cancel_display_name_button.clicked(actions) {
            // Reset the display name input and disable the name change buttons.
            let new_text = own_profile.username.as_deref().unwrap_or("");
            display_name_input.set_text(cx, new_text);
            display_name_input.set_cursor(
                cx,
                Cursor {
                    index: new_text.len(),
                    prefer_next_row: false,
                },
                false,
            );
            Self::enable_display_name_buttons(
                cx,
                false,
                &accept_display_name_button,
                &cancel_display_name_button,
            );
        }

        if accept_display_name_button.clicked(actions) {
            let new_display_name = match display_name_input.text().trim() {
                "" => None,
                name => Some(name.to_string()),
            };
            // While the request is in flight, show the loading spinner and disable the buttons & text input
            submit_async_request(MatrixRequest::SetDisplayName { new_display_name });
            self.view
                .widget(ids!(save_name_spinner))
                .set_visible(cx, true);
            display_name_input.set_disabled(cx, true);
            display_name_input.set_is_read_only(cx, true);
            Self::enable_display_name_buttons(
                cx,
                false,
                &accept_display_name_button,
                &cancel_display_name_button,
            );
            enqueue_popup_notification("Uploading new display name...", PopupKind::Info, Some(5.0));
        }

        if self.view.button(ids!(copy_user_id_button)).clicked(actions) {
            cx.copy_to_clipboard(own_profile.user_id.as_str());
            enqueue_popup_notification(
                "Copied your User ID to the clipboard.",
                PopupKind::Success,
                Some(3.0),
            );
        }

        if self
            .view
            .button(ids!(manage_account_button))
            .clicked(actions)
        {
            // TODO: support opening the user's account management page in a browser,
            //       or perhaps in an in-app pane if that's what is needed for regular UN+PW login.
            enqueue_popup_notification(
                "Account management is not yet implemented.",
                PopupKind::Warning,
                Some(4.0),
            );
        }

        if self.view.button(ids!(logout_button)).clicked(actions) {
            cx.action(LogoutConfirmModalAction::Open);
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

        let our_own_avatar = self.view.avatar(ids!(our_own_avatar));
        let mut drew_avatar = false;
        if let Some(avatar_img_data) = own_profile.avatar_state.data() {
            drew_avatar = our_own_avatar
                .show_image(
                    cx,
                    None, // don't make this avatar clickable; we handle clicks on this ProfileIcon widget directly.
                    |cx, img| utils::load_png_or_jpg(&img, cx, avatar_img_data),
                )
                .is_ok();
        }
        if !drew_avatar {
            our_own_avatar.show_text(
                cx,
                Some(COLOR_ROBRIX_PURPLE),
                None, // don't make this avatar clickable; we handle clicks on this ProfileIcon widget directly.
                own_profile.displayable_name(),
            );
        }

        Self::enable_upload_avatar_button(cx, true, &self.view.button(ids!(upload_avatar_button)));
        Self::enable_delete_avatar_button(
            cx,
            own_profile.avatar_state.has_avatar(),
            &self.view.button(ids!(delete_avatar_button)),
        );
    }

    /// Show and initializes the account settings within the SettingsScreen.
    pub fn populate(&mut self, cx: &mut Cx, own_profile: UserProfile) {
        self.view
            .label(ids!(user_id))
            .set_text(cx, own_profile.user_id.as_str());
        self.view
            .text_input(ids!(display_name_input))
            .set_text(cx, own_profile.username.as_deref().unwrap_or_default());
        Self::enable_display_name_buttons(
            cx,
            false,
            &self.view.button(ids!(accept_display_name_button)),
            &self.view.button(ids!(cancel_display_name_button)),
        );

        self.own_profile = Some(own_profile);
        self.populate_avatar_views(cx);

        self.view.button(ids!(upload_avatar_button)).reset_hover(cx);
        self.view.button(ids!(delete_avatar_button)).reset_hover(cx);
        self.view
            .button(ids!(accept_display_name_button))
            .reset_hover(cx);
        self.view
            .button(ids!(cancel_display_name_button))
            .reset_hover(cx);
        self.view.button(ids!(copy_user_id_button)).reset_hover(cx);
        self.view
            .button(ids!(manage_account_button))
            .reset_hover(cx);
        self.view.button(ids!(logout_button)).reset_hover(cx);
        self.view.redraw(cx);
    }

    /// Enable or disable the delete avatar button.
    fn enable_delete_avatar_button(cx: &mut Cx, enable: bool, delete_avatar_button: &ButtonRef) {
        let (delete_button_fg_color, delete_button_bg_color) = if enable {
            (COLOR_FG_DANGER_RED, COLOR_BG_DANGER_RED)
        } else {
            (COLOR_FG_DISABLED, COLOR_BG_DISABLED)
        };
        delete_avatar_button.apply_over(
            cx,
            live! {
                enabled: (enable),
                draw_bg: {
                    color: (delete_button_bg_color),
                    border_color: (delete_button_fg_color),
                }
                draw_icon: {
                    color: (delete_button_fg_color),
                }
                draw_text: {
                    color: (delete_button_fg_color),
                }
            },
        );
    }

    /// Enable or disable the upload avatar button.
    fn enable_upload_avatar_button(cx: &mut Cx, enable: bool, upload_avatar_button: &ButtonRef) {
        let (upload_button_fg_color, upload_button_bg_color) = if enable {
            (COLOR_PRIMARY, COLOR_ACTIVE_PRIMARY)
        } else {
            (COLOR_FG_DISABLED, COLOR_BG_DISABLED)
        };
        upload_avatar_button.apply_over(
            cx,
            live! {
                enabled: (enable),
                draw_bg: {
                    color: (upload_button_bg_color),
                    border_color: (upload_button_fg_color),
                }
                draw_icon: {
                    color: (upload_button_fg_color),
                }
                draw_text: {
                    color: (upload_button_fg_color),
                }
            },
        );
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

        accept_display_name_button.apply_over(
            cx,
            live!(
                enabled: (enable),
                draw_bg: {
                    color: (accept_button_bg_color),
                    border_color: (accept_button_fg_color),
                },
                draw_text: {
                    color: (accept_button_fg_color),
                },
                draw_icon: {
                    color: (accept_button_fg_color),
                }
            ),
        );
        cancel_display_name_button.apply_over(
            cx,
            live!(
                enabled: (enable),
                draw_bg: {
                    color: (cancel_button_bg_color),
                    border_color: (cancel_button_fg_color),
                },
                draw_text: {
                    color: (cancel_button_fg_color),
                },
                draw_icon: {
                    color: (cancel_button_fg_color),
                }
            ),
        );
    }
}

impl AccountSettingsRef {
    /// See [`AccountSettings::show()`].
    pub fn populate(&self, cx: &mut Cx, own_profile: UserProfile) {
        let Some(mut inner) = self.borrow_mut() else {
            return;
        };
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
