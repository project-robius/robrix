
use makepad_widgets::*;

use crate::{app::AppState, home::navigation_tab_bar::{NavigationBarAction, get_own_profile}, profile::user_profile::UserProfile, settings::{PopulateMode, account_settings::AccountSettingsWidgetExt, app_settings::AppSettingsWidgetExt}};

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    // The main, top-level settings screen widget.
    mod.widgets.SettingsScreen = #(SettingsScreen::register_widget(vm)) {
        width: Fill, height: Fill,
        flow: Overlay

        View {
            padding: Inset{top: 5, left: 15, right: 15, bottom: 0},
            flow: Down

            // The settings header shows a title, with a close button to the right.
            settings_header := View {
                flow: Right,
                width: Fill, height: Fit
                margin: Inset{top: 5, left: 5, right: 5}
                spacing: 10,

                settings_header_title := TitleLabel {
                    padding: 0,
                    margin: Inset{ left: 1, top: 11 },
                    text: "All Settings"
                    draw_text +: {
                        text_style: theme.font_regular {font_size: 18},
                    }
                }

                // The "X" close button on the top right
                close_button := RobrixNeutralIconButton {
                    width: Fit,
                    height: Fit,
                    spacing: 0,
                    margin: 0,
                    padding: 15,
                    draw_icon.svg: (ICON_CLOSE)
                    icon_walk: Walk{width: 14, height: 14}
                }
            }

            // Make sure the dividing line is aligned with the close_button
            LineH { padding: 10, margin: Inset{top: 10, right: 2} }

            ScrollYView {
                width: Fill, height: Fill
                flow: Down

                // The account settings section.
                account_settings := AccountSettings {}

                LineH { width: 400, padding: 10, margin: Inset{top: 20, bottom: 5} }

                // The Robrix app settings section.
                app_settings := AppSettings {}

                LineH { width: 400, padding: 10, margin: Inset{top: 20, bottom: 5} }

                // The TSP wallet settings section.
                tsp_settings_screen := TspSettingsScreen {}

                LineH { width: 400, padding: 10, margin: Inset{top: 20, bottom: 5} }

                // Add other settings sections here as needed.
                // Don't forget to add a `show()` fn to those settings sections
                // and call them in `SettingsScreen::show()`.
            }
        }

        // We want all modals to appear in front of the settings screen.
        create_wallet_modal := Modal {
            content +: {
                create_wallet_modal_inner := CreateWalletModal {}
            }
        }

        create_did_modal := Modal {
            content +: {
                create_did_modal_inner := CreateDidModal {}
            }
        }
    }
}


/// The top-level widget showing all app and user settings/preferences.
#[derive(Script, ScriptHook, Widget)]
pub struct SettingsScreen {
    #[deref] view: View,
}

impl Widget for SettingsScreen {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);

        // The ScriptReapply walk preserves text fields (String /
        // ArcStringMut bail out on it), but it still resets animator-driven
        // controls and `script_apply_eval`-driven things (avatar image,
        // button colors) back to their DSL defaults. So we re-apply just
        // those here. Crucially, do NOT re-`set_text` any user-editable
        // input — that would wipe out a partially-typed display name or
        // custom thumbnail size.
        if let Event::ScriptReapply = event {
            if let Some(app_state) = scope.data.get::<AppState>() {
                self.populate_subwidgets(cx, PopulateMode::AfterReapply, None, app_state);
            }
        }

        // Close the pane if:
        // 1. The close button is clicked,
        // 2. The back navigational gesture/action occurs (e.g., Back on Android),
        // 3. The escape key is pressed if this pane has key focus,
        // 4. The back mouse button is clicked within this view.
        let area = self.view.area();
        let close_pane = {
            matches!(
                event,
                Event::Actions(actions) if self.button(cx, ids!(close_button)).clicked(actions)
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
            use crate::tsp::{
                create_did_modal::CreateDidModalAction,
                create_wallet_modal::CreateWalletModalAction,
            };

            for action in actions {
                // Handle the create wallet modal being opened or closed.
                match action.downcast_ref() {
                    Some(CreateWalletModalAction::Open) => {
                        use crate::tsp::create_wallet_modal::CreateWalletModalWidgetExt;
                        self.view.create_wallet_modal(cx, ids!(create_wallet_modal_inner)).show(cx);
                        self.view.modal(cx, ids!(create_wallet_modal)).open(cx);
                    }
                    Some(CreateWalletModalAction::Close) => {
                        self.view.modal(cx, ids!(create_wallet_modal)).close(cx);
                    }
                    None => { }
                }

                // Handle the create DID modal being opened or closed.
                match action.downcast_ref() {
                    Some(CreateDidModalAction::Open) => {
                        use crate::tsp::create_did_modal::CreateDidModalWidgetExt;
                        self.view.create_did_modal(cx, ids!(create_did_modal_inner)).show(cx);
                        self.view.modal(cx, ids!(create_did_modal)).open(cx);
                    }
                    Some(CreateDidModalAction::Close) => {
                        self.view.modal(cx, ids!(create_did_modal)).close(cx);
                    }
                    None => { }
                }
            }
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl SettingsScreen {
    /// Fetches the current user's profile and uses it to populate the settings screen.
    pub fn populate(&mut self, cx: &mut Cx, own_profile: Option<UserProfile>, app_state: &AppState) {
        let Some(profile) = own_profile.or_else(|| get_own_profile(cx)) else {
            error!("Failed to get own profile for settings screen.");
            return;
        };
        self.populate_subwidgets(cx, PopulateMode::Initial, Some(profile), app_state);
        self.view.button(cx, ids!(close_button)).reset_hover(cx);
        cx.set_key_focus(self.view.area());
        self.redraw(cx);
    }

    /// Single place that decides which sub-widgets get (re)synced and how.
    /// Both the initial-open path and the `Event::ScriptReapply` path
    /// route through here, so adding a new sub-widget that participates
    /// in either sync only requires editing this match.
    ///
    /// `AppSettings` is intentionally missing from the `AfterReapply` arm —
    /// it restores itself synchronously from its own `on_after_apply` hook
    /// (during the apply walk, before any draw fires), which is what
    /// avoids the flicker the late path used to produce. `AccountSettings`
    /// still needs the late path for its `script_apply_eval`-driven bits
    /// (button colors, avatar repaint) cuz those can't run from inside an
    /// `on_after_apply` (the VM is swapped out there).
    fn populate_subwidgets(
        &mut self,
        cx: &mut Cx,
        mode: PopulateMode,
        profile: Option<UserProfile>,
        app_state: &AppState,
    ) {
        match mode {
            PopulateMode::Initial => {
                self.view.account_settings(cx, ids!(account_settings)).populate(cx, profile);
                self.view.app_settings(cx, ids!(app_settings)).populate(cx, &app_state.app_prefs);
            }
            PopulateMode::AfterReapply => {
                self.view.account_settings(cx, ids!(account_settings)).restore_after_reapply(cx);
            }
        }
    }
}

impl SettingsScreenRef {
    /// See [`SettingsScreen::populate()`].
    pub fn populate(&self, cx: &mut Cx, own_profile: Option<UserProfile>, app_state: &AppState) {
        let Some(mut inner) = self.borrow_mut() else { return; };
        inner.populate(cx, own_profile, app_state);
    }
}
