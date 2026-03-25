use makepad_widgets::*;

use crate::{
    app::{AppState, BotSettingsState},
    shared::popup_list::{PopupKind, enqueue_popup_notification},
};

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.BotSettingsInfoLabel = Label {
        width: Fill
        height: Fit
        margin: Inset{left: 5, top: 2, bottom: 2}
        draw_text +: {
            wrap: Word
            color: (MESSAGE_TEXT_COLOR)
            text_style: REGULAR_TEXT { font_size: 10.5 }
        }
        text: ""
    }

    mod.widgets.BotSettings = #(BotSettings::register_widget(vm)) {
        width: Fill
        height: Fit
        flow: Down
        spacing: 10

        TitleLabel {
            text: "App Service"
        }

        description := mod.widgets.BotSettingsInfoLabel {
            margin: Inset{left: 5, right: 8, bottom: 4}
            text: "Enable Matrix app service support here. Robrix stays a normal Matrix client: it binds BotFather to a room and sends the matching slash commands."
        }

        toggle_row := View {
            width: Fill
            height: Fit
            flow: Right
            align: Align{y: 0.5}
            spacing: 12
            margin: Inset{left: 5, bottom: 2}

            enable_label := SubsectionLabel {
                width: Fit
                height: Fit
                margin: 0
                text: "Enable App Service"
            }

            toggle_button := RobrixNeutralIconButton {
                width: Fit
                height: Fit
                padding: Inset{top: 10, bottom: 10, left: 12, right: 15}
                draw_icon.svg: (ICON_HIERARCHY)
                icon_walk: Walk{width: 16, height: 16}
                text: "Enable App Service"
            }
        }

        bot_details := View {
            visible: false
            width: Fill
            height: Fit
            flow: Down

            SubsectionLabel {
                text: "BotFather User ID:"
            }

            bot_user_id_input := RobrixTextInput {
                margin: Inset{top: 2, left: 5, right: 5, bottom: 8}
                width: 280
                height: Fit
                empty_text: "bot or @bot:server"
            }

            buttons := View {
                width: Fill
                height: Fit
                flow: Right
                spacing: 10

                save_button := RobrixPositiveIconButton {
                    width: Fit
                    height: Fit
                    padding: Inset{top: 10, bottom: 10, left: 12, right: 15}
                    margin: Inset{left: 5}
                    draw_icon.svg: (ICON_CHECKMARK)
                    icon_walk: Walk{width: 16, height: 16}
                    text: "Save"
                }
            }
        }
    }
}

#[derive(Script, ScriptHook, Widget)]
pub struct BotSettings {
    #[deref]
    view: View,
}

impl Widget for BotSettings {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        self.widget_match_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl WidgetMatchEvent for BotSettings {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions, _scope: &mut Scope) {
        let toggle_button = self.view.button(cx, ids!(toggle_button));
        let bot_details = self.view.view(cx, ids!(bot_details));
        let bot_user_id_input = self.view.text_input(cx, ids!(bot_user_id_input));
        let save_button = self.view.button(cx, ids!(buttons.save_button));

        let Some(app_state) = _scope.data.get_mut::<AppState>() else {
            return;
        };

        if toggle_button.clicked(actions) {
            let enabled = !app_state.bot_settings.enabled;
            app_state.bot_settings.enabled = enabled;
            self.sync_ui(cx, &app_state.bot_settings);
            bot_details.set_visible(cx, enabled);
            self.view.redraw(cx);
        }

        if save_button.clicked(actions) || bot_user_id_input.returned(actions).is_some() {
            app_state.bot_settings.botfather_user_id = bot_user_id_input.text().trim().to_string();
            enqueue_popup_notification(
                "Saved Matrix app service settings.",
                PopupKind::Success,
                Some(3.0),
            );
            self.sync_ui(cx, &app_state.bot_settings);
        }
    }
}

impl BotSettings {
    fn sync_ui(&mut self, cx: &mut Cx, bot_settings: &BotSettingsState) {
        self.view
            .view(cx, ids!(bot_details))
            .set_visible(cx, bot_settings.enabled);
        self.view
            .text_input(cx, ids!(bot_user_id_input))
            .set_text(cx, &bot_settings.botfather_user_id);

        let toggle_text = if bot_settings.enabled {
            "Disable App Service"
        } else {
            "Enable App Service"
        };
        self.view
            .button(cx, ids!(toggle_button))
            .set_text(cx, toggle_text);
        self.view.button(cx, ids!(toggle_button)).reset_hover(cx);
        self.view
            .button(cx, ids!(buttons.save_button))
            .reset_hover(cx);
        self.view.redraw(cx);
    }

    /// Populates the bot settings UI from the current persisted app state.
    pub fn populate(&mut self, cx: &mut Cx, bot_settings: &BotSettingsState) {
        self.sync_ui(cx, bot_settings);
    }
}

impl BotSettingsRef {
    /// See [`BotSettings::populate()`].
    pub fn populate(&self, cx: &mut Cx, bot_settings: &BotSettingsState) {
        let Some(mut inner) = self.borrow_mut() else {
            return;
        };
        inner.populate(cx, bot_settings);
    }
}
