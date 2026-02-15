//! A modal dialog for creating a new TSP Decentralized Identity (DID).

use makepad_widgets::*;

use crate::{shared::styles::*, tsp};

live_design! {
    link tsp_enabled

    use link::theme::*;
    use link::widgets::*;

    use crate::shared::styles::*;
    use crate::shared::helpers::*;
    use crate::shared::icon_button::RobrixIconButton;

    pub CreateDidModal = {{CreateDidModal}} {
        width: Fit
        height: Fit

        <RoundedView> {
            width: 400
            height: Fit
            align: {x: 0.5}
            flow: Down
            padding: {top: 30, right: 25, bottom: 20, left: 25}

            show_bg: true
            draw_bg: {
                color: (COLOR_PRIMARY)
                border_radius: 4.0
            }

            title_view = <View> {
                width: Fill,
                height: Fit,
                padding: {top: 0, bottom: 25}
                align: {x: 0.5, y: 0.0}

                title = <Label> {
                    flow: RightWrap,
                    draw_text: {
                        text_style: <TITLE_TEXT>{font_size: 13},
                        color: #000
                        wrap: Word
                    }
                    text: "Create New Identity (DID)"
                }
            }

            <RoundedView> {
                width: 350,
                height: Fit,
                spacing: 15,
                padding: 15,
                align: {x: 0.5}
                flow: Down,

                show_bg: true
                draw_bg: {
                    color: (COLOR_SECONDARY)
                    border_radius: 4.0
                }

                username_input = <RobrixTextInput> {
                    width: Fill,
                    height: Fit,
                    padding: 10,
                    draw_text: {
                        text_style: <REGULAR_TEXT>{font_size: 12},
                        color: #000
                    }
                    empty_text: "Identity Username",
                }

                alias_input = <RobrixTextInput> {
                    width: Fill,
                    height: Fit,
                    padding: 10,
                    draw_text: {
                        text_style: <REGULAR_TEXT>{font_size: 12},
                        color: #000
                    }
                    empty_text: "Enter an alias (optional)",
                }

                did_type_radio_buttons = <View> {
                    spacing: 20,
                    width: Fit, height: Fit,
                    did_web   = <RadioButtonFlat> {
                        text: "Web"
                        animator: { active = { default: on } }
                    }
                    did_webvh = <RadioButtonFlat> {
                        text: "WebVH"
                        animator: { disabled = { default: on } }
                    }
                    did_peer  = <RadioButtonFlat> {
                        text: "Peer",
                        animator: { disabled = { default: on } }
                    }
                }

                <View> {
                    width: Fill, height: Fit
                    flow: Down

                    server_input = <RobrixTextInput> {
                        width: Fill, height: Fit,
                        flow: Right, // do not wrap
                        padding: {top: 3, bottom: 3}
                        empty_text: "p.teaspoon.world",
                        draw_text: {
                            text_style: <REGULAR_TEXT>{font_size: 10.0}
                        }
                    }

                    <View> {
                        width: Fill,
                        height: Fit,
                        flow: Right,
                        padding: {top: 5, left: 2, right: 2, bottom: 2}
                        spacing: 0.0,
                        align: {x: 0.5, y: 0.5} // center horizontally and vertically

                        left_line = <LineH> {
                            draw_bg: { color: #C8C8C8 }
                        }

                        <Label> {
                            width: Fit, height: Fit
                            padding:  0
                            draw_text: {
                                color: #777777
                                text_style: <REGULAR_TEXT>{font_size: 9}
                            }
                            text: "Intermediary server domain"
                        }

                        right_line = <LineH> {
                            draw_bg: { color: #C8C8C8 }
                        }
                    }
                }

                <View> {
                    width: Fill, height: Fit
                    flow: Down

                    did_server_input = <RobrixTextInput> {
                        width: Fill, height: Fit,
                        flow: Right, // do not wrap
                        padding: {top: 3, bottom: 3}
                        empty_text: "did.teaspoon.world",
                        draw_text: {
                            text_style: <REGULAR_TEXT>{font_size: 10.0}
                        }
                    }

                    <View> {
                        width: Fill,
                        height: Fit,
                        flow: Right,
                        padding: {top: 5, left: 2, right: 2, bottom: 2}
                        spacing: 0.0,
                        align: {x: 0.5, y: 0.5} // center horizontally and vertically

                        left_line = <LineH> {
                            draw_bg: { color: #C8C8C8 }
                        }

                        <Label> {
                            width: Fit, height: Fit
                            padding: 0
                            draw_text: {
                                color: #777777
                                text_style: <REGULAR_TEXT>{font_size: 9}
                            }
                            text: "DID server domain"
                        }

                        right_line = <LineH> {
                            draw_bg: { color: #C8C8C8 }
                        }
                    }
                }
            }

            <View> {
                width: Fill, height: Fit
                flow: Right,
                padding: {top: 20, bottom: 20}
                align: {x: 1.0, y: 0.5}
                spacing: 20

                cancel_button = <RobrixIconButton> {
                    width: 100,
                    align: {x: 0.5, y: 0.5}
                    padding: 15,
                    draw_icon: {
                        svg_file: (ICON_FORBIDDEN)
                        color: (COLOR_FG_DANGER_RED),
                    }
                    icon_walk: {width: 16, height: 16, margin: {left: -2, right: -1} }

                    draw_bg: {
                        border_color: (COLOR_FG_DANGER_RED),
                        color: (COLOR_BG_DANGER_RED)
                    }
                    text: "Cancel"
                    draw_text:{
                        color: (COLOR_FG_DANGER_RED),
                    }
                }

                accept_button = <RobrixIconButton> {
                    width: 140
                    align: {x: 0.5, y: 0.5}
                    padding: 15,
                    draw_icon: {
                        svg_file: (ICON_CHECKMARK)
                        color: (COLOR_FG_ACCEPT_GREEN),
                    }
                    icon_walk: {width: 16, height: 16, margin: {left: -2, right: -1} }

                    draw_bg: {
                        border_color: (COLOR_FG_ACCEPT_GREEN),
                        color: (COLOR_BG_ACCEPT_GREEN)
                    }
                    text: "Create DID"
                    draw_text:{
                        color: (COLOR_FG_ACCEPT_GREEN),
                    }
                }
            }

            status_label = <Label> {
                width: Fill,
                height: Fit,
                padding: 0,
                margin: 0,
                flow: RightWrap,
                align: {x: 0.5, y: 0.0}
                draw_text: {
                    wrap: Word
                    text_style: <REGULAR_TEXT>{font_size: 11},
                    color: #000
                }
                text: "status label"
            }
        }
    }
}

/// Actions emitted by other widgets to instruct the main settings screen
/// to open or close the `CreateDidModal`.
#[derive(Clone, Copy, Debug)]
pub enum CreateDidModalAction {
    /// The settings screen should open the modal.
    Open,
    /// The settings screen should close the modal.
    Close,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum CreateDidModalState {
    /// Waiting for the user to enter identity details.
    #[default]
    WaitingForUserInput,
    /// Waiting for the identity to be created.
    WaitingForIdentityCreation,
    /// The identity was created successfully.
    IdentityCreated,
    /// An error occurred while creating the identity.
    IdentityCreationError,
}

#[derive(Live, LiveHook, Widget)]
pub struct CreateDidModal {
    #[deref]
    view: View,
    #[rust]
    state: CreateDidModalState,
    #[rust]
    is_showing_error: bool,
}

impl Widget for CreateDidModal {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        self.widget_match_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl WidgetMatchEvent for CreateDidModal {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions, _scope: &mut Scope) {
        let accept_button = self.view.button(ids!(accept_button));
        let cancel_button = self.view.button(ids!(cancel_button));

        // Handle canceling/closing the modal.
        let cancel_clicked = cancel_button.clicked(actions);
        if cancel_clicked
            || actions
                .iter()
                .any(|a| matches!(a.downcast_ref(), Some(ModalAction::Dismissed)))
        {
            // If the modal was dismissed by clicking outside of it, we MUST NOT emit
            // a `CreateDidModalAction::Close` action, as that would cause
            // an infinite action feedback loop.
            if cancel_clicked {
                cx.action(CreateDidModalAction::Close);
            }

            // TODO: if possible, cancel the wallet creation request if it's still pending.

            return;
        }

        let username_input = self.view.text_input(ids!(username_input));
        let alias_input = self.view.text_input(ids!(alias_input));
        let server_input = self.view.text_input(ids!(server_input));
        let did_server_input = self.view.text_input(ids!(did_server_input));
        let status_label = self.view.label(ids!(status_label));

        // Handle clicking the accept button.
        let mut needs_redraw = false;
        if accept_button.clicked(actions) {
            match self.state {
                // If the modal is in the "final" state, just close the modal.
                CreateDidModalState::IdentityCreated => {
                    self.state = CreateDidModalState::WaitingForUserInput;
                    cx.action(CreateDidModalAction::Close);
                }

                CreateDidModalState::WaitingForUserInput => {
                    let username_full = username_input.text();
                    let username = username_full.trim();

                    // Check to ensure that the user has entered all required fields.
                    if username.is_empty() {
                        self.is_showing_error = true;
                        status_label.apply_over(
                            cx,
                            live!(
                                text: "Please enter a DID username.",
                                draw_text: {
                                    color: (COLOR_FG_DANGER_RED),
                                },
                            ),
                        );
                    } else {
                        let alias = match alias_input.text().trim() {
                            "" => None,
                            non_empty => Some(non_empty.to_string()),
                        };
                        let server = match server_input.text().trim() {
                            "" => server_input.empty_text(),
                            non_empty => non_empty.to_string(),
                        };
                        let did_server = match did_server_input.text().trim() {
                            "" => did_server_input.empty_text(),
                            non_empty => non_empty.to_string(),
                        };

                        // Submit the identity creation request to the TSP async worker thread.
                        tsp::submit_tsp_request(tsp::TspRequest::CreateDid {
                            username: username.to_string(),
                            alias,
                            server,
                            did_server,
                        });

                        self.state = CreateDidModalState::WaitingForIdentityCreation;
                        self.is_showing_error = false;
                        status_label.apply_over(
                            cx,
                            live!(
                                text: "Waiting for identity to be created and published...",
                                draw_text: {
                                    color: (COLOR_ACTIVE_PRIMARY_DARKER),
                                },
                            ),
                        );
                        accept_button.set_enabled(cx, false);
                        cancel_button.set_enabled(cx, false); // TODO: support canceling the identity creation request?
                        username_input.set_is_read_only(cx, true);
                        alias_input.set_is_read_only(cx, true);
                        server_input.set_is_read_only(cx, true);
                        did_server_input.set_is_read_only(cx, true);
                    }

                    needs_redraw = true;
                }

                _ => {}
            }
        }

        // If the user changes any of the input fields, clear the error message
        // and reset the accept button to its default state.
        if self.is_showing_error {
            if username_input.changed(actions).is_some()
                || alias_input.changed(actions).is_some()
                || server_input.changed(actions).is_some()
                || did_server_input.changed(actions).is_some()
            {
                self.is_showing_error = false;
                self.view.label(ids!(status_label)).set_text(cx, "");
                self.state = CreateDidModalState::WaitingForUserInput;
                accept_button.apply_over(
                    cx,
                    live!(
                        text: "Create DID",
                        enabled: true,
                        draw_text: {
                            color: (COLOR_FG_ACCEPT_GREEN),
                        },
                    ),
                );
                needs_redraw = true;
            }
        }

        for action in actions {
            match action.downcast_ref() {
                Some(tsp::TspIdentityAction::DidCreationResult(Ok(did))) => {
                    self.state = CreateDidModalState::IdentityCreated;
                    self.is_showing_error = false;
                    let message = format!("Successfully created and published DID: \"{}\"", did);
                    status_label.apply_over(
                        cx,
                        live!(
                            text: (message),
                            draw_text: {
                                color: (COLOR_FG_ACCEPT_GREEN),
                            },
                        ),
                    );
                    accept_button.apply_over(
                        cx,
                        live!(
                            enabled: true,
                            text: "Okay",
                            draw_bg: {
                                color: (COLOR_ACTIVE_PRIMARY),
                            },
                            draw_icon: {
                                color: (COLOR_PRIMARY),
                            }
                            draw_text: {
                                color: (COLOR_PRIMARY),
                            },
                        ),
                    );
                    cancel_button.set_visible(cx, false);
                    needs_redraw = true;
                }

                // Upon an error, update the status label and disable the accept button.
                // Re-enable the input fields so the user can change the input values to try again.
                Some(tsp::TspIdentityAction::DidCreationResult(Err(e))) => {
                    self.state = CreateDidModalState::IdentityCreationError;
                    self.is_showing_error = true;
                    let message = format!("Failed to create DID: {e}");
                    status_label.apply_over(
                        cx,
                        live!(
                            text: (message),
                            draw_text: {
                                color: (COLOR_FG_DANGER_RED),
                            },
                        ),
                    );
                    accept_button.set_enabled(cx, false);
                    cancel_button.set_enabled(cx, true);
                    username_input.set_is_read_only(cx, false);
                    alias_input.set_is_read_only(cx, false);
                    server_input.set_is_read_only(cx, false);
                    did_server_input.set_is_read_only(cx, false);
                    needs_redraw = true;
                }

                _ => {}
            }
        }

        if needs_redraw {
            self.view.redraw(cx);
        }
    }
}

impl CreateDidModal {
    pub fn show(&mut self, cx: &mut Cx) {
        self.state = CreateDidModalState::WaitingForUserInput;
        let accept_button = self.view.button(ids!(accept_button));
        let cancel_button = self.view.button(ids!(cancel_button));
        accept_button.set_text(cx, "Create DID");
        cancel_button.set_text(cx, "Cancel");
        accept_button.reset_hover(cx);
        cancel_button.reset_hover(cx);
        accept_button.set_enabled(cx, true);
        cancel_button.set_enabled(cx, true);
        accept_button.set_visible(cx, true);
        cancel_button.set_visible(cx, true);
        // TODO: return buttons to their default state/appearance
        self.view
            .text_input(ids!(username_input))
            .set_is_read_only(cx, false);
        self.view
            .text_input(ids!(alias_input))
            .set_is_read_only(cx, false);
        self.view
            .text_input(ids!(server_input))
            .set_is_read_only(cx, false);
        self.view
            .text_input(ids!(did_server_input))
            .set_is_read_only(cx, false);
        self.view.label(ids!(status_label)).set_text(cx, "");
        self.is_showing_error = false;
        self.view.redraw(cx);
    }
}

impl CreateDidModalRef {
    pub fn show(&self, cx: &mut Cx) {
        let Some(mut inner) = self.borrow_mut() else {
            return;
        };
        inner.show(cx);
    }
}
