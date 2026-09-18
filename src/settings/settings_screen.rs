
use makepad_widgets::*;

use crate::{app::AppState, home::navigation_tab_bar::{NavigationBarAction, get_own_profile}, profile::user_profile::UserProfile, settings::{PopulateMode, account_settings::AccountSettingsWidgetExt, app_settings::AppSettingsWidgetExt, privacy_settings::PrivacySettingsWidgetExt}};

/// Selected appearance for a settings category tab.
fn apply_settings_tab_selected(cx: &mut Cx, button: &mut ButtonRef) {
    script_apply_eval!(cx, button, {
        draw_bg +: {
            color: #x0D7988,
            color_hover: #x0A6675,
            color_down: #x085460,
            border_size: 0.0,
            border_color: #0000,
            border_color_hover: #0000,
            border_color_down: #0000,
        }
        draw_text +: {
            color: #xFFFFFF,
            color_hover: #xFFFFFF,
            color_down: #xFFFFFF,
        }
    });
}

/// Ghost "unselected" style for a settings category tab (transparent fill,
/// secondary text, subtle hover wash).
fn apply_settings_tab_unselected(cx: &mut Cx, button: &mut ButtonRef) {
    script_apply_eval!(cx, button, {
        draw_bg +: {
            color: #0000,
            color_hover: #xEFF4FB,
            color_down: #xE7ECF3,
            border_size: 0.0,
            border_color: #0000,
            border_color_hover: #0000,
            border_color_down: #0000,
        }
        draw_text +: {
            color: #x5A6B86,
            color_hover: #x5A6B86,
            color_down: #x5A6B86,
        }
    });
}

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    // Selected/unselected colors are applied when the category changes.
    mod.widgets.SettingsCategoryTab = RobrixNeutralIconButton {
        width: Fit, height: Fit,
        padding: Inset{top: 8, bottom: 8, left: 12, right: 12}
        spacing: 0,
        icon_walk: Walk{width: 0, height: 0, margin: 0}
        draw_bg +: { border_radius: 4.0 }
        draw_text +: { text_style: theme.font_bold {font_size: 11, line_spacing: 1.35} }
        text: ""
    }

    // Each category retains its settings widgets when another tab is selected.
    mod.widgets.SettingsScreen = #(SettingsScreen::register_widget(vm)) {
        width: Fill, height: Fill,
        flow: Overlay

        SolidView {
            show_bg: true
            draw_bg.color: #xF7F9FC
            padding: Inset{top: 8, left: 16, right: 16 },
            flow: Down

            // Header: "Settings" title + close button.
            settings_header := View {
                flow: Right,
                width: Fill, height: Fit
                margin: Inset{top: 8, left: 0, right: 4}
                spacing: 8,
                align: Align{y: 0.5}

                settings_header_title := TitleLabel {
                    width: Fill
                    padding: 0,
                    margin: 0,
                    text: "Settings"
                    draw_text +: {
                        text_style: theme.font_bold {font_size: 17, line_spacing: 1.25},
                        color: #x16233B
                    }
                }

                // The "X" close button on the top right: bare icon, no fill.
                close_button := RobrixNeutralIconButton {
                    width: Fit,
                    height: Fit,
                    spacing: 0,
                    margin: 0,
                    padding: 12,
                    draw_bg +: {
                        color: #0000
                        color_hover: #xEFF4FB
                        color_down: #xE7ECF3
                        border_size: 0.0
                        border_color: #0000
                        border_color_hover: #0000
                        border_color_down: #0000
                        border_radius: 4.0
                    }
                    draw_icon +: { svg: (ICON_CLOSE), color: #x5A6B86 }
                    icon_walk: Walk{width: 14, height: 14}
                }
            }

            LineH { padding: 0, margin: Inset{top: 8, bottom: 8} }

            // Wrap the categories onto multiple rows on narrow windows.
            settings_category_tabs := View {
                width: Fill, height: Fit
                flow: Flow.Right{wrap: true}
                align: Align{y: 0.5}
                spacing: 8
                margin: Inset{left: 4, right: 4, bottom: 8}

                category_account_button := mod.widgets.SettingsCategoryTab { text: "Account" }
                category_preferences_button := mod.widgets.SettingsCategoryTab { text: "Preferences" }
                category_privacy_button := mod.widgets.SettingsCategoryTab { text: "Privacy" }
                category_about_button := mod.widgets.SettingsCategoryTab { text: "About" }
            }

            settings_sections := View {
                width: Fill, height: Fill
                flow: Overlay

                account_settings_page := ScrollYView {
                    width: Fill, height: Fill
                    flow: Down
                    account_settings := AccountSettings {}
                    // The TSP wallet settings section (a placeholder without the `tsp` feature).
                    tsp_settings_screen := TspSettingsScreen {}
                    View { width: Fill, height: 20 }
                }

                preferences_settings_page := ScrollYView {
                    visible: false
                    width: Fill, height: Fill
                    flow: Down
                    app_settings := AppSettings {}
                    View { width: Fill, height: 20 }
                }

                privacy_settings_page := ScrollYView {
                    visible: false
                    width: Fill, height: Fill
                    flow: Down
                    privacy_settings := PrivacySettings {}
                    View { width: Fill, height: 20 }
                }

                about_settings_page := ScrollYView {
                    visible: false
                    width: Fill, height: Fill
                    flow: Down
                    about_settings := AboutSettings {}
                    View { width: Fill, height: 20 }
                }
            }
        }

        // We want all modals to appear in front of the settings screen.
        create_wallet_modal := Modal {
            content := CreateWalletModal {}
        }

        create_did_modal := Modal {
            content := CreateDidModal {}
        }
    }
}

/// The settings categories, one per tab / page.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum SettingsCategory {
    #[default]
    Account,
    Preferences,
    Privacy,
    About,
}

/// The top-level widget showing all app and user settings/preferences.
#[derive(Script, ScriptHook, Widget)]
pub struct SettingsScreen {
    #[deref] view: View,
    /// Held after this widget is populated.
    ///
    /// Note that it's never cleared because Makepad itself prevents any widget from
    /// consuming a cancel gesture while it's hidden (not visible).
    #[rust] cancel_scope: Option<CancelScope>,
    #[rust] selected_category: SettingsCategory,
}

impl Widget for SettingsScreen {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);

        // ScriptReapply preserves text fields (String / ArcStringMut bail out),
        // but still resets animator-driven controls and `script_apply_eval`-driven
        // bits (avatar, button colors, the category tabs). Re-apply just those.
        // Never re-`set_text` user-editable inputs here, that would wipe in-progress edits.
        if let Event::ScriptReapply = event {
            if let Some(app_state) = scope.data.get::<AppState>() {
                self.populate_subwidgets(cx, PopulateMode::AfterReapply, None, app_state);
            }
            self.sync_selected_category(cx);
        }

        if let Event::Actions(actions) = event {
            if self.view.button(cx, ids!(category_account_button)).clicked(actions) {
                self.set_selected_category(cx, SettingsCategory::Account);
            } else if self.view.button(cx, ids!(category_preferences_button)).clicked(actions) {
                self.set_selected_category(cx, SettingsCategory::Preferences);
            } else if self.view.button(cx, ids!(category_privacy_button)).clicked(actions) {
                self.set_selected_category(cx, SettingsCategory::Privacy);
            } else if self.view.button(cx, ids!(category_about_button)).clicked(actions) {
                self.set_selected_category(cx, SettingsCategory::About);
            }
        }

        // Close the pane if:
        // 1. The close button is clicked,
        // 2. The back navigational gesture/action occurs (e.g., Back on Android),
        // 3. The escape key is pressed while this pane owns the gesture,
        // 4. The back mouse button is clicked while this pane owns the gesture.
        let area = self.view.area();
        // Ownership decides for every gesture, not key focus. Checked before
        // `back_pressed()` because that call consumes: only the owner may make it.
        let close_pane = {
            matches!(
                event,
                Event::Actions(actions) if self.button(cx, ids!(close_button)).clicked(actions)
            )
            || (self.cancel_scope.as_ref().is_some_and(|s| cx.owns_cancel(s))
                && (event.back_pressed()
                    || matches!(event, Event::KeyUp(key) if key.key_code == KeyCode::Escape)
                    // we must match on the raw event instead of hits, because the back button
                    // needs to work regardless of what widget captured its initial MouseDown event.
                    || matches!(event, Event::MouseUp(e) if e.button.is_back())
                )
            )
            || match event.hits(cx, area) {
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
                        self.view.create_wallet_modal(cx, ids!(create_wallet_modal.content)).show(cx);
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
                        self.view.create_did_modal(cx, ids!(create_did_modal.content)).show(cx);
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
        if self.cancel_scope.is_none() {
            self.cancel_scope = Some(self.begin_cancel_scope(cx));
        }
        let Some(profile) = own_profile.or_else(|| get_own_profile(cx)) else {
            error!("Failed to get own profile for settings screen.");
            return;
        };
        self.populate_subwidgets(cx, PopulateMode::Initial, Some(profile), app_state);
        self.view.button(cx, ids!(close_button)).reset_hover(cx);
        self.sync_selected_category(cx);
        cx.set_key_focus(self.view.area());
        self.redraw(cx);
    }

    /// Single place deciding which sub-widgets get (re)synced and how.
    /// Both the initial-open and `Event::ScriptReapply` paths route here.
    ///
    /// `AppSettings` is missing from `AfterReapply` on purpose, since it
    /// restores itself inline from `on_after_apply` to avoid the flicker
    /// the late path used to produce. `AccountSettings` still needs the
    /// late path for its `script_apply_eval`-driven bits (button colors,
    /// avatar), cuz those can't run from inside `on_after_apply`.
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
                self.view.privacy_settings(cx, ids!(privacy_settings)).populate(cx);
            }
            PopulateMode::AfterReapply => {
                self.view.account_settings(cx, ids!(account_settings)).restore_after_reapply(cx);
            }
        }
    }

    fn set_selected_category(&mut self, cx: &mut Cx, category: SettingsCategory) {
        self.selected_category = category;
        self.sync_selected_category(cx);
    }

    /// Shows the page for the selected category and restyles the tab row to match.
    fn sync_selected_category(&mut self, cx: &mut Cx) {
        for (category, page) in [
            (SettingsCategory::Account, ids!(account_settings_page)),
            (SettingsCategory::Preferences, ids!(preferences_settings_page)),
            (SettingsCategory::Privacy, ids!(privacy_settings_page)),
            (SettingsCategory::About, ids!(about_settings_page)),
        ] {
            self.view.view(cx, page).set_visible(cx, category == self.selected_category);
        }

        let tabs = [
            (SettingsCategory::Account, ids!(category_account_button)),
            (SettingsCategory::Preferences, ids!(category_preferences_button)),
            (SettingsCategory::Privacy, ids!(category_privacy_button)),
            (SettingsCategory::About, ids!(category_about_button)),
        ];
        for (category, id) in tabs {
            let mut button = self.view.button(cx, id);
            if category == self.selected_category {
                apply_settings_tab_selected(cx, &mut button);
            } else {
                apply_settings_tab_unselected(cx, &mut button);
            }
        }
        self.redraw(cx);
    }
}

impl SettingsScreenRef {
    /// See [`SettingsScreen::populate()`].
    pub fn populate(&self, cx: &mut Cx, own_profile: Option<UserProfile>, app_state: &AppState) {
        let Some(mut inner) = self.borrow_mut() else { return; };
        inner.populate(cx, own_profile, app_state);
    }
}

