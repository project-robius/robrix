use makepad_widgets::*;

use crate::{shared::{popup_list::{enqueue_popup_notification, PopupItem, PopupKind}, styles::*}, tsp::{create_did_modal::CreateDidModalAction, create_wallet_modal::CreateWalletModalAction, submit_tsp_request, tsp_state_ref, TspIdentityAction, TspRequest, TspWalletAction, TspWalletEntry, TspWalletMetadata}};

const REPUBLISH_IDENTITY_BUTTON_TEXT: &str = "Republish Current Identity to DID Server";

live_design! {
    link tsp_enabled

    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::helpers::*;
    use crate::shared::styles::*;
    use crate::shared::avatar::*;
    use crate::shared::icon_button::*;
    use crate::tsp::wallet_entry::*;

    REPUBLISH_IDENTITY_BUTTON_TEXT = "Republish Current Identity to DID Server"

    // The view containing all TSP-related settings.
    pub TspSettingsScreen = {{TspSettingsScreen}} {
        width: Fill, height: Fit
        flow: Down

        <TitleLabel> {
            text: "TSP Wallet Settings"
        }

        <SubsectionLabel> {
            text: "Your active identity:"
        }

        <View> {
            width: Fill, height: Fit
            flow: Right,
            spacing: 10

            copy_identity_button = <RobrixIconButton> {
                margin: {left: 5}
                padding: 12,
                spacing: 0,
                draw_bg: {
                    color: (COLOR_SECONDARY)
                }
                draw_icon: {
                    svg_file: (ICON_COPY)
                }
                icon_walk: {width: 16, height: 16, margin: {right: -2} }
            }

            current_identity_label = <Label> {
                width: Fill, height: Fit
                flow: RightWrap,
                margin: {top: 10}
                draw_text: {
                    wrap: Line,
                    text_style: <MESSAGE_TEXT_STYLE>{ font_size: 11 },
                }
            }
        }

        republish_identity_button = <RobrixIconButton> {
            width: Fit, height: Fit,
            padding: 10,
            margin: {top: 8, bottom: 10, left: 5},

            draw_bg: {
                color: (COLOR_ACTIVE_PRIMARY),
                border_radius: 5
            }
            draw_icon: {
                svg_file: (ICON_UPLOAD)
                color: (COLOR_PRIMARY),
            }
            icon_walk: {width: 16, height: 16}
            draw_text: {
                color: (COLOR_PRIMARY),
            }
            text: (REPUBLISH_IDENTITY_BUTTON_TEXT)
        }


        <SubsectionLabel> {
            text: "Your Wallets:"
        }

        no_wallets_label = <View> {
            width: Fill, height: Fit
            <Label> {
                width: Fill, height: Fit
                margin: {top: 10, bottom: 8, left: 13, right: 10},
                flow: RightWrap,
                draw_text: {
                    wrap: Line,
                    color: (COLOR_WARNING_NOT_FOUND),
                    text_style: <MESSAGE_TEXT_STYLE>{ font_size: 11 },
                }
                text: "No wallets found. Create or import a wallet."
            }
        }

        <RoundedView> {
            width: Fill, height: Fit
            margin: 5,

            show_bg: true,
            draw_bg: {
                color: #F6F8F9,
                border_radius: 4.0,
            }

            wallet_list = <FlatList> {
                width: Fill,
                height: Fit,
                spacing: 0.0
                flow: Down,

                grab_key_focus: true,
                drag_scrolling: true,
                scroll_bars: { show_scroll_x: false, show_scroll_y: false },

                wallet_entry = <WalletEntry> { }
            }
        }

        <View> {
            margin: {top: 5},
            width: Fill, height: Fit
            flow: RightWrap,
            align: {y: 0.5},
            spacing: 10

            create_did_button = <RobrixIconButton> {
                width: Fit, height: Fit,
                padding: 10,
                margin: {left: 5},

                draw_bg: {
                    border_color: (COLOR_FG_ACCEPT_GREEN),
                    color: (COLOR_BG_ACCEPT_GREEN),
                    border_radius: 5
                }
                draw_icon: {
                    svg_file: (ICON_ADD_USER)
                    color: (COLOR_FG_ACCEPT_GREEN),
                }
                icon_walk: {width: 21, height: Fit, margin: 0}
                draw_text: {
                    color: (COLOR_FG_ACCEPT_GREEN),
                }
                text: "Create New Identity (DID)"
            }

            create_wallet_button = <RobrixIconButton> {
                width: Fit, height: Fit,
                padding: 10,
                margin: {left: 5},

                draw_bg: {
                    border_color: (COLOR_FG_ACCEPT_GREEN),
                    color: (COLOR_BG_ACCEPT_GREEN),
                    border_radius: 5
                }
                draw_icon: {
                    svg_file: (ICON_ADD_WALLET)
                    color: (COLOR_FG_ACCEPT_GREEN),
                }
                icon_walk: {width: 21, height: Fit, margin: 0}
                draw_text: {
                    color: (COLOR_FG_ACCEPT_GREEN),
                }
                text: "Create New Wallet"
            }

            import_wallet_button = <RobrixIconButton> {
                padding: {top: 10, bottom: 10, left: 12, right: 15}
                margin: {left: 5}
                draw_bg: {
                    color: (COLOR_ACTIVE_PRIMARY)
                }
                draw_text: {
                    color: (COLOR_PRIMARY)
                    text_style: <REGULAR_TEXT> {}
                }
                text: "Import Existing Wallet"
                // TODO: fix this icon, or pick a different SVG
                // draw_icon: {
                //     svg_file: (ICON_IMPORT)
                //     color: (COLOR_PRIMARY)
                // }
                // icon_walk: {width: 16, height: 16}
                icon_walk: {width: 0, height: 0}
            }
        }
    }
}

#[derive(Debug, Default)]
struct WalletState {
    active_wallet: Option<TspWalletMetadata>,
    other_wallets: Vec<(TspWalletMetadata, WalletStatus)>,
    active_identity: Option<String>,
}
impl WalletState {
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn len(&self) -> usize {
        self.active_wallet.is_some() as usize + self.other_wallets.len()
    }

    fn get(&self, index: usize) -> Option<(&TspWalletMetadata, WalletStatusAndDefault)> {
        if let Some(active) = self.active_wallet.as_ref() {
            if index == 0 {
                Some((active, WalletStatusAndDefault::new(WalletStatus::Opened, true)))
            } else {
                self.other_wallets.get(index - 1).map(|(m, s)|
                    (m, WalletStatusAndDefault::new(*s, false))
                )
            }
        } else {
            self.other_wallets.get(index).map(|(m, s)|
                (m, WalletStatusAndDefault::new(*s, false))
            )
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WalletStatus {
    Opened,
    NotFound,
}

#[derive(Clone, Copy)]
pub struct WalletStatusAndDefault {
    pub status: WalletStatus,
    pub is_default: bool,
}
impl WalletStatusAndDefault {
    pub fn new(status: WalletStatus, is_default: bool) -> Self {
        Self { status, is_default }
    }
}

/// The view containing all TSP-related settings.
#[derive(Live, LiveHook, Widget)]
pub struct TspSettingsScreen {
    #[deref] view: View,

    /// The list of wallets that are known by this widget.
    ///
    /// * If `None`, this widget doesn't know about any wallets or is outdated,
    ///   and must retrieve them from the TSP state.
    /// * If `Some`, the wallets has been opened and is up-to-date.
    ///   * This doesn't mean that any wallets actually exist.
    ///
    /// This is sort of a "cache" of the wallets that have been drawn
    /// to avoid having to re-fetch them from the shared TSP state every time,
    /// as that requires locking the mutex and can be expensive.
    #[rust] wallets: Option<WalletState>,
}

impl Widget for TspSettingsScreen {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.match_event(cx, event);
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        if self.wallets.is_none() {
            // If we don't have any wallets, load them from the TSP state.
            self.refresh_wallets();
            log!("Wallets were refreshed: {:?}", self.wallets);
        }

        // Draw the current identity label and republish button based on the active identity.
        let (current_did_text, current_did_text_color, show_republish_button) = match
            self.wallets.as_ref().and_then(|ws| ws.active_identity.as_deref())
        {
            Some(current_did) => (current_did, COLOR_FG_ACCEPT_GREEN, true),
            None => ("No default identity has been set.", COLOR_WARNING_NOT_FOUND, false),
        };
        self.view.label(ids!(current_identity_label)).apply_over(cx, live!(
            text: (current_did_text),
            draw_text: { color: (current_did_text_color) },
        ));
        self.view.button(ids!(republish_identity_button)).set_visible(cx, show_republish_button);


        // If we don't have any wallets, show the "no wallets" label.
        let is_wallets_empty = self.wallets.as_ref().is_none_or(|w| w.is_empty());
        self.view.view(ids!(no_wallets_label)).set_visible(cx, is_wallets_empty);

        while let Some(subview) = self.view.draw_walk(cx, scope, walk).step() {
            // Here, we only need to handle drawing the wallet list.
            let flat_list_ref = subview.as_flat_list();
            let Some(mut list) = flat_list_ref.borrow_mut() else {
                error!("!!! TspSettingsScreen::draw_walk(): BUG: expected a FlatList widget, but got something else");
                continue;
            };
            let Some(wallets) = self.wallets.as_ref() else {
                return DrawStep::done();
            };

            for (metadata, mut status_and_default) in (0..wallets.len()).filter_map(|i| wallets.get(i)) {
                let item_live_id = LiveId::from_str(metadata.url.as_url_unencoded());
                let item = list.item(cx, item_live_id, id!(wallet_entry)).unwrap();
                // Pass the wallet metadata in through Scope via props,
                // and status/is_default via data.
                let mut scope = Scope::with_data_props(&mut status_and_default, metadata);
                item.draw_all(cx, &mut scope);
            }
        }
        DrawStep::done()
    }
}

impl MatchEvent for TspSettingsScreen {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        let republish_identity_button = self.view.button(ids!(republish_identity_button));

        for action in actions {
            match action.downcast_ref() {
                // Add the new wallet to the list of drawn wallets.
                Some(TspWalletAction::CreateWalletSuccess { metadata, is_default }) => {
                    let wallets = self.wallets.get_or_insert_default();
                    if *is_default {
                        wallets.active_wallet = Some(metadata.clone());
                    } else {
                        wallets.other_wallets.push((metadata.clone(), WalletStatus::Opened));
                    }
                    self.view.redraw(cx);
                    continue;
                }

                // Remove the wallet from the list of drawn wallets.
                Some(TspWalletAction::WalletRemoved { metadata, was_default }) => {
                    let Some(wallets) = &mut self.wallets.as_mut() else { continue };
                    if *was_default {
                        wallets.active_wallet = None;
                    }
                    else if let Some(pos) = wallets.other_wallets.iter().position(|(w, _)| w == metadata) {
                        wallets.other_wallets.remove(pos);
                    }
                    else {
                        continue;
                    }
                    enqueue_popup_notification(PopupItem {
                        message: format!("Removed wallet \"{}\".", metadata.wallet_name),
                        auto_dismissal_duration: Some(4.0),
                        kind: PopupKind::Success,
                    });
                    if *was_default {
                        // If the removed wallet was the default wallet, notify the user.
                        // The user should then select another wallet as the default.
                        enqueue_popup_notification(PopupItem {
                            message: String::from("The default wallet was removed.\n\n\
                                TSP features will not work properly until you set a default wallet."),
                            auto_dismissal_duration: None,
                            kind: PopupKind::Warning,
                        });
                    }
                    self.view.redraw(cx);
                    continue;
                }

                // Update the default/active wallet.
                Some(TspWalletAction::DefaultWalletChanged(Ok(metadata))) => {
                    let wallets = self.wallets.get_or_insert_default();
                    let previous_active = wallets.active_wallet.replace(metadata.clone());
                    // If the newly-default wallet was in the other wallets list, remove it
                    // and then add the previous active wallet back to that other wallets list.
                    if let Some(idx_to_remove) = wallets.other_wallets.iter().position(|(w, _)| w == metadata) {
                        wallets.other_wallets.remove(idx_to_remove);
                    }
                    if let Some(previous_active) = previous_active {
                        wallets.other_wallets.insert(0, (previous_active, WalletStatus::Opened));
                    }
                    self.view.redraw(cx);
                    continue;
                }
                Some(TspWalletAction::DefaultWalletChanged(Err(_))) => {
                    enqueue_popup_notification(PopupItem {
                        message: String::from("Failed to set default wallet, could not find or open selected wallet."),
                        auto_dismissal_duration: None,
                        kind: PopupKind::Error,
                    });
                    continue;
                }

                // Handle a newly-opened wallet.
                Some(TspWalletAction::WalletOpened(Ok(metadata))) => {
                    let wallets = self.wallets.get_or_insert_default();
                    if let Some((_m, status)) = wallets.other_wallets.iter_mut().find(|(w, _)| w == metadata) {
                        *status = WalletStatus::Opened;
                    } else {
                        wallets.other_wallets.push((metadata.clone(), WalletStatus::Opened));
                    }
                    self.view.redraw(cx);
                    continue;
                }
                Some(TspWalletAction::WalletOpened(Err(e))) => {
                    enqueue_popup_notification(PopupItem {
                        message: format!("Failed to open wallet: {e}"),
                        auto_dismissal_duration: None,
                        kind: PopupKind::Error,
                    });
                    continue;
                }

                // This is handled in the CreateWalletModal
                Some(TspWalletAction::CreateWalletError { .. }) => { continue; }
                None => { }
            }

            match action.downcast_ref() {
                Some(TspIdentityAction::DidCreationResult(result)) => {
                    // If there is no active identity, set the newly-created identity as active.
                    let wallets = self.wallets.get_or_insert_default();
                    if let (Ok(did), None) = (result, wallets.active_identity.as_ref()) {
                        wallets.active_identity = Some(did.clone());
                        self.view.redraw(cx);
                    }
                    continue;
                }
                Some(TspIdentityAction::DidRepublishResult(result)) => {
                    // restore the republish button to its original state.
                    republish_identity_button.apply_over(cx, live!(
                        enabled: true,
                        text: (REPUBLISH_IDENTITY_BUTTON_TEXT),
                    ));
                    match result {
                        Ok(did) => {
                            enqueue_popup_notification(PopupItem {
                                message: format!("Successfully republished identity \"{}\" to the DID server.", did),
                                auto_dismissal_duration: Some(5.0),
                                kind: PopupKind::Success,
                            });
                        }
                        Err(e) => {
                            enqueue_popup_notification(PopupItem {
                                message: format!("Failed to republish identity to the DID server: {e}"),
                                auto_dismissal_duration: None,
                                kind: PopupKind::Error,
                            });
                        }
                    }
                    continue;
                }
                Some(TspIdentityAction::SentDidAssociationRequest { .. }) => { continue; } // handled in the TspVerifyUser widget
                Some(TspIdentityAction::ErrorSendingDidAssociationRequest { .. }) => { continue; } // handled in the TspVerifyUser widget
                Some(TspIdentityAction::ReceivedDidAssociationResponse { .. }) => { continue; } // handled in the TspVerifyUser widget
                Some(TspIdentityAction::ReceivedDidAssociationRequest { .. }) => { continue; } // handled in the TspVerificationModal widget
                Some(TspIdentityAction::ReceiveLoopError { .. }) => { continue; } // handled in the top-level app
                None => { }
            }
        }


        if self.view.button(ids!(copy_identity_button)).clicked(actions) { 
            if let Some(did) = self.wallets.as_ref().and_then(|ws| ws.active_identity.as_deref()) {
                cx.copy_to_clipboard(did);
                enqueue_popup_notification(PopupItem {
                    message: String::from("Copied your default TSP identity to the clipboard."),
                    auto_dismissal_duration: Some(3.0),
                    kind: PopupKind::Success,
                });
            } else {
                enqueue_popup_notification(PopupItem {
                    message: String::from("No default TSP identity has been set."),
                    auto_dismissal_duration: Some(4.0),
                    kind: PopupKind::Warning,
                });
            }
        }

        // Allow the user to republish their identity to the DID server.
        // This is primarily needed because some DID servers (e.g., the test servers)
        // frequently wipe their identity storage after a certain period of time.
        if self.view.button(ids!(republish_identity_button)).clicked(actions) {
            if self.has_default_wallet() {
                if let Some(our_did) = self.wallets.as_ref().and_then(|ws| ws.active_identity.as_deref()) {
                    republish_identity_button.apply_over(cx, live!(
                        enabled: false,
                        text: "Republishing DID now...",
                    ));

                    submit_tsp_request(TspRequest::RepublishDid { did: our_did.to_string() });
                } else {
                    enqueue_popup_notification(PopupItem {
                        message: String::from("You must set a default TSP identity to be republished."),
                        auto_dismissal_duration: Some(5.0),
                        kind: PopupKind::Error,
                    });
                }
            }
        }

        if self.view.button(ids!(create_wallet_button)).clicked(actions) {
            cx.action(CreateWalletModalAction::Open);
        }

        if self.view.button(ids!(create_did_button)).clicked(actions) {
            if self.has_default_wallet() {
                cx.action(CreateDidModalAction::Open);
            }
        }

        if self.view.button(ids!(import_wallet_button)).clicked(actions) {
            // TODO: support importing an existing wallet.
            enqueue_popup_notification(PopupItem {
                message: String::from("Importing an existing wallet is not yet implemented."),
                auto_dismissal_duration: Some(4.0),
                kind: PopupKind::Warning,
            });
        }
    }
}

impl TspSettingsScreen {
    /// Re-fetches the TSP state and populates this widget's list of wallets.
    fn refresh_wallets(&mut self) {
        let tsp_state = tsp_state_ref().lock().unwrap();
        let current_wallet = tsp_state.current_wallet.as_ref().map(|w| w.metadata.clone());
        let other_wallets = tsp_state.other_wallets
            .iter()
            .map(|entry| match entry {
                TspWalletEntry::Opened(opened) => (opened.metadata.clone(), WalletStatus::Opened),
                TspWalletEntry::NotFound(metadata) => (metadata.clone(), WalletStatus::NotFound),
            })
            .collect::<Vec<_>>();
        self.wallets = Some(WalletState {
            active_wallet: current_wallet,
            other_wallets,
            active_identity: tsp_state.current_local_vid.clone(),
        });
    }

    /// Checks if the current TSP state has a default wallet set and ready to use.
    ///
    /// This function will display warnings to the user if no default wallet is set
    /// or if there are no wallets at all.
    ///
    /// Returns `true` if a default wallet is set, `false` otherwise.
    fn has_default_wallet(&self) -> bool {
        let Some(wallets) = self.wallets.as_ref() else {
            enqueue_popup_notification(PopupItem {
                message: String::from("No TSP wallets found.\n\nPlease create or import a wallet."),
                auto_dismissal_duration: Some(5.0),
                kind: PopupKind::Warning,
            });
            return false;
        };
        if wallets.active_wallet.is_none() {
            enqueue_popup_notification(PopupItem {
                message: String::from("No default TSP wallet is set.\n\nPlease select or create a default wallet."),
                auto_dismissal_duration: Some(5.0),
                kind: PopupKind::Warning,
            });
            return false;
        }
        true
    }
}
