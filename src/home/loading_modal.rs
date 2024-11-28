use makepad_widgets::*;
use matrix_sdk::ruma::OwnedEventId;

use crate::sliding_sync::TimelineRequestSender;

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::styles::*;
    use crate::shared::icon_button::RobrixIconButton;

    pub LoadingModal = {{LoadingModal}} {
        width: Fit
        height: Fit

        <RoundedView> {
            flow: Down
            width: 600
            height: Fit
            padding: {top: 25, right: 30 bottom: 30 left: 45}
            spacing: 10

            show_bg: true
            draw_bg: {
                color: #fff
                radius: 3.0
            }

            title_view = <View> {
                width: Fill,
                height: Fit,
                flow: Right
                padding: {top: 0, bottom: 40}
                align: {x: 0.5, y: 0.0}

                title = <Label> {
                    text: "Loading content..."
                    draw_text: {
                        text_style: <TITLE_TEXT>{font_size: 13},
                        color: #000
                    }
                }
            }

            body = <View> {
                width: Fill,
                height: Fit,
                flow: Down,
                spacing: 40,

                status = <Label> {
                    width: Fill
                    draw_text: {
                        text_style: <REGULAR_TEXT>{
                            font_size: 11.5,
                            height_factor: 1.3
                        },
                        color: #000
                        wrap: Word
                    }
                }

                <View> {
                    width: Fill, height: Fit
                    flow: Right,
                    align: {x: 1.0, y: 0.5}
                    spacing: 20

                    cancel_button = <RobrixIconButton> {
                        padding: {left: 15, right: 15}
                        // draw_icon: {
                        //     svg_file: (ICON_BLOCK_USER)
                        //     color: (COLOR_DANGER_RED),
                        // }
                        icon_walk: {width: 0, height: 0 }

                        draw_bg: {
                            border_color: (COLOR_DANGER_RED),
                            color: #fff0f0 // light red
                        }
                        text: "Cancel"
                        draw_text:{
                            color: (COLOR_DANGER_RED),
                        }
                    }
                }
            }
        }
    }
}

#[derive(Live, LiveHook, Widget)]
pub struct LoadingModal {
    #[deref] view: View,
    #[rust] state: LoadingModalState,
}
impl Drop for LoadingModal {
    fn drop(&mut self) {
        if let LoadingModalState::BackwardsPaginateUntilEvent { target_event_id, request_sender, .. } = &self.state {
            warning!("Dropping LoadingModal with target_event_id: {}", target_event_id);
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

/// An action sent from this LoadingModal widget to its parent widget,
/// which is currently required in order for the parent to close this modal.
#[derive(Clone, Debug, DefaultNone)]
pub enum LoadingModalAction {
    Close,
    None,
}

/// The state of a LoadingModal: the possible tasks that it may be performing.
#[derive(Clone, DefaultNone)]
pub enum LoadingModalState {
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
    /// The loading modal is displaying an error message until the user closes it.
    Error(String),
    /// The LoadingModal is not doing anything and can be hidden.
    None,
}

impl Widget for LoadingModal {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        self.widget_match_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl WidgetMatchEvent for LoadingModal {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions, scope: &mut Scope) {
        let widget_uid = self.widget_uid();
        let cancel_button = self.button(id!(cancel_button));

        let modal_dismissed = actions
            .iter()
            .any(|a| matches!(a.downcast_ref(), Some(ModalAction::Dismissed)));

        if cancel_button.clicked(actions) || modal_dismissed {
            log!("LoadingModal: close requested: {}", if modal_dismissed { "by modal dismiss" } else { "by cancel button" });
            if let LoadingModalState::BackwardsPaginateUntilEvent { target_event_id, request_sender, .. } = &self.state {
                let _did_send = request_sender.send_if_modified(|requests| {
                    let initial_len = requests.len();
                    requests.retain(|r| &r.target_event_id != target_event_id);
                    // if we actually cancelled this request, notify the receivers
                    // such that they can stop looking for the target event.
                    requests.len() != initial_len
                });
                log!("LoadingModal: {} cancel request for target_event_id: {target_event_id}",
                    if _did_send { "Sent" } else { "Did not send" },
                );
            }
            self.set_state(cx, LoadingModalState::None);

            // If the modal was dismissed by clicking outside of it, we MUST NOT emit
            // a `LoadingModalAction::Close` action, as that would cause
            // an infinite action feedback loop.
            if !modal_dismissed {
                cx.widget_action(widget_uid, &scope.path, LoadingModalAction::Close);
            }
        }
    }
}

impl LoadingModal {
    pub fn set_state(&mut self, cx: &mut Cx, state: LoadingModalState) {
        let cancel_button = self.button(id!(cancel_button));
        match &state {
            LoadingModalState::BackwardsPaginateUntilEvent {
                target_event_id,
                events_paginated,
                ..
            } => {
                self.set_title(cx, "Searching older messages...");
                self.set_status(cx, &format!(
                    "Looking for event {target_event_id}\n\n\
                    Fetched {events_paginated} messages so far...",
                ));
                cancel_button.set_text_and_redraw(cx, "Cancel");
            }
            LoadingModalState::Error(error_message) => {
                self.set_title(cx, "Error loading content");
                self.set_status(cx, error_message);
                cancel_button.set_text_and_redraw(cx, "Okay");
            }
            LoadingModalState::None => { }
        }

        self.state = state;
        self.redraw(cx);
    }

    pub fn set_status(&mut self, cx: &mut Cx, status: &str) {
        self.label(id!(status)).set_text_and_redraw(cx, status);
    }

    pub fn set_title(&mut self, cx: &mut Cx, title: &str) {
        self.label(id!(title)).set_text_and_redraw(cx, title);
    }
}

impl LoadingModalRef {
    pub fn take_state(&self) -> LoadingModalState {
        self.borrow_mut()
            .map(|mut inner| std::mem::take(&mut inner.state))
            .unwrap_or(LoadingModalState::None)
    }

    pub fn set_state(&self, cx: &mut Cx, state: LoadingModalState) {
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
