use makepad_widgets::{text::selection::Cursor, *};
use matrix_sdk::{
    room::edit::EditedContent,
    ruma::{
        OwnedRoomId,
        events::{
            poll::unstable_start::{UnstablePollAnswer, UnstablePollStartContentBlock},
            room::message::{FormattedBody, MessageType, RoomMessageEventContentWithoutRelation},
        },
    },
};
use matrix_sdk_ui::timeline::{EventTimelineItem, MsgLikeKind, TimelineEventItemId, TimelineItemContent};

use crate::shared::mentionable_text_input::MentionableTextInputWidgetExt;
use crate::{
    shared::popup_list::{enqueue_popup_notification, PopupItem, PopupKind}, sliding_sync::{submit_async_request, MatrixRequest}
};

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::helpers::*;
    use crate::shared::styles::*;
    use crate::shared::avatar::*;
    use crate::shared::icon_button::*;
    use crate::shared::mentionable_text_input::MentionableTextInput;

    EditingContent = <View> {
        width: Fill,
        height: Fit { max: Rel { base: Full, factor: 0.625 } }
        align: {x: 0.5, y: 1.0}, // centered horizontally, bottom-aligned
        padding: { left: 20, right: 20, top: 10, bottom: 10 }
        margin: {top: 2}
        spacing: 10,
        flow: Down,

        show_bg: false // don't cover up the RoomInputBar

        <View> {
            width: Fill, height: Fit
            flow: Right
            align: {y: 0.5}
            padding: {left: 5, right: 5}

            <Label> {
                width: Fill,
                flow: Right, // do not wrap
                margin: {top: 3}
                draw_text: {
                    text_style: <USERNAME_TEXT_STYLE> {},
                    color: #222,
                }
                text: "Editing:"
            }

            cancel_button = <RobrixIconButton> {
                width: Fit,
                height: Fit,
                padding: 13,
                spacing: 0,
                margin: {left: 5, right: 5},

                draw_bg: {
                    border_color: (COLOR_FG_DANGER_RED),
                    color: (COLOR_BG_DANGER_RED)
                    border_radius: 5
                }
                draw_icon: {
                    svg_file: (ICON_CLOSE),
                    color: (COLOR_FG_DANGER_RED)
                }
                icon_walk: {width: 16, height: 16, margin: 0}
            }

            accept_button = <RobrixIconButton> {
                width: Fit,
                height: Fit,
                padding: 13,
                spacing: 0,
                margin: {left: 5},

                draw_bg: {
                    border_color: (COLOR_FG_ACCEPT_GREEN),
                    color: (COLOR_BG_ACCEPT_GREEN)
                    border_radius: 5
                }
                draw_icon: {
                    svg_file: (ICON_CHECKMARK)
                    color: (COLOR_FG_ACCEPT_GREEN),
                }
                icon_walk: {width: 16, height: 16, margin: 0}
            }
        }

        <LineH> { }

        edit_text_input = <MentionableTextInput> {
            width: Fill
            height: Fit { max: Rel { base: Full, factor: 0.625 } }
            margin: { bottom: 5, top: 5 }
        }
    }


    pub EditingPane = {{EditingPane}} {
        visible: false,
        width: Fill,
        height: Fit { max: Rel { base: Full, factor: 0.625 } }
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
    Hidden,
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
    #[deref]
    view: View,
    #[animator]
    animator: Animator,

    #[rust]
    info: Option<EditingPaneInfo>,
    #[rust]
    is_animating_out: bool,
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
        if self.animator_in_state(cx, ids!(panel.hide)) {
            match (self.is_animating_out, animator_action.is_animating()) {
                (true, false) => {
                    self.visible = false;
                    self.info = None;
                    cx.widget_action(self.widget_uid(), &scope.path, EditingPaneAction::Hidden);
                    cx.revert_key_focus();
                    self.redraw(cx);
                    return;
                },
                (false, true) => {
                    self.is_animating_out = true;
                    return;
                },
                _ => {},
            }
        }

        if let Event::Actions(actions) = event {

            let edit_text_input = self.mentionable_text_input(ids!(editing_content.edit_text_input)).text_input_ref();

            // Hide the editing pane if the cancel button was clicked
            // or if the `Escape` key was pressed within the edit text input.
            if self.button(ids!(cancel_button)).clicked(actions)
                || edit_text_input.escaped(actions)
            {
                self.animator_play(cx, ids!(panel.hide));
                self.redraw(cx);
                return;
            }

            let Some(info) = self.info.as_ref() else { return };

            if self.button(ids!(accept_button)).clicked(actions)
                || edit_text_input.returned(actions).is_some_and(|(_, m)| m.is_primary())
            {
                let edited_text = edit_text_input.text().trim().to_string();
                let edited_content = match info.event_tl_item.content() {
                    TimelineItemContent::MsgLike(msg_like_content) => {
                        match &msg_like_content.kind {
                            MsgLikeKind::Message(message) => {
                                // Only these types of messages can be edited.
                                let mut edited_content = match message.msgtype() {
                                    // TODO: try to distinguish between plaintext, markdown, and html messages,
                                    //       For now, we just assume that all messages are markdown.
                                    //       But this is a problem, since the body of the text/emote message might not be markdown.

                                    // TODO: also handle "/html" or "/plain" prefixes, just like when sending new messages.
                                    MessageType::Text(_text) => EditedContent::RoomMessage(
                                        RoomMessageEventContentWithoutRelation::text_markdown(&edited_text),
                                    ),
                                    MessageType::Emote(_emote) => EditedContent::RoomMessage(
                                        RoomMessageEventContentWithoutRelation::emote_markdown(
                                            &edited_text,
                                        ),
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
                                                MessageType::Image(new_image_msg),
                                            ),
                                        )
                                    },
                                    MessageType::Audio(audio) => {
                                        let mut new_audio_msg = audio.clone();
                                        if audio.formatted.is_some() {
                                            new_audio_msg.formatted = FormattedBody::markdown(&edited_text);
                                        }
                                        new_audio_msg.body = edited_text.clone();
                                        EditedContent::RoomMessage(
                                            RoomMessageEventContentWithoutRelation::new(
                                                MessageType::Audio(new_audio_msg),
                                            ),
                                        )
                                    },
                                    MessageType::File(file) => {
                                        let mut new_file_msg = file.clone();
                                        if file.formatted.is_some() {
                                            new_file_msg.formatted = FormattedBody::markdown(&edited_text);
                                        }
                                        new_file_msg.body = edited_text.clone();
                                        EditedContent::RoomMessage(
                                            RoomMessageEventContentWithoutRelation::new(MessageType::File(
                                                new_file_msg,
                                            )),
                                        )
                                    },
                                    MessageType::Video(video) => {
                                        let mut new_video_msg = video.clone();
                                        if video.formatted.is_some() {
                                            new_video_msg.formatted = FormattedBody::markdown(&edited_text);
                                        }
                                        new_video_msg.body = edited_text.clone();
                                        EditedContent::RoomMessage(
                                            RoomMessageEventContentWithoutRelation::new(
                                                MessageType::Video(new_video_msg),
                                            ),
                                        )
                                    },
                                    _non_editable => {
                                        enqueue_popup_notification(PopupItem { message: "That message type cannot be edited.".into(), kind: PopupKind::Error, auto_dismissal_duration: None });
                                        self.animator_play(cx, ids!(panel.hide));
                                        self.redraw(cx);
                                        return;
                                    },
                                };

                                // TODO: extract mentions out of the new edited text and use them here.
                                if let Some(existing_mentions) = message.mentions() {
                                    if let EditedContent::RoomMessage(new_message_content) =
                                        &mut edited_content
                                    {
                                        new_message_content.mentions = Some(existing_mentions.clone());
                                    }
                                    // TODO: once we update the matrix-sdk dependency, uncomment this.
                                    // EditedContent::MediaCaption { mentions, .. }) => {
                                    //     mentions = Some(existing_mentions);
                                    // }
                                }

                                edited_content
                            }

                            MsgLikeKind::Poll(poll) => {
                                let poll_result = poll.results();
                                let poll_answers = poll_result.answers;
                                // TODO: support editing poll answers. For now, just keep the same answers.
                                let Ok(new_poll_answers) = poll_answers
                                    .into_iter()
                                    .map(|answer| UnstablePollAnswer::new(answer.id, answer.text))
                                    .collect::<Vec<_>>()
                                    .try_into()
                                else {
                                    enqueue_popup_notification(
                                        PopupItem { message: "Failed to obtain existing poll answers while editing poll.".into(),
                                        kind: PopupKind::Error,
                                        auto_dismissal_duration: None
                                    });
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
                                enqueue_popup_notification(PopupItem { message: "That event type cannot be edited.".into(), kind: PopupKind::Error, auto_dismissal_duration: None });
                                return;
                            }
                        }
                    }
                    _ => {
                        enqueue_popup_notification(PopupItem { message: "That event type cannot be edited.".into(), kind: PopupKind::Error, auto_dismissal_duration: None });
                        return;
                    },
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
                self.animator_play(cx, ids!(panel.hide));
            },
            Err(e) => {
                enqueue_popup_notification(PopupItem { message: format!("Failed to edit message: {}", e), kind: PopupKind::Error, auto_dismissal_duration: None });
            },
        }
    }

    /// Shows the editing pane and sets it up to edit the given `event`'s content.
    pub fn show(&mut self, cx: &mut Cx, event_tl_item: EventTimelineItem, room_id: OwnedRoomId) {
        if !event_tl_item.is_editable() {
            enqueue_popup_notification(PopupItem {message: "That message cannot be edited.".into(), kind: PopupKind::Error, auto_dismissal_duration: None });
            return;
        }

        let edit_text_input = self.mentionable_text_input(ids!(editing_content.edit_text_input));

        if let Some(message) = event_tl_item.content().as_message() {
            edit_text_input.set_text(cx, message.body());
        } else if let Some(poll) = event_tl_item.content().as_poll() {
            edit_text_input.set_text(cx, &poll.results().question);
        } else {
            enqueue_popup_notification(PopupItem { message: "That message cannot be edited.".into(), kind: PopupKind::Error, auto_dismissal_duration: None });
            return;
        }


        self.info = Some(EditingPaneInfo { event_tl_item, room_id: room_id.clone() });

        self.visible = true;
        self.button(ids!(accept_button)).reset_hover(cx);
        self.button(ids!(cancel_button)).reset_hover(cx);
        self.animator_play(cx, ids!(panel.show));

        // Set the text input's cursor to the end and give it key focus.
        let inner_text_input = edit_text_input.text_input_ref();
        let text_len = edit_text_input.text().len();
        inner_text_input.set_cursor(
            cx,
            Cursor { index: text_len, prefer_next_row: false },
            false,
        );
        inner_text_input.set_key_focus(cx);
        self.redraw(cx);
    }

    /// Returns the state of this `EditingPane`, if any.
    pub fn save_state(&self) -> Option<EditingPaneState> {
        self.info.as_ref().map(|info| EditingPaneState {
            event_tl_item: info.event_tl_item.clone(),
            text_input_state: self
                .mentionable_text_input(ids!(editing_content.edit_text_input))
                .text_input_ref()
                .save_state(),
        })
    }

    /// Restores the state of this `EditingPane` from the given `editing_pane_state`.
    pub fn restore_state(
        &mut self,
        cx: &mut Cx,
        editing_pane_state: EditingPaneState,
        room_id: OwnedRoomId,
    ) {
        let EditingPaneState { event_tl_item, text_input_state } = editing_pane_state;
        self.mentionable_text_input(ids!(editing_content.edit_text_input))
            .text_input_ref()
            .restore_state(cx, text_input_state);
        self.info = Some(EditingPaneInfo { event_tl_item, room_id: room_id.clone() });
        self.visible = true;
        self.button(ids!(accept_button)).reset_hover(cx);
        self.button(ids!(cancel_button)).reset_hover(cx);
        self.animator_play(cx, ids!(panel.show));
        self.redraw(cx);

        // In this function, we do not give key focus to the text input,
        // because we don't want the IME/soft keyboard to pop up immediately
        // when the user navigates back to a room they were previously editing a message in.
        // That soft-keyboard pop-up effect is jarring and unpleasant.
    }
}

impl EditingPaneRef {
    /// See [`EditingPane::is_currently_shown()`].
    pub fn is_currently_shown(&self, cx: &mut Cx) -> bool {
        let Some(inner) = self.borrow() else {
            return false;
        };
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

    /// Returns whether this `EditingPane` was hidden by the given actions, i.e.,
    /// `true` if `actions` contains an [`EditingPaneAction::Hidden`] for this widget.
    pub fn was_hidden(&self, actions: &Actions) -> bool {
        matches!(
            actions.find_widget_action(self.widget_uid()).cast_ref(),
            EditingPaneAction::Hidden,
        )
    }

    /// See [`EditingPane::show()`].
    pub fn show(&self, cx: &mut Cx, event_tl_item: EventTimelineItem, room_id: OwnedRoomId) {
        let Some(mut inner) = self.borrow_mut() else {
            return;
        };
        inner.show(cx, event_tl_item, room_id);
    }

    /// See [`EditingPane::save_state()`].
    pub fn save_state(&self) -> Option<EditingPaneState> {
        self.borrow()?.save_state()
    }

    /// Restores the state of this `EditingPane` from the given `event_tl_item` and `text_input_state`.
    ///
    /// The arguments should be the result of a previous call to [`Self::save_state()`].
    pub fn restore_state(
        &self,
        cx: &mut Cx,
        editing_pane_state: EditingPaneState,
        room_id: OwnedRoomId,
    ) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.restore_state(cx, editing_pane_state, room_id);
    }

    /// Hides the editing pane immediately and clears its state without animating it out.
    ///
    /// This function *DOES NOT* emit an [`EditingPaneAction::Hidden`] action.
    pub fn force_reset_hide(&self, cx: &mut Cx) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.visible = false;
        inner.animator_cut(cx, ids!(panel.hide));
        inner.is_animating_out = false;
        inner.info = None;
        inner.redraw(cx);
    }
}

/// The state of the EditingPane, used for saving/restoring its state.
pub struct EditingPaneState {
    event_tl_item: EventTimelineItem,
    text_input_state: TextInputState,
}
