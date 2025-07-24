use makepad_widgets::*;

use crate::shared::popup_list::{enqueue_popup_notification, PopupItem};

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::helpers::*;
    use crate::shared::styles::*;
    use crate::shared::avatar::*;
    use crate::shared::icon_button::*;

    WalletList = <View> {
        width: Fill, height: Fit
        flow: Down

        // Placeholder for wallet items, to be filled dynamically.
        no_wallets_label = <Label> {
            width: Fill, height: Fit
            flow: RightWrap,
            draw_text: {
                wrap: Line,
                color: (MESSAGE_TEXT_COLOR),
                text_style: <MESSAGE_TEXT_STYLE>{ font_size: 11 },
            }
            text: "No wallets found. Create or import a wallet below."
        }
    }

    // The view containing all TSP-related settings.
    pub TspSettings = {{TspSettings}} {
        width: Fill, height: Fit
        flow: Down

        <TitleLabel> {
            text: "TSP Wallet Settings"
        }

        <SubsectionLabel> {
            text: "Your Wallets:"
        }

        wallet_list = <WalletList> { }

        <View> {
            // margin: {top: 20},
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

/// The view containing all TSP-related settings.
#[derive(Live, LiveHook, Widget)]
pub struct TspSettings {
    #[deref] view: View,
}

impl Widget for TspSettings {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.match_event(cx, event);
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl MatchEvent for TspSettings {
    fn handle_actions(&mut self, _cx: &mut Cx, actions: &Actions) {
        if self.view.button(id!(create_wallet_button)).clicked(actions) {
            // TODO: support creating a new wallet.
            enqueue_popup_notification(PopupItem {
                message: String::from("Creating a new wallet is not yet implemented."),
                auto_dismissal_duration: Some(4.0),
            });
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
