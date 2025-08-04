use std::ops::DerefMut;

use makepad_widgets::*;

use crate::{shared::popup_list::{enqueue_popup_notification, PopupItem}, tsp::{create_wallet_modal::CreateWalletModalAction, tsp_state_ref, TspWalletAction, TspWalletEntry, TspWalletMetadata}};

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::helpers::*;
    use crate::shared::styles::*;
    use crate::shared::avatar::*;
    use crate::shared::icon_button::*;
    use crate::tsp::wallet_entry::*;

    // The view containing all TSP-related settings.
    pub TspSettingsScreen = {{TspSettingsScreen}} {
        width: Fill, height: Fit
        flow: Down

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
                    color: #953800,
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

            wallet_list = <PortalList> {
                width: Fill,
                height: 200,
                spacing: 0.0
                flow: Down,

                wallet_entry = <WalletEntry> { }
                empty = <View> { }
                bottom_filler = <View> {
                    width: Fill,
                    height: 100.0,
                }
            }
        }

        <View> {
            margin: {top: 5},
            width: Fill, height: Fit
            flow: RightWrap,
            align: {y: 0.5},
            spacing: 10

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
                draw_icon: {
                    svg_file: (ICON_IMPORT)
                    color: (COLOR_PRIMARY)
                }
                draw_text: {
                    color: (COLOR_PRIMARY)
                    text_style: <REGULAR_TEXT> {}
                }
                icon_walk: {width: 16, height: 16}
                text: "Import Existing Wallet"
            }
        }
    }
}

#[derive(Debug, Default)]
struct WalletState {
    active_wallet: Option<TspWalletMetadata>,
    other_wallets: Vec<(TspWalletMetadata, WalletStatus)>,
}
impl WalletState {
    fn is_empty(&self) -> bool {
        self.active_wallet.is_none() && self.other_wallets.is_empty()
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

#[derive(Debug, Clone, Copy)]
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
        // If we don't have any wallets, show the "no wallets" label.
        let is_wallets_empty = self.wallets.as_ref().is_none_or(|w| w.is_empty());
        self.view.view(id!(no_wallets_label)).set_visible(cx, is_wallets_empty);

        while let Some(subview) = self.view.draw_walk(cx, scope, walk).step() {
            // Here, we only need to handle drawing the portal list.
            let portal_list_ref = subview.as_portal_list();
            let Some(mut list_ref) = portal_list_ref.borrow_mut() else {
                error!("!!! TspSettingsScreen::draw_walk(): BUG: expected a PortalList widget, but got something else");
                continue;
            };
            let Some(wallets) = self.wallets.as_ref() else {
                return DrawStep::done();
            };
            let portal_list_height = if is_wallets_empty { 0.0 } else { 200.0 };
            // Hide the list if there are no wallets
            list_ref.apply_over(cx, live!(
                height: (portal_list_height),
            ));

            // Set the portal list's range based on the number of timeline items.
            let last_item_id = wallets.active_wallet.is_some() as usize + wallets.other_wallets.len();
            let list = list_ref.deref_mut();
            list.set_item_range(cx, 0, last_item_id);

            while let Some(item_id) = list.next_visible_item(cx) {
                if let Some((metadata, mut status_and_default)) = wallets.get(item_id) {
                    let item = list.item(cx, item_id, live_id!(wallet_entry));
                    // Pass the wallet metadata in through Scope via props,
                    // and status/is_default via data.
                    let mut scope = Scope::with_data_props(&mut status_and_default, metadata);
                    item.draw_all(cx, &mut scope);
                } else {
                    list.item(cx, item_id, live_id!(bottom_filler))
                        .draw_all(cx, scope);
                }
                
            }
        }
        DrawStep::done()
    }
}

impl MatchEvent for TspSettingsScreen {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions) {
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
                }

                // Remove the wallet from the list of drawn wallets.
                Some(TspWalletAction::WalletRemoved(metadata)) => {
                    if let Some(wallets) = &mut self.wallets {
                        // If the wallet was the active one, clear it.
                        if wallets.active_wallet.as_ref() == Some(metadata) {
                            wallets.active_wallet = None;
                            self.view.redraw(cx);
                        } else if let Some(pos) = wallets.other_wallets.iter().position(|(w, _)| w == metadata) {
                            wallets.other_wallets.remove(pos);
                            self.view.redraw(cx);
                        } else {
                            error!("BUG: TspSettingsScreen::handle_actions(): Wallet deleted, but not found in the list.");
                            self.refresh_wallets();
                        }
                    } else {
                        error!("BUG: TspSettingsScreen::handle_actions(): Wallet deleted, but no wallets list exists.");
                        self.refresh_wallets();
                    }
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
                }
                Some(TspWalletAction::DefaultWalletChanged(Err(_))) => {
                    enqueue_popup_notification(PopupItem {
                        message: String::from("Failed to set default wallet, could not find or open selected wallet."),
                        auto_dismissal_duration: None,
                        // PopupKind::Error,
                    });
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
                }
                Some(TspWalletAction::WalletOpened(Err(e))) => {
                    enqueue_popup_notification(PopupItem {
                        message: format!("Failed to open wallet: {e}"),
                        auto_dismissal_duration: None,
                        // PopupKind::Error,
                    });
                }

                Some(TspWalletAction::CreateWalletError { .. }) // handled in the CreateWalletModal
                | None => { }
            }
        }

        if self.view.button(id!(create_wallet_button)).clicked(actions) {
            cx.action(CreateWalletModalAction::Open);
        }

        if self.view.button(id!(import_wallet_button)).clicked(actions) {
            // TODO: support importing an existing wallet.
            enqueue_popup_notification(PopupItem {
                message: String::from("Importing an existing wallet is not yet implemented."),
                auto_dismissal_duration: Some(4.0),
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
        });
    }
}
