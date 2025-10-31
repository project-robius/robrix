
use std::cell::RefCell;

use makepad_widgets::*;

use crate::{
    shared::{confirmation_modal::ConfirmationModalContent, popup_list::{enqueue_popup_notification, PopupItem, PopupKind}},
    tsp::{submit_tsp_request, tsp_settings_screen::{WalletStatus, WalletStatusAndDefault}, TspRequest, TspWalletMetadata}
};

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::helpers::*;
    use crate::shared::styles::*;
    use crate::shared::avatar::*;
    use crate::shared::icon_button::*;

    // An entry in the list of wallets.
    pub WalletEntry = {{WalletEntry}} {
        width: Fill, height: Fit
        flow: Down

        <View> {
            width: Fill, height: Fit
            flow: RightWrap,
            padding: 10

            wallet_name = <Label> {
                width: Fit, height: Fit
                flow: Right,
                margin: {top: 2.4, left: 0}
                draw_text: {
                    color: (MESSAGE_TEXT_COLOR),
                    text_style: <THEME_FONT_BOLD>{ font_size: 12 },
                }
                text: "[Wallet Name]"
            }

            wallet_path = <Label> {
                width: Fit, height: Fit
                flow: Right,
                margin: {top: 2.9, left: 8, bottom: 2}
                draw_text: {
                    color: (MESSAGE_TEXT_COLOR),
                    text_style: <THEME_FONT_REGULAR>{ font_size: 11 },
                }
                text: "[Wallet Path/URL]"
            }

            is_default_label_view = <View> {
                visible: false,
                width: Fit, height: Fit
                margin: {left: 20}
                <Label> {
                    margin: {top: 2.9}
                    width: Fit, height: Fit
                    flow: Right,
                    draw_text: {
                        color: (COLOR_FG_ACCEPT_GREEN),
                        text_style: <THEME_FONT_BOLD>{ font_size: 11 },
                    }
                    text: "✅ Default"
                }
            }

            not_found_label_view = <View> {
                visible: false,
                width: Fit, height: Fit
                margin: {left: 20}
                <Label> {
                    margin: {top: 2.9}
                    width: Fit, height: Fit
                    flow: Right,
                    draw_text: {
                        color: (COLOR_FG_DANGER_RED),
                        text_style: <MESSAGE_TEXT_STYLE>{ font_size: 11 },
                    }
                    text: "Wallet not found!"
                }
            }

            set_default_wallet_button = <RobrixIconButton> {
                padding: {top: 10, bottom: 10, left: 12, right: 15}
                margin: {left: 20}
                draw_bg: {
                    color: (COLOR_ACTIVE_PRIMARY)
                }
                draw_icon: {
                    svg_file: (ICON_CHECKMARK)
                    color: (COLOR_PRIMARY)
                }
                draw_text: {
                    color: (COLOR_PRIMARY)
                    text_style: <REGULAR_TEXT> {}
                }
                icon_walk: {width: 16, height: 16}
                text: "Set As Default"
            }

            remove_wallet_button = <RobrixIconButton> {
                padding: {top: 10, bottom: 10, left: 12, right: 15}
                margin: {left: 20}
                draw_bg: {
                    color: (COLOR_BG_DANGER_RED)
                    border_color: (COLOR_FG_DANGER_RED)
                }
                draw_icon: {
                    svg_file: (ICON_CLOSE),
                    color: (COLOR_FG_DANGER_RED),
                }
                draw_text: {
                    color: (COLOR_FG_DANGER_RED),
                }
                icon_walk: { width: 16, height: 16 }
                text: "Remove From List"
            }

            delete_wallet_button = <RobrixIconButton> {
                padding: {top: 10, bottom: 10, left: 12, right: 15}
                margin: {left: 20}
                draw_bg: {
                    color: (COLOR_BG_DANGER_RED)
                    border_color: (COLOR_FG_DANGER_RED)
                }
                draw_icon: {
                    svg_file: (ICON_TRASH),
                    color: (COLOR_FG_DANGER_RED),
                }
                draw_text: {
                    color: (COLOR_FG_DANGER_RED),
                }
                icon_walk: { width: 16, height: 16 }
                text: "Delete Wallet"
            }
        }

        <LineH> { padding: 10, margin: {left: 5, right: 5} }
    }

}


/// A view showing the details of a single TSP wallet (one entry in the wallets list).
#[derive(Live, LiveHook, Widget)]
pub struct WalletEntry {
    #[deref] view: View,

    #[rust] metadata: Option<TspWalletMetadata>,
}

impl Widget for WalletEntry {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);

        let Some(metadata) = self.metadata.as_ref() else { return };
        if let Event::Actions(actions) = event {
            if self.view.button(ids!(set_default_wallet_button)).clicked(actions) {
                submit_tsp_request(TspRequest::SetDefaultWallet(metadata.clone()));
            }
            if self.view.button(ids!(remove_wallet_button)).clicked(actions) {
                let metadata_clone = metadata.clone();
                let content = ConfirmationModalContent {
                    title_text: "Remove Wallet".into(),
                    body_text: format!(
                        "Are you sure you want to remove the wallet \"{}\" \
                        from the list?\n\nThis won't delete the actual wallet file.",
                        metadata.wallet_name
                    ).into(),
                    accept_button_text: Some("Remove".into()),
                    on_accept_clicked: Some(Box::new(move |_cx| {
                        submit_tsp_request(TspRequest::RemoveWallet(metadata_clone));
                    })),
                    ..Default::default()
                };
                cx.action(TspWalletEntryAction::ShowConfirmationModal(RefCell::new(Some(content))));
            }
            if self.view.button(ids!(delete_wallet_button)).clicked(actions) {
                // TODO: Implement the delete wallet feature.
                enqueue_popup_notification(PopupItem {
                    message: "Delete wallet feature is not yet implemented.".to_string(),
                    auto_dismissal_duration: None,
                    kind: PopupKind::Warning,
                });
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

        self.label(ids!(wallet_name)).set_text(
            cx,
            &metadata.wallet_name,
        );
        self.label(ids!(wallet_path)).set_text(
            cx,
            metadata.url.as_url_unencoded()
        );
        // There is a weird makepad bug where if we re-style one instance of the
        // `set_default_wallet_button` in one WalletEntry, all other instances of that button
        // get their styling messed up in weird ways.
        // So, as a workaround, we just hide the button entirely and show a `is_default_label_view` instead.

        self.view(ids!(is_default_label_view)).set_visible(
            cx,
            sd.is_default
        );
        self.label(ids!(not_found_label_view)).set_visible(
            cx,
            sd.status == WalletStatus::NotFound,
        );
        self.button(ids!(set_default_wallet_button)).set_visible(
            cx,
            !sd.is_default && sd.status != WalletStatus::NotFound,
        );
        self.button(ids!(delete_wallet_button)).set_visible(
            cx,
            sd.status != WalletStatus::NotFound,
        );

        self.view.draw_walk(cx, scope, walk)
    }
}


/// Actions related to a single TSP wallet.
///
/// These are NOT widget actions, just regular actions.
#[derive(Debug)]
pub enum TspWalletEntryAction {
    /// Show a confirmation modal for an action related to a TSP wallet entry.
    ///
    /// The content is wrapped in a `RefCell` to ensure that only one entity handles it
    /// and that that one entity can take ownership of the content object,
    /// which avoids having to clone it.
    ShowConfirmationModal(RefCell<Option<ConfirmationModalContent>>),
}
