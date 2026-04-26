//! App-related (behavior & appearance) settings within the SettingsScreen.

use makepad_widgets::*;

use crate::{
    app::AppState,
    settings::app_preferences::{AppPreferences, AppPreferencesGlobal, ThumbnailMaxHeight, ViewModeOverride},
    shared::popup_list::{enqueue_popup_notification, PopupKind},
};

#[cfg(target_os = "macos")]
const SEND_SHORTCUT_TOGGLE_LABEL: &str = "Send with Cmd⌘ + Enter";
#[cfg(not(target_os = "macos"))]
const SEND_SHORTCUT_TOGGLE_LABEL: &str = "Send with Ctrl + Enter";

#[cfg(target_os = "macos")]
const SEND_SHORTCUT_DESC_CMD: &str = "Current choice: 'Cmd⌘ + Enter' to send, 'Enter' for a new line";
#[cfg(not(target_os = "macos"))]
const SEND_SHORTCUT_DESC_CMD: &str = "Current choice: 'Ctrl + Enter' to send, 'Enter' for a new line";


script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.SettingsSectionDescription = Label {
        width: Fill, height: Fit
        flow: Flow.Right{wrap: true}
        margin: Inset{left: 0.5, top: 0, bottom: 0, right: 5}
        draw_text +: {
            color: #666,
            text_style: MESSAGE_TEXT_STYLE { font_size: 11 },
        }
    }

    // A single item within a Robrix-styled settings DropDown popup menu.
    mod.widgets.RobrixSettingsPopupMenuItem = PopupMenuItem {
        width: Fill, height: Fit
        align: Align{y: 0.5}
        padding: Inset{top: 8, bottom: 8, left: 28, right: 14}

        draw_text +: {
            color: (MESSAGE_TEXT_COLOR),
            color_hover: (MESSAGE_TEXT_COLOR),
            color_active: (COLOR_ACTIVE_PRIMARY_DARKER),
            text_style: SETTINGS_REGULAR_TEXT_STYLE {},
        }

        draw_bg +: {
            color: (COLOR_PRIMARY),
            color_hover: (COLOR_BG_PREVIEW),
            color_active: (COLOR_BG_PREVIEW),
            border_color: vec4(0.0, 0.0, 0.0, 0.0),
            border_color_hover: vec4(0.0, 0.0, 0.0, 0.0),
            border_color_active: vec4(0.0, 0.0, 0.0, 0.0),
            border_size: 0.0,
            border_radius: 3.0,
            mark_color: vec4(0.0, 0.0, 0.0, 0.0),
            mark_color_active: (COLOR_ACTIVE_PRIMARY_DARKER),
        }
    }

    // The popup list shown when a RobrixSettingsDropDown is opened.
    mod.widgets.RobrixSettingsPopupMenu = PopupMenu {
        width: 260, height: Fit
        padding: 4,

        menu_item: mod.widgets.RobrixSettingsPopupMenuItem{}

        draw_bg +: {
            color: (COLOR_PRIMARY),
            border_color: (COLOR_SECONDARY_DARKER),
            border_size: 1.0,
            border_radius: 4.0,
        }
    }

    // A DropDown styled to match other Robrix settings controls.
    mod.widgets.RobrixSettingsDropDown = DropDownFlat {
        width: 239, height: (mod.widgets.SETTINGS_BUTTON_HEIGHT),
        padding: Inset{top: 8, bottom: 8, left: 12, right: 30}
        margin: Inset{left: 5, top: 5, bottom: 5}
        align: Align{x: 0.0, y: 0.5}

        // NOTE: PopupMenuPosition enum variants aren't exposed to script, so
        // we can't request `BelowInput`. The popup uses the default
        // OnSelected placement — styled to stay readable either way.
        popup_menu: mod.widgets.RobrixSettingsPopupMenu{}

        draw_text +: {
            color: (MESSAGE_TEXT_COLOR),
            color_hover: (MESSAGE_TEXT_COLOR),
            color_focus: (MESSAGE_TEXT_COLOR),
            color_down: (MESSAGE_TEXT_COLOR),
            text_style: SETTINGS_REGULAR_TEXT_STYLE {},
        }

        draw_bg +: {
            color: (COLOR_PRIMARY),
            color_hover: (COLOR_PRIMARY),
            color_down: (COLOR_PRIMARY),
            color_focus: (COLOR_PRIMARY),
            border_color: (COLOR_SECONDARY_DARKER),
            border_color_hover: (COLOR_ACTIVE_PRIMARY),
            border_color_focus: (COLOR_ACTIVE_PRIMARY_DARKER),
            border_color_down: (COLOR_ACTIVE_PRIMARY_DARKER),
            border_size: 1.0,
            border_radius: 4.0,
            arrow_color: (MESSAGE_TEXT_COLOR),
            arrow_color_hover: (COLOR_ACTIVE_PRIMARY_DARKER),
            arrow_color_focus: (COLOR_ACTIVE_PRIMARY_DARKER),
            arrow_color_down: (COLOR_ACTIVE_PRIMARY_DARKER),

            // The base DropDownFlat shader draws the arrow BEFORE the box,
            // so the box fill paints over it. Override to draw the rounded
            // rect first and then the arrow on top.
            pixel: fn() {
                let sdf = Sdf2d.viewport(self.pos * self.rect_size)

                sdf.box(
                    self.border_size
                    self.border_size
                    self.rect_size.x - self.border_size * 2.
                    self.rect_size.y - self.border_size * 2.
                    self.border_radius
                )

                let fill = self.color
                    .mix(self.color_focus, self.focus)
                    .mix(self.color_hover, self.hover)
                    .mix(self.color_down, self.down * self.hover)
                    .mix(self.color_disabled, self.disabled)

                let stroke = self.border_color
                    .mix(self.border_color_focus, self.focus)
                    .mix(self.border_color_hover, self.hover)
                    .mix(self.border_color_down, self.down * self.hover)
                    .mix(self.border_color_disabled, self.disabled)

                sdf.fill_keep(fill)
                sdf.stroke(stroke, self.border_size)

                // Draw the down-arrow triangle on top of the filled box.
                let c = vec2(self.rect_size.x - 14.0, self.rect_size.y * 0.5)
                let sz = 3.5
                sdf.move_to(c.x - sz, c.y - sz * 0.5)
                sdf.line_to(c.x + sz, c.y - sz * 0.5)
                sdf.line_to(c.x, c.y + sz * 0.75)
                sdf.close_path()

                let arrow = self.arrow_color
                    .mix(self.arrow_color_focus, self.focus)
                    .mix(self.arrow_color_hover, self.hover)
                    .mix(self.arrow_color_down, self.down * self.hover)
                    .mix(self.arrow_color_disabled, self.disabled)

                sdf.fill(arrow)

                return sdf.result
            }
        }
    }

    // A radio button styled to match other Robrix settings controls.
    mod.widgets.RobrixSettingsRadioButton = RadioButton {
        height: Fit,
        align: Align{y: 0.5},
        padding: Inset{top: 6, bottom: 6, left: 10, right: 4}

        draw_text +: {
            color: (MESSAGE_TEXT_COLOR),
            color_hover: (MESSAGE_TEXT_COLOR),
            color_active: (MESSAGE_TEXT_COLOR),
            color_focus: (MESSAGE_TEXT_COLOR),
            color_down: (MESSAGE_TEXT_COLOR),
            text_style: SETTINGS_REGULAR_TEXT_STYLE {},
        }

        draw_bg +: {
            color: (COLOR_PRIMARY),
            color_hover: (COLOR_PRIMARY),
            color_active: (COLOR_PRIMARY),
            color_focus: (COLOR_PRIMARY),
            color_down: (COLOR_PRIMARY),
            border_color: (COLOR_SECONDARY_DARKER),
            border_color_hover: (COLOR_ACTIVE_PRIMARY),
            border_color_active: (COLOR_ACTIVE_PRIMARY_DARKER),
            border_color_focus: (COLOR_ACTIVE_PRIMARY_DARKER),
            border_color_down: (COLOR_ACTIVE_PRIMARY_DARKER),
            mark_color: vec4(0.0, 0.0, 0.0, 0.0),
            mark_color_active: (COLOR_ACTIVE_PRIMARY_DARKER),
        }
    }


    // The view containing Robrix app-wide preferences/settings.
    mod.widgets.AppSettings = #(AppSettings::register_widget(vm)) {
        width: Fill, height: Fit
        flow: Down,

        TitleLabel {
            text: "App Settings"
        }

        View {
            width: Fill, height: Fit
            flow: Right{wrap: true}
            align: Align{y: 0.5}

            SubsectionLabel {
                width: Fit,
                margin: Inset{top: 4}
                text: "Force View Mode:"
            }

            view_mode_dropdown := mod.widgets.RobrixSettingsDropDown {
                labels: ["Automatic (default)", "Force wide (desktop)", "Force narrow (mobile)"]
                selected_item: 0
            }
        }
        mod.widgets.SettingsSectionDescription {
            text: "By default, the app layout auto-adapts based on width."
        }


        SubsectionLabel {
            text: "Send Message Keyboard Shortcut"
        }

        send_on_cmd_enter_toggle := ToggleFlat {
            margin: Inset{left: 6.5, top: 5, bottom: 5}
            padding: Inset { left: 15}
            active: false,
            draw_bg +: { size: 21 }
            text: "" // we set this text dynamically based on the toggle state and target platform
            draw_text +: {
                text_style: SETTINGS_REGULAR_TEXT_STYLE {},
            }
        }

        send_shortcut_description := mod.widgets.SettingsSectionDescription {
            text: "Current setting: \"Enter\" to send, \"Shift + Enter\" for a new line"
        }


        SubsectionLabel {
            text: "Maximum Height of Thumbnails"
        }

        View {
            width: Fill, height: Fit
            flow: Down,
            margin: Inset{left: 6},
            spacing: 4,

            thumb_small_radio := mod.widgets.RobrixSettingsRadioButton {
                text: "Small (200 pixels, default)"
            }

            thumb_medium_radio := mod.widgets.RobrixSettingsRadioButton {
                text: "Medium (400 pixels)"
            }

            thumb_unlimited_radio := mod.widgets.RobrixSettingsRadioButton {
                text: "Unlimited (not recommended)"
            }

            View {
                width: Fill, height: Fit
                flow: Right,
                align: Align{y: 0.5}
                spacing: 6,

                thumb_custom_radio := mod.widgets.RobrixSettingsRadioButton {
                    text: "Custom:"
                }

                // Starts read-only so it's inert before populate() runs with
                // the user's actual preference. populate() and the radio
                // handler then toggle editability based on whether the
                // "Custom" radio is selected.
                thumb_custom_input := RobrixTextInput {
                    width: 60, height: Fit
                    padding: Inset{left: 8, right: 8, top: 5, bottom: 5}
                    empty_text: "300"
                    is_read_only: true
                }

                Label {
                    width: Fit, height: Fit
                    draw_text +: {
                        color: (MESSAGE_TEXT_COLOR),
                        text_style: MESSAGE_TEXT_STYLE { font_size: 11 },
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
/// [`AppPreferencesAction`]s so other widgets can apply changes live.
#[derive(Script, Widget)]
pub struct AppSettings {
    #[deref] view: View,
}

impl ScriptHook for AppSettings {
    fn on_after_apply(
        &mut self,
        vm: &mut ScriptVm,
        apply: &Apply,
        _scope: &mut Scope,
        _value: ScriptValue,
    ) {
        // The apply walk just reset every code-set field here to its DSL
        // default. Restore them inline (before any draw) — doing it later
        // in `handle_event` produces a one-frame flicker.
        //
        // Prefs come from the global mirror cuz the apply walk runs with
        // an empty `Scope`. The mirror is kept in sync by `on_*_changed`
        // in `app_preferences.rs`.
        if !apply.is_script_reapply() {
            return;
        }
        let cx = vm.cx_mut();
        let prefs = cx.global::<AppPreferencesGlobal>().0.clone();
        Self::populate_safe(cx, &self.view, &prefs);
    }
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

        let view_mode_dropdown = self.view.drop_down(cx, ids!(view_mode_dropdown));
        if let Some(index) = view_mode_dropdown.changed(actions) {
            let new_mode = ViewModeOverride::from_index(index);
            if new_mode != app_state.app_prefs.view_mode {
                app_state.app_prefs.view_mode = new_mode;
                app_state.app_prefs.on_view_mode_changed(cx);
                enqueue_popup_notification(
                    "Updated view mode setting.",
                    PopupKind::Success,
                    Some(3.0),
                );
            }
        }

        let send_toggle = self.view.check_box(cx, ids!(send_on_cmd_enter_toggle));
        if let Some(cmd_enter_active) = send_toggle.changed(actions) {
            // The toggle's "active" state is the invsert of `send_on_enter`.
            let new_send_on_enter = !cmd_enter_active;
            if new_send_on_enter != app_state.app_prefs.send_on_enter {
                app_state.app_prefs.send_on_enter = new_send_on_enter;
                Self::update_send_shortcut_description(cx, &self.view, new_send_on_enter);
                app_state.app_prefs.on_send_on_enter_changed(cx);
                enqueue_popup_notification(
                    "Updated send message shortcut.",
                    PopupKind::Success,
                    Some(3.0),
                );
            }
        }

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
            Self::set_thumb_custom_input_read_only(cx, &self.view, custom_now);
            Self::set_thumb_custom_input_disabled(cx, &self.view, custom_now);
            if new_thumb != app_state.app_prefs.thumbnail_max_height {
                app_state.app_prefs.thumbnail_max_height = new_thumb;
                app_state.app_prefs.on_thumbnail_max_height_changed(cx);
                enqueue_popup_notification(
                    "Updated max image thumbnail height.",
                    PopupKind::Success,
                    Some(3.0),
                );
            }
            // If Custom is now selected, reflect the current value in the input.
            if let ThumbnailMaxHeight::Custom(v) = new_thumb {
                custom_input.set_text(cx, &v.to_string());
            }
        }

        // Only process the custom thumbnail input when the user presses Enter
        // or moves key focus away from the input, not on every keypress.
        if custom_input.returned(actions).is_some() || custom_input.key_focus_lost(actions) {
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
                                "Updated max image thumbnail height.",
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

    /// Populates the widget from the given prefs. Called on initial open
    /// or when fresh prefs arrive.
    ///
    /// Don't call from `Event::ScriptReapply` — code-set fields get
    /// handled in [`Self::on_after_apply`], and animator-driven fields
    /// are restored by the codegen apply chain.
    pub fn populate(&mut self, cx: &mut Cx, prefs: &AppPreferences) {
        Self::populate_safe(cx, &self.view, prefs);

        // The animator setup below uses `set_active(Animate::No)` →
        // `animator_cut` → `cx.with_vm`, which would panic from
        // `on_after_apply`. Fine here cuz we're outside any apply walk.
        let send_toggle = self.view.check_box(cx, ids!(send_on_cmd_enter_toggle));
        send_toggle.set_active(cx, !prefs.send_on_enter, Animate::No);

        let (small, medium, unlimited, custom, custom_text) = match prefs.thumbnail_max_height {
            ThumbnailMaxHeight::Small => (true, false, false, false, String::new()),
            ThumbnailMaxHeight::Medium => (false, true, false, false, String::new()),
            ThumbnailMaxHeight::Unlimited => (false, false, true, false, String::new()),
            ThumbnailMaxHeight::Custom(v) => (false, false, false, true, v.to_string()),
        };
        self.view.radio_button(cx, ids!(thumb_small_radio)).set_active(cx, small, Animate::No);
        self.view.radio_button(cx, ids!(thumb_medium_radio)).set_active(cx, medium, Animate::No);
        self.view.radio_button(cx, ids!(thumb_unlimited_radio)).set_active(cx, unlimited, Animate::No);
        self.view.radio_button(cx, ids!(thumb_custom_radio)).set_active(cx, custom, Animate::No);
        // `populate_safe` set `is_read_only`; pair it with the animator's
        // disabled state here so the input lands in the right state on
        // first paint. ScriptReapply only needs `is_read_only`.
        Self::set_thumb_custom_input_disabled(cx, &self.view, custom);

        // Only write `thumb_custom_input`'s text on initial populate.
        // `on_after_apply` leaves it alone so in-progress edits survive.
        self.view.text_input(cx, ids!(thumb_custom_input)).set_text(cx, &custom_text);
    }

    /// Restores the code-set fields the apply walk just reset to DSL
    /// defaults: toggle label, description, dropdown index, custom-input
    /// read-only flag. None go through `cx.with_vm`, so this is safe to
    /// call from `on_after_apply` — doing it inline is what prevents the
    /// one-frame flicker.
    fn populate_safe(cx: &mut Cx, view: &View, prefs: &AppPreferences) {
        view.drop_down(cx, ids!(view_mode_dropdown))
            .set_selected_item(cx, prefs.view_mode.to_index());

        view.check_box(cx, ids!(send_on_cmd_enter_toggle))
            .set_text(SEND_SHORTCUT_TOGGLE_LABEL);
        Self::update_send_shortcut_description(cx, view, prefs.send_on_enter);

        let custom_active = matches!(prefs.thumbnail_max_height, ThumbnailMaxHeight::Custom(_));
        Self::set_thumb_custom_input_read_only(cx, view, custom_active);
    }

    fn update_send_shortcut_description(cx: &mut Cx, view: &View, send_on_enter: bool) {
        let text = if send_on_enter {
            "Current choice: 'Enter' to send, 'Shift + Enter' for a new line"
        } else {
            SEND_SHORTCUT_DESC_CMD
        };
        view.label(cx, ids!(send_shortcut_description)).set_text(cx, text);
    }

    /// Sets `is_read_only` based on whether the custom radio is selected.
    /// It's a plain `#[live] bool` the apply walk resets to the DSL
    /// default, so we re-set it ourselves. Safe anywhere — just a field
    /// write plus redraw.
    fn set_thumb_custom_input_read_only(cx: &mut Cx, view: &View, enabled: bool) {
        view.text_input(cx, ids!(thumb_custom_input))
            .set_is_read_only(cx, !enabled);
    }

    /// Sets the disabled animator state.
    ///
    /// **Not safe inside `on_after_apply`** — `set_disabled` goes through
    /// `animator_toggle` → `cx.with_vm`, which panics when the VM is
    /// swapped out. Only call from outside an apply walk. ScriptReapply
    /// doesn't need it — the codegen chain restores animator state itself.
    fn set_thumb_custom_input_disabled(cx: &mut Cx, view: &View, enabled: bool) {
        view.text_input(cx, ids!(thumb_custom_input))
            .set_disabled(cx, !enabled);
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
