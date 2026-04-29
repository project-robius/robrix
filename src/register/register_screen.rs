//! RegisterScreen widget: homeserver picker + capability display.
//!
//! Phase 1 renders:
//!   - Back button (returns to login)
//!   - Screen title
//!   - Homeserver URL input
//!   - Next button (triggers capability discovery)
//!   - Three-state status area (MAS / UIAA / Disabled / errors)
//!
//! Phases 2-5 fill in OIDC launch / UIAA form / SSO buttons.

use makepad_widgets::*;

use crate::register::{HsCapabilities, RegisterAction, RegisterMode};
use crate::register::validation::{normalize_homeserver_url, HomeserverUrlError};
use crate::sliding_sync::{submit_async_request, MatrixRequest};

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.RegisterScreen = set_type_default() do #(RegisterScreen::register_widget(vm)) {
        ..mod.widgets.SolidView

        width: Fill, height: Fill,
        flow: Overlay
        align: Align{x: 0.5, y: 0.5}
        show_bg: true,
        draw_bg +: {
            color: COLOR_SECONDARY
        }

        ScrollYView {
            width: Fill,
            height: Fill,
            flow: Down,
            align: Align{x: 0.5, y: 0.5}
            show_bg: true,
            draw_bg.color: (COLOR_SECONDARY)

            scroll_bars: {
                show_scroll_x: false,
                show_scroll_y: true,
                scroll_bar_y: {
                    bar_size: 0.0
                    min_handle_size: 0.0
                    drag_scrolling: true
                }
            }

            RoundedView {
                margin: Inset{top: 50, bottom: 50}
                width: Fill,
                height: Fit,
                align: Align{x: 0.5, y: 0.5}
                flow: Overlay

                View {
                    width: Fill,
                    height: Fit,
                    flow: Down,
                    align: Align{x: 0.5, y: 0.5}
                    spacing: 15.0

                    logo_image := Image {
                        fit: ImageFit.Smallest,
                        width: 80
                        src: (mod.widgets.IMG_APP_LOGO),
                    }

                    title := Label {
                        width: Fit,
                        height: Fit,
                        margin: Inset{bottom: 5}
                        padding: 0,
                        draw_text +: {
                            color: (COLOR_TEXT)
                            text_style: TITLE_TEXT {font_size: 16.0}
                        }
                        text: "Create Account"
                    }

                    View {
                        width: 275,
                        height: Fit,
                        flow: Down,

                        homeserver_input := RobrixTextInput {
                            width: 275,
                            height: Fit,
                            flow: Right,
                            padding: Inset{top: 10, bottom: 10, left: 10, right: 10}
                            empty_text: "matrix.org"
                        }

                        View {
                            width: 275,
                            height: Fit,
                            flow: Right,
                            padding: Inset{top: 3, left: 2, right: 2}
                            spacing: 0.0,
                            align: Align{x: 0.5, y: 0.5}

                            LineH { draw_bg.color: #C8C8C8 }

                            homeserver_hint_label := Label {
                                width: Fit,
                                height: Fit,
                                padding: 0,
                                draw_text +: {
                                    color: #8C8C8C
                                    text_style: REGULAR_TEXT {font_size: 9}
                                }
                                text: "Homeserver URL"
                            }

                            LineH { draw_bg.color: #C8C8C8 }
                        }
                    }

                    next_button := RobrixIconButton {
                        width: 275,
                        height: 40
                        padding: 10
                        margin: Inset{top: 5, bottom: 10}
                        align: Align{x: 0.5, y: 0.5}
                        text: "Next"
                    }

                    status_area := View {
                        width: 275,
                        height: Fit,
                        flow: Down,
                        visible: false
                        padding: Inset{top: 2, bottom: 2, left: 4, right: 4}

                        status_label := Label {
                            width: Fill,
                            height: Fit,
                            draw_text +: {
                                color: (COLOR_TEXT)
                                text_style: REGULAR_TEXT {font_size: 10.5}
                            }
                            text: ""
                        }
                    }

                    LineH {
                        width: 275
                        margin: Inset{bottom: -5}
                        draw_bg.color: #C8C8C8
                    }

                    View {
                        width: 275,
                        height: Fit,
                        flow: Right,
                        spacing: 0.0,
                        align: Align{x: 0.5, y: 0.5}

                        LineH { draw_bg.color: #C8C8C8 }

                        account_prompt_label := Label {
                            width: Fit,
                            height: Fit,
                            padding: Inset{left: 1, right: 1, top: 0, bottom: 0}
                            draw_text +: {
                                color: #x6c6c6c
                                text_style: REGULAR_TEXT {}
                            }
                            text: "Already have an account?"
                        }

                        LineH { draw_bg.color: #C8C8C8 }
                    }

                    back_button := RobrixIconButton {
                        width: Fit,
                        height: Fit,
                        padding: Inset{left: 15, right: 15, top: 10, bottom: 10}
                        margin: Inset{bottom: 5}
                        align: Align{x: 0.5, y: 0.5}
                        text: "← Back to Login"
                    }
                }
            }
        }
    }
}

#[derive(Script, ScriptHook, Widget)]
pub struct RegisterScreen {
    #[deref] view: View,
    #[rust] last_discovery: Option<HsCapabilities>,
}

impl Widget for RegisterScreen {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        self.widget_match_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl WidgetMatchEvent for RegisterScreen {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions, _scope: &mut Scope) {
        let back = self.view.button(cx, ids!(back_button));
        let next = self.view.button(cx, ids!(next_button));

        if back.clicked(actions) {
            Cx::post_action(RegisterAction::NavigateToLogin);
            return;
        }

        if next.clicked(actions) {
            let raw = self.view.text_input(cx, ids!(homeserver_input)).text();
            match normalize_homeserver_url(&raw) {
                Ok(url) => {
                    self.show_status(cx, "Checking server capabilities...");
                    submit_async_request(MatrixRequest::DiscoverHomeserverCapabilities { url });
                }
                Err(HomeserverUrlError::Empty) => {
                    self.show_status(cx, "Please enter a homeserver URL (e.g. matrix.org).");
                }
                Err(HomeserverUrlError::UnsupportedScheme(s)) => {
                    self.show_status(cx, &format!("Unsupported scheme: {s}. Only http(s) is allowed."));
                }
                Err(HomeserverUrlError::Invalid) => {
                    self.show_status(cx, "That URL looks invalid. Please check and try again.");
                }
            }
        }

        // Capability discovery results.
        for action in actions {
            match action.downcast_ref::<RegisterAction>() {
                Some(RegisterAction::CapabilitiesDiscovered(caps)) => {
                    match caps.mode() {
                        RegisterMode::MasWebOnly => {
                            match caps.mas_signup_url.as_deref() {
                                Some(url) => match robius_open::Uri::new(url).open() {
                                    Ok(()) => {
                                        self.show_status(
                                            cx,
                                            "Browser opened. Complete registration in your web browser, \
                                             then click ← Back to Login and sign in with your new account.",
                                        );
                                    }
                                    Err(e) => {
                                        log!("robius_open failed for MAS signup url {url}: {e:?}");
                                        self.show_status(
                                            cx,
                                            &format!(
                                                "Could not open the browser automatically. Please visit this URL manually:\n{url}"
                                            ),
                                        );
                                    }
                                },
                                None => {
                                    self.show_status(
                                        cx,
                                        "This server advertises browser-based registration but no signup URL was found.",
                                    );
                                }
                            }
                        }
                        RegisterMode::Uiaa => {
                            self.show_status(
                                cx,
                                "This server allows direct account creation. Phase 3 will handle the form.",
                            );
                        }
                        RegisterMode::Disabled => {
                            self.show_status(
                                cx,
                                "This server does not allow registration. Please choose a different homeserver \
                                 or sign in with an existing account.",
                            );
                        }
                    }
                    self.last_discovery = Some(caps.clone());
                }
                Some(RegisterAction::DiscoveryFailed(err)) => {
                    self.show_status(cx, &format!("Could not reach that server: {err}"));
                    self.last_discovery = None;
                }
                _ => {}
            }
        }
    }
}

impl RegisterScreen {
    fn show_status(&mut self, cx: &mut Cx, message: &str) {
        self.view.view(cx, ids!(status_area)).set_visible(cx, true);
        self.view.label(cx, ids!(status_label)).set_text(cx, message);
        self.view.redraw(cx);
    }
}
