
use std::cell::RefCell;

use makepad_widgets::*;

use crate::{
    app::ConfirmDeleteAction,
    i18n::{AppLanguage, tr_fmt, tr_key},
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
                text: ""
            }

            wallet_path := Label {
                width: Fit, height: Fit
                flow: Right,
                margin: Inset{top: 2.9, left: 8, bottom: 2}
                draw_text +: {
                    color: (MESSAGE_TEXT_COLOR),
                    text_style: theme.font_regular { font_size: 11 },
                }
                text: ""
            }

            is_default_label_view := View {
                visible: false,
                width: Fit, height: Fit
                margin: Inset{left: 20}
                is_default_label := Label {
                    margin: Inset{top: 2.9}
                    width: Fit, height: Fit
                    flow: Right,
                    draw_text +: {
                        color: (COLOR_FG_ACCEPT_GREEN),
                        text_style: theme.font_bold { font_size: 11 },
                    }
                    text: ""
                }
            }

            not_found_label_view := View {
                visible: false,
                width: Fit, height: Fit
                margin: Inset{left: 20}
                not_found_label := Label {
                    margin: Inset{top: 2.9}
                    width: Fit, height: Fit
                    flow: Right,
                    draw_text +: {
                        color: (COLOR_FG_DANGER_RED),
                        text_style: MESSAGE_TEXT_STYLE { font_size: 11 },
                    }
                    text: ""
                }
            }

            set_default_wallet_button := RobrixIconButton {
                padding: Inset{top: 10, bottom: 10, left: 12, right: 15}
                margin: Inset{left: 20}
                draw_icon.svg: (ICON_CHECKMARK)
                icon_walk: Walk{width: 16, height: 16}
                text: ""
            }

            remove_wallet_button := RobrixNegativeIconButton {
                padding: Inset{top: 10, bottom: 10, left: 12, right: 15}
                margin: Inset{left: 20}
                draw_icon.svg: (ICON_CLOSE)
                icon_walk: Walk{ width: 16, height: 16 }
                text: ""
            }

            delete_wallet_button := RobrixNegativeIconButton {
                padding: Inset{top: 10, bottom: 10, left: 12, right: 15}
                margin: Inset{left: 20}
                draw_icon.svg: (ICON_TRASH)
                icon_walk: Walk{ width: 16, height: 16 }
                text: ""
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
    #[rust] app_language: AppLanguage,
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
                    title_text: tr_key(self.app_language, "tsp.wallet_entry.modal.remove.title").into(),
                    body_text: tr_fmt(self.app_language, "tsp.wallet_entry.modal.remove.body", &[
                        ("wallet_name", metadata.wallet_name.as_str()),
                    ]).into(),
                    accept_button_text: Some(tr_key(self.app_language, "tsp.wallet_entry.modal.remove.accept").into()),
                    on_accept_clicked: Some(Box::new(move |_cx| {
                        submit_tsp_request(TspRequest::RemoveWallet(metadata_clone));
                    })),
                    ..Default::default()
                };
                cx.action(ConfirmDeleteAction::Show(RefCell::new(Some(content))));
            }

            if self.view.button(cx, ids!(delete_wallet_button)).clicked(actions) {
                // TODO: Implement the delete wallet feature.
                enqueue_popup_notification(
                    tr_key(self.app_language, "tsp.wallet_entry.popup.delete_not_implemented"),
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
        self.app_language = sd.app_language;

        self.label(cx, ids!(wallet_name)).set_text(
            cx,
            &metadata.wallet_name,
        );
        self.label(cx, ids!(wallet_path)).set_text(
            cx,
            metadata.url.as_url_unencoded()
        );
        self.label(cx, ids!(is_default_label_view.is_default_label)).set_text(
            cx,
            tr_key(self.app_language, "tsp.wallet_entry.default_label"),
        );
        self.label(cx, ids!(not_found_label_view.not_found_label)).set_text(
            cx,
            tr_key(self.app_language, "tsp.wallet_entry.not_found"),
        );
        self.button(cx, ids!(set_default_wallet_button)).set_text(
            cx,
            tr_key(self.app_language, "tsp.wallet_entry.button.set_default"),
        );
        self.button(cx, ids!(remove_wallet_button)).set_text(
            cx,
            tr_key(self.app_language, "tsp.wallet_entry.button.remove"),
        );
        self.button(cx, ids!(delete_wallet_button)).set_text(
            cx,
            tr_key(self.app_language, "tsp.wallet_entry.button.delete"),
        );
        // There is a weird makepad bug where if we re-style one instance of the
        // `set_default_wallet_button` in one WalletEntry, all other instances of that button
        // get their styling messed up in weird ways.
        // So, as a workaround, we just hide the button entirely and show a `is_default_label_view` instead.

        self.view(cx, ids!(is_default_label_view)).set_visible(
            cx,
            sd.is_default
        );
        self.view(cx, ids!(not_found_label_view)).set_visible(
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
