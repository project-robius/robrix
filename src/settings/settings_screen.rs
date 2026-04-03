
use makepad_widgets::*;

use crate::{app::{AppState, BotSettingsState}, home::navigation_tab_bar::{NavigationBarAction, get_own_profile}, i18n::{AppLanguage, I18nKey, language_dropdown_labels, tr}, persistence, profile::user_profile::UserProfile, settings::{account_settings::AccountSettingsWidgetExt, bot_settings::BotSettingsWidgetExt}, shared::{popup_list::{PopupKind, enqueue_popup_notification}, styles::{apply_neutral_button_style, apply_primary_button_style}}, sliding_sync::current_user_id};

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
                align: Align{x: 1.0, y: 0.5},
                width: Fill, height: Fit
                margin: Inset{left: 5, right: 5}
                spacing: 10,

                settings_header_title := TitleLabel {
                    margin: Inset{top: 4} // line up with the close button
                    text: "All Settings"
                    draw_text +: {
                        text_style: theme.font_regular {font_size: 18},
                    }
                }

                // The "X" close button on the top right
                close_button := RobrixNeutralIconButton {
                    width: Fit,
                    height: Fit,
                    align: Align{x: 1.0, y: 0.0},
                    spacing: 0,
                    margin: Inset{top: 4.5} // vertically align with the title
                    padding: 15,
                    draw_icon.svg: (ICON_CLOSE)
                    icon_walk: Walk{width: 14, height: 14}
                }
            }

            // Make sure the dividing line is aligned with the close_button
            LineH { padding: 10, margin: Inset{top: 10, right: 2} }

            settings_category_cards := View {
                width: Fill, height: Fit
                flow: Flow.Right{wrap: true}
                align: Align{y: 0.5}
                spacing: 10
                margin: Inset{left: 5, right: 5, bottom: 8}

                category_account_button := RobrixNeutralIconButton {
                    width: Fit, height: Fit,
                    padding: Inset{top: 9, bottom: 9, left: 14, right: 14}
                    spacing: 0,
                    icon_walk: Walk{width: 0, height: 0, margin: 0}
                    text: "Account"
                }

                category_preferences_button := RobrixNeutralIconButton {
                    width: Fit, height: Fit,
                    padding: Inset{top: 9, bottom: 9, left: 14, right: 14}
                    spacing: 0,
                    icon_walk: Walk{width: 0, height: 0, margin: 0}
                    text: "Preferences"
                }

                category_labs_button := RobrixNeutralIconButton {
                    width: Fit, height: Fit,
                    padding: Inset{top: 9, bottom: 9, left: 14, right: 14}
                    spacing: 0,
                    icon_walk: Walk{width: 0, height: 0, margin: 0}
                    text: "Labs"
                }
            }

            ScrollXYView {
                width: Fill, height: Fill
                flow: Down

                settings_sections := View {
                    width: Fill, height: Fit
                    flow: Down

                    // The account settings section.
                    account_settings_section := View {
                        width: Fill, height: Fit
                        flow: Down
                        account_settings := AccountSettings {}
                    }

                    preferences_settings_section := View {
                        visible: false
                        width: Fill, height: Fit
                        flow: Down
                        spacing: 8

                        preferences_language_title := TitleLabel {
                            text: "Language"
                        }

                        preferences_application_language_label := SubsectionLabel {
                            text: "Application language"
                        }

                        language_dropdown := DropDownFlat {
                            width: 165
                            height: 40
                            margin: Inset{left: 5, top: 2, bottom: 2}
                            labels: ["English", "Simplified Chinese"]
                        }

                        preferences_language_hint_label := Label {
                            width: Fill
                            height: Fit
                            margin: Inset{left: 5, right: 8, top: 3, bottom: 4}
                            draw_text +: {
                                color: (MESSAGE_TEXT_COLOR)
                                text_style: REGULAR_TEXT { font_size: 10.5 }
                            }
                            text: "The app will reload after selecting another language"
                        }
                    }

                    labs_settings_section := View {
                        visible: false
                        width: Fill, height: Fit
                        flow: Down

                        bot_settings := BotSettings {}

                        LineH { width: 400, padding: 10, margin: Inset{top: 20, bottom: 5} }

                        // The TSP wallet settings section.
                        tsp_settings_screen := TspSettingsScreen {}
                    }
                }
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
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum SettingsCategory {
    #[default]
    Account,
    Preferences,
    Labs,
}

/// The top-level widget showing all app and user settings/preferences.
#[derive(Script, ScriptHook, Widget)]
pub struct SettingsScreen {
    #[deref] view: View,

    #[rust] selected_category: SettingsCategory,
    #[rust] app_language: AppLanguage,
}

impl Widget for SettingsScreen {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        let app_language = scope.data.get::<AppState>()
            .map(|app_state| app_state.app_language)
            .unwrap_or_default();
        if self.app_language != app_language {
            self.set_app_language(cx, app_language);
        }
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

        if let Event::Actions(actions) = event {
            if self.view.drop_down(cx, ids!(language_dropdown)).changed(actions).is_some() {
                let selected_language = AppLanguage::from_dropdown_index(
                    self.view.drop_down(cx, ids!(language_dropdown)).selected_item(),
                );
                if self.app_language != selected_language {
                    self.set_app_language(cx, selected_language);
                    if let Some(app_state) = scope.data.get_mut::<AppState>() {
                        if app_state.app_language != selected_language {
                            app_state.app_language = selected_language;
                            persist_app_state(app_state);
                            enqueue_popup_notification(
                                tr(selected_language, I18nKey::LanguageReloadHint),
                                PopupKind::Info,
                                Some(4.0),
                            );
                        }
                    }
                }
            }

            if self.view.button(cx, ids!(category_account_button)).clicked(actions) {
                self.set_selected_category(cx, SettingsCategory::Account);
            }
            else if self.view.button(cx, ids!(category_preferences_button)).clicked(actions) {
                self.set_selected_category(cx, SettingsCategory::Preferences);
            }
            else if self.view.button(cx, ids!(category_labs_button)).clicked(actions) {
                self.set_selected_category(cx, SettingsCategory::Labs);
            }

            #[cfg(feature = "tsp")]
            {
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

impl SettingsScreen {
    fn set_app_language(&mut self, cx: &mut Cx, app_language: AppLanguage) {
        self.app_language = app_language;
        self.sync_app_language(cx);
    }

    fn sync_app_language(&mut self, cx: &mut Cx) {
        self.view
            .label(cx, ids!(settings_header_title))
            .set_text(cx, tr(self.app_language, I18nKey::AllSettingsTitle));
        self.view
            .button(cx, ids!(category_account_button))
            .set_text(cx, tr(self.app_language, I18nKey::SettingsCategoryAccount));
        self.view
            .button(cx, ids!(category_preferences_button))
            .set_text(cx, tr(self.app_language, I18nKey::SettingsCategoryPreferences));
        self.view
            .button(cx, ids!(category_labs_button))
            .set_text(cx, tr(self.app_language, I18nKey::SettingsCategoryLabs));
        self.view
            .label(cx, ids!(preferences_language_title))
            .set_text(cx, tr(self.app_language, I18nKey::LanguageTitle));
        self.view
            .label(cx, ids!(preferences_application_language_label))
            .set_text(cx, tr(self.app_language, I18nKey::ApplicationLanguageLabel));
        self.view
            .label(cx, ids!(preferences_language_hint_label))
            .set_text(cx, tr(self.app_language, I18nKey::LanguageReloadHint));
        let language_dropdown = self.view.drop_down(cx, ids!(language_dropdown));
        language_dropdown.set_labels(cx, language_dropdown_labels(self.app_language));
        language_dropdown.set_selected_item(cx, self.app_language.dropdown_index());
        self.view
            .account_settings(cx, ids!(account_settings))
            .set_app_language(cx, self.app_language);
        self.view
            .bot_settings(cx, ids!(bot_settings))
            .set_app_language(cx, self.app_language);
        self.view.redraw(cx);
    }

    fn set_selected_category(&mut self, cx: &mut Cx, category: SettingsCategory) {
        self.selected_category = category;
        self.sync_selected_category(cx);
    }

    fn sync_selected_category(&mut self, cx: &mut Cx) {
        let show_account = self.selected_category == SettingsCategory::Account;
        let show_preferences = self.selected_category == SettingsCategory::Preferences;
        let show_labs = self.selected_category == SettingsCategory::Labs;

        self.view.view(cx, ids!(account_settings_section)).set_visible(cx, show_account);
        self.view.view(cx, ids!(preferences_settings_section)).set_visible(cx, show_preferences);
        self.view.view(cx, ids!(labs_settings_section)).set_visible(cx, show_labs);

        let mut category_account_button = self.view.button(cx, ids!(category_account_button));
        let mut category_preferences_button = self.view.button(cx, ids!(category_preferences_button));
        let mut category_labs_button = self.view.button(cx, ids!(category_labs_button));

        if show_account {
            apply_primary_button_style(cx, &mut category_account_button);
        } else {
            apply_neutral_button_style(cx, &mut category_account_button);
        }
        if show_preferences {
            apply_primary_button_style(cx, &mut category_preferences_button);
        } else {
            apply_neutral_button_style(cx, &mut category_preferences_button);
        }
        if show_labs {
            apply_primary_button_style(cx, &mut category_labs_button);
        } else {
            apply_neutral_button_style(cx, &mut category_labs_button);
        }

        category_account_button.reset_hover(cx);
        category_preferences_button.reset_hover(cx);
        category_labs_button.reset_hover(cx);
        self.view.redraw(cx);
    }

    /// Fetches the current user's profile and uses it to populate the settings screen.
    pub fn populate(&mut self, cx: &mut Cx, own_profile: Option<UserProfile>, bot_settings: &BotSettingsState, app_language: AppLanguage) {
        let Some(profile) = own_profile.or_else(|| get_own_profile(cx)) else {
            error!("Failed to get own profile for settings screen.");
            return;
        };
        self.view.account_settings(cx, ids!(account_settings)).populate(cx, profile);
        self.view.bot_settings(cx, ids!(bot_settings)).populate(cx, bot_settings);
        self.set_app_language(cx, app_language);
        self.set_selected_category(cx, SettingsCategory::Account);
        self.view.button(cx, ids!(close_button)).reset_hover(cx);
        cx.set_key_focus(self.view.area());
        self.redraw(cx);
    }
}

impl SettingsScreenRef {
    /// See [`SettingsScreen::populate()`].
    pub fn populate(&self, cx: &mut Cx, own_profile: Option<UserProfile>, bot_settings: &BotSettingsState, app_language: AppLanguage) {
        let Some(mut inner) = self.borrow_mut() else { return; };
        inner.populate(cx, own_profile, bot_settings, app_language);
    }
}

fn persist_app_state(app_state: &AppState) {
    if let Some(user_id) = current_user_id() {
        if let Err(e) = persistence::save_app_state(app_state.clone(), user_id) {
            error!("Failed to persist app state after updating language setting. Error: {e}");
        }
    }
}
