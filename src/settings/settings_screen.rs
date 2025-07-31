
use makepad_widgets::*;

use crate::{home::spaces_dock::get_own_profile, profile::user_profile::UserProfile, settings::{account_settings::AccountSettingsWidgetExt, SettingsAction}};

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::helpers::*;
    use crate::shared::styles::*;
    use crate::shared::icon_button::*;
    use crate::settings::account_settings::AccountSettings;

    // The main, top-level settings screen widget.
    pub SettingsScreen = {{SettingsScreen}} {
        width: Fill,
        height: Fill,
        padding: {top: 5, left: 15, right: 15, bottom: 15},
        spacing: 10,
        flow: Down,

        show_bg: true,
        draw_bg: {
            color: (COLOR_PRIMARY)
        }

        // The settings header shows a title, with a close button to the right.
        settings_header = <View> {
            flow: Right,
            align: {x: 1.0, y: 0.5},
            width: Fill, height: Fit
            margin: {left: 5, right: 5}
            spacing: 10,

            settings_header_title = <TitleLabel> {
                margin: {top: 4} // line up with the close button
                text: "All Settings"
                draw_text: {
                    text_style: {font_size: 18},
                }
            }

            // The "X" close button on the top right
            close_button = <RobrixIconButton> {
                width: Fit,
                height: Fit,
                align: {x: 1.0, y: 0.0},
                spacing: 0,
                margin: {top: 4.5} // vertically align with the title
                padding: 15,

                draw_bg: {
                    color: (COLOR_SECONDARY)
                }
                draw_icon: {
                    svg_file: (ICON_CLOSE),
                    fn get_color(self) -> vec4 {
                        return #x0;
                    }
                }
                icon_walk: {width: 14, height: 14}
            }
        }

        <LineH> { padding: 10 }

        <ScrollXYView> {
            width: Fill, height: Fill
            flow: Down

            // The account settings section.
            account_settings = <AccountSettings> {}

            // Add other settings sections here as needed.
            // Don't forget to add a `show()` fn to those settings sections
            // and call them in `SettingsScreen::show()`.
        }
    }
}


/// The top-level widget showing all app and user settings/preferences.
#[derive(Live, LiveHook, Widget)]
pub struct SettingsScreen {
    #[deref] view: View,
}

impl Widget for SettingsScreen {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);

        // Close the pane if:
        // 1. The close button is clicked,
        // 2. The back navigational gesture/action occurs (e.g., Back on Android),
        // 3. The escape key is pressed if this pane has key focus,
        // 4. The back mouse button is clicked within this view.
        let area = self.view.area();
        let close_pane = {
            matches!(
                event,
                Event::Actions(actions) if self.button(id!(close_button)).clicked(actions)
            )
            || event.back_pressed()
            || match event.hits_with_capture_overload(cx, area, true) {
                Hit::KeyUp(key) => key.key_code == KeyCode::Escape,
                Hit::FingerDown(_fde) => {
                    cx.set_key_focus(area);
                    false
                }
                _ => false,
            }
        };
        if close_pane {
            cx.action(SettingsAction::CloseSettings);
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl SettingsScreen {
    /// Fetches the current user's profile and uses it to populate the settings screen.
    pub fn populate(&mut self, cx: &mut Cx, own_profile: Option<UserProfile>) {
        let Some(profile) = own_profile.or_else(|| get_own_profile(cx)) else {
            error!("BUG: failed to get own profile for settings screen.");
            return;
        };
        self.view.account_settings(id!(account_settings)).populate(cx, profile);
        self.view.button(id!(close_button)).reset_hover(cx);
        cx.set_key_focus(self.view.area());
        self.redraw(cx);
    }
}

impl SettingsScreenRef {
    /// See [`SettingsScreen::populate()`].
    pub fn populate(&self, cx: &mut Cx, own_profile: Option<UserProfile>) {
        let Some(mut inner) = self.borrow_mut() else { return; };
        inner.populate(cx, own_profile);
    }
}
