//! A modal dialog for creating a new TSP wallet.

use makepad_widgets::*;

use crate::{
    shared::styles::*,
    tsp::{self, TspWalletMetadata},
};

live_design! {
    link tsp_enabled

    use link::theme::*;
    use link::widgets::*;

    use crate::shared::styles::*;
    use crate::shared::helpers::*;
    use crate::shared::icon_button::RobrixIconButton;

    pub CreateWalletModal = {{CreateWalletModal}} {
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
                border_radius: 4
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
                    text: "Create New TSP Wallet"
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

                wallet_name_input = <RobrixTextInput> {
                    width: Fill,
                    height: Fit,
                    padding: 10,
                    draw_text: {
                        text_style: <REGULAR_TEXT>{font_size: 12},
                        color: #000
                    }
                    empty_text: "Wallet Name",
                }

                password_input = <RobrixTextInput> {
                    width: Fill,
                    height: Fit,
                    padding: 10,
                    draw_text: {
                        text_style: <REGULAR_TEXT>{font_size: 12},
                        color: #000
                    }
                    is_password: true,
                    empty_text: "Wallet Password",
                }

                confirm_password_input = <RobrixTextInput> {
                    width: Fill,
                    height: Fit,
                    padding: 10,
                    draw_text: {
                        text_style: <REGULAR_TEXT>{font_size: 12},
                        color: #000
                    }
                    is_password: true,
                    empty_text: "Confirm Wallet Password",
                }

                <View> {
                    width: Fill, height: Fit
                    flow: Down

                    wallet_file_name_input = <RobrixTextInput> {
                        width: Fill, height: Fit,
                        flow: Right, // do not wrap
                        padding: {top: 3, bottom: 3}
                        empty_text: "my_wallet_file",
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
                            text: "Wallet File Name (optional)"
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
                    text: "Create Wallet"
                    draw_text:{
                        color: (COLOR_FG_ACCEPT_GREEN),
                    }
                }
            }

            status_label = <Label> {
                width: Fill,
                height: Fit,
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
/// to open or close the `CreateWalletModal`.
#[derive(Clone, Copy, Debug)]
pub enum CreateWalletModalAction {
    /// The settings screen should open the modal.
    Open,
    /// The settings screen should close the modal.
    Close,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum CreateWalletModalState {
    /// Waiting for the user to enter wallet details.
    #[default]
    WaitingForUserInput,
    /// Waiting for the wallet to be created.
    WaitingForWalletCreation,
    /// The wallet was created successfully.
    WalletCreated,
    /// An error occurred while creating the wallet.
    WalletCreationError,
}

#[derive(Live, LiveHook, Widget)]
pub struct CreateWalletModal {
    #[deref]
    view: View,
    #[rust]
    state: CreateWalletModalState,
    #[rust]
    is_showing_error: bool,
}

impl Widget for CreateWalletModal {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        self.widget_match_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl WidgetMatchEvent for CreateWalletModal {
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
            // a `CreateWalletModalAction::Close` action, as that would cause
            // an infinite action feedback loop.
            if cancel_clicked {
                cx.action(CreateWalletModalAction::Close);
            }

            // TODO: if possible, cancel the wallet creation request if it's still pending.

            return;
        }

        let wallet_name_input = self.view.text_input(ids!(wallet_name_input));
        let wallet_file_name_input = self.view.text_input(ids!(wallet_file_name_input));
        let password_input = self.view.text_input(ids!(password_input));
        let confirm_password_input = self.view.text_input(ids!(confirm_password_input));
        let status_label = self.view.label(ids!(status_label));

        // Handle clicking the accept button.
        let mut needs_redraw = false;
        if accept_button.clicked(actions) {
            match self.state {
                // If the modal is in the "final" state, just close the modal.
                CreateWalletModalState::WalletCreated => {
                    self.state = CreateWalletModalState::WaitingForUserInput;
                    cx.action(CreateWalletModalAction::Close);
                }

                CreateWalletModalState::WaitingForUserInput => {
                    let wallet_name = wallet_name_input.text();
                    let password = password_input.text();
                    let confirm_password = confirm_password_input.text();

                    // Check to ensure that the user has entered all required fields.
                    if password.is_empty() || confirm_password.is_empty() {
                        self.is_showing_error = true;
                        status_label.apply_over(
                            cx,
                            live!(
                                text: "Please enter a wallet password.",
                                draw_text: {
                                    color: (COLOR_FG_DANGER_RED),
                                },
                            ),
                        );
                    } else if password != confirm_password {
                        self.is_showing_error = true;
                        status_label.apply_over(
                            cx,
                            live!(
                                text: "Passwords do not match.",
                                draw_text: {
                                    color: (COLOR_FG_DANGER_RED),
                                },
                            ),
                        );
                    } else if wallet_name.is_empty() {
                        self.is_showing_error = true;
                        status_label.apply_over(
                            cx,
                            live!(
                                text: "Please enter a wallet name.",
                                draw_text: {
                                    color: (COLOR_FG_DANGER_RED),
                                },
                            ),
                        );
                    } else {
                        let url = tsp::TspWalletSqliteUrl::from_wallet_file_name(
                            match wallet_file_name_input.text() {
                                empty if empty.is_empty() => wallet_file_name_input.empty_text(),
                                non_empty => tsp::sanitize_wallet_name(&non_empty),
                            }
                            .as_str(),
                        );
                        let metadata = TspWalletMetadata {
                            wallet_name,
                            url,
                            password,
                        };
                        // Submit the wallet creation request to the TSP async worker thread.
                        tsp::submit_tsp_request(tsp::TspRequest::CreateWallet { metadata });
                        self.state = CreateWalletModalState::WaitingForWalletCreation;
                        self.is_showing_error = false;
                        status_label.apply_over(
                            cx,
                            live!(
                                text: "Waiting for wallet to be created...",
                                draw_text: {
                                    color: (COLOR_ACTIVE_PRIMARY_DARKER),
                                },
                            ),
                        );
                        accept_button.set_enabled(cx, false);
                        cancel_button.set_enabled(cx, false); // TODO: support canceling the wallet creation request?
                        wallet_name_input.set_is_read_only(cx, true);
                        wallet_file_name_input.set_is_read_only(cx, true);
                        password_input.set_is_read_only(cx, true);
                        confirm_password_input.set_is_read_only(cx, true);
                    }

                    needs_redraw = true;
                }

                _ => {}
            }
        }

        // Clear the error message if the user changes any of the input fields.
        if self.is_showing_error {
            if wallet_name_input.changed(actions).is_some()
                || wallet_file_name_input.changed(actions).is_some()
                || password_input.changed(actions).is_some()
                || confirm_password_input.changed(actions).is_some()
            {
                self.is_showing_error = false;
                self.view.label(ids!(status_label)).set_text(cx, "");
                self.state = CreateWalletModalState::WaitingForUserInput;
                accept_button.apply_over(
                    cx,
                    live!(
                        text: "Create Wallet",
                        enabled: true,
                        draw_text: {
                            color: (COLOR_FG_ACCEPT_GREEN),
                        },
                    ),
                );
                needs_redraw = true;
            }
        }

        // If the wallet name is changed, update the path's empty text to show
        // a sanitized version of the wallet name.
        if let Some(name) = wallet_name_input.changed(actions) {
            wallet_file_name_input.set_empty_text(cx, tsp::sanitize_wallet_name(&name));
        }

        for action in actions {
            match action.downcast_ref() {
                // Handle the wallet creation success action.
                Some(tsp::TspWalletAction::CreateWalletSuccess {
                    metadata,
                    is_default,
                }) => {
                    self.state = CreateWalletModalState::WalletCreated;
                    self.is_showing_error = false;
                    let message = if *is_default {
                        format!(
                            "Wallet \"{}\" created successfully and set as the default.",
                            metadata.wallet_name
                        )
                    } else {
                        format!("Wallet \"{}\" created successfully.", metadata.wallet_name)
                    };
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
                }

                // Handle the wallet creation error action.
                Some(tsp::TspWalletAction::CreateWalletError { error, .. }) => {
                    self.state = CreateWalletModalState::WalletCreationError;
                    self.is_showing_error = true;
                    let message = format!("Failed to create wallet: {error}.");
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
                    wallet_name_input.set_is_read_only(cx, false);
                    wallet_file_name_input.set_is_read_only(cx, false);
                    password_input.set_is_read_only(cx, false);
                    confirm_password_input.set_is_read_only(cx, false);
                }

                _ => {}
            }
        }

        if needs_redraw {
            self.view.redraw(cx);
        }
    }
}

impl CreateWalletModal {
    pub fn show(&mut self, cx: &mut Cx) {
        self.state = CreateWalletModalState::WaitingForUserInput;
        let accept_button = self.view.button(ids!(accept_button));
        let cancel_button = self.view.button(ids!(cancel_button));
        accept_button.set_text(cx, "Create Wallet");
        cancel_button.set_text(cx, "Cancel");
        accept_button.reset_hover(cx);
        cancel_button.reset_hover(cx);
        accept_button.set_enabled(cx, true);
        cancel_button.set_enabled(cx, true);
        accept_button.set_visible(cx, true);
        cancel_button.set_visible(cx, true);
        // TODO: return buttons to their default state/appearance
        self.view
            .text_input(ids!(wallet_name_input))
            .set_is_read_only(cx, false);
        self.view
            .text_input(ids!(wallet_file_name_input))
            .set_is_read_only(cx, false);
        self.view
            .text_input(ids!(password_input))
            .set_is_read_only(cx, false);
        self.view
            .text_input(ids!(confirm_password_input))
            .set_is_read_only(cx, false);
        self.view.label(ids!(status_label)).set_text(cx, "");
        self.is_showing_error = false;
        self.view.redraw(cx);
    }
}

impl CreateWalletModalRef {
    pub fn show(&self, cx: &mut Cx) {
        let Some(mut inner) = self.borrow_mut() else {
            return;
        };
        inner.show(cx);
    }
}
