use makepad_widgets::*;

use crate::{
    app::{AppState, BotSettingsState},
    shared::popup_list::{PopupKind, enqueue_popup_notification},
};

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*


    mod.widgets.BotSettings = #(BotSettings::register_widget(vm)) {
        width: Fill, height: Fit
        flow: Down
        spacing: 10

        TitleLabel {
            text: "App Service"
        }

        description := Label {
            width: Fill,
            height: Fit
            margin: Inset{left: 5, right: 8, bottom: 4}
            flow: Flow.Right{wrap: true}
            draw_text +: {
                color: (MESSAGE_TEXT_COLOR)
                text_style: REGULAR_TEXT {font_size: 10.5}
            }
            text: "Enable Matrix app service support here. Robrix stays a normal Matrix client: it binds BotFather to a room and sends the matching slash commands."
        }

        enable_row := View {
            width: Fill,
            height: Fit
            flow: Right
            align: Align{y: 0.5}
            spacing: 12
            margin: Inset{left: 5, bottom: 2}

            enable_label := SubsectionLabel {
                width: Fit, height: Fit
                margin: 0
                text: "Enable App Service"
            }

            enable_button := RobrixNeutralIconButton {
                width: Fit,
                height: Fit
                padding: Inset{top: 9, bottom: 9, left: 12, right: 14}
                spacing: 0
                text: "Disabled"
            }
        }

        bot_details := View {
            visible: false
            width: Fill, height: Fit
            flow: Down
            spacing: 8

            SubsectionLabel {
                text: "BotFather User ID:"
            }

            bot_user_id_input := RobrixTextInput {
                margin: Inset{top: 2, left: 5, right: 5, bottom: 2}
                width: 280, height: Fit
                empty_text: "bot or @bot:server"
            }

            details_hint := Label {
                width: Fill,
                height: Fit
                margin: Inset{left: 5, right: 8}
                flow: Flow.Right{wrap: true}
                draw_text +: {
                    color: #666
                    text_style: REGULAR_TEXT {font_size: 9.7}
                }
                text: "Use either a localpart like `bot` or a full Matrix user ID. Bind or unbind BotFather from a room via the room menu or `/bot`."
            }

            save_button := RobrixPositiveIconButton {
                width: Fit,
                height: Fit
                margin: Inset{left: 5}
                padding: Inset{top: 10, bottom: 10, left: 12, right: 15}
                draw_icon.svg: (ICON_CHECKMARK)
                icon_walk: Walk{width: 16, height: 16}
                text: "Save App Service Settings"
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
        if let Event::Actions(actions) = event {
            self.handle_actions(cx, actions, scope);
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl BotSettings {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions, scope: &mut Scope) {
        let Some(app_state) = scope.data.get_mut::<AppState>() else {
            return;
        };

        if self.view.button(cx, ids!(enable_row.enable_button)).clicked(actions) {
            app_state.bot_settings.enabled = !app_state.bot_settings.enabled;
            self.sync_ui(cx, &app_state.bot_settings);
            return;
        }

        if self.view.button(cx, ids!(bot_details.save_button)).clicked(actions) {
            let bot_user_id = self
                .view
                .text_input(cx, ids!(bot_details.bot_user_id_input))
                .text()
                .trim()
                .to_string();
            app_state.bot_settings.botfather_user_id = if bot_user_id.is_empty() {
                BotSettingsState::DEFAULT_BOTFATHER_LOCALPART.to_string()
            } else {
                bot_user_id
            };
            self.sync_ui(cx, &app_state.bot_settings);
            enqueue_popup_notification(
                "Saved Matrix app service settings.",
                PopupKind::Success,
                Some(3.0),
            );
        }
    }

    fn sync_enable_button(&mut self, cx: &mut Cx, enabled: bool) {
        let mut enable_button = self.view.button(cx, ids!(enable_row.enable_button));
        enable_button.set_text(cx, if enabled { "Enabled" } else { "Disabled" });
        if enabled {
            script_apply_eval!(cx, enable_button, {
                draw_bg +: {
                    color: mod.widgets.COLOR_ACTIVE_PRIMARY
                    color_hover: mod.widgets.COLOR_ACTIVE_PRIMARY_DARKER
                    color_down: #x0c5daa
                    border_color: mod.widgets.COLOR_ACTIVE_PRIMARY
                    border_color_hover: mod.widgets.COLOR_ACTIVE_PRIMARY_DARKER
                    border_color_down: #x0c5daa
                }
                draw_text +: {
                    color: mod.widgets.COLOR_PRIMARY
                    color_hover: mod.widgets.COLOR_PRIMARY
                    color_down: mod.widgets.COLOR_PRIMARY
                }
            });
        } else {
            script_apply_eval!(cx, enable_button, {
                draw_bg +: {
                    border_color: mod.widgets.COLOR_BG_DISABLED
                    border_color_hover: mod.widgets.COLOR_BG_DISABLED
                    border_color_down: mod.widgets.COLOR_BG_DISABLED
                    color: mod.widgets.COLOR_SECONDARY
                    color_hover: #D0D0D0
                    color_down: #C0C0C0
                }
                draw_text +: {
                    color: mod.widgets.COLOR_TEXT
                    color_hover: mod.widgets.COLOR_TEXT
                    color_down: mod.widgets.COLOR_TEXT
                }
            });
        }
    }

    fn sync_ui(&mut self, cx: &mut Cx, bot_settings: &BotSettingsState) {
        self.sync_enable_button(cx, bot_settings.enabled);
        self.view
            .view(cx, ids!(bot_details))
            .set_visible(cx, bot_settings.enabled);
        self.view
            .text_input(cx, ids!(bot_details.bot_user_id_input))
            .set_text(cx, &bot_settings.botfather_user_id);
        self.view
            .button(cx, ids!(bot_details.save_button))
            .reset_hover(cx);
        self.redraw(cx);
    }

    pub fn populate(&mut self, cx: &mut Cx, bot_settings: &BotSettingsState) {
        self.sync_ui(cx, bot_settings);
    }
}

impl BotSettingsRef {
    pub fn populate(&self, cx: &mut Cx, bot_settings: &BotSettingsState) {
        let Some(mut inner) = self.borrow_mut() else {
            return;
        };
        inner.populate(cx, bot_settings);
    }
}
