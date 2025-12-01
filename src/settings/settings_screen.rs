
use makepad_widgets::*;

use crate::{home::navigation_tab_bar::{NavigationBarAction, get_own_profile}, profile::user_profile::UserProfile, settings::account_settings::AccountSettingsWidgetExt};

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::helpers::*;
    use crate::shared::styles::*;
    use crate::shared::icon_button::*;
    use crate::shared::confirmation_modal::*;
    use crate::settings::account_settings::AccountSettings;
    use link::tsp_link::TspSettingsScreen;
    use link::tsp_link::CreateWalletModal;
    use link::tsp_link::CreateDidModal;

    // The main, top-level settings screen widget.
    pub SettingsScreen = {{SettingsScreen}} {
        width: Fill, height: Fill,
        flow: Overlay

        <View> {
            padding: {top: 5, left: 15, right: 15, bottom: 0},
            flow: Down

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

            // Make sure the dividing line is aligned with the close_button
            <LineH> { padding: 10, margin: {top: 10, right: 2} }

            <ScrollXYView> {
                width: Fill, height: Fill
                flow: Down

                // The account settings section.
                account_settings = <AccountSettings> {}

                <LineH> { width: 400, padding: 10, margin: {top: 20, bottom: 5} }

                // The TSP wallet settings section.
                tsp_settings_screen = <TspSettingsScreen> {}

                <LineH> { width: 400, padding: 10, margin: {top: 20, bottom: 5} }

                // Add other settings sections here as needed.
                // Don't forget to add a `show()` fn to those settings sections
                // and call them in `SettingsScreen::show()`.
            }
        }

        // We want all modals to appear in front of the settings screen.
        create_wallet_modal = <Modal> {
            content: {
                create_wallet_modal_inner = <CreateWalletModal> {}
            }
        }

        create_did_modal = <Modal> {
            content: {
                create_did_modal_inner = <CreateDidModal> {}
            }
        }

        remove_delete_wallet_modal = <Modal> {
            content: {
                remove_delete_wallet_modal_inner = <NegativeConfirmationModal> { }
            }
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
                Event::Actions(actions) if self.button(ids!(close_button)).clicked(actions)
            )
            || event.back_pressed()
            || match event.hits(cx, area) {
                Hit::KeyUp(key) => key.key_code == KeyCode::Escape,
                Hit::FingerDown(_fde) => {
                    cx.set_key_focus(area);
                    false
                }
                _ => false,
            }
        };
        if close_pane {
            cx.action(NavigationBarAction::CloseSettings);
        }

        #[cfg(feature = "tsp")]
        if let Event::Actions(actions) = event {
            use crate::shared::confirmation_modal::ConfirmationModalWidgetExt;
            use crate::tsp::{
                create_did_modal::CreateDidModalAction,
                create_wallet_modal::CreateWalletModalAction,
                wallet_entry::TspWalletEntryAction,
            };

            for action in actions {
                // Handle the create wallet modal being opened or closed.
                match action.downcast_ref() {
                    Some(CreateWalletModalAction::Open) => {
                        use crate::tsp::create_wallet_modal::CreateWalletModalWidgetExt;
                        self.view.create_wallet_modal(ids!(create_wallet_modal_inner)).show(cx);
                        self.view.modal(ids!(create_wallet_modal)).open(cx);
                    }
                    Some(CreateWalletModalAction::Close) => {
                        self.view.modal(ids!(create_wallet_modal)).close(cx);
                    }
                    None => { }
                }

                // Handle the create DID modal being opened or closed.
                match action.downcast_ref() {
                    Some(CreateDidModalAction::Open) => {
                        use crate::tsp::create_did_modal::CreateDidModalWidgetExt;
                        self.view.create_did_modal(ids!(create_did_modal_inner)).show(cx);
                        self.view.modal(ids!(create_did_modal)).open(cx);
                    }
                    Some(CreateDidModalAction::Close) => {
                        self.view.modal(ids!(create_did_modal)).close(cx);
                    }
                    None => { }
                }

                // Handle a request to show a TSP wallet confirmation modal.
                if let Some(TspWalletEntryAction::ShowConfirmationModal(content_opt)) = action.downcast_ref() {
                    if let Some(content) = content_opt.borrow_mut().take() {
                        self.view.confirmation_modal(ids!(remove_delete_wallet_modal_inner)).show(cx, content);
                        self.view.modal(ids!(remove_delete_wallet_modal)).open(cx);
                    }
                }
            }

            if let Some(_accepted) = self.view.confirmation_modal(ids!(remove_delete_wallet_modal_inner)).closed(actions) {
                self.view.modal(ids!(remove_delete_wallet_modal)).close(cx);
            }
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
            error!("Failed to get own profile for settings screen.");
            return;
        };
        self.view.account_settings(ids!(account_settings)).populate(cx, profile);
        self.view.button(ids!(close_button)).reset_hover(cx);
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
