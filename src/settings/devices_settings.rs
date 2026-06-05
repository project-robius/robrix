//! Devices settings page: lists every device this user has signed in with,
//! lets them remove individual sessions.
//!
//! Matches the matrix.org account-portal layout: a "Where you're signed in"
//! header, a device count, then a vertical list of cards. Each card has the
//! display_name (fallback "Unknown device"), the raw device_id, a "Last
//! Active" timestamp, a "Signed in" timestamp (currently last_seen for both —
//! Synapse doesn't expose creation time), and a destructive "Remove device"
//! button.
//!
//! Click → fires `ConfirmDeleteAction::Show(…)` which opens the global
//! delete-confirmation modal (defined in `app.rs`). On confirmation the
//! `on_accept_clicked` callback submits `MatrixRequest::DeleteDevice`. The
//! UIA-fallback popup is handled at app level in response to
//! `AccountDataAction::DeviceDeleteResult { outcome: NeedsAuth }`.

use std::borrow::Cow;
use std::cell::RefCell;

use chrono::{DateTime, Local};
use makepad_widgets::*;

use crate::app::{ConfirmDeleteAction, PositiveConfirmationModalAction};
use crate::shared::confirmation_modal::ConfirmationModalContent;
use crate::shared::popup_list::{PopupKind, enqueue_popup_notification};
use crate::sliding_sync::{
    AccountDataAction, DeviceInfo, MatrixRequest, submit_async_request,
};

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    // ─────────────────────────── DeviceCard ────────────────────────────
    // One row in the device list.
    mod.widgets.DeviceCard = #(DeviceCard::register_widget(vm)) {
        width: Fill, height: Fit
        flow: Down
        padding: Inset{top: 12, bottom: 12, left: 14, right: 14}
        margin: Inset{top: 6, bottom: 6}
        show_bg: true
        draw_bg +: {
            color: #xFFFFFF
            border_radius: 8.0
            border_size: 1.0
            border_color: #xE5E5EA
        }
        spacing: 6

        // Top row: icon + name/id column + remove button
        device_card_top_row := View {
            width: Fill, height: Fit
            flow: Right
            spacing: 12
            align: Align{y: 0.5}

            // Generic device glyph. Matrix's `/devices` endpoint doesn't
            // expose a device type, so we don't try to guess laptop vs
            // phone vs browser — one icon for all.
            device_card_icon := Label {
                width: 36, height: 36
                align: Align{x: 0.5, y: 0.5}
                draw_text +: {
                    color: #x606066
                    text_style: theme.font_regular { font_size: 22.0 }
                }
                text: "💻"
            }

            device_card_name_col := View {
                width: Fill, height: Fit
                flow: Down
                spacing: 2

                device_card_display_name := Label {
                    width: Fill, height: Fit
                    text: "Unknown device"
                    draw_text +: {
                        color: #x101012
                        text_style: theme.font_bold { font_size: 14.0 }
                    }
                }
                device_card_device_id := Label {
                    width: Fill, height: Fit
                    text: ""
                    draw_text +: {
                        color: #x606066
                        text_style: theme.font_regular { font_size: 11.0 }
                    }
                }
            }

            // Destructive action — red text on the standard "negative"
            // outlined background. The plain `Button` widget had white
            // text on white background, invisible.
            device_card_remove_button := RobrixNegativeIconButton {
                text: "Remove device"
                width: Fit, height: 32
            }
        }

        // Detail row: Last active + Signed in
        device_card_detail_row := View {
            width: Fill, height: Fit
            flow: Right
            spacing: 16
            margin: Inset{top: 6}

            device_card_last_active_col := View {
                width: Fill, height: Fit
                flow: Down

                device_card_last_active_label := Label {
                    text: "Last Active"
                    draw_text +: {
                        color: #x606066
                        text_style: theme.font_regular { font_size: 11.0 }
                    }
                }
                device_card_last_active_value := Label {
                    text: "—"
                    draw_text +: {
                        color: #x101012
                        text_style: theme.font_regular { font_size: 12.0 }
                    }
                }
            }
            device_card_device_id_col := View {
                width: Fill, height: Fit
                flow: Down

                device_card_id_label := Label {
                    text: "Device ID"
                    draw_text +: {
                        color: #x606066
                        text_style: theme.font_regular { font_size: 11.0 }
                    }
                }
                device_card_id_value := Label {
                    text: "—"
                    draw_text +: {
                        color: #x101012
                        text_style: theme.font_regular { font_size: 12.0 }
                    }
                }
            }
        }
    }

    // ────────────────────────── DevicesScreen ──────────────────────────
    mod.widgets.DevicesScreen = #(DevicesScreen::register_widget(vm)) {
        width: Fill, height: Fill
        flow: Down
        padding: Inset{top: 8, bottom: 8, left: 8, right: 8}
        spacing: 8

        // Header
        devices_header_row := View {
            width: Fill, height: Fit
            flow: Right
            align: Align{y: 0.5}
            spacing: 8

            devices_header_label := Label {
                width: Fill, height: Fit
                text: "Where you're signed in"
                draw_text +: {
                    color: #x101012
                    text_style: theme.font_bold { font_size: 16.0 }
                }
            }
            devices_refresh_button := RobrixNeutralIconButton {
                text: "Refresh"
                width: Fit, height: 32
            }
        }

        // Subtitle: count
        devices_count_label := Label {
            width: Fill, height: Fit
            text: "0 devices"
            margin: Inset{top: 4}
            draw_text +: {
                color: #x606066
                text_style: theme.font_regular { font_size: 12.0 }
            }
        }

        // Empty/loading status
        devices_status_label := Label {
            width: Fill, height: Fit
            text: "Loading…"
            margin: Inset{top: 16, bottom: 16}
            align: Align{x: 0.5, y: 0.5}
            draw_text +: {
                color: #x808080
                text_style: theme.font_regular { font_size: 13.0 }
            }
        }

        // The list itself.
        devices_list := FlatList {
            width: Fill
            height: Fill
            flow: Down
            spacing: 0
            grab_key_focus: false
            drag_scrolling: false
            scroll_bars +: { show_scroll_x: false, show_scroll_y: true }

            device_item := mod.widgets.DeviceCard {}
        }
    }
}

// ────────────────────────────── actions ──────────────────────────────

/// Emitted by a `DeviceCard` when the user clicks its Remove button. The
/// parent `DevicesScreen` listens and translates this into a
/// `ConfirmDeleteAction::Show`.
#[derive(Clone, Debug, Default)]
pub enum DeviceRowAction {
    #[default]
    None,
    RemoveClicked {
        device_id: String,
        display_label: String,
    },
}

// ───────────────────────────── DeviceCard ───────────────────────────────

#[derive(Script, ScriptHook, Widget)]
pub struct DeviceCard {
    #[deref] view: View,
    /// Set on every redraw from the parent's scope props.
    #[rust] device: Option<DeviceInfo>,
}

impl Widget for DeviceCard {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        if let Event::Actions(actions) = event {
            if self
                .view
                .button(cx, ids!(device_card_remove_button))
                .clicked(actions)
            {
                if let Some(device) = self.device.as_ref() {
                    let label = device
                        .display_name
                        .clone()
                        .unwrap_or_else(|| "Unknown device".to_string());
                    cx.action(DeviceRowAction::RemoveClicked {
                        device_id: device.device_id.clone(),
                        display_label: label,
                    });
                }
            }
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        if let Some(device) = scope.props.get::<DeviceInfo>() {
            self.device = Some(device.clone());
            self.view
                .label(cx, ids!(device_card_display_name))
                .set_text(
                    cx,
                    device.display_name.as_deref().unwrap_or("Unknown device"),
                );
            self.view
                .label(cx, ids!(device_card_device_id))
                .set_text(cx, &device.device_id);
            self.view
                .label(cx, ids!(device_card_id_value))
                .set_text(cx, &device.device_id);
            self.view
                .label(cx, ids!(device_card_last_active_value))
                .set_text(cx, &format_last_active(device));
        }
        self.view.draw_walk(cx, scope, walk)
    }
}

// ──────────────────────────── DevicesScreen ─────────────────────────────

#[derive(Script, ScriptHook, Widget)]
pub struct DevicesScreen {
    #[deref] view: View,
    /// The current device list, freshest-first (the homeserver returns them
    /// in arbitrary order; we sort by `last_seen_ts_ms` desc on update).
    #[rust] devices: Vec<DeviceInfo>,
    /// One-shot init flag so we only auto-fetch on first draw.
    #[rust] initialized: bool,
    /// `true` while we have a `GetDeviceList` in flight.
    #[rust] fetching: bool,
    /// Last status string shown in the empty-state label.
    #[rust] status_text: String,
}

impl Widget for DevicesScreen {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);

        if let Event::Actions(actions) = event {
            // Refresh button
            if self
                .view
                .button(cx, ids!(devices_refresh_button))
                .clicked(actions)
            {
                self.request_fetch(cx);
            }

            for action in actions {
                // DeviceCard Remove clicked
                if let Some(DeviceRowAction::RemoveClicked {
                    device_id,
                    display_label,
                }) = action.downcast_ref()
                {
                    self.open_remove_confirmation(cx, device_id.clone(), display_label.clone());
                    continue;
                }
                // Async results from the worker
                if let Some(account_action) = action.downcast_ref::<AccountDataAction>() {
                    match account_action {
                        AccountDataAction::DeviceListFetched(list) => {
                            self.apply_device_list(cx, list.clone());
                        }
                        AccountDataAction::DeviceListFetchFailed(err) => {
                            self.fetching = false;
                            self.status_text = format!("Failed to load devices: {err}");
                            self.view
                                .label(cx, ids!(devices_status_label))
                                .set_text(cx, &self.status_text);
                            self.view
                                .label(cx, ids!(devices_status_label))
                                .set_visible(cx, true);
                            self.view.redraw(cx);
                        }
                        AccountDataAction::DeviceDeleteResult { device_id, outcome } => {
                            use crate::sliding_sync::DeviceDeleteOutcome::*;
                            match outcome {
                                Removed => {
                                    // Drop the device from local state instantly
                                    // and trigger a background refresh to confirm.
                                    self.devices.retain(|d| &d.device_id != device_id);
                                    self.refresh_count_label(cx);
                                    self.view.redraw(cx);
                                    self.request_fetch(cx);
                                }
                                NeedsAuth { fallback_url } => {
                                    self.prompt_browser_reauth(cx, fallback_url.clone());
                                }
                                Error(msg) => {
                                    enqueue_popup_notification(
                                        format!("Failed to remove device: {msg}"),
                                        PopupKind::Error,
                                        Some(8.0),
                                    );
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        if !self.initialized {
            self.initialized = true;
            self.request_fetch(cx);
        }

        while let Some(subview) = self.view.draw_walk(cx, scope, walk).step() {
            let flat_list_ref = subview.as_flat_list();
            let Some(mut list) = flat_list_ref.borrow_mut() else {
                continue;
            };
            for (index, device) in self.devices.iter().enumerate() {
                let item_id = LiveId(index as u64);
                let item = list.item(cx, item_id, id!(device_item)).unwrap();
                let mut scope = Scope::with_props(device);
                item.draw_all(cx, &mut scope);
            }
        }
        DrawStep::done()
    }
}

impl DevicesScreen {
    fn request_fetch(&mut self, cx: &mut Cx) {
        self.fetching = true;
        self.status_text = "Loading…".to_string();
        self.view
            .label(cx, ids!(devices_status_label))
            .set_text(cx, &self.status_text);
        self.view
            .label(cx, ids!(devices_status_label))
            .set_visible(cx, true);
        self.view.redraw(cx);
        submit_async_request(MatrixRequest::GetDeviceList);
    }

    fn apply_device_list(&mut self, cx: &mut Cx, mut list: Vec<DeviceInfo>) {
        list.sort_by(|a, b| {
            b.last_seen_ts_ms
                .unwrap_or(0)
                .cmp(&a.last_seen_ts_ms.unwrap_or(0))
        });
        self.devices = list;
        self.fetching = false;
        self.refresh_count_label(cx);
        let empty = self.devices.is_empty();
        if empty {
            self.status_text = "No devices found.".to_string();
            self.view
                .label(cx, ids!(devices_status_label))
                .set_text(cx, &self.status_text);
        }
        self.view
            .label(cx, ids!(devices_status_label))
            .set_visible(cx, empty);
        self.view.redraw(cx);
    }

    fn refresh_count_label(&self, cx: &mut Cx) {
        let n = self.devices.len();
        let text = if n == 1 {
            "1 device".to_string()
        } else {
            format!("{n} devices")
        };
        self.view
            .label(cx, ids!(devices_count_label))
            .set_text(cx, &text);
    }

    /// Show the user a "your server wants you to re-authenticate in the
    /// browser" prompt. On Open they're sent to the homeserver's UIA
    /// fallback page; the UIA session is valid for ~10 min, so after they
    /// come back they can re-click Remove and it'll succeed.
    fn prompt_browser_reauth(&self, cx: &mut Cx, fallback_url: String) {
        let url_for_callback = fallback_url.clone();
        let content = ConfirmationModalContent {
            title_text: Cow::Borrowed("Re-authenticate to remove this device"),
            body_text: Cow::Owned(format!(
                "Your homeserver requires you to re-authenticate before deleting \
                 this device. We'll open the authentication page in your browser. \
                 After you finish there, come back here and click Remove device \
                 again to complete the removal.\n\n{fallback_url}"
            )),
            accept_button_text: Some(Cow::Borrowed("Open in browser")),
            cancel_button_text: Some(Cow::Borrowed("Cancel")),
            on_accept_clicked: Some(Box::new(move |_cx| {
                if let Err(e) = robius_open::Uri::new(&url_for_callback).open() {
                    enqueue_popup_notification(
                        format!("Couldn't open browser: {e:?}"),
                        PopupKind::Error,
                        Some(8.0),
                    );
                }
            })),
            on_cancel_clicked: None,
        };
        cx.action(PositiveConfirmationModalAction::Show(RefCell::new(Some(
            content,
        ))));
    }

    fn open_remove_confirmation(&self, cx: &mut Cx, device_id: String, display_label: String) {
        let body = format!(
            "Make sure you always have access to another verified device or your \
             recovery key to avoid losing your encrypted chat history.\n\n{display_label}\n{device_id}"
        );
        let device_id_for_callback = device_id.clone();
        let content = ConfirmationModalContent {
            title_text: Cow::Borrowed("Are you sure you want to remove this device?"),
            body_text: Cow::Owned(body),
            accept_button_text: Some(Cow::Borrowed("Remove device")),
            cancel_button_text: Some(Cow::Borrowed("Cancel")),
            on_accept_clicked: Some(Box::new(move |_cx| {
                submit_async_request(MatrixRequest::DeleteDevice {
                    device_id: device_id_for_callback.clone(),
                });
            })),
            on_cancel_clicked: None,
        };
        cx.action(ConfirmDeleteAction::Show(RefCell::new(Some(content))));
    }
}

// ────────────────────────────── helpers ──────────────────────────────

fn format_last_active(device: &DeviceInfo) -> String {
    let Some(ms) = device.last_seen_ts_ms else {
        return "—".to_string();
    };
    let Some(dt) = DateTime::from_timestamp_millis(ms) else {
        return "—".to_string();
    };
    let local = dt.with_timezone(&Local);
    // e.g. "Active Tue, May 19, 2026 at 5:17 PM"
    format!("Active {}", local.format("%a, %b %-d, %Y at %-I:%M %p"))
}

