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
use matrix_sdk_ui::timeline::{EventTimelineItem, TimelineEventItemId, TimelineItemContent};

use crate::{
    shared::popup_list::{enqueue_popup_notification, PopupItem},
    sliding_sync::{submit_async_request, MatrixRequest},
};

use crate::room::room_member_manager::{RoomMemberSubscriber, RoomMemberSubscription};
use crate::shared::mentionable_text_input::MentionableTextInputWidgetExt;
use std::sync::{Arc, Mutex};

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
                spacing: 0,
                margin: {left: 5, right: 5},

                draw_bg: {
                    border_color: (COLOR_DANGER_RED),
                    color: #fff0f0 // light red
                    border_radius: 5
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
                spacing: 0,
                margin: {left: 5, right: 5},

                draw_bg: {
                    border_color: (COLOR_ACCEPT_GREEN),
                    color: #f0fff0 // light green
                    border_radius: 5
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

        edit_text_input = <MentionableTextInput> {
            width: Fill, height: Fit,
            margin: { bottom: 5 }
            padding: { top: 3 }
            align: {y: 0.5}
            persistent = {
                center = {
                    text_input = {
                        empty_text: "Enter edited message..."
                    }
                }
            }
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

/// Actions specific to EditingPane for internal use
#[derive(Clone, Debug, DefaultNone)]
enum EditingPaneInternalAction {
    /// Room members data has been updated
    RoomMembersUpdated(Arc<Vec<matrix_sdk::room::RoomMember>>),
    None,
}

/// Subscriber for EditingPane to receive room member updates
struct EditingPaneSubscriber {
    widget_uid: WidgetUid,
    current_room_id: Option<OwnedRoomId>,
}

/// Implement `RoomMemberSubscriber` trait, receive member update notifications
impl RoomMemberSubscriber for EditingPaneSubscriber {
    fn on_room_members_updated(
        &mut self,
        cx: &mut Cx,
        room_id: &OwnedRoomId,
        members: Arc<Vec<matrix_sdk::room::RoomMember>>,
    ) {
        if let Some(current_room_id) = &self.current_room_id {
            if current_room_id == room_id {
                // Log with stable identifier
                log!(
                    "EditingPaneSubscriber({:?}) received members update for room {}",
                    self.widget_uid,
                    room_id
                );

                // cx.action(EditingPaneInternalAction::RoomMembersUpdated(members.clone()));
                cx.widget_action(
                    self.widget_uid,
                    &Scope::empty().path,
                    EditingPaneInternalAction::RoomMembersUpdated(members),
                );
            }else{
                log!("Ignoring update for different room {} (current: {})", room_id, current_room_id);
            }
        }


    }
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
    #[rust]
    member_subscription: Option<RoomMemberSubscription>,
}

impl Widget for EditingPane {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);

        if !self.visible {
            return;
        }

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
                    self.info = None;
                    cx.widget_action(self.widget_uid(), &scope.path, EditingPaneAction::Hide);
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
            let edit_text_input = self.mentionable_text_input(id!(editing_content.edit_text_input)).text_input(id!(text_input));

            // Check for room member update actions
            for action in actions {

                if let Some(widget_action) = action.as_widget_action().widget_uid_eq(self.widget_uid())  {
                    log!("Found widget action for my widget_uid: {:?}", self.widget_uid());
                    log!("Widget action type: {}", std::any::type_name_of_val(&widget_action));

                    if let Some(update_action) = widget_action.downcast_ref::<EditingPaneInternalAction>() {
                        if let EditingPaneInternalAction::RoomMembersUpdated(members) = update_action {
                            log!("EditingPane received EditingPaneInternalAction RoomMembersUpdated action with {} members", members.len());
                            self.handle_members_updated(members.clone());
                        }
                        continue;
                    }
                }
            }

            // Hide the editing pane if the cancel button was clicked
            // or if the `Escape` key was pressed within the edit text input.
            if self.button(id!(cancel_button)).clicked(actions)
                || edit_text_input.escaped(actions)
            {
                self.animator_play(cx, id!(panel.hide));
                self.redraw(cx);
                return;
            }

            let Some(info) = self.info.as_ref() else {
                return;
            };


            if self.button(id!(accept_button)).clicked(actions)
                || edit_text_input.returned(actions).is_some_and(
                    |(_text, modifiers)| modifiers.is_primary()
                )
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
                                RoomMessageEventContentWithoutRelation::text_markdown(&edited_text),
                            ),
                            MessageType::Emote(_) => EditedContent::RoomMessage(
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
                                enqueue_popup_notification(PopupItem { 
                                    message: "That message type cannot be edited.".into(), 
                                    auto_dismissal_duration: None
                                });
                                self.animator_play(cx, id!(panel.hide));
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
                    },

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
                            enqueue_popup_notification(PopupItem { message: "Failed to obtain existing poll answers while editing poll.".into(), auto_dismissal_duration: None });
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
                    },
                    _ => {
                        enqueue_popup_notification(PopupItem { message: "That event type cannot be edited.".into(), auto_dismissal_duration: None });
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
                self.animator_play(cx, id!(panel.hide));
            },
            Err(e) => {
                enqueue_popup_notification(PopupItem { message: format!("Failed to edit message: {}", e), auto_dismissal_duration: None});
            },
        }
    }

    /// Shows the editing pane and sets it up to edit the given `event`'s content.
    pub fn show(&mut self, cx: &mut Cx, event_tl_item: EventTimelineItem, room_id: OwnedRoomId) {
        if !event_tl_item.is_editable() {
            enqueue_popup_notification(PopupItem { message: "That message cannot be edited.".into(), auto_dismissal_duration: None });
            return;
        }

        let edit_text_input = self.mentionable_text_input(id!(editing_content.edit_text_input));
        match event_tl_item.content() {
            TimelineItemContent::Message(message) => {
                edit_text_input.set_text(cx, message.body());
            },
            TimelineItemContent::Poll(poll) => {
                edit_text_input.set_text(cx, &poll.results().question);
            },
            _ => {
                enqueue_popup_notification(PopupItem { message: "That message cannot be edited.".into(), auto_dismissal_duration: None });
                return;
            },
        }

        self.info = Some(EditingPaneInfo { event_tl_item, room_id: room_id.clone() });

        // Create room member subscription
        self.create_room_subscription(cx, room_id.clone());

        self.visible = true;
        self.button(id!(accept_button)).reset_hover(cx);
        self.button(id!(cancel_button)).reset_hover(cx);

        // Set the text input's cursor to the end and give it key focus.
        let inner_text_input = edit_text_input.text_input_ref();
        let text_len = edit_text_input.text().len();
        inner_text_input.set_cursor(
            cx,
            Cursor { index: text_len, prefer_next_row: false },
            false,
        );
        self.animator_play(cx, id!(panel.show));
        inner_text_input.set_key_focus(cx);
        self.redraw(cx);
    }

    /// Create room member subscription for the editing pane
    fn create_room_subscription(&mut self, cx: &mut Cx, room_id: OwnedRoomId) {
        // Cancel previous subscription if any
        self.member_subscription = None;

        log!("Creating room member subscription for EditingPane, ID: {:?}", self.widget_uid());

        // Create new subscriber
        let subscriber = Arc::new(Mutex::new(EditingPaneSubscriber {
            widget_uid: self.widget_uid(),
            current_room_id: Some(room_id.clone()),
        }));

        // Create and save subscription
        self.member_subscription = Some(RoomMemberSubscription::new(cx, room_id.clone(), subscriber));

        submit_async_request(MatrixRequest::GetRoomMembers {
            room_id,
            memberships: matrix_sdk::RoomMemberships::JOIN,
            local_only: false,
        });
    }

    /// Handle room members update event
    fn handle_members_updated(&mut self, members: Arc<Vec<matrix_sdk::room::RoomMember>>) {
        if let Some(_info) = &self.info {
            // Pass room member data to MentionableTextInput
            let message_input = self.mentionable_text_input(id!(edit_text_input));
            message_input.set_room_members(members);
        }
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
        let Some(mut inner) = self.borrow_mut() else {
            return;
        };
        inner.handle_edit_result(cx, timeline_event_item_id, edit_result);
    }

    /// See [`EditingPane::show()`].
    pub fn show(&self, cx: &mut Cx, event_tl_item: EventTimelineItem, room_id: OwnedRoomId) {
        let Some(mut inner) = self.borrow_mut() else {
            return;
        };
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
        let Some(mut inner) = self.borrow_mut() else {
            return;
        };
        inner.visible = false;
        inner.redraw(cx);
    }
}
