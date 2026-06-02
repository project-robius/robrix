use makepad_widgets::*;

use crate::{
    app::AppState,
    i18n::{AppLanguage, tr_fmt, tr_key},
    persistence,
    room::translation::{self, TranslationConfig},
    sliding_sync::current_user_id,
};

const TEST_TRANSLATION_REQUEST_ID: LiveId = live_id!(test_translation);

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.TranslationSettings = #(TranslationSettings::register_widget(vm)) {
        width: Fill
        height: Fit
        flow: Down
        spacing: (SPACE_SM)

        translation_header := View {
            width: Fill
            height: Fit
            flow: Down
            spacing: (SPACE_XS)
            margin: Inset{bottom: 2}

            translation_title := TitleLabel {
                width: Fit
                text: "Real-time Translation"
            }

            description := Label {
                width: Fill
                height: Fit
                margin: 0
                draw_text +: {
                    color: (COLOR_DESCRIPTION_TEXT)
                    text_style: REGULAR_TEXT { font_size: 9.5 }
                }
                text: "Configure an OpenAI-compatible API for real-time message translation in the input bar."
            }
        }

        toggle_row := View {
            width: Fill
            height: Fit
            flow: Right
            align: Align{x: 0.0, y: 0.5}
            spacing: (SPACE_XS)
            padding: Inset{left: 6}
            margin: Inset{bottom: 2}

            translation_switch := Toggle {
                width: Fit
                height: Fit
                padding: Inset{top: (SPACE_SM), right: (SPACE_SM), bottom: (SPACE_SM), left: (SPACE_SM)}
                text: ""
                active: false
                draw_bg +: {
                    size: 20.0
                    color_active: (COLOR_ACTIVE_PRIMARY)
                    border_color_active: (COLOR_ACTIVE_PRIMARY)
                    mark_color_active: #fff
                }
            }

            switch_state_label := Label {
                width: Fit
                height: Fit
                draw_text +: {
                    color: (COLOR_DISABLED_TEXT)
                    text_style: REGULAR_TEXT { font_size: 10.5 }
                }
                text: "Disabled"
            }
        }

        config_section := View {
            visible: false
            width: Fill
            height: Fit
            flow: Down
            spacing: (SPACE_SM)
            margin: 0

            View {
                width: Fill, height: Fit
                flow: Down, spacing: 4
                api_url_label := Label {
                    width: Fit, height: Fit
                    draw_text +: {
                        color: (COLOR_FIELD_LABEL)
                        text_style: REGULAR_TEXT { font_size: 10 }
                    }
                    text: "API URL"
                }
                api_url_input := RobrixTextInput {
                    width: Fill, height: Fit
                    padding: 8
                    empty_text: "http://localhost:18080"
                }
            }

            View {
                width: Fill, height: Fit
                flow: Down, spacing: 4
                api_key_label := Label {
                    width: Fit, height: Fit
                    draw_text +: {
                        color: (COLOR_FIELD_LABEL)
                        text_style: REGULAR_TEXT { font_size: 10 }
                    }
                    text: "API Key"
                }
                api_key_input := RobrixTextInput {
                    width: Fill, height: Fit
                    padding: 8
                    empty_text: "sk-..."
                    is_password: true
                }
            }

            View {
                width: Fill, height: Fit
                flow: Down, spacing: 4
                model_label := Label {
                    width: Fit, height: Fit
                    draw_text +: {
                        color: (COLOR_FIELD_LABEL)
                        text_style: REGULAR_TEXT { font_size: 10 }
                    }
                    text: "Model"
                }
                model_input := RobrixTextInput {
                    width: Fill, height: Fit
                    padding: 8
                    empty_text: "qwen3-4b"
                }
            }

            View {
                width: Fill, height: Fit
                flow: Right
                spacing: (SPACE_SM)
                margin: Inset{top: (SPACE_XS)}

                save_button := RobrixIconButton {
                    padding: Inset{top: 8, bottom: 8, left: 16, right: 16}
                    icon_walk: Walk{width: 0, height: 0}
                    spacing: 0
                    text: "Save"
                }

                test_button := RobrixNeutralIconButton {
                    padding: Inset{top: 8, bottom: 8, left: 16, right: 16}
                    icon_walk: Walk{width: 0, height: 0}
                    spacing: 0
                    text: "Test Connection"
                }

                test_result_label := Label {
                    width: Fit, height: Fit
                    margin: Inset{left: (SPACE_SM)}
                    align: Align{y: 0.5}
                    draw_text +: {
                        color: (COLOR_DISABLED_TEXT)
                        text_style: REGULAR_TEXT { font_size: 10 }
                    }
                    text: ""
                }
            }
        }
    }
}

#[derive(Script, ScriptHook, Widget)]
pub struct TranslationSettings {
    #[deref]
    view: View,
    #[rust]
    app_language: AppLanguage,
    #[rust]
    app_language_initialized: bool,
}

impl Widget for TranslationSettings {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        let app_language = scope.data.get::<AppState>()
            .map(|app_state| app_state.app_language)
            .unwrap_or_default();
        if !self.app_language_initialized || self.app_language != app_language {
            self.set_app_language(cx, app_language);
        }

        // Handle test connection HTTP response
        if let Event::NetworkResponses(responses) = event {
            for response in responses {
                if let NetworkResponse::HttpResponse { request_id, response } = response {
                    if *request_id == TEST_TRANSLATION_REQUEST_ID {
                        log!("Test translation response: status={}, body={:?}",
                            response.status_code,
                            response.body_string().unwrap_or_default().chars().take(200).collect::<String>());
                        let label = self.view.label(cx, ids!(test_result_label));
                        match translation::parse_translation_response(response) {
                            Ok(result) => {
                                let mut lbl = label;
                                script_apply_eval!(cx, lbl, {
                                    draw_text +: { color: #x00AA00 }
                                });
                                lbl.set_text(cx, &tr_fmt(self.app_language, "settings.labs.translation.test.ok", &[("result", &result)]));
                            }
                            Err(e) => {
                                let mut lbl = label;
                                script_apply_eval!(cx, lbl, {
                                    draw_text +: { color: #xCC0000 }
                                });
                                lbl.set_text(cx, &tr_fmt(self.app_language, "settings.labs.translation.test.failed", &[("error", &e)]));
                            }
                        }
                        self.view.redraw(cx);
                    }
                }
                if let NetworkResponse::HttpError { request_id, error } = response {
                    if *request_id == TEST_TRANSLATION_REQUEST_ID {
                        let mut label = self.view.label(cx, ids!(test_result_label));
                        script_apply_eval!(cx, label, {
                            draw_text +: { color: #xCC0000 }
                        });
                        label.set_text(cx, &tr_fmt(self.app_language, "settings.labs.translation.test.error", &[("error", &error.message)]));
                        self.view.redraw(cx);
                    }
                }
            }
        }

        self.view.handle_event(cx, event, scope);
        self.widget_match_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        let app_language = scope.data.get::<AppState>()
            .map(|app_state| app_state.app_language)
            .unwrap_or_default();
        if !self.app_language_initialized || self.app_language != app_language {
            self.set_app_language(cx, app_language);
        }
        self.view.draw_walk(cx, scope, walk)
    }
}

impl WidgetMatchEvent for TranslationSettings {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions, scope: &mut Scope) {
        let translation_switch = self.view.check_box(cx, ids!(translation_switch));

        let Some(app_state) = scope.data.get_mut::<AppState>() else {
            return;
        };

        if let Some(enabled) = translation_switch.changed(actions) {
            app_state.translation.enabled = enabled;
            self.sync_ui(cx, &app_state.translation);
            translation::set_global_config(&app_state.translation);
            persist_translation_config(app_state);
            self.view.redraw(cx);
        }

        if self.view.button(cx, ids!(save_button)).clicked(actions) {
            let api_url = self.view.text_input(cx, ids!(api_url_input)).text().trim().to_string();
            let api_key = self.view.text_input(cx, ids!(api_key_input)).text().trim().to_string();
            let model = self.view.text_input(cx, ids!(model_input)).text().trim().to_string();

            if !api_url.is_empty() {
                app_state.translation.api_base_url = api_url;
            }
            app_state.translation.api_key = api_key;
            if !model.is_empty() {
                app_state.translation.model = model;
            }
            translation::set_global_config(&app_state.translation);
            persist_translation_config(app_state);
            self.view.redraw(cx);
        }

        // Test connection button: send a simple translation request to validate API
        if self.view.button(cx, ids!(test_button)).clicked(actions) {
            let api_url = self.view.text_input(cx, ids!(api_url_input)).text().trim().to_string();
            let api_key = self.view.text_input(cx, ids!(api_key_input)).text().trim().to_string();
            let model = self.view.text_input(cx, ids!(model_input)).text().trim().to_string();

            if api_url.is_empty() {
                self.view
                    .label(cx, ids!(test_result_label))
                    .set_text(cx, tr_key(self.app_language, "settings.labs.translation.validation.api_url_empty"));
                self.view.redraw(cx);
                return;
            }

            // Show testing status
            let mut label = self.view.label(cx, ids!(test_result_label));
            script_apply_eval!(cx, label, {
                draw_text +: { color: #x999999 }
            });
            label.set_text(cx, tr_key(self.app_language, "settings.labs.translation.test.testing"));
            self.view.redraw(cx);

            // Send a test translation request
            let test_config = TranslationConfig {
                enabled: true,
                api_base_url: api_url,
                api_key,
                model: if model.is_empty() { "qwen3-4b".to_string() } else { model },
            };

            let url = format!(
                "{}/v1/chat/completions",
                test_config.api_base_url.trim().trim_end_matches('/')
            );
            log!("Test connection URL: '{}', model: '{}'", url, test_config.model);
            let body = format!(
                r#"{{"model":"{}","messages":[{{"role":"user","content":"Say OK"}}],"temperature":0.1,"max_tokens":10}}"#,
                test_config.model,
            );
            let mut req = HttpRequest::new(url, HttpMethod::POST);
            req.set_header("Content-Type".into(), "application/json".into());
            if !test_config.api_key.is_empty() {
                req.set_header("Authorization".into(), format!("Bearer {}", test_config.api_key));
            }
            req.set_body(body.into_bytes());
            cx.http_request(TEST_TRANSLATION_REQUEST_ID, req);
        }
    }
}

impl TranslationSettings {
    fn set_app_language(&mut self, cx: &mut Cx, app_language: AppLanguage) {
        self.app_language = app_language;
        self.app_language_initialized = true;
        self.sync_app_language(cx);
    }

    fn sync_app_language(&mut self, cx: &mut Cx) {
        self.view
            .label(cx, ids!(translation_title))
            .set_text(cx, tr_key(self.app_language, "settings.labs.translation.title"));
        self.view
            .label(cx, ids!(description))
            .set_text(cx, tr_key(self.app_language, "settings.labs.translation.description"));
        self.view
            .label(cx, ids!(api_url_label))
            .set_text(cx, tr_key(self.app_language, "settings.labs.translation.field.api_url"));
        self.view
            .label(cx, ids!(api_key_label))
            .set_text(cx, tr_key(self.app_language, "settings.labs.translation.field.api_key"));
        self.view
            .label(cx, ids!(model_label))
            .set_text(cx, tr_key(self.app_language, "settings.labs.translation.field.model"));
        self.view
            .button(cx, ids!(save_button))
            .set_text(cx, tr_key(self.app_language, "settings.labs.translation.button.save"));
        self.view
            .button(cx, ids!(test_button))
            .set_text(cx, tr_key(self.app_language, "settings.labs.translation.button.test_connection"));
        self.set_switch_state_label(
            cx,
            self.view.check_box(cx, ids!(translation_switch)).active(cx),
        );
        self.view.redraw(cx);
    }

    fn set_switch_state_label(&mut self, cx: &mut Cx, enabled: bool) {
        let mut switch_state_label = self.view.label(cx, ids!(switch_state_label));
        if enabled {
            script_apply_eval!(cx, switch_state_label, {
                text: #(tr_key(self.app_language, "settings.labs.translation.status.enabled")),
                draw_text +: {
                    color: mod.widgets.COLOR_ACTIVE_PRIMARY
                }
            });
        } else {
            script_apply_eval!(cx, switch_state_label, {
                text: #(tr_key(self.app_language, "settings.labs.translation.status.disabled")),
                draw_text +: {
                    color: #999
                }
            });
        }
    }

    fn sync_ui(&mut self, cx: &mut Cx, config: &TranslationConfig) {
        self.view.view(cx, ids!(config_section))
            .set_visible(cx, config.enabled);

        self.view.check_box(cx, ids!(translation_switch))
            .set_active(cx, config.enabled, Animate::No);
        self.set_switch_state_label(cx, config.enabled);
    }

    /// Populates the translation settings UI from the current app state.
    pub fn populate(&mut self, cx: &mut Cx, config: &TranslationConfig) {
        translation::set_global_config(config);
        self.sync_ui(cx, config);

        self.view.text_input(cx, ids!(api_url_input))
            .set_text(cx, &config.api_base_url);
        self.view.text_input(cx, ids!(api_key_input))
            .set_text(cx, &config.api_key);
        self.view.text_input(cx, ids!(model_input))
            .set_text(cx, &config.model);
    }
}

impl TranslationSettingsRef {
    pub fn populate(&self, cx: &mut Cx, config: &TranslationConfig) {
        let Some(mut inner) = self.borrow_mut() else {
            return;
        };
        inner.populate(cx, config);
    }

    pub fn set_app_language(&self, cx: &mut Cx, app_language: AppLanguage) {
        let Some(mut inner) = self.borrow_mut() else {
            return;
        };
        inner.set_app_language(cx, app_language);
    }
}

fn persist_translation_config(app_state: &AppState) {
    if let Some(user_id) = current_user_id() {
        if let Err(e) = persistence::save_app_state(app_state.clone(), user_id) {
            error!("Failed to persist translation settings. Error: {e}");
        }
    }
}
