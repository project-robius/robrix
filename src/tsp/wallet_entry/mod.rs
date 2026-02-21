
use std::cell::RefCell;

use makepad_widgets::*;

use crate::{
    app::ConfirmDeleteAction,
    shared::{confirmation_modal::ConfirmationModalContent, popup_list::{enqueue_popup_notification, PopupKind}},
    tsp::{submit_tsp_request, tsp_settings_screen::{WalletStatus, WalletStatusAndDefault}, TspRequest, TspWalletMetadata}
};

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*


    // An entry in the list of wallets.
    mod.widgets.WalletEntry = #(WalletEntry::register_widget(vm)) {
        width: Fill, height: Fit
        flow: Down

        View {
            width: Fill, height: Fit
            flow: Flow.Right{wrap: true},
            padding: 10

            wallet_name := Label {
                width: Fit, height: Fit
                flow: Right,
                margin: Inset{top: 2.4, left: 0}
                draw_text +: {
                    color: (MESSAGE_TEXT_COLOR),
                    text_style: theme.font_bold { font_size: 12 },
                }
                text: "[Wallet Name]"
            }

            wallet_path := Label {
                width: Fit, height: Fit
                flow: Right,
                margin: Inset{top: 2.9, left: 8, bottom: 2}
                draw_text +: {
                    color: (MESSAGE_TEXT_COLOR),
                    text_style: theme.font_regular { font_size: 11 },
                }
                text: "[Wallet Path/URL]"
            }

            is_default_label_view := View {
                visible: false,
                width: Fit, height: Fit
                margin: Inset{left: 20}
                Label {
                    margin: Inset{top: 2.9}
                    width: Fit, height: Fit
                    flow: Right,
                    draw_text +: {
                        color: (COLOR_FG_ACCEPT_GREEN),
                        text_style: theme.font_bold { font_size: 11 },
                    }
                    text: "✅ Default"
                }
            }

            not_found_label_view := View {
                visible: false,
                width: Fit, height: Fit
                margin: Inset{left: 20}
                Label {
                    margin: Inset{top: 2.9}
                    width: Fit, height: Fit
                    flow: Right,
                    draw_text +: {
                        color: (COLOR_FG_DANGER_RED),
                        text_style: MESSAGE_TEXT_STYLE { font_size: 11 },
                    }
                    text: "Wallet not found!"
                }
            }

            set_default_wallet_button := RobrixIconButton {
                padding: Inset{top: 10, bottom: 10, left: 12, right: 15}
                margin: Inset{left: 20}
                draw_bg +: {
                    color: (COLOR_ACTIVE_PRIMARY)
                }
                draw_icon +: {
                    svg_file: (ICON_CHECKMARK)
                    color: (COLOR_PRIMARY)
                }
                draw_text +: {
                    color: (COLOR_PRIMARY)
                    text_style: REGULAR_TEXT {}
                }
                icon_walk: Walk{width: 16, height: 16}
                text: "Set As Default"
            }

            remove_wallet_button := RobrixIconButton {
                padding: Inset{top: 10, bottom: 10, left: 12, right: 15}
                margin: Inset{left: 20}
                draw_bg +: {
                    color: (COLOR_BG_DANGER_RED)
                    border_color: (COLOR_FG_DANGER_RED)
                }
                draw_icon +: {
                    svg_file: (ICON_CLOSE),
                    color: (COLOR_FG_DANGER_RED),
                }
                draw_text +: {
                    color: (COLOR_FG_DANGER_RED),
                }
                icon_walk: Walk{ width: 16, height: 16 }
                text: "Remove From List"
            }

            delete_wallet_button := RobrixIconButton {
                padding: Inset{top: 10, bottom: 10, left: 12, right: 15}
                margin: Inset{left: 20}
                draw_bg +: {
                    color: (COLOR_BG_DANGER_RED)
                    border_color: (COLOR_FG_DANGER_RED)
                }
                draw_icon +: {
                    svg_file: (ICON_TRASH),
                    color: (COLOR_FG_DANGER_RED),
                }
                draw_text +: {
                    color: (COLOR_FG_DANGER_RED),
                }
                icon_walk: Walk{ width: 16, height: 16 }
                text: "Delete Wallet"
            }
        }

        LineH { padding: 10, margin: Inset{left: 5, right: 5} }
    }

}


/// A view showing the details of a single TSP wallet (one entry in the wallets list).
#[derive(Script, ScriptHook, Widget)]
pub struct WalletEntry {
    #[deref] view: View,

    #[rust] metadata: Option<TspWalletMetadata>,
}

impl Widget for WalletEntry {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);

        let Some(metadata) = self.metadata.as_ref() else { return };
        if let Event::Actions(actions) = event {
            if self.view.button(cx, ids!(set_default_wallet_button)).clicked(actions) {
                submit_tsp_request(TspRequest::SetDefaultWallet(metadata.clone()));
            }

            if self.view.button(cx, ids!(remove_wallet_button)).clicked(actions) {
                let metadata_clone = metadata.clone();
                let content = ConfirmationModalContent {
                    title_text: "Remove Wallet".into(),
                    body_text: format!(
                        "Are you sure you want to remove the wallet \"{}\" \
                        from the list?\n\nThis won't delete the actual wallet file.",
                        metadata.wallet_name
                    ).into(),
                    accept_button_text: Some("Remove".into()),
                    on_accept_clicked: Some(Box::new(move |cx| {
                        submit_tsp_request(TspRequest::RemoveWallet(metadata_clone));
                    })),
                    ..Default::default()
                };
                cx.action(ConfirmDeleteAction::Show(RefCell::new(Some(content))));
            }

            if self.view.button(cx, ids!(delete_wallet_button)).clicked(actions) {
                // TODO: Implement the delete wallet feature.
                enqueue_popup_notification(
                    "Delete wallet feature is not yet implemented.",
                    PopupKind::Warning,
                    None,
                );
            }
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        // The metadata was pasmatchsed in through Scope via props, and status/is_default via data.
        let metadata = scope.props.get::<TspWalletMetadata>().unwrap();
        let sd = scope.data.get::<WalletStatusAndDefault>().unwrap();
        // Store the passed-in metadata (for event handling) if it has changed.
        if self.metadata.as_ref().is_none_or(|m| m != metadata) {
            self.metadata = Some(metadata.clone());
        }

        self.label(cx, ids!(wallet_name)).set_text(
            cx,
            &metadata.wallet_name,
        );
        self.label(cx, ids!(wallet_path)).set_text(
            cx,
            metadata.url.as_url_unencoded()
        );
        // There is a weird makepad bug where if we re-style one instance of the
        // `set_default_wallet_button` in one WalletEntry, all other instances of that button
        // get their styling messed up in weird ways.
        // So, as a workaround, we just hide the button entirely and show a `is_default_label_view` instead.

        self.view(cx, ids!(is_default_label_view)).set_visible(
            cx,
            sd.is_default
        );
        self.label(cx, ids!(not_found_label_view)).set_visible(
            cx,
            sd.status == WalletStatus::NotFound,
        );
        self.button(cx, ids!(set_default_wallet_button)).set_visible(
            cx,
            !sd.is_default && sd.status != WalletStatus::NotFound,
        );
        self.button(cx, ids!(delete_wallet_button)).set_visible(
            cx,
            sd.status != WalletStatus::NotFound,
        );

        self.view.draw_walk(cx, scope, walk)
    }
}
