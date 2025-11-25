use makepad_widgets::{text::selection::Cursor, *};

use crate::{logout::logout_confirm_modal::{LogoutAction, LogoutConfirmModalAction}, profile::user_profile::UserProfile, shared::{avatar::AvatarWidgetExt, popup_list::{enqueue_popup_notification, PopupItem, PopupKind}, styles::*}, utils};

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

                upload_avatar_button = <RobrixIconButton> {
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

                delete_avatar_button = <RobrixIconButton> {
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
        }

        <SubsectionLabel> {
            text: "Your User ID:"
        }

        <View> {
            width: Fill, height: Fit
            flow: Right,
            spacing: 10

            copy_user_id_button = <RobrixIconButton> {
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
    #[deref] view: View,

    #[rust] own_profile: Option<UserProfile>,
}

impl Widget for AccountSettings {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.match_event(cx, event);
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl MatchEvent for AccountSettings {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        // Handle LogoutAction::InProgress to update button state
        for action in actions {
            if let Some(LogoutAction::InProgress(value)) = action.downcast_ref() {
                let logout_button = self.view.button(ids!(logout_button));
                if *value {
                    logout_button.set_text(cx, "Log out in progress...");
                    logout_button.set_enabled(cx, false);
                    logout_button.reset_hover(cx);
                } else {
                    logout_button.set_text(cx, "Log out");
                    logout_button.set_enabled(cx, true);
                }
            }
        }
        
        let Some(own_profile) = &self.own_profile else { return };

        if self.view.button(ids!(upload_avatar_button)).clicked(actions) {
            // TODO: support uploading a new avatar picture.
            enqueue_popup_notification(PopupItem {
                message: String::from("Avatar uploading is not yet implemented."),
                auto_dismissal_duration: Some(4.0),
                kind: PopupKind::Warning
            });
        }

        if self.view.button(ids!(delete_avatar_button)).clicked(actions) {
            // TODO: support removing the avatar picture.
            enqueue_popup_notification(PopupItem {
                message: String::from("Avatar deletion is not yet implemented."),
                auto_dismissal_duration: Some(4.0),
                kind: PopupKind::Warning,
            });
        }

        let accept_display_name_button = self.view.button(ids!(accept_display_name_button));
        let cancel_display_name_button = self.view.button(ids!(cancel_display_name_button));
        let display_name_input = self.view.text_input(ids!(display_name_input));
        let enable_buttons = |cx: &mut Cx, enable: bool| {
            accept_display_name_button.set_enabled(cx, enable);
            cancel_display_name_button.set_enabled(cx, enable);
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
            accept_display_name_button.apply_over(cx, live!(
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
            ));
            cancel_display_name_button.apply_over(cx, live!(
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
            ));
        };

        if let Some(new_name) = display_name_input.changed(actions) {
            let should_enable = new_name.as_str() != own_profile.username.as_deref().unwrap_or("");
            enable_buttons(cx, should_enable);
        }

        if cancel_display_name_button.clicked(actions) {
            // Reset the display name input and disable the name change buttons.
            let new_text = own_profile.username.as_deref().unwrap_or("");
            display_name_input.set_text(cx, new_text);
            display_name_input.set_cursor(cx, Cursor { index: new_text.len(), prefer_next_row: false }, false);
            enable_buttons(cx, false);
        }

        if accept_display_name_button.clicked(actions) {
            // TODO: support changing the display name.
            enqueue_popup_notification(PopupItem {
                message: String::from("Display name change is not yet implemented."),
                auto_dismissal_duration: Some(4.0),
                kind: PopupKind::Warning
            });
        }

        if self.view.button(ids!(copy_user_id_button)).clicked(actions) {
            cx.copy_to_clipboard(own_profile.user_id.as_str());
            enqueue_popup_notification(PopupItem {
                message: String::from("Copied your User ID to the clipboard."),
                auto_dismissal_duration: Some(3.0),
                kind: PopupKind::Success
            });
        }

        if self.view.button(ids!(manage_account_button)).clicked(actions) {
            // TODO: support opening the user's account management page in a browser,
            //       or perhaps in an in-app pane if that's what is needed for regular UN+PW login.
            enqueue_popup_notification(PopupItem {
                message: String::from("Account management is not yet implemented."),
                auto_dismissal_duration: Some(4.0),
                kind: PopupKind::Warning
            });
        }

        if self.view.button(ids!(logout_button)).clicked(actions) {
            cx.action(LogoutConfirmModalAction::Open);
        }
    }
}

impl AccountSettings {
    /// Populate the account settings view with the user's profile data.
    ///
    /// This does nothing if `self.own_profile` is `None`.
    fn populate_from_profile(&mut self, cx: &mut Cx) {
        let Some(own_profile) = &self.own_profile else {
            error!("BUG: AccountSettings::populate_from_profile() called with no profile data.");
            return;
        };

        let our_own_avatar = self.view.avatar(ids!(our_own_avatar));
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

        self.view
            .text_input(ids!(display_name_input))
            .set_text(cx, own_profile.username.as_deref().unwrap_or_default());
        self.view
            .label(ids!(user_id))
            .set_text(cx, own_profile.user_id.as_str());
    }

    /// Show and initializes the account settings within the SettingsScreen.
    pub fn populate(&mut self, cx: &mut Cx, own_profile: UserProfile) {
        self.own_profile = Some(own_profile);
        self.populate_from_profile(cx);

        self.view.button(ids!(upload_avatar_button)).reset_hover(cx);
        self.view.button(ids!(delete_avatar_button)).reset_hover(cx);
        self.view.button(ids!(accept_display_name_button)).reset_hover(cx);
        self.view.button(ids!(cancel_display_name_button)).reset_hover(cx);
        self.view.button(ids!(copy_user_id_button)).reset_hover(cx);
        self.view.button(ids!(manage_account_button)).reset_hover(cx);
        self.view.button(ids!(logout_button)).reset_hover(cx);
        self.view.redraw(cx);
    }
}

impl AccountSettingsRef {
    /// See [`AccountSettings::show()`].
    pub fn populate(&self, cx: &mut Cx, own_profile: UserProfile) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.populate(cx, own_profile);
    }
}
