//! The "App Settings" section of the settings screen: view-mode override,
//! message-send shortcut, and image-thumbnail max height.

use makepad_widgets::*;

use crate::{
    app::AppState,
    settings::app_settings_data::{AppPreferences, ThumbnailMaxHeight, ViewModeOverride},
    shared::popup_list::{enqueue_popup_notification, PopupKind},
};

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*


    // A DropDown styled to match other Robrix settings controls.
    mod.widgets.RobrixSettingsDropDown = DropDownFlat {
        width: Fit,
        height: (mod.widgets.SETTINGS_BUTTON_HEIGHT),
        padding: Inset{top: 8, bottom: 8, left: 12, right: 30}
        margin: Inset{left: 5, top: 5, bottom: 5}

        draw_text +: {
            color: (COLOR_TEXT),
            text_style: theme.font_regular { font_size: 12 },
        }

        draw_bg +: {
            color: (COLOR_PRIMARY),
            color_hover: (COLOR_SECONDARY),
            color_down: (COLOR_SECONDARY),
            color_focus: (COLOR_PRIMARY),
            border_color: (COLOR_ACTIVE_PRIMARY_DARKER),
            border_color_hover: (COLOR_ACTIVE_PRIMARY_DARKER),
            border_color_focus: (COLOR_ACTIVE_PRIMARY),
            border_size: 1.0,
            border_radius: 4.0,
        }
    }


    // The view containing Robrix app-wide settings.
    mod.widgets.AppSettings = #(AppSettings::register_widget(vm)) {
        width: Fill, height: Fit
        flow: Down

        TitleLabel {
            text: "App Settings"
        }

        SubsectionLabel {
            text: "View Mode:"
        }

        Label {
            width: Fill, height: Fit
            flow: Flow.Right{wrap: true}
            margin: Inset{left: 5, top: 2, bottom: 4, right: 5}
            draw_text +: {
                color: (MESSAGE_TEXT_COLOR),
                text_style: MESSAGE_TEXT_STYLE { font_size: 11 },
            }
            text: "Override the layout used by the main screen. 'Automatic' adapts based on window width."
        }

        view_mode_dropdown := mod.widgets.RobrixSettingsDropDown {
            labels: ["Automatic (width-based)", "Force Wide (desktop)", "Force Narrow (mobile)"]
            selected_item: 0
        }

        SubsectionLabel {
            text: "Send Message Shortcut:"
        }

        View {
            width: Fill, height: Fit
            flow: Right,
            align: Align{y: 0.5}
            margin: Inset{left: 5, top: 5}

            send_on_cmd_enter_toggle := ToggleFlat {
                text: "Send with Cmd/Ctrl + Enter",
                active: false,
                draw_text +: {
                    color: (COLOR_TEXT),
                    text_style: theme.font_regular { font_size: 12 },
                }
            }
        }

        send_shortcut_description := Label {
            width: Fill, height: Fit
            flow: Flow.Right{wrap: true}
            margin: Inset{left: 10, top: 4, bottom: 4, right: 5}
            draw_text +: {
                color: (MESSAGE_TEXT_COLOR),
                text_style: MESSAGE_TEXT_STYLE { font_size: 11 },
            }
            text: "Enter to send, Shift + Enter for a new line"
        }

        SubsectionLabel {
            text: "Maximum Image Thumbnail Height:"
        }

        Label {
            width: Fill, height: Fit
            flow: Flow.Right{wrap: true}
            margin: Inset{left: 5, top: 2, bottom: 6, right: 5}
            draw_text +: {
                color: (MESSAGE_TEXT_COLOR),
                text_style: MESSAGE_TEXT_STYLE { font_size: 11 },
            }
            text: "Limits how tall image thumbnails appear in room timelines."
        }

        View {
            width: Fill, height: Fit
            flow: Down,
            margin: Inset{left: 5},
            spacing: 2,

            thumb_small_radio := RadioButton {
                text: "Small (200 pixels, default)"
            }

            thumb_medium_radio := RadioButton {
                text: "Medium (400 pixels)"
            }

            thumb_unlimited_radio := RadioButton {
                text: "Unlimited (not recommended)"
            }

            View {
                width: Fill, height: Fit
                flow: Right,
                align: Align{y: 0.5}
                spacing: 4,

                thumb_custom_radio := RadioButton {
                    text: "Custom:"
                }

                thumb_custom_input := RobrixTextInput {
                    width: 50, height: Fit
                    margin: Inset{top: 1, bottom: 1}
                    padding: Inset{left: 8, right: 8, top: 5, bottom: 5}
                    empty_text: "500"
                }

                Label {
                    width: Fit, height: Fit
                    draw_text +: {
                        color: (MESSAGE_TEXT_COLOR),
                        text_style: MESSAGE_TEXT_STYLE { font_size: 10 },
                    }
                    text: "pixels"
                }
            }
        }
    }
}


/// The "App Settings" widget: controls app-wide user preferences.
///
/// Field-level state lives in [`AppState::app_prefs`]; this widget reads and
/// writes that state in response to user interactions and emits
/// [`AppSettingsAction`]s so other widgets can apply changes live.
#[derive(Script, ScriptHook, Widget)]
pub struct AppSettings {
    #[deref] view: View,
}

impl Widget for AppSettings {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if let Event::Actions(actions) = event {
            self.handle_actions(cx, actions, scope);
        }
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl AppSettings {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions, scope: &mut Scope) {
        let app_state = scope.data.get_mut::<AppState>().unwrap();

        // --- View mode dropdown ---
        let view_mode_dropdown = self.view.drop_down(cx, ids!(view_mode_dropdown));
        if let Some(index) = view_mode_dropdown.changed(actions) {
            let new_mode = ViewModeOverride::from_index(index);
            if new_mode != app_state.app_prefs.view_mode {
                app_state.app_prefs.view_mode = new_mode;
                app_state.app_prefs.on_view_mode_changed(cx);
                enqueue_popup_notification(
                    format!("View mode set to: {}", view_mode_label(new_mode)),
                    PopupKind::Success,
                    Some(3.0),
                );
            }
        }

        // --- Send-on-enter toggle ---
        let send_toggle = self.view.check_box(cx, ids!(send_on_cmd_enter_toggle));
        if let Some(cmd_enter_active) = send_toggle.changed(actions) {
            // Toggle label reads "Send with Cmd/Ctrl + Enter":
            // checked -> Cmd/Ctrl+Enter sends, so `send_on_enter` is false.
            let new_send_on_enter = !cmd_enter_active;
            if new_send_on_enter != app_state.app_prefs.send_on_enter {
                app_state.app_prefs.send_on_enter = new_send_on_enter;
                Self::update_send_shortcut_description(cx, &self.view, new_send_on_enter);
                app_state.app_prefs.on_send_on_enter_changed(cx);
                enqueue_popup_notification(
                    format!(
                        "Send shortcut set to: {}",
                        send_shortcut_label(new_send_on_enter),
                    ),
                    PopupKind::Success,
                    Some(3.0),
                );
            }
        }

        // --- Thumbnail radio buttons ---
        let radios = self.view.radio_button_set(cx, ids_array!(
            thumb_small_radio,
            thumb_medium_radio,
            thumb_unlimited_radio,
            thumb_custom_radio,
        ));
        let custom_input = self.view.text_input(cx, ids!(thumb_custom_input));
        if let Some(selected) = radios.selected(cx, actions) {
            let existing_custom = match app_state.app_prefs.thumbnail_max_height {
                ThumbnailMaxHeight::Custom(v) => Some(v),
                _ => parse_custom_thumb_height(&custom_input.text()),
            };
            let new_thumb = match selected {
                0 => ThumbnailMaxHeight::Small,
                1 => ThumbnailMaxHeight::Medium,
                2 => ThumbnailMaxHeight::Unlimited,
                3 => ThumbnailMaxHeight::Custom(existing_custom.unwrap_or(DEFAULT_CUSTOM_THUMB_HEIGHT)),
                _ => ThumbnailMaxHeight::default(),
            };
            let custom_now = matches!(new_thumb, ThumbnailMaxHeight::Custom(_));
            Self::set_thumb_custom_input_state(cx, &self.view, custom_now);
            if new_thumb != app_state.app_prefs.thumbnail_max_height {
                app_state.app_prefs.thumbnail_max_height = new_thumb;
                app_state.app_prefs.on_thumbnail_max_height_changed(cx);
                enqueue_popup_notification(
                    format!(
                        "Max image thumbnail height set to: {}",
                        thumbnail_max_height_label(new_thumb),
                    ),
                    PopupKind::Success,
                    Some(3.0),
                );
            }
            // If Custom is now selected, reflect the current value in the input.
            if let ThumbnailMaxHeight::Custom(v) = new_thumb {
                custom_input.set_text(cx, &v.to_string());
            }
        }

        // --- Custom thumbnail value input ---
        //
        // Only commit the value when the user presses Enter or blurs the
        // input — not on every keystroke — so mid-typing values like "4"
        // (before "400") don't briefly become the active setting.
        if custom_input.returned(actions).is_some() || custom_input.key_focus_lost(actions) {
            // Only act while Custom is selected. Otherwise typing shouldn't
            // override the Small/Medium/Unlimited choice.
            let custom_selected = matches!(
                app_state.app_prefs.thumbnail_max_height,
                ThumbnailMaxHeight::Custom(_)
            );
            if custom_selected {
                let text = custom_input.text();
                match parse_custom_thumb_height(&text) {
                    Some(v) => {
                        let new_thumb = ThumbnailMaxHeight::Custom(v);
                        if new_thumb != app_state.app_prefs.thumbnail_max_height {
                            app_state.app_prefs.thumbnail_max_height = new_thumb;
                            app_state.app_prefs.on_thumbnail_max_height_changed(cx);
                            enqueue_popup_notification(
                                format!(
                                    "Max image thumbnail height set to: {}",
                                    thumbnail_max_height_label(new_thumb),
                                ),
                                PopupKind::Success,
                                Some(3.0),
                            );
                        }
                    }
                    None if !text.trim().is_empty() => {
                        enqueue_popup_notification(
                            "Custom thumbnail height must be a positive whole number.",
                            PopupKind::Error,
                            Some(4.0),
                        );
                    }
                    None => { /* empty: leave the preference unchanged */ }
                }
            }
        }
    }

    /// Populates the widget from the given preferences.
    ///
    /// This should be called whenever the settings screen is shown.
    pub fn populate(&mut self, cx: &mut Cx, prefs: &AppPreferences) {
        // View mode dropdown.
        self.view
            .drop_down(cx, ids!(view_mode_dropdown))
            .set_selected_item(cx, prefs.view_mode.to_index());

        // Send-on-enter toggle (checked means "Cmd/Ctrl+Enter to send").
        self.view
            .check_box(cx, ids!(send_on_cmd_enter_toggle))
            .set_active(cx, !prefs.send_on_enter);
        Self::update_send_shortcut_description(cx, &self.view, prefs.send_on_enter);

        // Thumbnail radios.
        let (small, medium, unlimited, custom, custom_text) = match prefs.thumbnail_max_height {
            ThumbnailMaxHeight::Small => (true, false, false, false, String::new()),
            ThumbnailMaxHeight::Medium => (false, true, false, false, String::new()),
            ThumbnailMaxHeight::Unlimited => (false, false, true, false, String::new()),
            ThumbnailMaxHeight::Custom(v) => (false, false, false, true, v.to_string()),
        };
        self.view.radio_button(cx, ids!(thumb_small_radio)).set_active(cx, small);
        self.view.radio_button(cx, ids!(thumb_medium_radio)).set_active(cx, medium);
        self.view.radio_button(cx, ids!(thumb_unlimited_radio)).set_active(cx, unlimited);
        self.view.radio_button(cx, ids!(thumb_custom_radio)).set_active(cx, custom);
        self.view.text_input(cx, ids!(thumb_custom_input)).set_text(cx, &custom_text);
        Self::set_thumb_custom_input_state(cx, &self.view, custom);
    }

    fn update_send_shortcut_description(cx: &mut Cx, view: &View, send_on_enter: bool) {
        let text = if send_on_enter {
            "Enter to send, Shift + Enter for a new line"
        } else {
            "Ctrl/Cmd + Enter to send, Enter for a new line"
        };
        view.label(cx, ids!(send_shortcut_description)).set_text(cx, text);
    }

    fn set_thumb_custom_input_state(cx: &mut Cx, view: &View, enabled: bool) {
        let custom_input = view.text_input(cx, ids!(thumb_custom_input));
        custom_input.set_disabled(cx, !enabled);
        custom_input.set_is_read_only(cx, !enabled);
    }
}

impl AppSettingsRef {
    /// See [`AppSettings::populate`].
    pub fn populate(&self, cx: &mut Cx, prefs: &AppPreferences) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.populate(cx, prefs);
    }
}

const DEFAULT_CUSTOM_THUMB_HEIGHT: u32 = 300;

fn parse_custom_thumb_height(text: &str) -> Option<u32> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    trimmed.parse::<u32>().ok().filter(|v| *v > 0)
}

// --- Human-readable labels used in success popups -------------------------

fn view_mode_label(mode: ViewModeOverride) -> &'static str {
    match mode {
        ViewModeOverride::Automatic => "Automatic (width-based)",
        ViewModeOverride::ForceWide => "Force Wide (desktop)",
        ViewModeOverride::ForceNarrow => "Force Narrow (mobile)",
    }
}

fn send_shortcut_label(send_on_enter: bool) -> &'static str {
    if send_on_enter {
        "Enter to send, Shift+Enter for newline"
    } else {
        "Cmd/Ctrl + Enter to send, Enter for newline"
    }
}

fn thumbnail_max_height_label(thumb: ThumbnailMaxHeight) -> String {
    match thumb {
        ThumbnailMaxHeight::Small => "Small (200 px)".to_string(),
        ThumbnailMaxHeight::Medium => "Medium (400 px)".to_string(),
        ThumbnailMaxHeight::Unlimited => "Unlimited".to_string(),
        ThumbnailMaxHeight::Custom(v) => format!("Custom ({v} px)"),
    }
}
