use makepad_widgets::*;

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

        <Label> {
            width: 110, height: Fit
            align: {x: 0.5}
            flow: Right,
            draw_text: {
                color: (COLOR_TEXT),
                text_style: <USERNAME_TEXT_STYLE>{ font_size: 11 },
            }
            text: "Your Avatar:"
        }

        <View> {
            width: Fill, height: Fit
            // TODO: I'd like to use RightWrap here, but Makepad doesn't yet
            //       support RightWrap with align: {y: 0.5}.
            flow: Right,
            align: {y: 0.5}

            avatar = <Avatar> {
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
                    // TODO: support uploading a new avatar picture.
                    enabled: false,
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

                remove_avatar_button = <RobrixIconButton> {
                    // TODO: support removing the avatar picture.
                    enabled: false,
                    padding: {top: 10, bottom: 10, left: 12, right: 15}
                    margin: 0,
                    draw_bg: {
                        color: #f8d0d0 // light red
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

        <View> {
            width: Fill, height: Fit
            flow: Down

            <View> {
                width: Fill, height: Fit,
                flow: Right,
                padding: { left: 10, right: 10 }
                align: {x: 0.5, y: 0.5} // center horizontally and vertically

                <LineH> {
                    draw_bg: { color: #C8C8C8 }
                }

                <Label> {
                    width: Fit, height: Fit
                    padding: 0
                    draw_text: {
                        color: (COLOR_TEXT),
                        text_style: <REGULAR_TEXT>{font_size: 9}
                    }
                    text: "Display Name"
                }

                right_line = <LineH> {
                    draw_bg: { color: #C8C8C8 }
                }
            }

            display_name_input = <RobrixTextInput> {
                padding: 10,
                width: Fit, height: Fit
                flow: RightWrap,
                draw_text: {
                    wrap: Word,
                }
                empty_text: "Enter a display name..."
            }

            <View> {
                visible: false, // Buttons will be shown when the display_name_input is modified.
                width: Fill, height: Fit
                flow: RightWrap,
                margin: {bottom: 20}
                align: {x: 1.0, y: 0.5}
                spacing: 20

                cancel_display_name_button = <RobrixIconButton> {
                    width: 100,
                    align: {x: 0.5, y: 0.5}
                    padding: 15,
                    draw_icon: {
                        svg_file: (ICON_FORBIDDEN)
                        color: (COLOR_DANGER_RED),
                    }
                    icon_walk: {width: 16, height: 16, margin: {left: -2, right: -1} }

                    draw_bg: {
                        border_color: (COLOR_DANGER_RED),
                        color: #fff0f0 // light red
                    }
                    text: "Cancel Display Name"
                    draw_text:{
                        color: (COLOR_DANGER_RED),
                    }
                }

                save_display_name_button = <RobrixIconButton> {
                    width: 100,
                    align: {x: 0.5, y: 0.5}
                    padding: 15,
                    draw_icon: {
                        svg_file: (ICON_CHECKMARK)
                        color: (COLOR_ACCEPT_GREEN),
                    }
                    icon_walk: {width: 16, height: 16, margin: {left: -2, right: -1} }

                    draw_bg: {
                        border_color: (COLOR_ACCEPT_GREEN),
                        color: #f0fff0 // light green
                    }
                    text: "Save Display Name"
                    draw_text:{
                        color: (COLOR_ACCEPT_GREEN),
                    }
                }
            }
        }

        user_id = <Label> {
            margin: {top: 20},
            width: Fit, height: Fit
            draw_text: {
                wrap: Line,
                color: (MESSAGE_TEXT_COLOR),
                text_style: <MESSAGE_TEXT_STYLE>{ font_size: 11 },
            }
            text: "User ID: <Unknown>"
        }

        <View> {
            margin: {top: 20},
            width: Fill, height: Fit
            flow: RightWrap,
            align: {y: 0.5},
            spacing: 20

            copy_user_id_button = <RobrixIconButton> {
                padding: {top: 10, bottom: 10, left: 12, right: 15}
                margin: 0,
                draw_bg: {
                    color: (COLOR_SECONDARY)
                }
                draw_icon: {
                    svg_file: (ICON_COPY)
                }
                icon_walk: {width: 16, height: 16, margin: {right: -2} }
                text: "Copy User ID"
            }

            manage_account_button = <RobrixIconButton> {
                // TODO: support opening the user's account management page in a browser,
                //       or perhaps in an in-app pane if that's what is needed for regular UN+PW login.
                enabled: false,
                padding: {top: 10, bottom: 10, left: 12, right: 15}
                margin: 0,
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
                // TODO: enable this once logout is implemented; see PR #432
                enabled: false,
                padding: {top: 10, bottom: 10, left: 12, right: 15}
                margin: 0,
                draw_bg: {
                    color: #f8d0d0 // light red
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
}

impl Widget for AccountSettings {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}


impl AccountSettings {
    /// Show and initializes the account settings within the SettingsScreen.
    pub fn show(&mut self, cx: &mut Cx) {
        self.view.button(id!(copy_user_id_button)).reset_hover(cx);
        self.view.button(id!(manage_account_button)).reset_hover(cx);
        self.view.button(id!(logout_button)).reset_hover(cx);
    }
}

impl AccountSettingsRef {
    /// See [`AccountSettings::show()`].
    pub fn show(&self, cx: &mut Cx) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.show(cx)
    }
}
