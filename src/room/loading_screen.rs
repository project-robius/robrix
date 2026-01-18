use crossbeam_queue::SegQueue;
use makepad_widgets::*;

/// Pending actions that should be applied on the UI thread.
static PENDING_LOADING_ACTIONS: SegQueue<RoomLoadingScreenAction> = SegQueue::new();

pub fn show_room_loading_tab(
    tab_id: LiveId,
    tab_name: impl Into<String>,
    title: impl Into<Option<String>>,
    details: impl Into<Option<String>>,
) {
    PENDING_LOADING_ACTIONS.push(RoomLoadingScreenAction::ShowTab {
        tab_id,
        tab_name: tab_name.into(),
        title: title.into(),
        details: details.into(),
    });
    SignalToUI::set_ui_signal();
}

pub fn hide_room_loading_tab(tab_id: LiveId) {
    PENDING_LOADING_ACTIONS.push(RoomLoadingScreenAction::HideTab { tab_id });
    SignalToUI::set_ui_signal();
}

pub fn get_room_loading_screen_actions() -> impl Iterator<Item = RoomLoadingScreenAction> {
    std::iter::from_fn(|| PENDING_LOADING_ACTIONS.pop())
}

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
                wrap: Word,
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
                wrap: Word,
            }
            text: ""
        }
    }
}

#[derive(Debug, Clone)]
pub enum RoomLoadingScreenAction {
    ShowTab {
        tab_id: LiveId,
        tab_name: String,
        title: Option<String>,
        details: Option<String>,
    },
    HideTab {
        tab_id: LiveId,
    }
}

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

impl RoomLoadingScreen {
    pub fn show(&mut self, cx: &mut Cx, title: Option<&str>, details: Option<&str>) {
        self.visible = true;
        self.view.set_visible(cx, true);
        let title_label_ref = self.view.label(ids!(title));
        let details_lable_ref = self.view.label(ids!(details));

        // Set text for title label
        let title_text = title.unwrap_or("Loading...");
        title_label_ref.set_text(cx, title_text);

        // Set text for details label
        if let Some(details_text) = details {
            details_lable_ref.set_visible(cx, true);
            details_lable_ref.set_text(cx, details_text);
        } else {
            details_lable_ref.set_visible(cx, false);
            details_lable_ref.set_text(cx, "");
        };
    }
}
impl RoomLoadingScreenRef {
    pub fn show(&self, cx: &mut Cx, title: Option<&str>, details: Option<&str>) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.show(cx, title, details);
    }

    pub fn hide(&self, cx: &mut Cx) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.visible = false;
            inner.view.set_visible(cx, false);
        }
    }

    pub fn set_title_text(&self, cx: &mut Cx, title: Option<&str>) {
        if let Some(inner) = self.borrow() {
            let text = title.unwrap_or("Loading...");
            inner.view.label(ids!(title)).set_text(cx, text);
        }
    }
}
