//! The `SendStatusIndicator` shows whether one of our own messages is still sending,
//! queued while offline, failed, or sent. It sits in the read receipts slot of a message,
//! so a message keeps its height as it goes from local echo to remote echo to "seen by".

use std::sync::Arc;

use makepad_widgets::*;
use matrix_sdk::{HttpError, QueueWedgeError, media::MediaError, ruma::{api::error::ErrorKind, events::room::message::MessageType}};
use matrix_sdk_base::crypto::{OlmError, SessionRecipientCollectionError};
use matrix_sdk_ui::timeline::{EventSendState, EventTimelineItem};

use crate::{LivePtr, settings::app_preferences::AppPreferencesGlobal, shared::styles::{COLOR_FG_ACCEPT_GREEN, COLOR_FG_DANGER_RED}, sliding_sync::is_offline, utils::format_decimal_file_size, widget_ref_from_live_ptr};

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.SEND_STATUS_ICON_COLOR = #8C8C8C

    mod.widgets.SendStatusIndicator = #(SendStatusIndicator::register_widget(vm)) {
        width: Fit,
        height: Fit{min: FitBound.Abs(15.0)},
        margin: Inset{top: 5},
        flow: Right,
        align: Align{y: 0.0},
        spacing: 3,

        sending_icon: LoadingSpinner {
            width: 15,
            // a bit of a hack, but this moves it up a tiny bit without cutting anything off,
            // while the LoadingSpinner widget itself ensures it's still a perfect circle
            height: 14.5,
            draw_bg +: {
                color: (COLOR_ACTIVE_PRIMARY)
                stroke_width: 2.0
            }
        }
        queued_icon: Icon {
            width: 15, height: 15,
            align: Align{x: 0.5, y: 1.0}
            draw_icon +: {
                svg: (ICON_CLOUD_OFFLINE),
                color: (mod.widgets.SEND_STATUS_ICON_COLOR),
            }
            icon_walk: Walk{width: 13.5, height: Fit}
        }
        sent_icon: CircleView {
            width: 15, height: 15,
            align: Align{x: 0.5, y: 0.5}
            show_bg: true,
            draw_bg.color: (COLOR_FG_ACCEPT_GREEN)

            check := Icon {
                draw_icon +: {
                    svg: (ICON_CHECKMARK),
                    color: #FFFFFF,
                }
                icon_walk: Walk{width: 8.5, height: Fit}
            }
        }
        failed_icon: CircleView {
            width: 15, height: 15,
            align: Align{x: 0.5, y: 0.5}
            show_bg: true,
            draw_bg.color: (COLOR_FG_DANGER_RED)

            exclamation := View {
                width: 3.0, height: 10.5,
                show_bg: true,
                draw_bg +: {
                    pixel: fn() {
                        let sdf = Sdf2d.viewport(self.pos * self.rect_size)
                        let w = self.rect_size.x
                        let dot_r = w * 0.5
                        let gap = w * 0.4
                        sdf.box(0.0, 0.0, w, self.rect_size.y - dot_r * 2.0 - gap, dot_r)
                        sdf.fill(#ffffff)
                        sdf.circle(dot_r, self.rect_size.y - dot_r, dot_r)
                        sdf.fill(#ffffff)
                        return sdf.result
                    }
                }
            }
        }
        progress_label: Label {
            padding: 0,
            margin: Inset{top: 1.0},
            flow: Flow.Right { wrap: false },
            draw_text +: {
                text_style: theme.font_regular { font_size: 8.0 },
                color: (mod.widgets.SEND_STATUS_ICON_COLOR),
            }
            text: ""
        }
        failed_label: Label {
            padding: 0,
            margin: 0,
            flow: Flow.Right { wrap: true },
            max_lines: 2,
            text_overflow: Ellipsis,
            draw_text +: {
                text_style: theme.font_regular { font_size: 9.5 },
                color: (COLOR_FG_DANGER_RED),
            }
            text: "Send failed, tap to retry."
        }
        queued_label: Label {
            padding: 0,
            margin: 0,
            flow: Flow.Right { wrap: true },
            max_lines: 2,
            text_overflow: Ellipsis,
            draw_text +: {
                text_style: theme.font_regular { font_size: 9.5 },
                color: (mod.widgets.SEND_STATUS_ICON_COLOR),
            }
            text: "Will send when online."
        }
    }
}

/// Which label the indicator draws to the left of its icon.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum SendStatusLabel {
    Progress,
    Queued,
    Failed,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum SendStatusIcon {
    Sending,
    Queued,
    Retry,
    Failed,
    Sent,
}

/// The inputs the indicator's text and icon are derived from,
/// so `set_from_event` can skip the rebuild when nothing changed.
#[derive(Clone)]
struct SendStatusKey {
    icon: SendStatusIcon,
    upload_percent: Option<u8>,
    /// A queued attachment whose upload hasn't started, so it's still being encrypted.
    is_encrypting: bool,
    error: Option<Arc<matrix_sdk::Error>>,
    /// Only the newest sent message shows its checkmark, older ones just hold the row's height.
    is_newest_sent: bool,
    /// Own remote echoes hide behind their read receipts while those are shown.
    has_read_receipts: bool,
}
impl SendStatusKey {
    fn label(&self) -> Option<SendStatusLabel> {
        match self.icon {
            SendStatusIcon::Sending => (self.upload_percent.is_some() || self.is_encrypting)
                .then_some(SendStatusLabel::Progress),
            SendStatusIcon::Queued => Some(SendStatusLabel::Queued),
            SendStatusIcon::Retry | SendStatusIcon::Failed => Some(SendStatusLabel::Failed),
            SendStatusIcon::Sent => None,
        }
    }
}

impl PartialEq for SendStatusKey {
    fn eq(&self, other: &Self) -> bool {
        self.icon == other.icon
            && self.upload_percent == other.upload_percent
            && self.is_encrypting == other.is_encrypting
            && self.is_newest_sent == other.is_newest_sent
            && self.has_read_receipts == other.has_read_receipts
            && match (&self.error, &other.error) {
                (None, None) => true,
                (Some(a), Some(b)) => Arc::ptr_eq(a, b),
                _ => false,
            }
    }
}

/// Emitted when the user clicks a failed message's indicator.
#[derive(Clone, Debug, Default)]
pub enum SendStatusIndicatorAction {
    Clicked { abs_pos: DVec2 },
    #[default]
    None,
}

#[derive(Script, Widget, ScriptHook)]
pub struct SendStatusIndicator {
    #[deref] view: View,
    #[walk] walk: Walk,
    #[layout] layout: Layout,
    #[redraw] #[area] #[rust] area: Area,

    #[live] sending_icon: Option<LivePtr>,
    #[live] queued_icon: Option<LivePtr>,
    #[live] sent_icon: Option<LivePtr>,
    #[live] failed_icon: Option<LivePtr>,
    #[live] progress_label: Option<LivePtr>,
    #[live] failed_label: Option<LivePtr>,
    #[live] queued_label: Option<LivePtr>,

    /// `None` means this message shows no indicator (e.g., someone else's message).
    #[rust] key: Option<SendStatusKey>,
    #[rust] tooltip_text: String,
    #[rust] has_failed: bool,
    #[rust] icon_widget: Option<(SendStatusIcon, WidgetRef)>,
    #[rust] label_widget: Option<(SendStatusLabel, LabelRef)>,
}

/// The icon is only 13pt wide, so we widen where it accepts hovers and clicks
/// rather than its walk, which would change the row's height.
pub const HIT_MARGIN: Inset = Inset { left: 8.0, top: 5.0, right: 8.0, bottom: 5.0 };

/// What every icon plus its spacing takes up, which a label has to leave free.
const ICON_AND_SPACING: f64 = 18.0;

impl Widget for SendStatusIndicator {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, _scope: &mut Scope) {
        if self.key.is_none() { return }
        let uid = self.widget_uid();
        match event.hits_with_options(cx, self.area, HitOptions::new().with_margin(HIT_MARGIN)) {
            Hit::FingerHoverIn(..) | Hit::FingerLongPress(_) => {
                let mut options = CalloutTooltipOptions {
                    position: TooltipPosition::Left,
                    ..Default::default()
                };
                match self.key.as_ref().map(|key| key.icon) {
                    Some(SendStatusIcon::Sent) => options.bg_color = COLOR_FG_ACCEPT_GREEN,
                    Some(SendStatusIcon::Retry | SendStatusIcon::Failed) => options.bg_color = COLOR_FG_DANGER_RED,
                    _ => { }
                }
                cx.widget_action(
                    uid,
                    TooltipAction::HoverIn {
                        text: self.tooltip_text.clone(),
                        widget_rect: self.area.rect(cx),
                        options,
                    },
                );
            }
            Hit::FingerHoverOut(_) => {
                cx.widget_action(uid, TooltipAction::HoverOut);
            }
            Hit::FingerUp(fue) if self.has_failed && fue.is_over && fue.is_primary_hit() && fue.was_tap() => {
                cx.widget_action(uid, SendStatusIndicatorAction::Clicked { abs_pos: fue.abs });
            }
            _ => {}
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        let Some(key) = &self.key else {
            self.area = Area::Empty;
            return DrawStep::done();
        };
        if key.has_read_receipts && cx.global::<AppPreferencesGlobal>().0.show_read_receipts {
            self.area = Area::Empty;
            return DrawStep::done();
        }
        if key.icon == SendStatusIcon::Sent && !key.is_newest_sent {
            self.area = Area::Empty;
            return DrawStep::done();
        }
        let icon = key.icon;
        let show_label = key.label().is_some();
        // Bound the label to the row so a long message wraps instead of running off it.
        if show_label
            && let Some(available) = cx.find_line_available_width()
            && let Some((_, label)) = &self.label_widget
            && let Some(mut label) = label.borrow_mut()
        {
            label.walk.width = Size::Fit {
                min: None,
                max: Some(FitBound::Abs((available - ICON_AND_SPACING).max(0.0))),
            };
        }
        let mut icon_widget = match &self.icon_widget {
            Some((cached, widget)) if *cached == icon => widget.clone(),
            _ => {
                let template = match icon {
                    SendStatusIcon::Sending => self.sending_icon,
                    SendStatusIcon::Queued => self.queued_icon,
                    SendStatusIcon::Retry | SendStatusIcon::Failed => self.failed_icon,
                    SendStatusIcon::Sent => self.sent_icon,
                };
                let widget = widget_ref_from_live_ptr(cx, template);
                self.icon_widget = Some((icon, widget.clone()));
                widget
            }
        };
        cx.begin_turtle(walk, self.layout);
        if show_label && let Some((_, label)) = &mut self.label_widget {
            let _ = label.draw(cx, scope);
        }
        let _ = icon_widget.draw(cx, scope);
        cx.end_turtle_with_area(&mut self.area);
        DrawStep::done()
    }
}

impl SendStatusIndicator {
    /// Derives the indicator from the event's send state; cheap to call on every populate.
    fn set_from_event(
        &mut self,
        cx: &mut Cx,
        event_tl_item: &EventTimelineItem,
        is_newest_sent: bool,
        is_room_encrypted: bool,
    ) {
        let upload = match event_tl_item.send_state() {
            Some(EventSendState::NotSentYet { progress: Some(p) }) => Some((p.progress.current, p.progress.total)),
            _ => None,
        };
        // The send queue encrypts a queued attachment in full before its first byte
        // goes out, which on a large file is the longest part of the send.
        let is_encrypting = is_room_encrypted
            && upload.is_none()
            && matches!(event_tl_item.send_state(), Some(EventSendState::NotSentYet { .. }))
            && event_tl_item.content().as_message().is_some_and(|msg| matches!(
                msg.msgtype(),
                MessageType::Image(_) | MessageType::Video(_) | MessageType::File(_) | MessageType::Audio(_),
            ));
        let new_key = match event_tl_item.send_state() {
            None if !event_tl_item.is_own() => None,
            None => Some(SendStatusKey {
                icon: SendStatusIcon::Sent,
                upload_percent: None,
                is_encrypting: false,
                error: None,
                is_newest_sent,
                has_read_receipts: !event_tl_item.read_receipts().is_empty(),
            }),
            Some(EventSendState::Sent { .. }) => Some(SendStatusKey {
                icon: SendStatusIcon::Sent,
                upload_percent: None,
                is_encrypting: false,
                error: None,
                is_newest_sent,
                has_read_receipts: false,
            }),
            Some(EventSendState::NotSentYet { .. }) => Some(SendStatusKey {
                icon: if upload.is_none() && is_offline() { SendStatusIcon::Queued } else { SendStatusIcon::Sending },
                upload_percent: upload.map(|(current, total)| upload_percent(current, total)),
                is_encrypting,
                error: None,
                is_newest_sent: false,
                has_read_receipts: false,
            }),
            Some(EventSendState::SendingFailed { error, is_recoverable }) => Some(SendStatusKey {
                icon: match (is_recoverable, is_offline()) {
                    (true, true) => SendStatusIcon::Queued,
                    (true, false) => SendStatusIcon::Retry,
                    (false, _) => SendStatusIcon::Failed,
                },
                upload_percent: None,
                is_encrypting: false,
                error: Some(error.clone()),
                is_newest_sent: false,
                has_read_receipts: false,
            }),
        };
        if new_key == self.key { return }

        let Some(key) = &new_key else {
            self.key = None;
            self.tooltip_text.clear();
            self.has_failed = false;
            self.redraw(cx);
            return;
        };

        let error = key.error.as_ref().map(|e| describe_send_error(e));
        self.has_failed = matches!(key.icon, SendStatusIcon::Retry | SendStatusIcon::Failed);
        self.tooltip_text = match (key.icon, upload, error) {
            (SendStatusIcon::Sending, Some((current, total)), _) => upload_status_text(current, total),
            (SendStatusIcon::Sending, None, _) if key.is_encrypting => "Encrypting the file...".into(),
            (SendStatusIcon::Sending, None, _) => "Sending...".into(),
            (SendStatusIcon::Queued, _, None) => "Queued while offline.".into(),
            (SendStatusIcon::Queued, _, Some(e)) => format!("Queued: {e}"),
            (SendStatusIcon::Retry, _, e) => format!(
                "Couldn't send: {} Retrying...",
                e.unwrap_or(UNKNOWN_ERROR_TEXT),
            ),
            (SendStatusIcon::Failed, _, e) => format!(
                "Couldn't send: {}",
                e.unwrap_or(UNKNOWN_ERROR_TEXT),
            ),
            (SendStatusIcon::Sent, ..) => "Sent".into(),
        };
        if let Some(kind) = key.label() {
            let label = match &self.label_widget {
                Some((cached, label)) if *cached == kind => label.clone(),
                _ => {
                    let template = match kind {
                        SendStatusLabel::Progress => self.progress_label,
                        SendStatusLabel::Queued => self.queued_label,
                        SendStatusLabel::Failed => self.failed_label,
                    };
                    let label = widget_ref_from_live_ptr(cx, template).as_label();
                    self.label_widget = Some((kind, label.clone()));
                    label
                }
            };
            match kind {
                SendStatusLabel::Progress => match key.upload_percent {
                    None => label.set_text(cx, "Encrypting"),
                    Some(percent) if percent < 100 => label.set_text(cx, &format!("{percent}%")),
                    Some(_) => label.set_text(cx, "Sending"),
                },
                SendStatusLabel::Failed => label.set_text(cx, match key.icon {
                    // The queue is already going to try this one again on its own.
                    SendStatusIcon::Retry => "Send failed, retrying...",
                    // Don't offer a retry the message's menu won't have.
                    _ if key.error.as_deref().is_none_or(is_send_error_retryable) => "Send failed, tap to retry.",
                    _ => "Send failed, tap for options.",
                }),
                SendStatusLabel::Queued => { }
            }
        }
        self.key = new_key;
        self.redraw(cx);
    }
}

impl SendStatusIndicatorRef {
    /// See [`SendStatusIndicator::set_from_event()`].
    pub fn set_from_event(
        &self,
        cx: &mut Cx,
        event_tl_item: &EventTimelineItem,
        is_newest_sent: bool,
        is_room_encrypted: bool,
    ) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.set_from_event(cx, event_tl_item, is_newest_sent, is_room_encrypted);
    }
}
fn upload_percent(current: usize, total: usize) -> u8 {
    if total > 0 { (current * 100 / total).min(100) as u8 } else { 0 }
}

/// How far along an upload is, e.g. "Uploading... 40% (2 MB / 5 MB)".
/// Past the last byte the server still has to answer, so it stops counting.
pub fn upload_status_text(current: usize, total: usize) -> String {
    if current >= total {
        return "Uploaded, sending...".to_string();
    }
    format!(
        "Uploading... {}% ({} / {})",
        upload_percent(current, total),
        format_decimal_file_size(current as u64),
        format_decimal_file_size(total as u64),
    )
}

/// Cheap check for whether retrying a failed send could ever succeed;
/// only a media file that vanished from the local cache can't be retried.
pub fn is_send_error_retryable(error: &matrix_sdk::Error) -> bool {
    !matches!(error, matrix_sdk::Error::SendQueueWedgeError(wedge) if matches!(**wedge, QueueWedgeError::MissingMediaContent))
}

/// Boils a send failure down to one short phrase that reads after a colon,
/// e.g. "your session has expired."
pub fn describe_send_error(error: &matrix_sdk::Error) -> &'static str {
    // Only set when the server answered with a real Matrix errcode.
    if let Some(kind) = error.client_api_error_kind() {
        match kind {
            ErrorKind::Forbidden { .. } => "you don't have permission to post.",
            ErrorKind::LimitExceeded { .. } => RATE_LIMITED_TEXT,
            ErrorKind::TooLarge => "it's too large to send.",
            ErrorKind::UnknownToken { .. } | ErrorKind::MissingToken => "your session has expired.",
            ErrorKind::UserDeactivated => "your account is deactivated.",
            ErrorKind::UserLocked => "your account is locked.",
            ErrorKind::UserSuspended => "your account is suspended.",
            ErrorKind::ResourceLimitExceeded { .. } => "your homeserver has reached a limit.",
            ErrorKind::DuplicateAnnotation => "you already sent that reaction.",
            ErrorKind::Unrecognized => "your homeserver doesn't support this.",
            ErrorKind::NotFound => "the server couldn't find it.",
            ErrorKind::Unknown => SERVER_PROBLEM_TEXT,
            _ => SERVER_REJECTED_TEXT,
        }
    } else {
        match error {
            matrix_sdk::Error::OlmError(olm) => match &**olm {
                OlmError::SessionRecipientCollectionError(e) => match e {
                    SessionRecipientCollectionError::VerifiedUserHasUnsignedDevice(_) => UNVERIFIED_DEVICES_TEXT,
                    SessionRecipientCollectionError::VerifiedUserChangedIdentity(_) => IDENTITY_CHANGED_TEXT,
                    SessionRecipientCollectionError::CrossSigningNotSetup
                    | SessionRecipientCollectionError::SendingFromUnverifiedDevice => OWN_VERIFICATION_TEXT,
                },
                _ => "this message couldn't be encrypted.",
            },
            matrix_sdk::Error::SendQueueWedgeError(wedge) => match &**wedge {
                QueueWedgeError::InsecureDevices { .. } => UNVERIFIED_DEVICES_TEXT,
                QueueWedgeError::IdentityViolations { .. } => IDENTITY_CHANGED_TEXT,
                QueueWedgeError::CrossVerificationRequired => OWN_VERIFICATION_TEXT,
                QueueWedgeError::MissingMediaContent => "the attached file is missing.",
                QueueWedgeError::InvalidMimeType { .. } => "that file type isn't supported.",
                QueueWedgeError::GenericApiError { .. } => SERVER_REJECTED_TEXT,
            },
            matrix_sdk::Error::Http(http) => match &**http {
                HttpError::Reqwest(e) if e.is_timeout() => SERVER_SLOW_TEXT,
                HttpError::Reqwest(_) => "no connection to the server.",
                // No errcode, so fall back to the raw status the server sent.
                other => match other.as_client_api_error().map(|e| e.status_code.as_u16()) {
                    Some(429) => RATE_LIMITED_TEXT,
                    Some(code) if code >= 500 => SERVER_PROBLEM_TEXT,
                    Some(_) => SERVER_REJECTED_TEXT,
                    None => "the server sent a bad response.",
                },
            },
            matrix_sdk::Error::Media(MediaError::MediaTooLargeToUpload { .. }) => "the file is too large to upload.",
            matrix_sdk::Error::Media(_) => "the file couldn't be uploaded.",
            matrix_sdk::Error::AuthenticationRequired => "you're signed out.",
            matrix_sdk::Error::WrongRoomState(_) => "you're no longer in this room.",
            _ => UNKNOWN_ERROR_TEXT,
        }
    }
}

const UNVERIFIED_DEVICES_TEXT: &str = "some recipients have unverified devices.";
const IDENTITY_CHANGED_TEXT: &str = "someone's verified identity changed.";
const OWN_VERIFICATION_TEXT: &str = "this device isn't verified yet.";
const RATE_LIMITED_TEXT: &str = "you're sending messages too quickly.";
const SERVER_REJECTED_TEXT: &str = "the server rejected it.";
const SERVER_PROBLEM_TEXT: &str = "the server had a problem.";
const SERVER_SLOW_TEXT: &str = "the server took too long.";
/// Shown when we can't tell what went wrong, and when there's no error to describe.
pub const UNKNOWN_ERROR_TEXT: &str = "something went wrong.";
