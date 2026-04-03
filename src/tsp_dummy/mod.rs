//! This module provides dummy TSP-related widgets that do nothing.
//!
//! We only need to define dummy widgets for TSP-specific widgets that are used
//! from non-TSP DSL code, i.e., any widgets that exist on the boundary between
//! TSP and non-TSP code.
//!
//! The real TSP widgets are all defined in the `tsp_enabled` namespace,
//! and their live_design DSL blocks all start with `link tsp_enabled`,
//! which declares the namespace that they exist within.
//!
//! The "active" namespace is selected via the `cx.link()` call in `App::live_register()`,
//! which connects the `tsp_link` DSL namespace to the `tsp_disabled` namespace
//! defined in this module, only when the `tsp` feature is not enabled.
//!
//! This allows the rest of the application's DSL to directly use TSP widgets,
//! but the widgets that actually get imported under the `tsp_link` namespace
//! will be replaced with these dummy widgets when the `tsp` feature is not enabled.

use makepad_widgets::*;
use crate::{app::AppState, i18n::{AppLanguage, tr_key}};

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*


    mod.widgets.TspSettingsScreen = #(TspSettingsScreen::register_widget(vm)) {
        width: Fill, height: Fit
        flow: Down
        align: Align{x: 0}

        title := TitleLabel {
            text: ""
        }

        message := Label {
            width: Fill, height: Fit
            flow: Flow.Right{wrap: true},
            align: Align{x: 0}
            margin: Inset{top: 10, bottom: 10}
            draw_text +: {
                color: (MESSAGE_TEXT_COLOR),
                text_style: MESSAGE_TEXT_STYLE { font_size: 11 },
            }
            text: ""
        }
    }

    mod.widgets.CreateWalletModal = View {
        visible: false,
    }

    mod.widgets.CreateDidModal = View {
        visible: false,
    }

    mod.widgets.TspVerifyUser = View {
        height: 50
        width: Fill,
    }

    mod.widgets.TspVerificationModal = View {
        visible: false
    }

    mod.widgets.TspSignAnycastCheckbox = View {
        visible: false
    }

    mod.widgets.TspSignIndicator = View {
        visible: false
    }
}

#[derive(Script, ScriptHook, Widget)]
pub struct TspSettingsScreen {
    #[deref] view: View,
    #[rust] app_language: AppLanguage,
    #[rust] app_language_initialized: bool,
}

impl Widget for TspSettingsScreen {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        let app_language = scope.data.get::<AppState>()
            .map(|app_state| app_state.app_language)
            .unwrap_or_default();
        if !self.app_language_initialized || self.app_language != app_language {
            self.set_app_language(cx, app_language);
        }
        self.view.handle_event(cx, event, scope);
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

impl TspSettingsScreen {
    fn set_app_language(&mut self, cx: &mut Cx, app_language: AppLanguage) {
        self.app_language = app_language;
        self.app_language_initialized = true;
        self.view
            .label(cx, ids!(title))
            .set_text(cx, tr_key(self.app_language, "tsp.settings.title"));
        self.view
            .label(cx, ids!(message))
            .set_text(cx, tr_key(self.app_language, "tsp_dummy.message.disabled"));
        self.view.redraw(cx);
    }
}
