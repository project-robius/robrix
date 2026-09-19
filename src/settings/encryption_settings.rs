//! Settings related to encryption: recovery key management and encryption identity reset.

use std::cell::RefCell;
use makepad_widgets::*;
use matrix_sdk::encryption::recovery::RecoveryState;
use url::Url;

use crate::{
    app::{ConfirmDeleteAction, PositiveConfirmationModalAction},
    shared::password_input::PasswordTextInputWidgetExt,
    login::login_screen::LoginAction,
    logout::logout_confirm_modal::LogoutAction,
    shared::{confirmation_modal::ConfirmationModalContent, popup_list::{enqueue_popup_notification, PopupKind}},
    sliding_sync::{submit_async_request, IdentityResetAuth, MatrixRequest, RecoveryAction},
    utils,
};

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.EncryptionSettingsButton = mod.widgets.RobrixIconButton {
        width: Fit, height: mod.widgets.SETTINGS_BUTTON_HEIGHT
        padding: Inset{top: 10, bottom: 10, left: 15, right: 15}
        margin: Inset{top: 10, left: 5, bottom: 4}
        icon_walk: Walk{width: 0, height: 0, margin: 0}
        spacing: 0
    }

    mod.widgets.EncryptionSettingsText = Label {
        width: Fill {max: 478}, height: Fit
        flow: Flow.Right{wrap: true}
        margin: Inset{top: 4}
        draw_text +: {
            color: (MESSAGE_TEXT_COLOR),
            text_style: theme.font_regular { font_size: 11.5 },
        }
    }

    mod.widgets.EncryptionModalButton = mod.widgets.RobrixNeutralIconButton {
        width: Fit{min: FitBound.Abs(110.0)},
        align: Align{x: 0.5, y: 0.5}
        padding: 15,
        icon_walk: Walk{width: 0, height: 0, margin: 0}
    }

    mod.widgets.EncryptionModalInput = mod.widgets.RobrixTextInput {
        width: Fill {max: 350}, height: Fit
        padding: 10
        margin: Inset{top: 5, bottom: 5}
        flow: Flow.Right { wrap: false }
        autocapitalize: None,
        autocorrect: Disabled,
        draw_text +: {
            text_style: REGULAR_TEXT {font_size: 12},
            color: #000
        }
    }

    mod.widgets.EncryptionSettings = #(EncryptionSettings::register_widget(vm)) {
        width: Fill, height: Fit
        flow: Down

        TitleLabel { text: "Encryption Settings" }

        SubsectionLabel { text: "Key backup & recovery" }

        View {
            width: Fill, height: Fit
            flow: Flow.Right{wrap: false}
            align: Align{y: 0.5}
            margin: Inset{top: 4}
            spacing: 6

            recovery_status_spinner := LoadingSpinner {
                width: 13, height: 13
                margin: Inset{left: 5}
                draw_bg.color: (COLOR_ACTIVE_PRIMARY)
            }

            recovery_status_label := mod.widgets.EncryptionSettingsText {
                margin: 0
                text: "Checking your key backup..."
            }
        }

        View {
            width: Fill, height: Fit
            flow: Flow.Right{wrap: true}

            enable_recovery_button := mod.widgets.EncryptionSettingsButton {
                visible: false
                text: "Set up key backup"
            }
            recover_button := mod.widgets.EncryptionSettingsButton {
                visible: false
                text: "Enter recovery key"
            }
            change_key_button := mod.widgets.EncryptionSettingsButton {
                visible: false
                text: "Change recovery key"
            }
        }

        SubsectionLabel { text: "Encryption identity" }

        mod.widgets.EncryptionSettingsText {
            text: "If you've lost your recovery key and have no other verified devices, you can reset your encryption identity. This deletes your key backup, so you'll lose any encrypted message history that isn't already on this device. Other users will need to verify your new identity again."
        }

        reset_identity_button := RobrixNegativeIconButton {
            width: Fit, height: mod.widgets.SETTINGS_BUTTON_HEIGHT
            padding: Inset{top: 10, bottom: 10, left: 15, right: 15}
            margin: Inset{top: 10, left: 5, bottom: 4}
            icon_walk: Walk{width: 0, height: 0, margin: 0}
            spacing: 0
            text: "Reset encryption identity"
        }
    }

    mod.widgets.EncryptionModal = set_type_default() do #(EncryptionModal::register_widget(vm)) {
        ..mod.widgets.SmallModal
        align: Align{x: 0.5}

        title := ModalTitle { text: "" }

        View {
            width: Fill, height: Fit
            flow: Flow.Right{wrap: false}
            align: Align{y: 0.5}
            margin: Inset{top: 5, bottom: 5}
            spacing: 8

            busy_spinner := LoadingSpinner {
                visible: false
                width: 13, height: 13
                draw_bg.color: (COLOR_ACTIVE_PRIMARY)
            }

            body := ModalBody {
                margin: 0
                text: ""
            }
        }

        key_view := RoundedView {
            visible: false
            width: Fill {max: 350}, height: Fit
            padding: 12
            margin: Inset{top: 5, bottom: 5}
            show_bg: true
            draw_bg +: {
                color: (COLOR_SECONDARY)
                border_radius: 4.0
            }

            key_label := Label {
                width: Fill, height: Fit
                flow: Flow.Right{wrap: true}
                draw_text +: {
                    color: #000
                    text_style: theme.font_regular { font_size: 12 }
                }
                text: ""
            }
        }

        // wrap TextInput in a view so we can hide it
        key_input_view := View {
            visible: false
            width: Fill {max: 350}, height: Fit
            key_input := mod.widgets.EncryptionModalInput {
                empty_text: "Recovery key"
            }
        }

        password_input := mod.widgets.PasswordTextInput {
            visible: false
            width: Fill {max: 350}
            margin: Inset{top: 5, bottom: 5}
            text_input +: {
                draw_text +: {
                    text_style: REGULAR_TEXT {font_size: 12},
                    color: #000
                }
            }
        }

        error_label := ModalBody {
            visible: false
            margin: Inset{top: 5}
            draw_text +: {
                text_style: REGULAR_TEXT {font_size: 11}
                color: (COLOR_FG_DANGER_RED)
            }
            text: ""
        }

        buttons_view := ModalButtonsRow {
            cancel_button := mod.widgets.EncryptionModalButton {
                text: "Cancel"
            }
            done_button := mod.widgets.EncryptionModalButton {
                visible: false
                text: "Done"
            }
            accept_button := RobrixPositiveIconButton {
                width: Fit{min: FitBound.Abs(110.0)},
                align: Align{x: 0.5, y: 0.5}
                padding: 15,
                icon_walk: Walk{width: 0, height: 0, margin: 0}
                text: "OK"
            }
        }
    }
}

/// The state of the `EncryptionModal`.
#[derive(Clone)]
pub enum EncryptionModalState {
    /// Waiting on something, no buttons are shown.
    Busy {
        title: &'static str,
        body: &'static str,
    },
    /// The recovery key is being shown.
    ShowRecoveryKey {
        key: String,
        is_replacement: bool,
    },
    /// Waiting for the user to enter their recovery key.
    EnterRecoveryKey {
        error: Option<String>,
    },
    /// Waiting for the user to enter their password to reset their encryption identity.
    EnterPassword {
        error: Option<String>,
    },
    /// Waiting on the already-open homeserver's approval page (in the browser) for an identity reset.
    WaitingForApproval {
        error: Option<String>,
    },
    BackupHeldByOtherDevice,
}
impl std::fmt::Debug for EncryptionModalState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Busy { title, .. } => write!(f, "Busy({title})"),
            Self::ShowRecoveryKey { is_replacement, .. } => write!(f, "ShowRecoveryKey(is_replacement: {is_replacement})"),
            Self::EnterRecoveryKey { error } => write!(f, "EnterRecoveryKey({error:?})"),
            Self::EnterPassword { error } => write!(f, "EnterPassword({error:?})"),
            Self::WaitingForApproval { error } => write!(f, "WaitingForApproval({error:?})"),
            Self::BackupHeldByOtherDevice => write!(f, "BackupHeldByOtherDevice"),
        }
    }
}

/// Plain actions telling the app, which hosts the modal, to show or close it.
#[derive(Clone, Debug)]
pub enum EncryptionModalAction {
    Show(EncryptionModalState),
    Close,
}

/// Widget actions the modal's buttons post back to the settings section.
#[derive(Clone, Default)]
pub enum EncryptionModalResponse {
    /// Includes whatever the user typed into the state's input, if any.
    Accepted(String),
    Cancelled,
    #[default]
    None,
}
impl std::fmt::Debug for EncryptionModalResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Accepted(_) => write!(f, "Accepted(<redacted>)"),
            Self::Cancelled => write!(f, "Cancelled"),
            Self::None => write!(f, "None"),
        }
    }
}

/// Which recovery request the settings screen is waiting on.
#[derive(Clone, Copy, PartialEq, Eq)]
enum PendingRecoveryRequest {
    EnableRecovery,
    ChangeKey,
    Recover,
}

/// How far an identity reset has gotten.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum IdentityResetStage {
    #[default]
    None,
    /// Requested, but the homeserver hasn't said how to approve it yet.
    Starting,
    /// Waiting for the user to enter their account password.
    Password,
    /// Waiting for the user to approve the reset on the homeserver's page.
    Browser,
}

/// Actions posted by the confirmation modal so we know what the user confirmed.
#[derive(Clone, Copy, Debug)]
enum EncryptionConfirmedAction {
    ChangeKey,
    ResetIdentity,
    SavedRecoveryKey,
}

#[derive(Script, Widget)]
pub struct EncryptionSettings {
    #[deref] view: View,

    /// The latest known recovery state, used to determine what content to show here.
    #[rust(RecoveryState::Unknown)] recovery_state: RecoveryState,
    /// What the modal is showing, or `None` while it's closed.
    #[rust] modal_mode: Option<EncryptionModalState>,
    /// The recovery request we're waiting on, so its answer goes to the right place.
    #[rust] pending_request: Option<PendingRecoveryRequest>,
    /// How far the in-progress identity reset has gotten.
    #[rust] reset_stage: IdentityResetStage,
    /// The homeserver's approval page for the in-progress reset, which the user can reopen.
    #[rust] approval_url: Option<Url>,
    /// An abandoned or failed reset leaves the identity half reset until it's run again.
    #[rust] is_reset_incomplete: bool,
}

impl ScriptHook for EncryptionSettings {
    fn on_after_apply(
        &mut self,
        vm: &mut ScriptVm,
        _apply: &Apply,
        _scope: &mut Scope,
        _value: ScriptValue,
    ) {
        self.populate_inner(vm.cx_mut());
    }
}

impl Widget for EncryptionSettings {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        self.match_event(cx, event);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl MatchEvent for EncryptionSettings {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        if self.view.button(cx, ids!(enable_recovery_button)).clicked(actions) {
            self.pending_request = Some(PendingRecoveryRequest::EnableRecovery);
            submit_async_request(MatrixRequest::EnableRecovery);
            self.show_modal(cx, EncryptionModalState::Busy {
                title: "Setting up key backup...",
                body: "Creating your recovery key and backing up your encryption keys.",
            });
        }
        if self.view.button(cx, ids!(change_key_button)).clicked(actions) {
            cx.action(PositiveConfirmationModalAction::Show(RefCell::new(Some(ConfirmationModalContent {
                title_text: "Change your recovery key?".into(),
                body_text: "Your current recovery key will stop working right away, and you'll receive a new one that you should save and keep safe.".into(),
                accept_button_text: Some("Change key".into()),
                on_accept_clicked: Some(Box::new(|cx| cx.action(EncryptionConfirmedAction::ChangeKey))),
                ..Default::default()
            }))));
        }
        if self.view.button(cx, ids!(recover_button)).clicked(actions) {
            self.show_modal(cx, EncryptionModalState::EnterRecoveryKey { error: None });
        }
        if self.view.button(cx, ids!(reset_identity_button)).clicked(actions) {
            cx.action(ConfirmDeleteAction::Show(RefCell::new(Some(ConfirmationModalContent {
                title_text: "Reset encryption identity?".into(),
                body_text: "This creates a new encryption identity and deletes your key backup and recovery key right away. \
                    You'll lose any encrypted message history that isn't already on this device, even if it was backed up, \
                    and other users will need to verify your new identity again.\n\n\
                    Your homeserver may ask you to approve the reset. If you stop before approving, \
                    the reset stays incomplete until you run it again.".into(),
                accept_button_text: Some("Reset".into()),
                on_accept_clicked: Some(Box::new(|cx| cx.action(EncryptionConfirmedAction::ResetIdentity))),
                ..Default::default()
            }))));
        }

        for action in actions {
            match action.as_widget_action().cast() {
                EncryptionModalResponse::Accepted(input) => match self.modal_mode.clone() {
                    Some(EncryptionModalState::EnterRecoveryKey { .. }) => {
                        self.pending_request = Some(PendingRecoveryRequest::Recover);
                        submit_async_request(MatrixRequest::RecoverWithKey { recovery_key: input.trim().to_owned() });
                        self.show_modal(cx, EncryptionModalState::Busy {
                            title: "Restoring from your recovery key...",
                            body: "Importing your backed-up encryption keys.",
                        });
                    }
                    Some(EncryptionModalState::EnterPassword { .. }) => {
                        submit_async_request(MatrixRequest::ContinueIdentityReset { password: Some(input) });
                        self.show_modal(cx, EncryptionModalState::Busy {
                            title: "Resetting encryption identity...",
                            body: "Waiting for your homeserver...",
                        });
                    }
                    Some(EncryptionModalState::WaitingForApproval { error: None }) => {
                        if let Some(url) = &self.approval_url {
                            utils::open_url(url.as_str());
                        }
                    }
                    Some(EncryptionModalState::WaitingForApproval { error: Some(_) }) => {
                        submit_async_request(MatrixRequest::ContinueIdentityReset { password: None });
                        self.show_modal(cx, EncryptionModalState::WaitingForApproval { error: None });
                    }
                    Some(EncryptionModalState::BackupHeldByOtherDevice) => {
                        submit_async_request(MatrixRequest::RequestSelfVerification);
                        self.close_modal(cx);
                    }
                    Some(EncryptionModalState::ShowRecoveryKey { .. }) => {
                        cx.action(PositiveConfirmationModalAction::Show(RefCell::new(Some(ConfirmationModalContent {
                            title_text: "Have you saved your recovery key?".into(),
                            body_text: "You won't be able to see this key again. Keep it somewhere safe, like a password manager.".into(),
                            accept_button_text: Some("Yes, I saved it".into()),
                            cancel_button_text: Some("No, show it again".into()),
                            on_accept_clicked: Some(Box::new(|cx| cx.action(EncryptionConfirmedAction::SavedRecoveryKey))),
                            ..Default::default()
                        }))));
                    }
                    Some(EncryptionModalState::Busy { .. }) | None => {}
                },
                EncryptionModalResponse::Cancelled => {
                    if matches!(
                        self.modal_mode,
                        Some(EncryptionModalState::EnterPassword { .. } | EncryptionModalState::WaitingForApproval { .. })
                    ) {
                        submit_async_request(MatrixRequest::AbandonIdentityReset);
                    }
                    self.close_modal(cx);
                }
                _ => {}
            }

            match action.downcast_ref() {
                Some(EncryptionConfirmedAction::ChangeKey) => {
                    self.pending_request = Some(PendingRecoveryRequest::ChangeKey);
                    submit_async_request(MatrixRequest::ResetRecoveryKey);
                    self.show_modal(cx, EncryptionModalState::Busy {
                        title: "Changing your recovery key...",
                        body: "Creating your new recovery key.",
                    });
                }
                Some(EncryptionConfirmedAction::ResetIdentity) => {
                    self.reset_stage = IdentityResetStage::Starting;
                    submit_async_request(MatrixRequest::ResetIdentity);
                    self.show_modal(cx, EncryptionModalState::Busy {
                        title: "Resetting encryption identity...",
                        body: "Waiting for your homeserver...",
                    });
                    self.populate_inner(cx);
                }
                Some(EncryptionConfirmedAction::SavedRecoveryKey) => self.close_modal(cx),
                None => {}
            }

            match action.downcast_ref() {
                Some(RecoveryAction::StateChanged(state)) => {
                    self.recovery_state = *state;
                    self.populate_inner(cx);
                }
                Some(RecoveryAction::RecoveryKeyCreated(key)) => {
                    let is_replacement = self.pending_request == Some(PendingRecoveryRequest::ChangeKey);
                    self.pending_request = None;
                    self.show_modal(cx, EncryptionModalState::ShowRecoveryKey { key: key.clone(), is_replacement });
                }
                Some(RecoveryAction::BackupHeldByOtherDevice) => {
                    self.pending_request = None;
                    self.show_modal(cx, EncryptionModalState::BackupHeldByOtherDevice);
                }
                Some(RecoveryAction::Recovered) => {
                    self.pending_request = None;
                    self.close_modal(cx);
                    enqueue_popup_notification(
                        "Restored your backed-up encryption keys from the recovery key.",
                        PopupKind::Success,
                        Some(5.0),
                    );
                }
                Some(RecoveryAction::RecoveryFailed(error)) => {
                    if self.pending_request.take() == Some(PendingRecoveryRequest::Recover) {
                        self.show_modal(cx, EncryptionModalState::EnterRecoveryKey { error: Some(error.clone()) });
                    } else {
                        self.close_modal(cx);
                        enqueue_popup_notification(error.clone(), PopupKind::Error, None);
                    }
                }
                Some(RecoveryAction::IdentityResetNeedsApproval(auth)) => {
                    match auth {
                        IdentityResetAuth::BrowserApproval(url) => {
                            self.reset_stage = IdentityResetStage::Browser;
                            self.approval_url = Some(url.clone());
                            utils::open_url(url.as_str());
                            submit_async_request(MatrixRequest::ContinueIdentityReset { password: None });
                            self.show_modal(cx, EncryptionModalState::WaitingForApproval { error: None });
                        }
                        IdentityResetAuth::Password => {
                            self.reset_stage = IdentityResetStage::Password;
                            self.show_modal(cx, EncryptionModalState::EnterPassword { error: None });
                        }
                    }
                    self.populate_inner(cx);
                }
                Some(RecoveryAction::IdentityResetDone) => {
                    self.finish_identity_reset(cx, false);
                    enqueue_popup_notification(
                        "Your encryption identity was reset. Set up key backup again for your new encryption keys.",
                        PopupKind::Success,
                        Some(8.0),
                    );
                }
                Some(RecoveryAction::IdentityResetFailed { error, is_incomplete }) => match self.reset_stage {
                    IdentityResetStage::Password => {
                        self.show_modal(cx, EncryptionModalState::EnterPassword { error: Some(error.clone()) });
                    }
                    IdentityResetStage::Browser => {
                        self.show_modal(cx, EncryptionModalState::WaitingForApproval { error: Some(error.clone()) });
                    }
                    IdentityResetStage::Starting => {
                        self.finish_identity_reset(cx, *is_incomplete);
                        enqueue_popup_notification(error.clone(), PopupKind::Error, None);
                    }
                    IdentityResetStage::None => enqueue_popup_notification(error.clone(), PopupKind::Error, None),
                },
                Some(RecoveryAction::IdentityResetAbandoned) => {
                    self.finish_identity_reset(cx, true);
                    enqueue_popup_notification(
                        "The encryption identity reset was cancelled. Try it again to finish the process.",
                        PopupKind::Warning,
                        Some(8.0),
                    );
                }
                None => {}
            }

            // A new session needs to reset everything here, since encryption identity has changed.
            if matches!(action.downcast_ref(), Some(LoginAction::LoginSuccess))
                || matches!(action.downcast_ref(), Some(LogoutAction::ClearAppState { .. }))
            {
                self.recovery_state = RecoveryState::Unknown;
                self.pending_request = None;
                self.reset_stage = IdentityResetStage::None;
                self.approval_url = None;
                self.is_reset_incomplete = false;
                if self.modal_mode.is_some() {
                    self.close_modal(cx);
                }
                self.populate_inner(cx);
            }
        }
    }
}

impl EncryptionSettings {
    /// Asks for the current recovery state; the section is created lazily, so it may
    /// have missed the subscriber's earlier updates.
    fn populate(&mut self, cx: &mut Cx) {
        submit_async_request(MatrixRequest::GetRecoveryState);
        self.populate_inner(cx);
    }

    fn show_modal(&mut self, cx: &mut Cx, mode: EncryptionModalState) {
        self.modal_mode = Some(mode.clone());
        cx.action(EncryptionModalAction::Show(mode));
    }

    fn close_modal(&mut self, cx: &mut Cx) {
        self.modal_mode = None;
        cx.action(EncryptionModalAction::Close);
    }

    fn finish_identity_reset(&mut self, cx: &mut Cx, is_incomplete: bool) {
        self.reset_stage = IdentityResetStage::None;
        self.approval_url = None;
        self.is_reset_incomplete = is_incomplete;
        self.close_modal(cx);
        self.populate_inner(cx);
    }

    fn populate_inner(&mut self, cx: &mut Cx) {
        let is_resetting = self.reset_stage != IdentityResetStage::None;
        let (status, can_enable, can_recover, can_change_key) = if self.is_reset_incomplete {
            ("Your encryption identity reset didn't finish. Run it again to complete it; you can set up key backup afterwards.", false, false, false)
        } else {
            match self.recovery_state {
                RecoveryState::Unknown => ("Checking your key backup...", false, false, false),
                RecoveryState::Enabled => (
                    "Your encryption keys are already backed up. Enter your recovery key on a new device to read your encrypted messages there. If you've lost the key, you can change it to get a new one.",
                    false, false, true,
                ),
                RecoveryState::Disabled => (
                    "Only this device can read your encrypted messages. Please back up your encryption keys now, and then save the recovery key Robrix gives you in order to restore them on a new device in the future. This prevents you from losing your encrypted message history.",
                    true, false, false,
                ),
                RecoveryState::Incomplete => (
                    "This device is missing some of your backed-up encryption keys, so it can't read your full encrypted history. Enter your recovery key to restore them.",
                    false, true, false,
                ),
            }
        };
        let is_checking = !self.is_reset_incomplete && matches!(self.recovery_state, RecoveryState::Unknown);
        self.view.widget(cx, ids!(recovery_status_spinner)).set_visible(cx, is_checking);
        self.view.label(cx, ids!(recovery_status_label)).set_text(cx, status);
        self.view.button(cx, ids!(enable_recovery_button)).set_visible(cx, can_enable && !is_resetting);
        self.view.button(cx, ids!(recover_button)).set_visible(cx, can_recover && !is_resetting);
        self.view.button(cx, ids!(change_key_button)).set_visible(cx, can_change_key && !is_resetting);
        self.view.button(cx, ids!(reset_identity_button)).set_enabled(cx, !is_resetting);
        self.redraw(cx);
    }
}

impl EncryptionSettingsRef {
    /// See [`EncryptionSettings::populate()`].
    pub fn populate(&self, cx: &mut Cx) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.populate(cx);
    }
}

#[derive(Script, ScriptHook, Widget)]
pub struct EncryptionModal {
    #[deref] view: View,

    #[rust] mode: Option<EncryptionModalState>,
    #[rust] is_input_focus_pending: bool,
}

impl Widget for EncryptionModal {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        self.widget_match_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        let step = self.view.draw_walk(cx, scope, walk);
        if self.is_input_focus_pending {
            self.is_input_focus_pending = false;
            match self.mode {
                Some(EncryptionModalState::EnterRecoveryKey { .. }) => self.view.text_input(cx, ids!(key_input)).set_key_focus(cx),
                Some(EncryptionModalState::EnterPassword { .. }) => self.view.password_text_input(cx, ids!(password_input)).set_key_focus(cx),
                _ => {}
            }
        }
        step
    }
}

impl WidgetMatchEvent for EncryptionModal {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions, _scope: &mut Scope) {
        let widget_uid = self.widget_uid();
        let key_input = self.view.text_input(cx, ids!(key_input));
        let password_input = self.view.password_text_input(cx, ids!(password_input));

        let accept_clicked = self.view.button(cx, ids!(accept_button)).clicked(actions);
        let done_clicked = self.view.button(cx, ids!(done_button)).clicked(actions);

        // Copying is the primary action on the key modal, so the green button does that
        // and "Done" sits beside it.
        if accept_clicked
            && let Some(EncryptionModalState::ShowRecoveryKey { key, .. }) = &self.mode
        {
            cx.copy_to_clipboard(key);
            enqueue_popup_notification("Copied your recovery key to the clipboard.", PopupKind::Success, Some(3.0));
        }
        else if accept_clicked
            || done_clicked
            || key_input.returned(actions).is_some()
            || password_input.returned(actions).is_some()
        {
            let input = match self.mode {
                Some(EncryptionModalState::EnterRecoveryKey { .. }) => key_input.text(),
                Some(EncryptionModalState::EnterPassword { .. }) => password_input.text(),
                _ => String::new(),
            };
            cx.widget_action(widget_uid, EncryptionModalResponse::Accepted(input));
        }

        if self.view.button(cx, ids!(cancel_button)).clicked(actions) {
            cx.widget_action(widget_uid, EncryptionModalResponse::Cancelled);
        }
    }
}

impl EncryptionModal {
    fn show(&mut self, cx: &mut Cx, mode: EncryptionModalState) {
        #[derive(Default)]
        struct Content<'a> {
            show_spinner: bool,
            title: &'a str,
            body: &'a str,
            key: Option<&'a str>,
            show_key_input: bool,
            show_password_input: bool,
            error: Option<&'a str>,
            cancel_text: Option<&'a str>,
            show_done: bool,
            accept_text: Option<&'a str>,
        }
        let content = match &mode {
            EncryptionModalState::Busy { title, body } => Content { title, body, show_spinner: true, ..Default::default() },
            EncryptionModalState::ShowRecoveryKey { key, is_replacement } => Content {
                title: "Save your recovery key",
                body: if *is_replacement {
                    "Your old recovery key no longer works. Store this new key somewhere safe, like a password manager. You won't be able to see it again."
                } else {
                    "Store this key somewhere safe, like a password manager. You'll need it to restore your encrypted messages on a new device, and you won't be able to see it again."
                },
                key: Some(key),
                show_done: true,
                accept_text: Some("Copy key"),
                ..Default::default()
            },
            EncryptionModalState::EnterRecoveryKey { error } => Content {
                title: "Enter your recovery key",
                body: "Enter the recovery key you saved when you set up key backup. If it no longer matches the backup on your homeserver, that backup is replaced with a fresh one.",
                show_key_input: true,
                error: error.as_deref(),
                cancel_text: Some("Cancel"),
                accept_text: Some("Restore"),
                ..Default::default()
            },
            EncryptionModalState::EnterPassword { error } => Content {
                title: "Confirm your password",
                body: "Your homeserver needs your account password to approve resetting your encryption identity.",
                show_password_input: true,
                error: error.as_deref(),
                cancel_text: Some("Cancel"),
                accept_text: Some("Reset"),
                ..Default::default()
            },
            EncryptionModalState::WaitingForApproval { error } => Content {
                title: "Approve the reset in your browser",
                body: "We opened your homeserver's approval page in your browser. Once you've approved the reset there, Robrix finishes it automatically.",
                error: error.as_deref(),
                cancel_text: Some("Cancel"),
                accept_text: Some(if error.is_some() { "Try again" } else { "Open page again" }),
                ..Default::default()
            },
            EncryptionModalState::BackupHeldByOtherDevice => Content {
                title: "Your key backup is on another device",
                body: "Another device already holds your key backup. Verify this device from that one so it can share the backup, then set up key backup here. Resetting your encryption identity would delete that backup instead.",
                cancel_text: Some("Close"),
                accept_text: Some("Verify this device"),
                ..Default::default()
            },
        };

        self.view.widget(cx, ids!(busy_spinner)).set_visible(cx, content.show_spinner);
        self.view.label(cx, ids!(title)).set_text(cx, content.title);
        self.view.label(cx, ids!(body)).set_text(cx, content.body);
        self.view.view(cx, ids!(key_view)).set_visible(cx, content.key.is_some());
        self.view.label(cx, ids!(key_label)).set_text(cx, content.key.unwrap_or_default());
        self.view.view(cx, ids!(key_input_view)).set_visible(cx, content.show_key_input);
        self.view.widget(cx, ids!(password_input)).set_visible(cx, content.show_password_input);
        // A rejected key stays in the field for fixing up; a rejected password never does.
        if content.show_key_input && content.error.is_none() {
            self.view.text_input(cx, ids!(key_input)).set_text(cx, "");
        }
        self.view.password_text_input(cx, ids!(password_input)).set_text(cx, "");
        // A just-shown input has no area until it's drawn, so we focus it from `draw_walk`.
        self.is_input_focus_pending = content.show_key_input || content.show_password_input;
        let error_label = self.view.label(cx, ids!(error_label));
        error_label.set_visible(cx, content.error.is_some());
        error_label.set_text(cx, content.error.unwrap_or_default());

        let cancel_button = self.view.button(cx, ids!(cancel_button));
        cancel_button.set_visible(cx, content.cancel_text.is_some());
        cancel_button.set_text(cx, content.cancel_text.unwrap_or_default());
        cancel_button.reset_hover(cx);
        let done_button = self.view.button(cx, ids!(done_button));
        done_button.set_visible(cx, content.show_done);
        done_button.reset_hover(cx);
        let accept_button = self.view.button(cx, ids!(accept_button));
        accept_button.set_visible(cx, content.accept_text.is_some());
        accept_button.set_text(cx, content.accept_text.unwrap_or_default());
        accept_button.reset_hover(cx);

        self.mode = Some(mode);
        self.redraw(cx);
    }
}

impl EncryptionModalRef {
    /// See [`EncryptionModal::show()`].
    pub fn show(&self, cx: &mut Cx, mode: EncryptionModalState) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.show(cx, mode);
    }
}
