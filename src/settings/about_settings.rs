//! An "About Robrix" section within the SettingsScreen.
//!
//! Shows the app version and a set of external links (e.g., privacy policy).

use makepad_widgets::*;

use crate::shared::popup_list::{enqueue_popup_notification, PopupKind};

const PRIVACY_POLICY_URL: &str = "https://robrix.app/privacy/";
const HOMEPAGE_URL: &str = "https://robrix.app";
const SOURCE_URL: &str = "https://github.com/project-robius/robrix";
const ISSUES_URL: &str = "https://github.com/project-robius/robrix/issues";

const ROBRIX_VERSION: &str = env!("CARGO_PKG_VERSION");
const ROBRIX_GIT_COMMIT_HASH: &str = env!("ROBRIX_GIT_COMMIT_HASH");
const ROBRIX_GIT_COMMIT_URL: &str = env!("ROBRIX_GIT_COMMIT_URL");
const MATRIX_SDK_VERSION: &str = env!("MATRIX_SDK_VERSION");
const MATRIX_SDK_GIT_REV: &str = env!("MATRIX_SDK_GIT_REV");
const TESTFLIGHT_BUILD_NUMBER: &str = env!("TESTFLIGHT_BUILD_NUMBER");
const MATRIX_SDK_URL: &str = env!("MATRIX_SDK_URL");

const ROBRIX_PREFIX: &str = "Robrix: ";
const TESTFLIGHT_PREFIX: &str = "TestFlight build: ";
const SDK_PREFIX: &str = "Matrix Rust SDK: ";


script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.SmallCopyButton = RobrixNeutralIconButton {
        enable_long_press: true,
        margin: 0,
        padding: 5,
        spacing: 0,
        draw_icon.svg: (ICON_COPY)
        icon_walk: Walk{width: 13, height: 13, margin: 0}
    }

    mod.widgets.VersionHtml = mod.widgets.MessageHtml {
        width: Fill, height: Fit
        font_size: 11.5
        margin: Inset {top: 4}
        body: ""
    }

    // The About / Help section: version info and external links.
    mod.widgets.AboutSettings = #(AboutSettings::register_widget(vm)) {
        width: Fill, height: Fit
        flow: Down

        TitleLabel {
            text: "About Robrix"
        }

        SubsectionLabel {
            text: "Version Info"
        }

        robrix_version_row := View {
            width: Fill, height: Fit
            flow: Right,
            spacing: 10
            margin: Inset{top: 3, bottom: 3, left: 10}

            copy_robrix_version_button := mod.widgets.SmallCopyButton {}
            robrix_version_html := mod.widgets.VersionHtml {}
        }

        testflight_row := View {
            visible: false,
            width: Fill, height: Fit
            flow: Right,
            spacing: 10
            margin: Inset{top: 3, bottom: 3, left: 10}

            copy_testflight_button := mod.widgets.SmallCopyButton {}
            testflight_html := mod.widgets.VersionHtml {}
        }

        sdk_version_row := View {
            width: Fill, height: Fit
            flow: Right,
            spacing: 10
            margin: Inset{top: 3, bottom: 3, left: 10}

            copy_sdk_version_button := mod.widgets.SmallCopyButton {}
            sdk_version_html := mod.widgets.VersionHtml {}
        }


        SubsectionLabel {
            text: "Privacy & Legal"
        }

        privacy_policy_button := RobrixIconButton {
            height: mod.widgets.SETTINGS_BUTTON_HEIGHT,
            padding: Inset{left: 12, right: 15}
            margin: Inset{left: 5, top: 5, bottom: 5}
            draw_icon.svg: (ICON_EXTERNAL_LINK)
            icon_walk: Walk{width: 16, height: 16}
            text: "View Privacy Policy"
        }


        SubsectionLabel {
            text: "Project & Support"
        }

        View {
            width: Fill, height: Fit
            flow: Flow.Right{wrap: true},
            align: Align{y: 0.5},
            spacing: 10,
            wrap_spacing: 2

            homepage_button := RobrixIconButton {
                height: mod.widgets.SETTINGS_BUTTON_HEIGHT,
                padding: Inset{left: 12, right: 15}
                margin: Inset{left: 5, top: 5, bottom: 5}
                draw_icon.svg: (ICON_EXTERNAL_LINK)
                icon_walk: Walk{width: 16, height: 16}
                text: "Robrix Homepage"
            }

            source_button := RobrixIconButton {
                height: mod.widgets.SETTINGS_BUTTON_HEIGHT,
                padding: Inset{left: 12, right: 15}
                margin: Inset{left: 5, top: 5, bottom: 5}
                draw_icon.svg: (ICON_EXTERNAL_LINK)
                icon_walk: Walk{width: 16, height: 16}
                text: "Source Code (GitHub)"
            }

            issues_button := RobrixNegativeIconButton {
                height: mod.widgets.SETTINGS_BUTTON_HEIGHT,
                padding: Inset{left: 12, right: 15}
                margin: Inset{left: 5, top: 5, bottom: 5}
                draw_icon.svg: (ICON_EXTERNAL_LINK)
                icon_walk: Walk{width: 16, height: 16}
                text: "Report an Issue"
            }
        }
    }
}


/// The About / Help section of the SettingsScreen.
#[derive(Script, Widget)]
pub struct AboutSettings {
    #[deref] view: View,
}

impl ScriptHook for AboutSettings {
    fn on_after_apply(
        &mut self,
        vm: &mut ScriptVm,
        _apply: &Apply,
        _scope: &mut Scope,
        _value: ScriptValue,
    ) {
        let cx = vm.cx_mut();
        Self::populate_text(cx, &self.view);
    }
}

impl Widget for AboutSettings {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if let Event::Actions(actions) = event {
            for action in actions {
                if let HtmlLinkAction::Clicked { url, .. } = action.as_widget_action().cast() {
                    if !url.is_empty() {
                        open_url(&url);
                    }
                }
            }
            self.handle_actions(cx, actions);
        }

        // Long-press / hover tooltips per copy button, same pattern as
        // the user-id copy button in account_settings.rs.
        let robrix_btn = self.view.button(cx, ids!(copy_robrix_version_button));
        Self::handle_copy_tooltip(cx, event, &robrix_btn, "Copy Robrix version");
        if !TESTFLIGHT_BUILD_NUMBER.is_empty() {
            let testflight_btn = self.view.button(cx, ids!(copy_testflight_button));
            Self::handle_copy_tooltip(cx, event, &testflight_btn, "Copy TestFlight build");
        }
        let sdk_btn = self.view.button(cx, ids!(copy_sdk_version_button));
        Self::handle_copy_tooltip(cx, event, &sdk_btn, "Copy Matrix SDK version");

        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl AboutSettings {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        if self.view.button(cx, ids!(copy_robrix_version_button)).clicked(actions) {
            Self::copy_with_toast(cx, &robrix_plaintext(), "Copied Robrix version.");
        }
        if !TESTFLIGHT_BUILD_NUMBER.is_empty()
            && self.view.button(cx, ids!(copy_testflight_button)).clicked(actions)
        {
            Self::copy_with_toast(cx, &testflight_plaintext(), "Copied TestFlight build ID.");
        }
        if self.view.button(cx, ids!(copy_sdk_version_button)).clicked(actions) {
            Self::copy_with_toast(cx, &sdk_plaintext(), "Copied Matrix SDK version.");
        }

        if self.view.button(cx, ids!(privacy_policy_button)).clicked(actions) {
            open_url(PRIVACY_POLICY_URL);
        }
        if self.view.button(cx, ids!(homepage_button)).clicked(actions) {
            open_url(HOMEPAGE_URL);
        }
        if self.view.button(cx, ids!(source_button)).clicked(actions) {
            open_url(SOURCE_URL);
        }
        if self.view.button(cx, ids!(issues_button)).clicked(actions) {
            open_url(ISSUES_URL);
        }
    }

    fn populate_text(cx: &mut Cx, view: &View) {
        view.html(cx, ids!(robrix_version_html))
            .set_text(cx, &robrix_html());
        view.html(cx, ids!(sdk_version_html))
            .set_text(cx, &sdk_html());

        // Only show the TestFlight row if testflight env var is set.
        let has_testflight = !TESTFLIGHT_BUILD_NUMBER.is_empty();
        view.view(cx, ids!(testflight_row)).set_visible(cx, has_testflight);
        if has_testflight {
            view.html(cx, ids!(testflight_html))
                .set_text(cx, &testflight_html());
        }
    }

    fn copy_with_toast(cx: &mut Cx, payload: &str, success_msg: &str) {
        cx.copy_to_clipboard(payload);
        enqueue_popup_notification(success_msg.to_string(), PopupKind::Success, Some(3.0));
    }

    fn handle_copy_tooltip(cx: &mut Cx, event: &Event, button: &ButtonRef, tooltip_text: &str) {
        let area = button.area();
        match event.hits(cx, area) {
            Hit::FingerHoverIn(_) | Hit::FingerLongPress(_) => {
                cx.widget_action(
                    button.widget_uid(),
                    TooltipAction::HoverIn {
                        text: tooltip_text.to_string(),
                        widget_rect: area.rect(cx),
                        options: CalloutTooltipOptions {
                            position: TooltipPosition::Top,
                            ..Default::default()
                        },
                    },
                );
            }
            Hit::FingerHoverOut(_) => {
                cx.widget_action(button.widget_uid(), TooltipAction::HoverOut);
            }
            _ => {}
        }
    }
}

fn robrix_html() -> String {
    let value = robrix_plaintext();
    if ROBRIX_GIT_COMMIT_URL.is_empty() {
        format!("<b>{ROBRIX_PREFIX}</b>{value}")
    } else {
        format!("<b>{ROBRIX_PREFIX}</b><a href=\"{ROBRIX_GIT_COMMIT_URL}\">{value}</a>")
    }
}

fn robrix_plaintext() -> String {
    if ROBRIX_GIT_COMMIT_HASH.is_empty() {
        format!("v{ROBRIX_VERSION}")
    } else {
        format!("v{ROBRIX_VERSION} ({ROBRIX_GIT_COMMIT_HASH})")
    }
}

fn testflight_html() -> String {
    format!("<b>{TESTFLIGHT_PREFIX}</b>{}", testflight_plaintext())
}

fn testflight_plaintext() -> String {
    TESTFLIGHT_BUILD_NUMBER.to_string()
}

fn sdk_html() -> String {
    let value = sdk_plaintext();
    if MATRIX_SDK_URL.is_empty() {
        format!("<b>{SDK_PREFIX}</b>{value}")
    } else {
        format!("<b>{SDK_PREFIX}</b><a href=\"{MATRIX_SDK_URL}\">{value}</a>")
    }
}

fn sdk_plaintext() -> String {
    sdk_value_text()
}

fn sdk_value_text() -> String {
    if MATRIX_SDK_GIT_REV.is_empty() {
        format!("v{MATRIX_SDK_VERSION}")
    } else {
        format!("v{MATRIX_SDK_VERSION} ({MATRIX_SDK_GIT_REV})")
    }
}

fn open_url(url: &str) {
    log!("Opening URL \"{}\"", url);
    if let Err(e) = robius_open::Uri::new(url).open() {
        error!("Failed to open URL {:?}. Error: {:?}", url, e);
        enqueue_popup_notification(
            format!("Could not open URL: {url}"),
            PopupKind::Error,
            Some(10.0),
        );
    }
}
