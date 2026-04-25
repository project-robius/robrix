use makepad_widgets::{text::selection::Cursor, *};
use matrix_sdk::{
    room::edit::EditedContent,
    ruma::{
        events::{
            poll::unstable_start::{UnstablePollAnswer, UnstablePollStartContentBlock},
            room::message::{FormattedBody, MessageType, RoomMessageEventContentWithoutRelation},
        },
    },
};
use matrix_sdk_ui::timeline::{EventTimelineItem, MsgLikeKind, TimelineEventItemId, TimelineItemContent};

use crate::shared::mentionable_text_input::{MentionableTextInputWidgetExt, MentionableTextInputWidgetRefExt};
use crate::{
    settings::app_preferences::{AppPreferencesGlobal, AppPreferencesAction},
    shared::popup_list::{enqueue_popup_notification, PopupKind},
    sliding_sync::{submit_async_request, MatrixRequest, TimelineKind},
};

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*


    mod.widgets.EditingContent = RoundedView {
        width: Fill,
        height: Fit,
        padding: Inset{ left: 20, right: 20, top: 10, bottom: 10 }
        spacing: 10,
        flow: Down,

        // this must match the RoomInputBar exactly such that it overlaps atop it.
        margin: Inset{left: -4, right: -4, bottom: -4 }
        show_bg: true,
        draw_bg +: {
            color: (COLOR_PRIMARY)
            border_radius: 5.0
            border_color: (COLOR_SECONDARY)
            border_size: 2.0
            // shadow_color: #0006
            // shadow_radius: 0.0
            // shadow_offset: vec2(0.0,0.0)
        }

        View {
            width: Fill, height: Fit
            flow: Right
            align: Align{y: 0.5}
            padding: Inset{left: 5, right: 5}

            Label {
                width: Fill,
                flow: Right, // do not wrap
                margin: Inset{top: 3}
                draw_text +: {
                    text_style: USERNAME_TEXT_STYLE {},
                    color: #222,
                }
                text: "Editing:"
            }

            cancel_button := RobrixNegativeIconButton {
                width: Fit,
                height: Fit,
                padding: 13,
                spacing: 0,
                margin: Inset{left: 5, right: 5},

                draw_icon.svg: (ICON_CLOSE)
                icon_walk: Walk{width: 16, height: 16, margin: 0}
            }

            accept_button := RobrixPositiveIconButton {
                width: Fit,
                height: Fit,
                padding: 13,
                spacing: 0,
                margin: Inset{left: 5},

                draw_icon.svg: (ICON_CHECKMARK)
                icon_walk: Walk{width: 16, height: 16, margin: 0}
            }
        }

        LineH { }

        edit_text_input := MentionableTextInput {
            width: Fill
            height: Fit{max: FitBound.Rel{base: Base.Full, factor: 0.75}}
            margin: Inset{ bottom: 5, top: 5 }
        }
    }


    mod.widgets.EditingPane = #(EditingPane::register_widget(vm)) {
        ..mod.widgets.RoundedView

        visible: false,
        width: Fill,
        height: Fit{max: FitBound.Rel{base: Base.Full, factor: 0.75}}
        align: Align{x: 0.5, y: 1.0}

        editing_content := mod.widgets.EditingContent { }
        
        slide: 1.0,

        animator: Animator{
            panel: {
                default: @hide
                show: AnimatorState{
                    redraw: true,
                    from: {all: Forward {duration: 0.5}}
                    ease: ExpDecay {d1: 0.80, d2: 0.97}
                    apply: { slide: 0.0 }
                }
                hide: AnimatorState{
                    redraw: true,
                    from: {all: Forward {duration: 0.5}}
                    ease: ExpDecay {d1: 0.80, d2: 0.97}
                    apply: { slide: 1.0 }
                }
            }
        }
    }
}

/// Action emitted by the EditingPane widget.
#[derive(Clone, Default, Debug)]
pub enum EditingPaneAction {
    /// The editing pane's hide animation has started.
    HideAnimationStarted,
    /// The editing pane has been fully closed/hidden.
    Hidden,
    #[default]
    None,
}

impl ActionDefaultRef for EditingPaneAction {
    fn default_ref() -> &'static Self {
        static DEFAULT: EditingPaneAction = EditingPaneAction::None;
        &DEFAULT
    }
}

/// The information maintained by the EditingPane widget.
struct EditingPaneInfo {
    event_tl_item: EventTimelineItem,
    timeline_kind: TimelineKind,
}

/// A view that slides in from the bottom of the screen to allow editing a message.
#[derive(Script, Widget, Animator)]
pub struct EditingPane {
    #[source] source: ScriptObjectRef,
    #[deref] view: View,
    #[apply_default] animator: Animator,
    #[live] slide: f32,

    #[rust] info: Option<EditingPaneInfo>,
    #[rust] is_animating_out: bool,
    #[rust] last_content_height: f64,
    /// Used to force this widget's parent to do a re-draw
    /// after the hide animation completes on this pane.
    #[rust] next_frame: NextFrame,
}

impl ScriptHook for EditingPane {
    fn on_after_new(&mut self, vm: &mut ScriptVm) {
        vm.with_cx_mut(|cx| {
            let send_on_enter = cx.global::<AppPreferencesGlobal>().0.send_on_enter;
            self.mentionable_text_input(cx, ids!(editing_content.edit_text_input))
                .text_input_ref()
                .set_submit_on_enter(send_on_enter);
        });
    }
}

impl Widget for EditingPane {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        // Handle the next-frame event scheduled after hide animation completes.
        // This forces a full redraw cycle so the parent relayouts properly.
        if self.next_frame.is_event(event).is_some() {
            cx.redraw_all();
        }

        self.view.handle_event(cx, event, scope);

        if let Event::Actions(actions) = event {
            for action in actions {
                if let Some(AppPreferencesAction::SendOnEnterChanged(v)) = action.downcast_ref() {
                    self.mentionable_text_input(cx, ids!(editing_content.edit_text_input))
                        .text_input_ref()
                        .set_submit_on_enter(*v);
                }
            }
        }

        if !self.visible { return; }

        let animator_action = self.animator_handle_event(cx, event);
        if animator_action.must_redraw() {
            // During hide, redraw the entire UI so the parent RoomInputBar
            // can animate the input_bar height in its draw_walk.
            // During show, only this widget needs to redraw.
            if self.is_animating_out {
                cx.redraw_all();
            } else {
                self.redraw(cx);
            }
        }
        // If we started animating the hide, check if the track has finished.
        // `is_track_animating` returns false once the track has fully completed,
        // even on the same frame that returned the last `Animating` action.
        if self.is_animating_out {
            if !self.animator.is_track_animating(id!(panel)) {
                self.visible = false;
                self.is_animating_out = false;
                self.info = None;
                cx.widget_action(self.widget_uid(), EditingPaneAction::Hidden);
                cx.revert_key_focus();
                self.redraw(cx);
                self.next_frame = cx.new_next_frame();
                return;
            }
        } else if self.animator_in_state(cx, ids!(panel.hide))
            && matches!(animator_action, AnimatorAction::Animating { .. })
        {
            self.is_animating_out = true;
        }

        if let Event::Actions(actions) = event {
            let edit_text_input = self
                .mentionable_text_input(cx, ids!(editing_content.edit_text_input))
                .text_input_ref();

            // Hide the editing pane if the cancel button was clicked
            // or if the `Escape` key was pressed within the edit text input.
            if self.button(cx, ids!(cancel_button)).clicked(actions)
                || edit_text_input.escaped(actions)
            {
                self.animator_play(cx, ids!(panel.hide));
                cx.widget_action(self.widget_uid(), EditingPaneAction::HideAnimationStarted);
                self.redraw(cx);
                return;
            }

            let Some(info) = self.info.as_ref() else { return };

            if self.button(cx, ids!(accept_button)).clicked(actions)
                || edit_text_input.returned(actions).is_some()
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
                                        enqueue_popup_notification(
                                            "That message type cannot be edited.",
                                            PopupKind::Error,
                                            None,
                                        );
                                        self.animator_play(cx, ids!(panel.hide));
                                        cx.widget_action(self.widget_uid(), EditingPaneAction::HideAnimationStarted);
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
                                        "Failed to obtain existing poll answers while editing poll.",
                                        PopupKind::Error,
                                        None,
                                    );
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
                                enqueue_popup_notification(
                                    "That event type cannot be edited.",
                                    PopupKind::Error,
                                    None,
                                );
                                return;
                            }
                        }
                    }
                    _ => {
                        enqueue_popup_notification(
                            "That event type cannot be edited.",
                            PopupKind::Error,
                            None,
                        );
                        return;
                    },
                };

                submit_async_request(MatrixRequest::EditMessage {
                    timeline_kind: info.timeline_kind.clone(),
                    timeline_event_item_id: info.event_tl_item.identifier(),
                    edited_content,
                });

                // TODO: show a loading spinner within the accept button.
            }
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, mut walk: Walk) -> DrawStep {
        if self.info.is_none() {
            self.visible = false;
        };

        // Animate both the layout height and content position simultaneously:
        // 1. walk.height grows from 0 to ch (and shrinks back during hide),
        //    so the RoomInputBar border grows/shrinks smoothly.
        // 2. Balanced margins on editing_content slide it within the pane:
        //    margin.top pushes content below the clip boundary,
        //    margin.bottom compensates so the Fit height stays constant.
        //    The pane's show_bg provides the clipping.
        let ch = self.last_content_height;
        if self.slide > 0.001 {
            let offset = if ch > 0.0 {
                ch * self.slide as f64
            } else {
                10000.0
            };
            if let Some(mut ec) = self.view(cx, ids!(editing_content)).borrow_mut() {
                ec.walk.margin.top = offset;
                ec.walk.margin.bottom = -offset;
            }
            // Animate the layout height alongside the content slide,
            // so the RoomInputBar border grows/shrinks smoothly.
            if ch > 0.0 {
                walk.height = Size::Fixed((ch * (1.0 - self.slide as f64)).max(0.0));
            } else {
                walk.height = Size::Fixed(0.0);
            }
        } else {
            // Fully shown or not animating: reset margins.
            if let Some(mut ec) = self.view(cx, ids!(editing_content)).borrow_mut() {
                ec.walk.margin.top = 0.0;
                ec.walk.margin.bottom = 0.0;
            }
        }

        let step = self.view.draw_walk(cx, scope, walk);

        // Read area rect AFTER drawing to capture this frame's layout.
        let ec_height = self.view(cx, ids!(editing_content)).area().rect(cx).size.y;
        if ec_height > 0.0 {
            self.last_content_height = ec_height;
        }

        step
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
                cx.widget_action(self.widget_uid(), EditingPaneAction::HideAnimationStarted);
            },
            Err(e) => {
                enqueue_popup_notification(
                    format!("Failed to edit message: {}", e),
                    PopupKind::Error,
                    None,
                );
            },
        }
    }

    /// Shows the editing pane and sets it up to edit the given `event`'s content.
    pub fn show(
        &mut self,
        cx: &mut Cx,
        event_tl_item: EventTimelineItem,
        timeline_kind: TimelineKind,
    ) {
        if !event_tl_item.is_editable() {
            enqueue_popup_notification(
                "That message cannot be edited.",
                PopupKind::Error,
                None,
            );
            return;
        }

        let edit_text_input = self.mentionable_text_input(cx, ids!(editing_content.edit_text_input));

        if let Some(message) = event_tl_item.content().as_message() {
            edit_text_input.set_text(cx, message.body());
        } else if let Some(poll) = event_tl_item.content().as_poll() {
            edit_text_input.set_text(cx, &poll.results().question);
        } else {
            enqueue_popup_notification(
                "That message cannot be edited.",
                PopupKind::Error,
                Some(4.0),
            );
            return;
        }


        self.info = Some(EditingPaneInfo {
            event_tl_item,
            timeline_kind,
        });

        self.visible = true;
        self.is_animating_out = false;
        self.button(cx, ids!(accept_button)).reset_hover(cx);
        self.button(cx, ids!(cancel_button)).reset_hover(cx);
        self.animator_play(cx, ids!(panel.show));

        // Set the text input's cursor to the end and give it key focus.
        let inner_text_input = edit_text_input.text_input_ref();
        let text_len = edit_text_input.text().len();
        inner_text_input.set_cursor(
            cx,
            Cursor { index: text_len, prefer_next_row: false },
            false,
        );
        // TODO: this doesn't work, likely because of Makepad's bug in which you cannot
        // give key focus to a widget that hasn't been drawn yet (as it has no Area).
        inner_text_input.set_key_focus(cx);
        self.redraw(cx);
    }

    /// Returns the state of this `EditingPane`, if any.
    pub fn save_state(&self) -> Option<EditingPaneState> {
        self.info.as_ref().map(|info| EditingPaneState {
            event_tl_item: info.event_tl_item.clone(),
            text_input_state: self.child_by_path(ids!(editing_content.edit_text_input))
                .as_mentionable_text_input()
                .text_input_ref()
                .save_state(),
        })
    }

    /// Restores the state of this `EditingPane` from the given `editing_pane_state`.
    pub fn restore_state(
        &mut self,
        cx: &mut Cx,
        editing_pane_state: EditingPaneState,
        timeline_kind: TimelineKind,
    ) {
        let EditingPaneState { event_tl_item, text_input_state } = editing_pane_state;
        self.mentionable_text_input(cx, ids!(editing_content.edit_text_input))
            .text_input_ref()
            .restore_state(cx, text_input_state);
        self.info = Some(EditingPaneInfo {
            event_tl_item,
            timeline_kind,
        });
        self.visible = true;
        self.is_animating_out = false;
        self.button(cx, ids!(accept_button)).reset_hover(cx);
        self.button(cx, ids!(cancel_button)).reset_hover(cx);
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

    /// Returns the current slide value (0.0 = fully shown, 1.0 = fully hidden).
    pub fn slide(&self) -> f32 {
        self.borrow().map_or(1.0, |inner| inner.slide)
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

    /// Returns whether this `EditingPane`'s hide animation started in the given actions.
    pub fn was_hide_animation_started(&self, actions: &Actions) -> bool {
        matches!(
            actions.find_widget_action(self.widget_uid()).cast_ref(),
            EditingPaneAction::HideAnimationStarted,
        )
    }

    /// See [`EditingPane::show()`].
    pub fn show(
        &self,
        cx: &mut Cx,
        event_tl_item: EventTimelineItem,
        timeline_kind: TimelineKind,
    ) {
        let Some(mut inner) = self.borrow_mut() else { return; };
        inner.show(cx, event_tl_item, timeline_kind);
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
        timeline_kind: TimelineKind,
    ) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.restore_state(cx, editing_pane_state, timeline_kind);
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
        // Reset editing_content margins in case we interrupted an animation.
        if let Some(mut ec) = inner.view(cx, ids!(editing_content)).borrow_mut() {
            ec.walk.margin.top = 0.0;
            ec.walk.margin.bottom = 0.0;
        }
        // Redraw all so the parent RoomInputBar restores the input_bar
        // height (its draw_walk reads the slide value, which is now 1.0).
        cx.redraw_all();
    }
}

/// The state of the EditingPane, used for saving/restoring its state.
pub struct EditingPaneState {
    event_tl_item: EventTimelineItem,
    text_input_state: TextInputState,
}
