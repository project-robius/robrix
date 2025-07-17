use makepad_widgets::*;

use crate::{profile::user_profile::UserProfile, shared::{avatar::AvatarWidgetExt, popup_list::{enqueue_popup_notification, PopupItem}, styles::COLOR_ROBRIX_PURPLE}, utils};

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::helpers::*;
    use crate::shared::styles::*;
    use crate::shared::avatar::*;
    use crate::shared::icon_button::*;

    SubsectionLabel = <Label> {
        width: Fill, height: Fit
        margin: {top: 5},
        flow: Right,
        draw_text: {
            color: (COLOR_TEXT),
            text_style: <USERNAME_TEXT_STYLE>{ font_size: 11 },
        }
    }

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
                        color: #fff0f0 // light red
                        border_color: (COLOR_DANGER_RED)
                    }
                    draw_icon: {
                        svg_file: (ICON_TRASH),
                        color: (COLOR_DANGER_RED),
                    }
                    draw_text: {
                        color: (COLOR_DANGER_RED),
                    }
                    icon_walk: { width: 16, height: 16 }
                    text: "Delete Avatar"
                }
            }
        }

        <SubsectionLabel> {
            text: "Your Display Name:"
        }

        display_name_input = <RobrixTextInput> {
            padding: 10,
            width: Fit, height: Fit
            flow: Right,
            draw_text: {
                wrap: Word,
            }
            empty_text: "Add a new display name..."
        }

        <View> {
            width: Fill, height: Fit
            flow: RightWrap,
            align: {y: 0.5},
            spacing: 10

            accept_display_name_button = <RobrixIconButton> {
                width: Fit, height: Fit,
                padding: 10,
                margin: {left: 5},

                draw_bg: {
                    border_color: (COLOR_ACCEPT_GREEN),
                    color: #f0fff0 // light green
                    border_radius: 5
                }
                draw_icon: {
                    svg_file: (ICON_CHECKMARK)
                    color: (COLOR_ACCEPT_GREEN),
                }
                draw_text: {
                    color: (COLOR_ACCEPT_GREEN),
                }
                icon_walk: {width: 16, height: 16, margin: 0}
                text: "Save Name"
            }

            cancel_display_name_button = <RobrixIconButton> {
                width: Fit, height: Fit,
                padding: 10,
                margin: {left: 5},

                draw_bg: {
                    color: (COLOR_SECONDARY)
                }
                draw_icon: {
                    svg_file: (ICON_FORBIDDEN),
                    color: (COLOR_TEXT)
                }
                icon_walk: {width: 16, height: 16, margin: 0}
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
                    color: #fff0f0 // light red
                    border_color: (COLOR_DANGER_RED)
                }
                draw_icon: {
                    svg_file: (ICON_LOGOUT),
                    color: (COLOR_DANGER_RED),
                }
                draw_text: {
                    color: (COLOR_DANGER_RED),
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
        let Some(own_profile) = &self.own_profile else { return };

        if self.view.button(id!(upload_avatar_button)).clicked(actions) {
            // TODO: support uploading a new avatar picture.
            enqueue_popup_notification(PopupItem {
                message: String::from("Avatar uploading is not yet implemented."),
                auto_dismissal_duration: Some(4.0),
            });
        }

        if self.view.button(id!(delete_avatar_button)).clicked(actions) {
            // TODO: support removing the avatar picture.
            enqueue_popup_notification(PopupItem {
                message: String::from("Avatar deletion is not yet implemented."),
                auto_dismissal_duration: Some(4.0),
            });
        }

        if self.view.button(id!(accept_display_name_button)).clicked(actions) {
            // TODO: support changing the display name.
            enqueue_popup_notification(PopupItem {
                message: String::from("Display name change is not yet implemented."),
                auto_dismissal_duration: Some(4.0),
            });
        }

        let display_name_input = self.view.text_input(id!(display_name_input));
        let accept_display_name_button = self.view.button(id!(accept_display_name_button));
        let cancel_display_name_button = self.view.button(id!(cancel_display_name_button));

        if cancel_display_name_button.clicked(actions) {
            // Reset the display name input and disable the name change buttons.
            display_name_input.set_text(cx, own_profile.username.as_deref().unwrap_or(""));
            accept_display_name_button.set_enabled(cx, false);
            cancel_display_name_button.set_enabled(cx, false);
        }

        if let Some(new_name) = display_name_input.changed(actions) {
            let enable_buttons = new_name.as_str() != own_profile.username.as_deref().unwrap_or("");
            log!("Display name changed to: {}, buttons enabled? {}", new_name, enable_buttons);
            accept_display_name_button.set_enabled(cx, enable_buttons);
            cancel_display_name_button.set_enabled(cx, enable_buttons);
        }

        if self.view.button(id!(copy_user_id_button)).clicked(actions) {
            cx.copy_to_clipboard(own_profile.user_id.as_str());
            enqueue_popup_notification(PopupItem {
                message: String::from("Copied your User ID to the clipboard."),
                auto_dismissal_duration: Some(3.0),
            });
        }

        if self.view.button(id!(manage_account_button)).clicked(actions) {
            // TODO: support opening the user's account management page in a browser,
            //       or perhaps in an in-app pane if that's what is needed for regular UN+PW login.
            enqueue_popup_notification(PopupItem {
                message: String::from("Account management is not yet implemented."),
                auto_dismissal_duration: Some(4.0),
            });
        }

        if self.view.button(id!(logout_button)).clicked(actions) {
            // TODO: support logging out the user.
            enqueue_popup_notification(PopupItem {
                message: String::from("Logout is not yet implemented."),
                auto_dismissal_duration: Some(4.0),
            });
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

        let our_own_avatar = self.view.avatar(id!(our_own_avatar));
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
            .text_input(id!(display_name_input))
            .set_text(cx, own_profile.username.as_deref().unwrap_or_default());
        self.view
            .label(id!(user_id))
            .set_text(cx, own_profile.user_id.as_str());
    }

    /// Show and initializes the account settings within the SettingsScreen.
    pub fn show(&mut self, cx: &mut Cx, own_profile: UserProfile) {
        self.own_profile = Some(own_profile);
        self.populate_from_profile(cx);

        self.view.button(id!(upload_avatar_button)).reset_hover(cx);
        self.view.button(id!(delete_avatar_button)).reset_hover(cx);
        self.view.button(id!(accept_display_name_button)).reset_hover(cx);
        self.view.button(id!(cancel_display_name_button)).reset_hover(cx);
        self.view.button(id!(copy_user_id_button)).reset_hover(cx);
        self.view.button(id!(manage_account_button)).reset_hover(cx);
        self.view.button(id!(logout_button)).reset_hover(cx);
    }
}

impl AccountSettingsRef {
    /// See [`AccountSettings::show()`].
    pub fn show(&self, cx: &mut Cx, own_profile: UserProfile) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.show(cx, own_profile);
    }
}
