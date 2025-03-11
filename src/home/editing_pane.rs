use makepad_widgets::*;
use matrix_sdk::{room::edit::EditedContent, ruma::{events::{poll::unstable_start::{UnstablePollAnswer, UnstablePollStartContentBlock}, room::message::{FormattedBody, MessageType, RoomMessageEventContentWithoutRelation}}, OwnedRoomId}};
use matrix_sdk_ui::timeline::{EventTimelineItem, TimelineEventItemId, TimelineItemContent};

use crate::{shared::popup_list::enqueue_popup_notification, sliding_sync::{submit_async_request, MatrixRequest}};


live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::helpers::*;
    use crate::shared::styles::*;
    use crate::shared::avatar::*;
    use crate::shared::icon_button::*;

    // Copied from Moxin
    FadeView = <CachedView> {
        draw_bg: {
            instance opacity: 1.0

            fn pixel(self) -> vec4 {
                let color = sample2d_rt(self.image, self.pos * self.scale + self.shift) + vec4(self.marked, 0.0, 0.0, 0.0);
                return Pal::premul(vec4(color.xyz, color.w * self.opacity))
            }
        }
    }

    EditingContent = <View> {
        width: Fill,
        height: Fit,
        align: {x: 0.5, y: 1.0}, // centered horizontally, bottom-aligned
        padding: { left: 20, right: 20, top: 10, bottom: 10 }
        spacing: 10,
        flow: Down,

        show_bg: true,
        draw_bg: {
            color: (COLOR_PRIMARY)
        }

        <View> {
            width: Fill
            height: Fit
            flow: Right
            align: {y: 0.5}
            padding: {left: 5, right: 5}

            <Label> {
                width: Fill,
                draw_text: {
                    text_style: <USERNAME_TEXT_STYLE> {},
                    color: #222,
                    wrap: Ellipsis,
                }
                text: "Editing message:"
            }

            cancel_button = <RobrixIconButton> {
                width: Fit,
                height: Fit,
                padding: 13,
                margin: {left: 5, right: 5},

                draw_bg: {
                    border_color: (COLOR_DANGER_RED),
                    color: #fff0f0 // light red
                    radius: 5
                }
                draw_icon: {
                    svg_file: (ICON_CLOSE),
                    color: (COLOR_DANGER_RED)
                }
                icon_walk: {width: 16, height: 16, margin: 0}
            }

            accept_button = <RobrixIconButton> {
                width: Fit,
                height: Fit,
                padding: 13,
                margin: {left: 5, right: 5},

                draw_bg: {
                    border_color: (COLOR_ACCEPT_GREEN),
                    color: #f0fff0 // light green
                    radius: 5
                }
                draw_icon: {
                    svg_file: (ICON_CHECKMARK)
                    color: (COLOR_ACCEPT_GREEN),
                }
                icon_walk: {width: 16, height: 16, margin: 0}
            }
        }

        <LineH> {
            draw_bg: {color: (COLOR_DIVIDER_DARK)}
        }

        edit_text_input = <RobrixTextInput> {
            width: Fill, height: Fit,
            margin: { bottom: 5 }
            padding: { top: 3 }
            align: {y: 0.5}
            empty_message: "Enter edited message..."
        }
    }


    pub EditingPane = {{EditingPane}} {
        visible: false,
        width: Fill,
        height: Fit,
        align: {x: 0.5, y: 1.0}
        // TODO: FIXME: this is a hack to make the editing pane
        //              able to slide out of the bottom of the screen.
        //              (Waiting on a Makepad-level fix for this.)
        margin: {top: 1000}

        editing_content = <EditingContent> { }

        animator: {
            panel = {
                default: hide,
                show = {
                    redraw: true,
                    from: {all: Forward {duration: 0.8}}
                    ease: ExpDecay {d1: 0.80, d2: 0.97}
                    apply: { margin: {top: 0} }
                }
                hide = {
                    redraw: true,
                    from: {all: Forward {duration: 0.8}}
                    ease: ExpDecay {d1: 0.80, d2: 0.97}
                    // TODO: FIXME: this is a hack to make the editing pane
                    //              able to slide out of the bottom of the screen.
                    //              (Waiting on a Makepad-level fix for this.)
                    apply: { margin: {top: 1000} }
                }
            }
        }
    }
}


/// Action emitted by the EditingPane widget.
#[derive(Clone, DefaultNone, Debug)]
pub enum EditingPaneAction {
    /// The editing pane has been closed/hidden.
    Hide,
    None,
}

/// The information maintained by the EditingPane widget.
struct EditingPaneInfo {
    event_tl_item: EventTimelineItem,
    room_id: OwnedRoomId,
}

/// A view that slides in from the bottom of the screen to allow editing a message.
#[derive(Live, LiveHook, Widget)]
pub struct EditingPane {
    #[deref] view: View,
    #[animator] animator: Animator,

    #[rust] info: Option<EditingPaneInfo>,
    #[rust] is_animating_out: bool,
}

impl Widget for EditingPane {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);

        if !self.visible { return; }

        let animator_action = self.animator_handle_event(cx, event);
        if animator_action.must_redraw() {
            self.redraw(cx);
        }
        // If the animator is in the `hide` state and has finished animating out,
        // that means it has fully animated off-screen and can be set to invisible.
        if self.animator_in_state(cx, id!(panel.hide)) {
            match (self.is_animating_out, animator_action.is_animating()) {
                (true, false) => {
                    self.visible = false;
                    cx.widget_action(
                        self.widget_uid(),
                        &scope.path,
                        EditingPaneAction::Hide,
                    );
                    cx.revert_key_focus();
                    self.redraw(cx);
                    return;
                }
                (false, true) => {
                    self.is_animating_out = true;
                    return;
                }
                _ => { }
            }
        }

        if let Event::Actions(actions) = event {
            let edit_text_input = self.text_input(id!(edit_text_input));

            // Hide the editing pane if the cancel button was clicked
            // or if the `Escape` key was pressed within the edit text input.
            if self.button(id!(cancel_button)).clicked(actions)
                || edit_text_input.escape(actions)
            {
                self.animator_play(cx, id!(panel.hide));
                self.redraw(cx);
                return;
            }

            let Some(info) = self.info.as_ref() else { return };

            if self.button(id!(accept_button)).clicked(actions) ||
                edit_text_input
                    .key_down_unhandled(actions)
                    .is_some_and(|ke| ke.key_code == KeyCode::ReturnKey && ke.modifiers.is_primary())
            {
                let edited_text = edit_text_input.text().trim().to_string();
                let edited_content = match info.event_tl_item.content() {
                    TimelineItemContent::Message(message) => {
                        // Only these types of messages can be edited.
                        let mut edited_content = match message.msgtype() {
                            // TODO: try to distinguish between plaintext, markdown, and html messages,
                            //       For now, we just assume that all messages are markdown.
                            //       But this is a problem, since the body of the text/emote message might not be markdown.

                            // TODO: also handle "/html" or "/plain" prefixes, just like when sending new messages.
                            MessageType::Text(_) => EditedContent::RoomMessage(
                                RoomMessageEventContentWithoutRelation::text_markdown(&edited_text)
                            ),
                            MessageType::Emote(_) => EditedContent::RoomMessage(
                                RoomMessageEventContentWithoutRelation::emote_markdown(&edited_text)
                            ),
                            // TODO: support adding/removing attachments.
                            //       For now, we just support modifying the body/formatted body of the message.
                            // TODO: once we update the matrix-sdk dependency, we can use the new
                            //       `EditedContent::MediaCaption` variant to edit media messages captions only.
                            MessageType::Image(image) => {
                                let mut new_image_msg = image.clone();
                                if image.formatted.is_some() {
                                    new_image_msg.formatted = FormattedBody::markdown(&edited_text);
                                }
                                new_image_msg.body = edited_text.clone();
                                EditedContent::RoomMessage(
                                    RoomMessageEventContentWithoutRelation::new(
                                        MessageType::Image(new_image_msg)
                                    )
                                )
                            }
                            MessageType::Audio(audio) => {
                                let mut new_audio_msg = audio.clone();
                                if audio.formatted.is_some() {
                                    new_audio_msg.formatted = FormattedBody::markdown(&edited_text);
                                }
                                new_audio_msg.body = edited_text.clone();
                                EditedContent::RoomMessage(
                                    RoomMessageEventContentWithoutRelation::new(
                                        MessageType::Audio(new_audio_msg)
                                    )
                                )
                            }
                            MessageType::File(file) => {
                                let mut new_file_msg = file.clone();
                                if file.formatted.is_some() {
                                    new_file_msg.formatted = FormattedBody::markdown(&edited_text);
                                }
                                new_file_msg.body = edited_text.clone();
                                EditedContent::RoomMessage(
                                    RoomMessageEventContentWithoutRelation::new(
                                        MessageType::File(new_file_msg)
                                    )
                                )
                            }
                            MessageType::Video(video) => {
                                let mut new_video_msg = video.clone();
                                if video.formatted.is_some() {
                                    new_video_msg.formatted = FormattedBody::markdown(&edited_text);
                                }
                                new_video_msg.body = edited_text.clone();
                                EditedContent::RoomMessage(
                                    RoomMessageEventContentWithoutRelation::new(
                                        MessageType::Video(new_video_msg)
                                    )
                                )
                            }
                            _non_editable => {
                                enqueue_popup_notification("That message type cannot be edited.".into());
                                self.animator_play(cx, id!(panel.hide));
                                self.redraw(cx);
                                return;
                            }
                        };

                        // TODO: extract mentions out of the new edited text and use them here.
                        if let Some(existing_mentions) = message.mentions() {
                            match &mut edited_content {
                                EditedContent::RoomMessage(new_message_content) => {
                                    new_message_content.mentions = Some(existing_mentions.clone());
                                }
                                // TODO: once we update the matrix-sdk dependency, uncomment this.
                                // EditedContent::MediaCaption { mentions, .. }) => {
                                //     mentions = Some(existing_mentions);
                                // }
                                _ => { }
                            }
                        }

                        edited_content
                    }

                    TimelineItemContent::Poll(poll) => {
                        let poll_result = poll.results();
                        let poll_answers = poll_result.answers;
                        // TODO: support editing poll answers. For now, just keep the same answers.
                        let Ok(new_poll_answers) = poll_answers
                            .into_iter()
                            .map(|answer| UnstablePollAnswer::new(answer.id, answer.text))
                            .collect::<Vec<_>>()
                            .try_into()
                        else {
                            enqueue_popup_notification("Failed to obtain existing poll answers while editing poll.".into());
                            return;
                        };
                        let mut new_content_block = UnstablePollStartContentBlock::new(
                            edited_text.clone(),
                            new_poll_answers,
                        );
                        new_content_block.kind = poll_result.kind;
                        new_content_block.max_selections = poll_result.max_selections
                            .try_into()
                            .inspect_err(|e| error!("BUG: failed to obtain existing poll max selections while editing: {}", e))
                            .unwrap_or_default();
                        EditedContent::PollStart {
                            fallback_text: edited_text,
                            new_content: new_content_block,
                        }
                    }
                    _ => {
                        enqueue_popup_notification("That event type cannot be edited.".into());
                        return;
                    }
                };

                submit_async_request(MatrixRequest::EditMessage {
                    room_id: info.room_id.clone(),
                    timeline_event_item_id: info.event_tl_item.identifier(),
                    edited_content,
                });

                // TODO: show a loading spinner within the accept button.
            }
        }
    }


    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        if self.info.is_none() {
            self.visible = false;
        };
        self.view.draw_walk(cx, scope, walk)
    }
}


impl EditingPane {
    /// Returns `true` if this pane is currently being shown.
    pub fn is_currently_shown(&self, _cx: &mut Cx) -> bool {
        self.visible
    }

    /// Call this when the result of an edit operation is received.
    ///
    /// This will handle the result, and either show a success message
    /// and hide this editing pane, or show an error message.
    pub fn handle_edit_result(
        &mut self,
        cx: &mut Cx,
        timeline_event_item_id: TimelineEventItemId,
        edit_result: Result<(), matrix_sdk_ui::timeline::Error>,
    ) {
        let Some(info) = self.info.as_ref() else {
            error!("Editing pane received and edit result but had no info set.");
            return;
        };
        if info.event_tl_item.identifier() != timeline_event_item_id {
            error!("Editing pane received an edit result for a different event.");
            return;
        }
        match edit_result {
            Ok(()) => {
                self.animator_play(cx, id!(panel.hide));
                self.redraw(cx);
            }
            Err(e) => {
                enqueue_popup_notification(format!("Failed to edit message: {}", e));
            }
        }
    }

    /// Shows the editing pane and sets it up to edit the given `event`'s content.
    pub fn show(&mut self, cx: &mut Cx, event_tl_item: EventTimelineItem, room_id: OwnedRoomId) {
        if !event_tl_item.is_editable() {
            enqueue_popup_notification("That message cannot be edited.".into());
            return;
        }
        let text_input = self.text_input(id!(editing_content.edit_text_input));
        match event_tl_item.content() {
            TimelineItemContent::Message(message) => {
                text_input.set_text(cx, message.body());
            }
            TimelineItemContent::Poll(poll) => {
                text_input.set_text(cx, &poll.results().question);
            }
            _ => {
                enqueue_popup_notification("That message cannot be edited.".into());
                return;
            }
        }

        self.info = Some(EditingPaneInfo {
            event_tl_item,
            room_id,
        });

        self.visible = true;
        self.button(id!(accept_button)).reset_hover(cx);
        self.button(id!(cancel_button)).reset_hover(cx);

        // Set the text input's cursor to the end and give it key focus.
        let text_len = text_input.text().len();
        text_input.set_cursor(text_len, text_len);
        text_input.set_key_focus(cx);

        self.animator_play(cx, id!(panel.show));
        self.redraw(cx);
    }
}

impl EditingPaneRef {
    /// See [`EditingPane::is_currently_shown()`].
    pub fn is_currently_shown(&self, cx: &mut Cx) -> bool {
        let Some(inner) = self.borrow() else { return false };
        inner.is_currently_shown(cx)
    }

    /// See [`EditingPane::handle_edit_result()`].
    pub fn handle_edit_result(
        &self,
        cx: &mut Cx,
        timeline_event_item_id: TimelineEventItemId,
        edit_result: Result<(), matrix_sdk_ui::timeline::Error>,
    ) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.handle_edit_result(cx, timeline_event_item_id, edit_result);
    }

    /// See [`EditingPane::show()`].
    pub fn show(&self, cx: &mut Cx, event_tl_item: EventTimelineItem, room_id: OwnedRoomId) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.show(cx, event_tl_item, room_id);
    }

    /// Returns the event that is currently being edited, if any.
    pub fn get_event_being_edited(&self) -> Option<EventTimelineItem> {
        self.borrow()?
            .info
            .as_ref()
            .map(|info| info.event_tl_item.clone())
    }

    /// Hides the editing pane immediately without animating it out.
    pub fn force_hide(&self, cx: &mut Cx) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.visible = false;
        inner.redraw(cx);
    }
}
