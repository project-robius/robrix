use makepad_widgets::*;
use matrix_sdk::ruma::OwnedEventId;

use crate::sliding_sync::TimelineRequestSender;


live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::helpers::*;
    use crate::shared::styles::*;
    use crate::shared::avatar::*;
    use crate::shared::icon_button::*;

    pub LoadingPane = {{LoadingPane}} {
        visible: false,
        flow: Overlay,
        width: Fill,
        height: Fill,
        align: {x: 0.5, y: 0.5}

        show_bg: true
        draw_bg: {
            fn pixel(self) -> vec4 {
                return vec4(0., 0., 0., 0.7)
            }
        }

        main_content = <RoundedView> {
            flow: Down
            width: 400
            height: Fit
            padding: {top: 25, right: 30 bottom: 30 left: 45}
            spacing: 10

            show_bg: true
            draw_bg: {
                color: #fff
                border_radius: 3.0
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
                height: Fit, // TODO: ideally this would be a range, maybe like 300-500 px
                flow: Down,
                spacing: 40,

                status = <Label> {
                    width: Fill,
                    height: Fit,
                    draw_text: {
                        text_style: <REGULAR_TEXT>{
                            font_size: 11.5,
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
                        align: {x: 0.5, y: 0.5}
                        padding: 15
                        // draw_icon: {
                        //     svg_file: (ICON_FORBIDDEN)
                        //     color: (COLOR_FG_DANGER_RED),
                        // }
                        icon_walk: {width: 0, height: 0 }

                        draw_bg: {
                            border_color: (COLOR_FG_DANGER_RED),
                            color: (COLOR_BG_DANGER_RED)
                        }
                        text: "Cancel"
                        draw_text:{
                            color: (COLOR_FG_DANGER_RED),
                        }
                    }
                }
            }
        }
    }
}



/// The state of a LoadingPane: the possible tasks that it may be performing.
#[derive(Clone, DefaultNone)]
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
    None,
}


#[derive(Live, LiveHook, Widget)]
pub struct LoadingPane {
    #[deref] view: View,
    #[rust] state: LoadingPaneState,
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
                Event::Actions(actions) if self.button(ids!(cancel_button)).clicked(actions)
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
                    || !self.view(ids!(main_content)).area().rect(cx).contains(fue.abs)
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
        let cancel_button = self.button(ids!(cancel_button));
        match &state {
            LoadingPaneState::BackwardsPaginateUntilEvent {
                target_event_id,
                events_paginated,
                ..
            } => {
                self.set_title(cx, "Searching older messages...");
                self.set_status(cx, &format!(
                    "Looking for event {target_event_id}\n\n\
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
        self.label(ids!(status)).set_text(cx, status);
    }

    pub fn set_title(&mut self, cx: &mut Cx, title: &str) {
        self.label(ids!(title)).set_text(cx, title);
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
