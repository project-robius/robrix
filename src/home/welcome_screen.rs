use makepad_widgets::*;
use crate::{app::AppState, i18n::{AppLanguage, tr_key}};

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*


    mod.widgets.WELCOME_TEXT_COLOR = #x4

    mod.widgets.WelcomeScreen = #(WelcomeScreen::register_widget(vm)) {
        width: Fill, height: Fill
        align: Align{x: 0.5, y: 0.5}

        show_bg: true,
        draw_bg.color: (COLOR_PRIMARY)

        // make this a ScrollYView
        scroll_bars: mod.widgets.ScrollBars {
            show_scroll_x: false show_scroll_y: true
            scroll_bar_y.drag_scrolling: true
        }

        welcome_message := RoundedView {
            padding: 40.
            width: Fill, height: Fill
            flow: Down, spacing: 20
            align: Align{x: 0.5, y: 0.5}

            draw_bg.color: (COLOR_PRIMARY)

            title := Label {
                text: ""
                align: Align{x: 0.5, y: 0.5}
                draw_text +: {
                    color: (mod.widgets.WELCOME_TEXT_COLOR),
                    text_style: theme.font_bold {
                        font_size: 22.0
                    }
                }
            }

            // Using the HTML widget to taking advantage of embedding a link within text with proper vertical alignment
            body := MessageHtml {
                padding: Inset{top: 12, left: 0.}
                font_size: 14.
                font_color: (mod.widgets.WELCOME_TEXT_COLOR)
                text_style_normal: theme.font_regular { font_size: 14.0 }
                a: {
                    padding: Inset{left: 8., right: 8., top: 4., bottom: 5.},
                    // draw_text +: {
                    //     text_style: theme.font_bold {top_drop: 1.2, font_size: 11. },
                    //     color: #f,
                    //     color_pressed: #f00,
                    //     color_hover: #0f0,
                    // }
                }
                body:""
            }
        }
    }
}

#[derive(Script, ScriptHook, Widget)]
pub struct WelcomeScreen {
    #[deref] view: View,
    #[rust] app_language: AppLanguage,
    #[rust] app_language_initialized: bool,
}

impl Widget for WelcomeScreen {
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

impl WelcomeScreen {
    fn set_app_language(&mut self, cx: &mut Cx, app_language: AppLanguage) {
        self.app_language = app_language;
        self.app_language_initialized = true;
        self.view
            .label(cx, ids!(title))
            .set_text(cx, tr_key(self.app_language, "welcome_screen.title"));
        self.view
            .html(cx, ids!(body))
            .set_text(cx, tr_key(self.app_language, "welcome_screen.body_html"));
        self.view.redraw(cx);
    }
}
