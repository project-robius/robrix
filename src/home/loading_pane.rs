use makepad_widgets::*;
use matrix_sdk::ruma::OwnedEventId;

use crate::{app::AppState, i18n::{AppLanguage, tr_fmt, tr_key}, sliding_sync::TimelineRequestSender};


script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*


    mod.widgets.LoadingPane = set_type_default() do #(LoadingPane::register_widget(vm)) {
        ..mod.widgets.SolidView

        visible: false,
        flow: Overlay,
        width: Fill,
        height: Fill,
        align: Align{x: 0.5, y: 0.5}

        show_bg: true
        draw_bg +: {
            color: #000000b2
        }

        main_content := RoundedView {
            flow: Down
            width: 400
            height: Fit
            padding: Inset{top: 25, right: 30 bottom: 30 left: 45}
            spacing: 10

            show_bg: true
            draw_bg +: {
                color: (COLOR_PRIMARY)
                border_radius: 4.0
            }

            title_view := View {
                width: Fill,
                height: Fit,
                flow: Right
                padding: Inset{top: 0, bottom: 40}
                align: Align{x: 0.5, y: 0.0}

                title := Label {
                    text: "Loading content..."
                    draw_text +: {
                        text_style: TITLE_TEXT {font_size: 13},
                        color: #000
                    }
                }
            }

            body := View {
                width: Fill,
                height: Fit, // TODO: ideally this would be a range, maybe like 300-500 px
                flow: Down,
                spacing: 40,

                status := Label {
                    width: Fill,
                    height: Fit,
                    flow: Flow.Right{wrap: true},
                    draw_text +: {
                        text_style: REGULAR_TEXT {
                            font_size: 11.5,
                        },
                        color: #000
                    }
                }

                View {
                    width: Fill, height: Fit
                    flow: Right,
                    align: Align{x: 1.0, y: 0.5}
                    spacing: 20

                    cancel_button := RobrixNegativeIconButton {
                        align: Align{x: 0.5, y: 0.5}
                        padding: 15
                        icon_walk: Walk{width: 0, height: 0 }
                        text: "Cancel"
                    }
                }
            }
        }
    }
}



/// The state of a LoadingPane: the possible tasks that it may be performing.
#[derive(Clone, Default)]
pub enum LoadingPaneState {
    /// The room is being backwards paginated until the target event is reached.
    BackwardsPaginateUntilEvent {
        target_event_id: OwnedEventId,
        /// The number of events paginated so far, which is only used to display progress.
        events_paginated: usize,
        /// The sender for timeline requests for the room that is showing this modal.
        /// This is used to inform the `timeline_subscriber_handler` that the user has
        /// cancelled the request, so that it can stop looking for the target event.
        request_sender: TimelineRequestSender,
    },
    /// The loading pane is displaying an error message until the user closes it.
    Error(String),
    /// The LoadingPane is not doing anything and can be hidden.
    #[default]
    None,
}


#[derive(Script, ScriptHook, Widget)]
pub struct LoadingPane {
    #[deref] view: View,
    #[rust] state: LoadingPaneState,
    #[rust] app_language: AppLanguage,
}
impl Drop for LoadingPane {
    fn drop(&mut self) {
        if let LoadingPaneState::BackwardsPaginateUntilEvent { target_event_id, request_sender, .. } = &self.state {
            warning!("Dropping LoadingPane with target_event_id: {}", target_event_id);
            request_sender.send_if_modified(|requests| {
                let initial_len = requests.len();
                requests.retain(|r| &r.target_event_id != target_event_id);
                // if we actually cancelled this request, notify the receivers
                // such that they can stop looking for the target event.
                requests.len() != initial_len
            });
        }
    }
}


impl Widget for LoadingPane {
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        let app_language = scope.data.get::<AppState>()
            .map(|app_state| app_state.app_language)
            .unwrap_or_default();
        if self.app_language != app_language {
            self.set_app_language(cx, app_language);
        }
        self.visible = true;
        if matches!(self.state, LoadingPaneState::None) {
            self.visible = false;
            return self.view.draw_walk(cx, scope, walk);
        }

        self.view.draw_walk(cx, scope, walk)
    }

    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        let app_language = scope.data.get::<AppState>()
            .map(|app_state| app_state.app_language)
            .unwrap_or_default();
        if self.app_language != app_language {
            self.set_app_language(cx, app_language);
        }
        if !self.visible { return; }
        self.view.handle_event(cx, event, scope);

        let area = self.view.area();

        // Close the pane if:
        // 1. The cancel button is clicked,
        // 2. The back navigational gesture/action occurs (e.g., Back on Android),
        // 3. The escape key is pressed if this pane has key focus,
        // 4. The back mouse button is clicked within this view,
        // 5. The user clicks/touches outside the main_content view area.
        let close_pane = {
            matches!(
                event,
                Event::Actions(actions) if self.button(cx, ids!(cancel_button)).clicked(actions)
            )
            || event.back_pressed()
            || match event.hits_with_capture_overload(cx, area, true) {
                Hit::KeyUp(key) => key.key_code == KeyCode::Escape,
                Hit::FingerDown(_fde) => {
                    cx.set_key_focus(area);
                    false
                }
                Hit::FingerUp(fue) if fue.is_over => {
                    fue.mouse_button().is_some_and(|b| b.is_back())
                    || !self.view(cx, ids!(main_content)).area().rect(cx).contains(fue.abs)
                }
                _ => false,
            }
        };
        if close_pane {
            if let LoadingPaneState::BackwardsPaginateUntilEvent { target_event_id, request_sender, .. } = &self.state {
                let _did_send = request_sender.send_if_modified(|requests| {
                    let initial_len = requests.len();
                    requests.retain(|r| &r.target_event_id != target_event_id);
                    // if we actually cancelled this request, notify the receivers
                    // such that they can stop looking for the target event.
                    requests.len() != initial_len
                });
                log!("LoadingPane: {} cancel request for target_event_id: {target_event_id}",
                    if _did_send { "Sent" } else { "Did not send" },
                );
            }
            self.set_state(cx, LoadingPaneState::None);
            cx.revert_key_focus();
            self.visible = false;
        }
    }
}


impl LoadingPane {
    fn set_app_language(&mut self, cx: &mut Cx, app_language: AppLanguage) {
        self.app_language = app_language;
        self.sync_state_text(cx);
        self.view.redraw(cx);
    }

    fn sync_state_text(&mut self, cx: &mut Cx) {
        let (title, status, cancel_text) = match &self.state {
            LoadingPaneState::BackwardsPaginateUntilEvent {
                target_event_id,
                events_paginated,
                ..
            } => {
                let events_paginated_str = events_paginated.to_string();
                (
                    tr_key(self.app_language, "loading_pane.title.searching_older").to_string(),
                    Some(tr_fmt(self.app_language, "loading_pane.status.searching_event", &[
                        ("target_event_id", target_event_id.as_str()),
                        ("events_paginated", events_paginated_str.as_str()),
                    ])),
                    tr_key(self.app_language, "loading_pane.button.cancel").to_string(),
                )
            }
            LoadingPaneState::Error(error_message) => (
                tr_key(self.app_language, "loading_pane.title.error").to_string(),
                Some(error_message.clone()),
                tr_key(self.app_language, "loading_pane.button.okay").to_string(),
            ),
            LoadingPaneState::None => (
                tr_key(self.app_language, "loading_pane.title.default").to_string(),
                None,
                tr_key(self.app_language, "loading_pane.button.cancel").to_string(),
            ),
        };

        self.set_title(cx, &title);
        if let Some(status) = status {
            self.set_status(cx, &status);
        }
        let cancel_button = self.button(cx, ids!(cancel_button));
        cancel_button.set_text(cx, &cancel_text);
    }

    /// Returns `true` if this pane is currently being shown.
    pub fn is_currently_shown(&self, _cx: &mut Cx) -> bool {
        self.visible
    }

    pub fn show(&mut self, cx: &mut Cx) {
        self.visible = true;
        cx.set_key_focus(self.view.area());
        self.redraw(cx);
    }

    pub fn set_state(&mut self, cx: &mut Cx, state: LoadingPaneState) {
        self.state = state;
        self.sync_state_text(cx);
        self.redraw(cx);
    }

    pub fn set_status(&mut self, cx: &mut Cx, status: &str) {
        self.label(cx, ids!(status)).set_text(cx, status);
    }

    pub fn set_title(&mut self, cx: &mut Cx, title: &str) {
        self.label(cx, ids!(title)).set_text(cx, title);
    }
}

impl LoadingPaneRef {
    /// See [`LoadingPane::is_currently_shown()`]
    pub fn is_currently_shown(&self, cx: &mut Cx) -> bool {
        let Some(inner) = self.borrow() else { return false };
        inner.is_currently_shown(cx)
    }

    /// See [`LoadingPane::show()`]
    pub fn show(&self, cx: &mut Cx) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.show(cx);
    }

    pub fn take_state(&self) -> LoadingPaneState {
        self.borrow_mut()
            .map(|mut inner| std::mem::take(&mut inner.state))
            .unwrap_or(LoadingPaneState::None)
    }

    pub fn set_state(&self, cx: &mut Cx, state: LoadingPaneState) {
        let Some(mut inner) = self.borrow_mut() else { return }; 
        inner.set_state(cx, state);
    }

    pub fn set_status(&self, cx: &mut Cx, status: &str) {
        let Some(mut inner) = self.borrow_mut() else { return }; 
        inner.set_status(cx, status);
    }

    pub fn set_title(&self, cx: &mut Cx, title: &str) {
        let Some(mut inner) = self.borrow_mut() else { return }; 
        inner.set_title(cx, title);
    }
}
