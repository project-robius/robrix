use makepad_widgets::*;
use matrix_sdk::ruma::OwnedEventId;

use crate::sliding_sync::TimelineRequestSender;


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
#[derive(Default)]
pub enum LoadingPaneState {
    /// The room is being backwards paginated until the target event is reached.
    BackwardsPaginateUntilEvent {
        target_event_id: OwnedEventId,
        /// A human-friendly description of what message/event we're searching for.
        description: String,
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
impl Drop for LoadingPaneState {
    fn drop(&mut self) {
        // upon drop, tell the background async task to stop looking for the target event,
        // because the UI side no longer cares about it (the user closed the room, thread, or loading pane).
        let Self::BackwardsPaginateUntilEvent { target_event_id, request_sender, .. } = self else { return };
        request_sender.send_if_modified(|req| {
            let initial_len = req.backwards_paginate.len();
            req.backwards_paginate.retain(|r| &r.target_event_id != target_event_id);
            req.backwards_paginate.len() != initial_len
        });
    }
}


#[derive(Script, ScriptHook, Widget)]
pub struct LoadingPane {
    #[deref] view: View,
    #[rust] state: LoadingPaneState,
}


impl Widget for LoadingPane {
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.visible = true;
        if matches!(self.state, LoadingPaneState::None) {
            self.visible = false;
            return self.view.draw_walk(cx, scope, walk);
        }

        self.view.draw_walk(cx, scope, walk)
    }

    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
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
            self.hide(cx);
        }
    }
}


impl LoadingPane {
    /// Returns `true` if this pane is currently being shown.
    pub fn is_currently_shown(&self, _cx: &mut Cx) -> bool {
        self.visible
    }

    /// Hides this pane, which also cancels any search it was showing.
    pub fn hide(&mut self, cx: &mut Cx) {
        self.set_state(cx, LoadingPaneState::None);
        // Only give back key focus if we still had it; something else may have taken it.
        if cx.has_key_focus(self.view.area()) {
            cx.revert_key_focus();
        }
        self.visible = false;
    }

    pub fn show(&mut self, cx: &mut Cx) {
        self.visible = true;
        cx.set_key_focus(self.view.area());
        self.redraw(cx);
    }

    pub fn set_state(&mut self, cx: &mut Cx, state: LoadingPaneState) {
        let cancel_button = self.button(cx, ids!(cancel_button));
        match &state {
            LoadingPaneState::BackwardsPaginateUntilEvent {
                description,
                events_paginated,
                ..
            } => {
                self.set_title(cx, "Searching older messages...");
                self.set_status(cx, &format!(
                    "Looking for {description}...\n\n\
                    Fetched {events_paginated} messages so far...",
                ));
                cancel_button.set_text(cx, "Cancel");
            }
            LoadingPaneState::Error(error_message) => {
                self.set_title(cx, "Error loading content");
                self.set_status(cx, error_message);
                cancel_button.set_text(cx, "Okay");
            }
            LoadingPaneState::None => { }
        }

        self.state = state;
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

    /// See [`LoadingPane::hide()`]
    pub fn hide(&self, cx: &mut Cx) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.hide(cx);
    }

    /// Returns `true` if this pane is currently searching for any event.
    pub fn is_searching(&self) -> bool {
        self.borrow().is_some_and(|inner|
            matches!(inner.state, LoadingPaneState::BackwardsPaginateUntilEvent { .. })
        )
    }

    /// Returns the target event ID this pane is currently searching for, if any.
    pub fn searching_for(&self) -> Option<OwnedEventId> {
        self.borrow().and_then(|inner| match &inner.state {
            LoadingPaneState::BackwardsPaginateUntilEvent { target_event_id, .. } => Some(target_event_id.clone()),
            _ => None,
        })
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
