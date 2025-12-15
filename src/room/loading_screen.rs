//! A dock tab loading view for room operations.
//! Use `show_room_loading_tab()` / `hide_room_loading_tab()` to toggle.

use crossbeam_queue::SegQueue;
use makepad_widgets::*;

/// Actions for loading tabs inside the dock.
#[derive(Debug, Clone)]
pub enum RoomLoadingScreenAction {
    /// Show or create a loading tab with the given id/name/message.
    ShowTab {
        tab_id: LiveId,
        tab_name: String,
        message: Option<String>,
        details: Option<String>,
    },
    /// Close the loading tab with the given id.
    HideTab {
        tab_id: LiveId,
    },
}

/// Pending actions that should be applied on the UI thread.
static PENDING_LOADING_ACTIONS: SegQueue<RoomLoadingScreenAction> = SegQueue::new();

/// Show (or create) a dock tab that only contains a room loading screen.
///
/// `tab_id` should be unique within the dock; a common pattern is
/// `LiveId::from_str(room_id.as_str())` or `LiveId::from_str(&format!(\"loading_{room_id}\"))`.
pub fn show_room_loading_tab(
    tab_id: LiveId,
    tab_name: impl Into<String>,
    message: impl Into<Option<String>>,
    details: impl Into<Option<String>>,
) {
    PENDING_LOADING_ACTIONS.push(RoomLoadingScreenAction::ShowTab {
        tab_id,
        tab_name: tab_name.into(),
        message: message.into(),
        details: details.into(),
    });
    SignalToUI::set_ui_signal();
}

/// Hide and close the loading tab with the given id, if it exists.
pub fn hide_room_loading_tab(tab_id: LiveId) {
    PENDING_LOADING_ACTIONS.push(RoomLoadingScreenAction::HideTab { tab_id });
    SignalToUI::set_ui_signal();
}

/// Drain all pending actions for loading tabs.
pub fn drain_room_loading_screen_actions(
) -> impl Iterator<Item = RoomLoadingScreenAction> {
    std::iter::from_fn(|| PENDING_LOADING_ACTIONS.pop())
}

/// Deterministic helper to derive a unique LiveId for a loading tab
/// from any stable string (e.g., room id or alias). This keeps the same tab
/// reusable across multiple jumps/clicks.
pub fn loading_tab_live_id(key: &str) -> LiveId {
    LiveId::from_str(&format!("loading_{key}"))
}

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::helpers::*;
    use crate::shared::styles::*;


    pub RoomLoadingScreen = {{RoomLoadingScreen}}<ScrollXYView> {
        width: Fill, height: Fill,
        flow: Down,
        align: {x: 0.5, y: 0.5},
        spacing: 10.0,

        show_bg: true,
        draw_bg: {
            color: (COLOR_PRIMARY_DARKER),
        }

        loading_spinner = <LoadingSpinner> {
            width: 60,
            height: 60,
            visible: true,
            draw_bg: {
                color: (COLOR_ACTIVE_PRIMARY)
                border_size: 4.0,
            }
        }

        title = <Label> {
            width: Fill, height: Fit,
            align: {x: 0.5, y: 0.0},
            padding: {left: 5.0, right: 0.0}
            margin: {top: 10.0},
            flow: RightWrap,
            draw_text: {
                color: (TYPING_NOTICE_TEXT_COLOR),
            }
            text: "Loading..."
        }

        details = <Label> {
            width: Fill, height: Fit,
            align: {x: 0.5, y: 0.0},
            padding: {left: 5.0, right: 0.0}
            margin: {top: 5.0},
            flow: RightWrap,
            draw_text: {
                color: (TYPING_NOTICE_TEXT_COLOR),
            }
            text: ""
        }
    }
}

/// A centered overlay with a spinner and status text.
#[derive(Live, LiveHook, Widget)]
pub struct RoomLoadingScreen {
    #[deref]
    view: View,

    #[live(false)]
    visible: bool,
}

impl Widget for RoomLoadingScreen {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if self.visible {
            self.view.handle_event(cx, event, scope);
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        if self.visible {
            self.view.draw_walk(cx, scope, walk)
        } else {
            DrawStep::done()
        }
    }
}

impl RoomLoadingScreenRef {
    /// Show the overlay and update the displayed message and optional details.
    pub fn show(&self, cx: &mut Cx, message: Option<&str>, details: Option<&str>) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.visible = true;
            inner.view.set_visible(cx, true);
            let text = message.unwrap_or("Loading...");
            inner.view.label(ids!(title)).set_text(cx, text);
            let details_label = inner.view.label(ids!(details));
            if let Some(detail_text) = details {
                details_label.set_visible(cx, true);
                details_label.set_text(cx, detail_text);
            } else {
                details_label.set_visible(cx, false);
                details_label.set_text(cx, "");
            }
        }
    }

    /// Update the message without toggling visibility.
    pub fn set_message(&self, cx: &mut Cx, message: Option<&str>) {
        if let Some(inner) = self.borrow() {
            let text = message.unwrap_or("Loading...");
            inner.view.label(ids!(title)).set_text(cx, text);
        }
    }

    /// Hide the overlay.
    pub fn hide(&self, cx: &mut Cx) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.visible = false;
            inner.view.set_visible(cx, false);
        }
    }
}
