use makepad_widgets::*;
use matrix_sdk::ruma::OwnedUserId;

use crate::{shared::popup_list::{enqueue_popup_notification, PopupItem, PopupKind}, tsp::{submit_tsp_request, tsp_state_ref, TspIdentityAction, TspRequest}};

live_design! {
    link tsp_enabled

    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::helpers::*;
    use crate::shared::styles::*;
    use crate::shared::avatar::*;
    use crate::shared::icon_button::*;

    // A view that allows the user to verify a new DID and associate it
    // with a particular Matrix User ID.
    // This is currently shown as part of the UserProfileSlidingPane.
    pub TspVerifyUser = {{TspVerifyUser}} {
        width: Fill, height: Fit
        flow: Down
        spacing: 20,

        <LineH> { padding: 15 }

        <View> {
            width: Fill, height: Fit
            flow: Down
            spacing: 10
            padding: { left: 10, right: 10, bottom: 10}

            <Label> {
                width: Fill, height: Fit
                draw_text: {
                    wrap: Word,
                    text_style: <USERNAME_TEXT_STYLE>{ font_size: 11.5 },
                    color: #000
                }
                text: "TSP User Verification"
            }

            // Content shown when this user has been verified via TSP.
            verified_tsp = <View> {
                visible: false,
                width: Fill, height: Fit
                flow: Down,
                spacing: 10,
                // margin: { left: 7 }

                <Label> {
                    width: Fill, height: Fit
                    draw_text: {
                        wrap: Line,
                        color: (COLOR_FG_ACCEPT_GREEN),
                        text_style: <MESSAGE_TEXT_STYLE>{ font_size: 11 },
                    }
                    text: "✅ Verified via TSP"
                }

                tsp_did_read_only_input = <SimpleTextInput> {
                    is_read_only: true
                }

                remove_tsp_association_button = <RobrixIconButton> {
                    padding: {top: 10, bottom: 10, left: 12, right: 15}
                    draw_bg: {
                        border_color: (COLOR_FG_DANGER_RED),
                        color: (COLOR_BG_DANGER_RED)
                    }
                    draw_icon: {
                        svg_file: (ICON_CLOSE)
                        color: (COLOR_FG_DANGER_RED),
                    }
                    draw_text: {
                        color: (COLOR_FG_DANGER_RED),
                        text_style: <REGULAR_TEXT> {}
                    }
                    icon_walk: {width: 22, height: 16, margin: {left: -5, right: -3, top: 1, bottom: -1} }
                    text: "Remove TSP Association"
                }
            }


            // Content shown when this user has NOT been verified via TSP.
            unverified_tsp = <View> {
                visible: true,
                width: Fill, height: Fit
                flow: Down,
                spacing: 10,
                // margin: { left: 7 }

                <Label> {
                    width: Fill, height: Fit
                    flow: RightWrap,
                    draw_text: {
                        wrap: Word,
                        color: (MESSAGE_TEXT_COLOR),
                        text_style: <MESSAGE_TEXT_STYLE>{ font_size: 11 },
                    }
                    text: "Interactively verify this user by associating their TSP identity (DID) with their Matrix User ID:"
                }

                tsp_did_input = <SimpleTextInput> {
                    empty_text: "Enter their TSP DID..."
                }

                verify_user_button = <RobrixIconButton> {
                    padding: {top: 10, bottom: 10, left: 12, right: 15}
                    draw_bg: {
                        border_color: (COLOR_FG_ACCEPT_GREEN),
                        color: (COLOR_BG_ACCEPT_GREEN)
                    }
                    draw_icon: {
                        svg_file: (ICON_CHECKMARK)
                        color: (COLOR_FG_ACCEPT_GREEN),
                    }
                    draw_text: {
                        color: (COLOR_FG_ACCEPT_GREEN)
                        text_style: <REGULAR_TEXT> {}
                    }
                    icon_walk: {width: 22, height: 16, margin: {left: -5, right: -3, top: 1, bottom: -1} }
                    text: "Verify this user via TSP"
                }
            }
        }
    }
}

/// Whether another user has been verified using TSP.
#[derive(Default)]
pub enum TspVerifiedInfo {
    #[default]
    Unverified,
    Verified {
        did: String,
    },
}

#[derive(Live, LiveHook, Widget)]
pub struct TspVerifyUser {
    #[deref] view: View,
    /// The Matrix User ID of the other user that we want to verify.
    #[rust] user_id: Option<OwnedUserId>,
    /// Info about whether the other user has or has not been verified via TSP.
    #[rust] verified_info: TspVerifiedInfo,
}

impl Widget for TspVerifyUser {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        self.match_event(cx, event);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}
impl MatchEvent for TspVerifyUser {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        if self.view.button(ids!(remove_tsp_association_button)).clicked(actions) {
            enqueue_popup_notification(PopupItem {
                message: "Removing a TSP association is not yet implemented".into(),
                auto_dismissal_duration: Some(5.0),
                kind: PopupKind::Warning,
            });
        }

        let verify_user_button = self.view.button(ids!(verify_user_button));
        if verify_user_button.clicked(actions) {
            let did_input = self.view.view(ids!(tsp_did_input));
            let did = did_input.text().trim().to_string();
            log!("verify_user_button was clicked. DID: {}", did);
            if did.is_empty() {
                enqueue_popup_notification(PopupItem {
                    message: "Please enter a valid TSP DID to verify this user.".into(),
                    auto_dismissal_duration: Some(5.0),
                    kind: PopupKind::Error,
                });
            } else if let Some(user_id) = self.user_id.clone() {
                submit_tsp_request(TspRequest::AssociateDidWithUserId { did, user_id });
                verify_user_button.set_enabled(cx, false);
                verify_user_button.set_text(cx, "Sending request...");
            }
        }

        for action in actions {
            match action.downcast_ref() {
                Some(TspIdentityAction::SentDidAssociationRequest { user_id, .. })
                    if Some(user_id) == self.user_id.as_ref() =>
                {
                    verify_user_button.set_text(cx, "Sent request!");
                    enqueue_popup_notification(PopupItem {
                        message: format!("Sent TSP verification request.\n\nWaiting for \"{user_id}\" to respond..."),
                        auto_dismissal_duration: Some(5.0),
                        kind: PopupKind::Info,
                    });
                }
                Some(TspIdentityAction::ErrorSendingDidAssociationRequest { user_id, error, .. })
                    if Some(user_id) == self.user_id.as_ref() =>
                {
                    verify_user_button.set_enabled(cx, true);
                    verify_user_button.set_text(cx, "Verify this user via TSP");
                    enqueue_popup_notification(PopupItem {
                        message: format!("Error sending TSP verification request to \"{user_id}\": {error}"),
                        auto_dismissal_duration: None,
                        kind: PopupKind::Error,
                    });
                }
                Some(TspIdentityAction::ReceivedDidAssociationResponse { did, user_id, accepted })
                    if Some(user_id) == self.user_id.as_ref() =>
                {
                    if *accepted {
                        enqueue_popup_notification(PopupItem {
                            message: format!("User \"{user_id}\" accepted your TSP verification request."),
                            auto_dismissal_duration: None,
                            kind: PopupKind::Success,
                        });
                        self.verified_info = TspVerifiedInfo::Verified { did: did.clone() };
                    } else {
                        enqueue_popup_notification(PopupItem {
                            message: format!("User \"{user_id}\" rejected your TSP verification request."),
                            auto_dismissal_duration: None,
                            kind: PopupKind::Warning,
                        });
                    }
                    // Repopulate the content of this widget.
                    self.refresh_from_verified_info(cx);
                    self.redraw(cx);
                }
                _ => {}
            }
        }
    }
}

impl TspVerifyUser {
    /// Repopulates this widget's UI content from its inner verified info.
    fn refresh_from_verified_info(&mut self, cx: &mut Cx) {
        let verified_tsp_view = self.view.view(ids!(verified_tsp));
        let unverified_tsp_view = self.view.view(ids!(unverified_tsp));
        match &self.verified_info {
            TspVerifiedInfo::Verified { did } => {
                verified_tsp_view.set_visible(cx, true);
                unverified_tsp_view.set_visible(cx, false);
                verified_tsp_view.text_input(ids!(tsp_did_read_only_input)).set_text(cx, did);
            }
            TspVerifiedInfo::Unverified => {
                verified_tsp_view.set_visible(cx, false);
                unverified_tsp_view.set_visible(cx, true);
                unverified_tsp_view.text_input(ids!(tsp_did_input)).set_text(cx, "");
                let verify_user_button = unverified_tsp_view.button(ids!(verify_user_button));
                verify_user_button.set_enabled(cx, true);
                verify_user_button.set_text(cx, "Verify this user via TSP");
            }
        }
    }

    fn show(&mut self, cx: &mut Cx, user_id: OwnedUserId) {
        let verified_info = tsp_state_ref().lock().unwrap()
            .get_associated_did(&user_id)
            .map_or(
                TspVerifiedInfo::Unverified,
                |did| TspVerifiedInfo::Verified { did: did.to_string() },
            );

        self.verified_info = verified_info;
        self.user_id = Some(user_id);
        self.refresh_from_verified_info(cx);
    }
}

impl TspVerifyUserRef {
    pub fn show(&self, cx: &mut Cx, user_id: OwnedUserId) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.show(cx, user_id);
    }
}
