//! The `SendStatusIndicator` shows whether one of our own messages is still sending,
//! queued while offline, failed, or sent. It sits in the read receipts slot of a message,
//! so a message keeps its height as it goes from local echo to remote echo to "seen by".

use std::sync::Arc;

use makepad_widgets::*;
use matrix_sdk::{QueueWedgeError, ruma::{OwnedDeviceId, OwnedUserId}};
use matrix_sdk_base::crypto::{OlmError, SessionRecipientCollectionError};
use matrix_sdk_ui::timeline::{EventSendState, EventTimelineItem};

use crate::{LivePtr, settings::app_preferences::AppPreferencesGlobal, sliding_sync::is_offline, utils::format_decimal_file_size, widget_ref_from_live_ptr};

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.SEND_STATUS_ICON_COLOR = #8C8C8C

    mod.widgets.SendStatusIndicator = #(SendStatusIndicator::register_widget(vm)) {
        width: Fit,
        height: 15,
        margin: Inset{top: 5},
        flow: Right,
        align: Align{y: 0.5},
        spacing: 3,

        // A small clock: a ring plus two hands.
        sending_icon: View {
            width: 13, height: 13,
            show_bg: true,
            draw_bg +: {
                color: uniform(mod.widgets.SEND_STATUS_ICON_COLOR)
                pixel: fn() {
                    let sdf = Sdf2d.viewport(self.pos * self.rect_size)
                    let c = self.rect_size * 0.5
                    let r = min(c.x, c.y) - 1.0
                    sdf.circle(c.x, c.y, r)
                    sdf.stroke(self.color, 1.2)
                    sdf.rect(c.x - 0.6, c.y - r * 0.55, 1.2, r * 0.55)
                    sdf.fill(self.color)
                    sdf.rect(c.x, c.y - 0.6, r * 0.45, 1.2)
                    sdf.fill(self.color)
                    return sdf.result
                }
            }
        }
        queued_icon: Icon {
            draw_icon +: {
                svg: (ICON_CLOUD_OFFLINE),
                color: (mod.widgets.SEND_STATUS_ICON_COLOR),
            }
            icon_walk: Walk{width: 15, height: Fit}
        }
        sent_icon: Icon {
            draw_icon +: {
                svg: (ICON_CHECKMARK),
                color: (mod.widgets.SEND_STATUS_ICON_COLOR),
            }
            icon_walk: Walk{width: 13, height: Fit}
        }
        retry_icon: Icon {
            draw_icon +: {
                svg: (ICON_WARNING),
                color: (COLOR_TEXT_WARNING_NOT_FOUND),
            }
            icon_walk: Walk{width: 13, height: Fit}
        }
        failed_icon: Icon {
            draw_icon +: {
                svg: (ICON_WARNING),
                color: (COLOR_FG_DANGER_RED),
            }
            icon_walk: Walk{width: 13, height: Fit}
        }
        progress_label: Label {
            padding: 0,
            margin: 0,
            flow: Flow.Right { wrap: false },
            draw_text +: {
                text_style: theme.font_regular { font_size: 8.0 },
                color: (mod.widgets.SEND_STATUS_ICON_COLOR),
            }
            text: ""
        }
    }
}

/// Which icon template the indicator currently draws.
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
    error: Option<Arc<matrix_sdk::Error>>,
    /// Own remote echoes hide behind their read receipts while those are shown.
    has_read_receipts: bool,
}
impl PartialEq for SendStatusKey {
    fn eq(&self, other: &Self) -> bool {
        self.icon == other.icon
            && self.upload_percent == other.upload_percent
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
    #[deref] deref: View,
    #[walk] walk: Walk,
    #[layout] layout: Layout,
    #[redraw] #[area] #[rust] area: Area,

    #[live] sending_icon: Option<LivePtr>,
    #[live] queued_icon: Option<LivePtr>,
    #[live] sent_icon: Option<LivePtr>,
    #[live] retry_icon: Option<LivePtr>,
    #[live] failed_icon: Option<LivePtr>,
    #[live] progress_label: Option<LivePtr>,

    /// `None` means this message shows no indicator (e.g., someone else's message).
    #[rust] key: Option<SendStatusKey>,
    #[rust] tooltip_text: String,
    #[rust] has_failed: bool,
    #[rust] icon_widget: Option<(SendStatusIcon, WidgetRef)>,
    #[rust] label_widget: Option<LabelRef>,
}

impl Widget for SendStatusIndicator {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, _scope: &mut Scope) {
        if self.key.is_none() { return }
        let uid = self.widget_uid();
        match event.hits(cx, self.area) {
            Hit::FingerHoverIn(..) | Hit::FingerLongPress(_) => {
                cx.widget_action(
                    uid,
                    TooltipAction::HoverIn {
                        text: self.tooltip_text.clone(),
                        widget_rect: self.area.rect(cx),
                        options: CalloutTooltipOptions {
                            position: TooltipPosition::Left,
                            ..Default::default()
                        },
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
        let icon = key.icon;
        let show_label = key.upload_percent.is_some();
        let mut icon_widget = match &self.icon_widget {
            Some((cached, widget)) if *cached == icon => widget.clone(),
            _ => {
                let template = match icon {
                    SendStatusIcon::Sending => self.sending_icon,
                    SendStatusIcon::Queued => self.queued_icon,
                    SendStatusIcon::Retry => self.retry_icon,
                    SendStatusIcon::Failed => self.failed_icon,
                    SendStatusIcon::Sent => self.sent_icon,
                };
                let widget = widget_ref_from_live_ptr(cx, template);
                self.icon_widget = Some((icon, widget.clone()));
                widget
            }
        };
        cx.begin_turtle(walk, self.layout);
        let _ = icon_widget.draw(cx, scope);
        if show_label && let Some(label) = &mut self.label_widget {
            let _ = label.draw(cx, scope);
        }
        cx.end_turtle_with_area(&mut self.area);
        DrawStep::done()
    }
}

impl SendStatusIndicator {
    /// Derives the indicator from the event's send state; cheap to call on every populate.
    fn set_from_event(&mut self, cx: &mut Cx, event_tl_item: &EventTimelineItem) {
        let upload = match event_tl_item.send_state() {
            Some(EventSendState::NotSentYet { progress: Some(p) }) => Some((p.progress.current, p.progress.total)),
            _ => None,
        };
        let new_key = match event_tl_item.send_state() {
            None if !event_tl_item.is_own() => None,
            None => Some(SendStatusKey {
                icon: SendStatusIcon::Sent,
                upload_percent: None,
                error: None,
                has_read_receipts: !event_tl_item.read_receipts().is_empty(),
            }),
            Some(EventSendState::Sent { .. }) => Some(SendStatusKey {
                icon: SendStatusIcon::Sent,
                upload_percent: None,
                error: None,
                has_read_receipts: false,
            }),
            Some(EventSendState::NotSentYet { .. }) => Some(SendStatusKey {
                icon: if upload.is_none() && is_offline() { SendStatusIcon::Queued } else { SendStatusIcon::Sending },
                upload_percent: upload.map(|(current, total)| upload_percent(current, total)),
                error: None,
                has_read_receipts: false,
            }),
            Some(EventSendState::SendingFailed { error, is_recoverable }) => Some(SendStatusKey {
                icon: match (is_recoverable, is_offline()) {
                    (true, true) => SendStatusIcon::Queued,
                    (true, false) => SendStatusIcon::Retry,
                    (false, _) => SendStatusIcon::Failed,
                },
                upload_percent: None,
                error: Some(error.clone()),
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
            (SendStatusIcon::Sending, Some((current, total)), _) => format!(
                "Uploading... {}% ({} / {})",
                upload_percent(current, total),
                format_decimal_file_size(current as u64),
                format_decimal_file_size(total as u64),
            ),
            (SendStatusIcon::Sending, None, _) => "Sending...".into(),
            (SendStatusIcon::Queued, _, None) =>
                "Queued while offline.\nThis message will be sent automatically once you're back online.".into(),
            (SendStatusIcon::Queued, _, Some(e)) => format!(
                "Couldn't send while offline: {}\nIt will be sent automatically once you're back online.",
                e.description,
            ),
            (SendStatusIcon::Retry, _, e) => format!(
                "Couldn't send this message: {}\n\nRobrix will retry automatically. Click to retry now or cancel it.",
                e.map_or_else(|| "unknown error".into(), |e| e.description),
            ),
            (SendStatusIcon::Failed, _, e) => {
                let (description, can_retry) = e.map_or_else(
                    || ("unknown error".to_string(), true),
                    |e| (e.description, e.can_retry),
                );
                let hint = if can_retry { "Click to retry or cancel it." } else { "Click to cancel it." };
                format!("Failed to send this message: {description}\n\n{hint}")
            }
            (SendStatusIcon::Sent, ..) => "Sent".into(),
        };
        if let Some(percent) = key.upload_percent {
            let label = self.label_widget.get_or_insert_with(||
                widget_ref_from_live_ptr(cx, self.progress_label).as_label()
            );
            label.set_text(cx, &format!("{percent}%"));
        }
        self.key = new_key;
        self.redraw(cx);
    }
}

impl SendStatusIndicatorRef {
    /// See [`SendStatusIndicator::set_from_event()`].
    pub fn set_from_event(&self, cx: &mut Cx, event_tl_item: &EventTimelineItem) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.set_from_event(cx, event_tl_item);
    }
}

/// A user-facing explanation of why a send failed.
pub struct SendErrorDetails {
    pub description: String,
    /// `false` when retrying can't succeed (e.g., the attached file is gone from the cache).
    pub can_retry: bool,
}

/// Cheap check for whether retrying a failed send could ever succeed;
/// only a media file that vanished from the local cache can't be retried.
pub fn is_send_error_retryable(error: &matrix_sdk::Error) -> bool {
    !matches!(error, matrix_sdk::Error::SendQueueWedgeError(wedge) if matches!(**wedge, QueueWedgeError::MissingMediaContent))
}

/// Explains a send failure in plain words, spelling out the users/devices
/// involved for the encryption cases so the user knows what to fix.
pub fn describe_send_error(error: &matrix_sdk::Error) -> SendErrorDetails {
    let description = match error {
        matrix_sdk::Error::OlmError(olm) => match &**olm {
            OlmError::SessionRecipientCollectionError(e) => match e {
                SessionRecipientCollectionError::VerifiedUserHasUnsignedDevice(map) => insecure_devices_text(map),
                SessionRecipientCollectionError::VerifiedUserChangedIdentity(users) => identity_changed_text(users),
                SessionRecipientCollectionError::CrossSigningNotSetup
                | SessionRecipientCollectionError::SendingFromUnverifiedDevice => OWN_VERIFICATION_TEXT.into(),
            },
            other => format!("encryption error: {other}"),
        },
        matrix_sdk::Error::SendQueueWedgeError(wedge) => match &**wedge {
            QueueWedgeError::InsecureDevices { user_device_map } => insecure_devices_text(user_device_map),
            QueueWedgeError::IdentityViolations { users } => identity_changed_text(users),
            QueueWedgeError::CrossVerificationRequired => OWN_VERIFICATION_TEXT.into(),
            QueueWedgeError::MissingMediaContent =>
                "the attached file is no longer available on this device, so it can't be uploaded. \
                Cancel this message and attach the file again.".into(),
            QueueWedgeError::InvalidMimeType { mime_type } => format!("the file type \"{mime_type}\" isn't supported."),
            QueueWedgeError::GenericApiError { msg } => msg.clone(),
        },
        other => other.to_string(),
    };
    SendErrorDetails { description, can_retry: is_send_error_retryable(error) }
}

fn upload_percent(current: usize, total: usize) -> u8 {
    if total > 0 { (current * 100 / total).min(100) as u8 } else { 0 }
}

const OWN_VERIFICATION_TEXT: &str =
    "this device isn't verified yet. Verify it from Settings, then retry.";

fn insecure_devices_text(map: &std::collections::BTreeMap<OwnedUserId, Vec<OwnedDeviceId>>) -> String {
    let users: Vec<String> = map.iter()
        .map(|(user, devices)| {
            let devices: Vec<&str> = devices.iter().map(|d| d.as_str()).collect();
            format!("{user} ({})", devices.join(", "))
        })
        .collect();
    format!(
        "some verified users have unverified devices, and this encrypted room doesn't allow sending to them: {}.\n\
        Verify or block those devices, then retry.",
        users.join("; "),
    )
}

fn identity_changed_text(users: &[OwnedUserId]) -> String {
    let users: Vec<&str> = users.iter().map(|u| u.as_str()).collect();
    format!(
        "these previously verified users have changed their identity: {}.\n\
        Re-verify them (or withdraw their verification), then retry.",
        users.join(", "),
    )
}
