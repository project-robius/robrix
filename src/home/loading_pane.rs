use makepad_widgets::*;
use matrix_sdk::ruma::{EventId, OwnedEventId};

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


/// The error shown when a jumped-to event can't be found.
/// `noun` is what the search covered, i.e. "room" or "thread".
fn jump_target_not_found_msg(noun: &str) -> String {
    format!("Couldn't find that event in this {noun}'s history.\n\n\
        It may have been deleted, or the homeserver may not be able to return it.")
}


/// The state of a LoadingPane: the possible tasks that it may be performing.
#[derive(Default)]
enum LoadingPaneState {
    /// The room is being backwards paginated until the target event is reached.
    BackwardsPaginateUntilEvent {
        target_event_id: OwnedEventId,
        /// A human-friendly description of what message/event we're searching for.
        description: String,
        /// The number of events paginated so far, which is only used to display progress.
        events_paginated: usize,
        /// "room" or "thread", used when telling the user we couldn't find the event.
        timeline_noun: &'static str,
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
    #[rust] orig_key_focus_area: Option<Area>,
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

    /// Returns `true` if this pane is currently searching for any event.
    pub fn is_searching(&self) -> bool {
        matches!(self.state, LoadingPaneState::BackwardsPaginateUntilEvent { .. })
    }

    /// Returns the event ID this pane is currently searching for, if any.
    pub fn searching_for(&self) -> Option<OwnedEventId> {
        match &self.state {
            LoadingPaneState::BackwardsPaginateUntilEvent { target_event_id, .. } => Some(target_event_id.clone()),
            _ => None,
        }
    }

    /// Shows this pane and starts searching backwards for the given event.
    ///
    /// If another search was in progress, it will be cancelled.
    pub fn start_search(
        &mut self,
        cx: &mut Cx,
        target_event_id: OwnedEventId,
        description: String,
        timeline_noun: &'static str,
        request_sender: TimelineRequestSender,
    ) {
        self.set_state(cx, LoadingPaneState::BackwardsPaginateUntilEvent {
            target_event_id,
            description,
            events_paginated: 0,
            timeline_noun,
            request_sender,
        });
        self.visible = true;
        let area = self.view.area();
        self.orig_key_focus_area = Some(area);
        cx.set_key_focus(area);
        self.redraw(cx);
    }

    /// Adds `num_events` to the progress shown by an in-progress search.
    ///
    /// This is additive, not a replacement.
    pub fn paginated_more_events(&mut self, cx: &mut Cx, num_events: usize) {
        let LoadingPaneState::BackwardsPaginateUntilEvent { events_paginated, .. } = &mut self.state else { return };
        *events_paginated += num_events;
        self.populate(cx);
    }

    /// Sets the timeline request sender for this loading pane's current search.
    ///
    /// Does nothing if there's no search in progress.
    ///
    /// This is useful for when the timeline endpoint channels get re-created,
    /// and we need to update them so that the loading pane can send requests
    /// back to the new backend task that's doing the searching.
    pub fn set_timeline_request_sender(&mut self, request_sender: TimelineRequestSender) {
        let LoadingPaneState::BackwardsPaginateUntilEvent { request_sender: sender, .. } = &mut self.state else { return };
        *sender = request_sender;
    }

    /// Ends the current search and shows an error (we couldn't find the event).
    pub fn search_failed(&mut self, cx: &mut Cx) {
        let LoadingPaneState::BackwardsPaginateUntilEvent { timeline_noun, .. } = &self.state else { return };
        let error_message = jump_target_not_found_msg(timeline_noun);
        self.set_state(cx, LoadingPaneState::Error(error_message));
    }

    /// Hides this pane, which also cancels any search it was showing.
    pub fn hide(&mut self, cx: &mut Cx) {
        self.set_state(cx, LoadingPaneState::None);
        // Give key focus back if we're still holding the focus we initially took.
        if let Some(area) = self.orig_key_focus_area.take() && cx.has_key_focus(area) {
            cx.revert_key_focus();
        }
        self.visible = false;
    }

    /// Returns `true` if this pane is searching for the given target event.
    pub fn is_searching_for(&self, target_event_id: &EventId) -> bool {
        matches!(
            &self.state,
            LoadingPaneState::BackwardsPaginateUntilEvent { target_event_id: id, .. }
                if &**id == target_event_id
        )
    }

    fn set_state(&mut self, cx: &mut Cx, state: LoadingPaneState) {
        // This will drop the previous `self.state`, which cancels its background request.
        self.state = state;
        self.populate(cx);
    }

    /// Populates this pane's labels and button from its current state.
    fn populate(&mut self, cx: &mut Cx) {
        let ui_text = match &self.state {
            LoadingPaneState::BackwardsPaginateUntilEvent { description, events_paginated, .. } => Some((
                "Searching older messages...",
                format!(
                    "Looking for {description}...\n\n\
                    Fetched {events_paginated} messages so far...",
                ),
                "Cancel",
            )),
            LoadingPaneState::Error(error_message) => Some((
                "Error loading content",
                error_message.clone(),
                "Okay",
            )),
            LoadingPaneState::None => None,
        };
        if let Some((title_text, status_text, button_text)) = ui_text {
            self.label(cx, ids!(title)).set_text(cx, title_text);
            self.label(cx, ids!(status)).set_text(cx, &status_text);
            self.button(cx, ids!(cancel_button)).set_text(cx, button_text);
        }
        self.redraw(cx);
    }
}

impl LoadingPaneRef {
    /// See [`LoadingPane::is_currently_shown()`]
    pub fn is_currently_shown(&self, cx: &mut Cx) -> bool {
        let Some(inner) = self.borrow() else { return false };
        inner.is_currently_shown(cx)
    }

    /// See [`LoadingPane::is_searching()`]
    pub fn is_searching(&self) -> bool {
        self.borrow().is_some_and(|inner| inner.is_searching())
    }

    /// See [`LoadingPane::is_searching_for()`]
    pub fn is_searching_for(&self, target_event_id: &EventId) -> bool {
        self.borrow().is_some_and(|inner| inner.is_searching_for(target_event_id))
    }

    /// See [`LoadingPane::searching_for()`]
    pub fn searching_for(&self) -> Option<OwnedEventId> {
        self.borrow().and_then(|inner| inner.searching_for())
    }

    /// See [`LoadingPane::start_search()`]
    pub fn start_search(
        &self,
        cx: &mut Cx,
        target_event_id: OwnedEventId,
        description: String,
        timeline_noun: &'static str,
        request_sender: TimelineRequestSender,
    ) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.start_search(cx, target_event_id, description, timeline_noun, request_sender);
    }

    /// See [`LoadingPane::paginated_more_events()`]
    pub fn paginated_more_events(&self, cx: &mut Cx, num_events: usize) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.paginated_more_events(cx, num_events);
    }

    /// See [`LoadingPane::set_timeline_request_sender()`]
    pub fn set_timeline_request_sender(&self, request_sender: TimelineRequestSender) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.set_timeline_request_sender(request_sender);
    }

    /// See [`LoadingPane::search_failed()`]
    pub fn search_failed(&self, cx: &mut Cx) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.search_failed(cx);
    }

    /// See [`LoadingPane::hide()`]
    pub fn hide(&self, cx: &mut Cx) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.hide(cx);
    }
}
