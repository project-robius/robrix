//! A context menu that appears when the user right-clicks
//! or long-presses on a message/event in a room timeline.

use bitflags::bitflags;
use makepad_widgets::*;
use matrix_sdk::ruma::{OwnedEventId, events::room::message::MessageType};
use matrix_sdk_ui::timeline::{EventSendState, EventTimelineItem, MsgLikeContent, MsgLikeKind, TimelineEventItemId};

use crate::{home::send_status_indicator::is_send_error_retryable, shared::context_menu::{BUTTON_HEIGHT, ContextMenuClosed, expected_menu_size}, sliding_sync::UserPowerLevels};

use super::room_screen::MessageAction;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.NewMessageContextMenu = set_type_default() do #(NewMessageContextMenu::register_widget(vm)) {
        ..mod.widgets.SolidView

        visible: false,
        flow: Overlay,
        width: Fill,
        height: Fill,
        cursor: MouseCursor.Default,
        // Align to top-left such that our coordinate adjustment
        // when showing this menu pane will work correctly.
        align: Align{x: 0, y: 0}

        // Show a slightly darkened translucent background to make the menu stand out.
        show_bg: true
        draw_bg +: {
            color: #0000004D
        }

        main_content := mod.widgets.ContextMenuContent {
            retry_send_button := mod.widgets.ContextMenuButton {
                draw_icon +: { svg: (ICON_ROTATE_CW) }
                text: "Retry Sending"
            }

            divider_after_retry := mod.widgets.ContextMenuDivider { }

            // Shows either the "Add Reaction" button or a reaction text input.
            react_view := View {
                flow: Overlay
                height: #(BUTTON_HEIGHT)
                align: Align{y: 0.5}

                react_button := mod.widgets.ContextMenuButton {
                    draw_icon +: { svg: (ICON_ADD_REACTION) }
                    text: "Add Reaction"
                }

                reaction_input_view := View {
                    width: Fill,
                    height: #(BUTTON_HEIGHT)
                    align: Align{y: 0.5}
                    flow: Right,
                    visible: false, // will be shown once the react_button is clicked

                    reaction_text_input := RobrixTextInput {
                        width: Fill,
                        height: Fit,
                        align: Align{x: 0, y: 0.5}
                        padding: 7
                        // TODO: we want the TextInput flow to show all text
                        // within the single-line box by scrolling horizontally
                        // when the text is too long, upon a user typing/pasting
                        // or navigating with the mouse or arrow keys.
                        // However, makepad doesn't yet support this feature,
                        // so we just make the TextInput non-wrap.
                        flow: Flow.Right{wrap: false}, // do not wrap
                        draw_bg.border_size: 0.0
                        empty_text: "Enter reaction..."
                    }
                    reaction_send_button := RobrixPositiveIconButton {
                        height: #(BUTTON_HEIGHT)
                        align: Align{x: 0.5, y: 0.5}
                        padding: Inset{left: 10, right: 10, top: 8, bottom: 8}
                        spacing: 0,
                        draw_icon.svg: (ICON_SEND)
                        icon_walk: Walk{width: 16, height: 16, margin: Inset{left: -2, right: -1} }
                    }
                }
            }

            reply_button := mod.widgets.ContextMenuButton {
                draw_icon +: { svg: (ICON_REPLY) }
                icon_walk +: { margin: Inset{top: 1} }
                text: "Reply"
            }

            reply_in_thread_button := mod.widgets.ContextMenuButton {
                draw_icon +: { svg: (ICON_REPLY_IN_THREAD) }
                icon_walk +: { margin: Inset{top: 1} }
                text: "Reply In Thread"
            }

            divider_after_react_reply := mod.widgets.ContextMenuDivider { }

            edit_message_button := mod.widgets.ContextMenuButton {
                draw_icon +: { svg: (ICON_EDIT) }
                icon_walk +: { margin: Inset{top: -3} }
                text: "Edit Message"
            }

            // TODO: check if the current user is allowed to pin/unpin messages:
            //       <https://matrix-org.github.io/matrix-rust-sdk/matrix_sdk_base/struct.RoomMember.html#method.can_pin_or_unpin_event>
            pin_button := mod.widgets.ContextMenuButton {
                draw_icon +: { svg: (ICON_PIN) }
                text: "" // set dynamically to "Pin Message" or "Unpin Message"
            }

            copy_text_button := mod.widgets.ContextMenuButton {
                draw_icon +: { svg: (ICON_COPY) }
                text: "Copy Text"
            }

            copy_html_button := mod.widgets.ContextMenuButton {
                draw_icon +: { svg: (ICON_HTML_FILE) }
                text: "Copy Text as HTML"
            }

            copy_link_to_message_button := mod.widgets.ContextMenuButton {
                draw_icon +: { svg: (ICON_LINK) }
                text: "Copy Link to Message"
            }

            view_source_button := mod.widgets.ContextMenuButton {
                draw_icon +: { svg: (ICON_VIEW_SOURCE) }
                text: "View Source"
            }

            jump_to_related_button := mod.widgets.ContextMenuButton {
                draw_icon +: { svg: (ICON_JUMP) }
                text: "Jump to Related Event"
            }

            divider_before_report_delete := mod.widgets.ContextMenuDivider { }

            // report_button = mod.widgets.ContextMenuDangerButton {
            //     draw_icon.svg: (ICON_TRASH) // TODO: ICON_REPORT/WARNING/FLAG
            //     text: "Report"
            // }

            // Note: we don't yet support deleting others' messages via admin/moderator power levels.
            //       For now we only consider whether its the user's own message.
            //       The caller needs to use `can_redact_own()` or `can_redact_other()`:
            //       https://matrix-org.github.io/matrix-rust-sdk/matrix_sdk_base/struct.RoomMember.html#method.can_redact_own

            delete_button := mod.widgets.ContextMenuDangerButton {
                draw_icon.svg: (ICON_TRASH)
                text: "Delete"
            }
        }
    }
}


bitflags! {
    /// Possible actions that the user can perform on a message.
    ///
    /// This is used to determine which buttons to show in the message context menu.
    #[derive(Copy, Clone, Debug)]
    pub struct MessageAbilities: u16 {
        /// Whether the user can react to this message.
        const CanReact = 1 << 0;
        /// Whether the user can reply to this message.
        const CanReplyTo = 1 << 1;
        /// Whether the user can edit this message.
        const CanEdit = 1 << 2;
        /// Whether the user can pin this message.
        /// This should only be set for non-pinned messages.
        const CanPin = 1 << 3;
        /// Whether the user can unpin this message.
        /// This should only be set for currently-pinned messages.
        const CanUnpin = 1 << 4;
        /// Whether the user can delete/redact this message.
        const CanDelete = 1 << 5;
        /// Whether this message contains HTML content that the user can copy.
        const HasHtml = 1 << 6;
        /// Whether the user can reply to this message in a separate thread.
        /// This is false when already viewing a thread timeline.
        const CanReplyInThread = 1 << 7;
        /// Whether this message failed to send and can be retried.
        const CanRetrySend = 1 << 8;
        /// Whether this message hasn't been sent yet, so sending it can be cancelled.
        const CanCancelSend = 1 << 9;
        /// Whether this message has a real event ID, i.e., it isn't a local echo.
        const HasEventId = 1 << 10;
    }
}
impl MessageAbilities {
    pub fn from_user_power_and_event(
        user_power_levels: &UserPowerLevels,
        event_tl_item: &EventTimelineItem,
        message: &MsgLikeContent,
        pinned_events: &[OwnedEventId],
        has_html: bool,
        is_thread_timeline: bool,
    ) -> Self {
        let mut abilities = Self::empty();
        let is_local_echo = event_tl_item.is_local_echo();
        // The SDK doesn't support editing a queued file upload, only its caption
        let is_unsent_upload = is_local_echo && matches!(
            &message.kind,
            MsgLikeKind::Message(msg) if matches!(
                msg.msgtype(),
                MessageType::Image(_) | MessageType::Video(_) | MessageType::File(_) | MessageType::Audio(_),
            )
        );
        abilities.set(Self::CanEdit, event_tl_item.is_editable() && !is_unsent_upload);
        // Currently we only support deleting one's own messages.
        // But for unsent messages, we show "Cancel Sending" instead of "Delete".
        if event_tl_item.is_own() && !is_local_echo {
            abilities.set(Self::CanDelete, user_power_levels.can_redact_own());
        }
        abilities.set(Self::HasEventId, event_tl_item.event_id().is_some());
        match event_tl_item.send_state() {
            Some(EventSendState::SendingFailed { error, .. }) => {
                abilities.set(Self::CanRetrySend, is_send_error_retryable(error));
                abilities.set(Self::CanCancelSend, true);
            }
            Some(EventSendState::NotSentYet { .. }) => abilities.set(Self::CanCancelSend, true),
            _ => {}
        }
        let can_reply_to = event_tl_item.can_be_replied_to();
        abilities.set(Self::CanReplyTo, can_reply_to);
        // No point offering "reply in thread" from within a thread, since the message is already in one.
        abilities.set(Self::CanReplyInThread, can_reply_to && !is_thread_timeline);
        if let Some(event_id) = event_tl_item.event_id() && user_power_levels.can_pin() {
            if pinned_events.iter().any(|ev| ev == event_id) {
                abilities.set(Self::CanUnpin, true);
            } else {
                abilities.set(Self::CanPin, true);
            }
        }
        abilities.set(
            Self::CanReact,
            // don't let the user react to unsent messages, that doesn't make sense
            user_power_levels.can_send_reaction() && event_tl_item.event_id().is_some(),
        );
        abilities.set(Self::HasHtml, has_html);
        abilities
    }

}

/// Details about the message that define its context menu content.
#[derive(Clone, Debug)]
pub struct MessageDetails {
    /// The index of this message in its room's timeline.
    pub item_id: usize,
    /// The stable identifier of this event timeline item.
    pub timeline_event_id: TimelineEventItemId,
    /// The event ID of the message that this message is related to, if any,
    /// such as the replied-to message.
    pub related_event_id: Option<OwnedEventId>,
    /// The event ID of the thread root if this message is part of a thread
    /// (or if this message is itself the thread root).
    pub thread_root_event_id: Option<OwnedEventId>,
    /// The widget ID of the RoomScreen that contains this message.
    pub room_screen_widget_uid: WidgetUid,
    /// Whether this message should be highlighted, i.e.,
    /// if it mentions the room/current user or is a reply to the current user.
    pub should_be_highlighted: bool,
    /// The abilities that the user has on this message.
    pub abilities: MessageAbilities,
}

impl MessageDetails {
    pub fn event_id(&self) -> Option<&OwnedEventId> {
        match &self.timeline_event_id {
            TimelineEventItemId::EventId(id) => Some(id),
            TimelineEventItemId::TransactionId(_) => None,
        }
    }
}

#[derive(Script, ScriptHook, Widget)]
pub struct NewMessageContextMenu {
    #[deref] view: View,
    #[source] source: ScriptObjectRef,
    #[rust] details: Option<MessageDetails>,
}

impl Widget for NewMessageContextMenu {
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        if self.details.is_none() {
            self.visible = false;
        };

        let step = self.view.draw_walk(cx, scope, walk);
        if self.visible {
            let main_content_area = self.view(cx, ids!(main_content)).area();
            cx.block_scrolling_except_within(main_content_area);
        }
        step
    }

    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if !self.visible { return; }
        self.view.handle_event(cx, event, scope);

        let area = self.view.area();

        // Close the menu if:
        // 1. The back navigational gesture/action occurs (e.g., Back on Android),
        // 2. The escape key is pressed if this menu has key focus,
        // 3. The user clicks/touches outside the main_content view area.
        let close_menu = {
            event.back_pressed()
            || match event.hits_with_capture_overload(cx, area, true) {
                Hit::KeyUp(key) => key.key_code == KeyCode::Escape,
                Hit::FingerDown(fde) => {
                    let reaction_text_input = self.view.text_input(cx, ids!(reaction_input_view.reaction_text_input));
                    if reaction_text_input.area().rect(cx).contains(fde.abs) {
                        reaction_text_input.set_key_focus(cx);
                    } else {
                        cx.set_key_focus(area);
                    }
                    false
                }
                Hit::FingerUp(fue) if fue.is_over => {
                    !self.view(cx, ids!(main_content)).area().rect(cx).contains(fue.abs)
                }
                _ => false,
            }
        };
        if close_menu {
            self.close(cx);
            return;
        }

        self.widget_match_event(cx, event, scope);
    }
}

impl WidgetMatchEvent for NewMessageContextMenu {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions, _scope: &mut Scope) {
        let Some(details) = self.details.as_ref() else { return };
        let mut close_menu = false;

        let reaction_text_input = self.view.text_input(cx, ids!(reaction_input_view.reaction_text_input));
        let reaction_send_button = self.view.button(cx, ids!(reaction_input_view.reaction_send_button));
        if reaction_send_button.clicked(actions)
            || reaction_text_input.returned(actions).is_some()
        {
            cx.widget_action(
                details.room_screen_widget_uid, 
                MessageAction::React {
                    details: details.clone(),
                    reaction: reaction_text_input.text(),
                },
            );
            close_menu = true;
        }
        else if reaction_text_input.escaped(actions) {
            close_menu = true;
        }
        else if self.button(cx, ids!(react_button)).clicked(actions) {
            // Show a box to allow the user to input the reaction.
            // In the future, we'll show an emoji chooser.
            self.view.button(cx, ids!(react_button)).set_visible(cx, false);
            self.view.view(cx, ids!(reaction_input_view)).set_visible(cx, true);
            self.text_input(cx, ids!(reaction_input_view.reaction_text_input)).set_key_focus(cx);
            self.redraw(cx);
            close_menu = false;
        }
        else if self.button(cx, ids!(retry_send_button)).clicked(actions) {
            cx.widget_action(
                details.room_screen_widget_uid,
                MessageAction::RetrySend(details.clone()),
            );
            close_menu = true;
        }
        else if self.button(cx, ids!(reply_button)).clicked(actions) {
            cx.widget_action(
                details.room_screen_widget_uid, 
                MessageAction::Reply(details.clone()),
            );
            close_menu = true;
        }
        else if self.button(cx, ids!(reply_in_thread_button)).clicked(actions) {
            cx.widget_action(
                details.room_screen_widget_uid,
                MessageAction::ReplyInThread(details.clone()),
            );
            close_menu = true;
        }
        else if self.button(cx, ids!(edit_message_button)).clicked(actions) {
            cx.widget_action(
                details.room_screen_widget_uid, 
                MessageAction::Edit(details.clone()),
            );
            close_menu = true;
        }
        else if self.button(cx, ids!(pin_button)).clicked(actions) {
            if details.abilities.contains(MessageAbilities::CanPin) {
                cx.widget_action(
                    details.room_screen_widget_uid, 
                    MessageAction::Pin(details.clone()),
                );
            } else if details.abilities.contains(MessageAbilities::CanUnpin) {
                cx.widget_action(
                    details.room_screen_widget_uid, 
                    MessageAction::Unpin(details.clone()),
                );
            }
            close_menu = true;
        }
        else if self.button(cx, ids!(copy_text_button)).clicked(actions) {
            cx.widget_action(
                details.room_screen_widget_uid, 
                MessageAction::CopyText(details.clone()),
            );
            close_menu = true;
        }
        else if self.button(cx, ids!(copy_html_button)).clicked(actions) {
            cx.widget_action(
                details.room_screen_widget_uid, 
                MessageAction::CopyHtml(details.clone()),
            );
            close_menu = true;
        }
        else if self.button(cx, ids!(copy_link_to_message_button)).clicked(actions) {
            cx.widget_action(
                details.room_screen_widget_uid, 
                MessageAction::CopyLink(details.clone()),
            );
            close_menu = true;
        }
        else if self.button(cx, ids!(view_source_button)).clicked(actions) {
            cx.widget_action(
                details.room_screen_widget_uid, 
                MessageAction::ViewSource(details.clone()),
            );
            close_menu = true;
        }
        else if self.button(cx, ids!(jump_to_related_button)).clicked(actions) {
            cx.widget_action(
                details.room_screen_widget_uid, 
                MessageAction::JumpToRelated(details.clone()),
            );
            close_menu = true;
        }
        // else if self.button(cx, ids!(report_button)).clicked(actions) {
        //     cx.widget_action(
        //         details.room_screen_widget_uid,
        //         &scope.path,
        //         // TODO: display a dialog to confirm the report reason.
        //         MessageAction::Report {
        //             event_id: details.event_id.clone(),
        //             item_id: details.item_id,
        //         },
        //     );
        //    close_menu = true;
        // }
        else if self.button(cx, ids!(delete_button)).clicked(actions) {
            cx.widget_action(
                details.room_screen_widget_uid, 
                MessageAction::Redact {
                    details: details.clone(),
                    // TODO: show a Modal to confirm deletion, and get the reason.
                    reason: None,
                },
            );
            close_menu = true;
        }

        if close_menu {
            self.close(cx);
        }
    }
}

impl NewMessageContextMenu {
    /// Returns `true` if this menu is currently being shown.
    pub fn is_currently_shown(&self, _cx: &mut Cx) -> bool {
        self.visible
    }

    /// Shows this context menu with the given message details.
    ///
    /// Returns the expected dimensions of the context menu,
    /// which can be used to proactively reposition it such that it fits on screen.
    pub fn show(&mut self, cx: &mut Cx, details: MessageDetails) -> DVec2 {
        self.details = Some(details);
        self.visible = true;
        cx.set_key_focus(self.view.area());

        // log!("Showing context menu for message: {:?}", self.details);
        self.set_button_visibility(cx)
    }

    /// Sets up all of the buttons based this context menu's inner details.
    ///
    /// Returns the expected dimensions of all visible items.
    fn set_button_visibility(&mut self, cx: &mut Cx) -> DVec2 {
        let Some(details) = self.details.as_ref() else { return DVec2::default() };

        let retry_send_button = self.view.button(cx, ids!(retry_send_button));
        let react_button = self.view.button(cx, ids!(react_button));
        let reply_button = self.view.button(cx, ids!(reply_button));
        let reply_in_thread_button = self.view.button(cx, ids!(reply_in_thread_button));
        let edit_button = self.view.button(cx, ids!(edit_message_button));
        let pin_button = self.view.button(cx, ids!(pin_button));
        let copy_text_button = self.view.button(cx, ids!(copy_text_button));
        let copy_html_button = self.view.button(cx, ids!(copy_html_button));
        let copy_link_button = self.view.button(cx, ids!(copy_link_to_message_button));
        let view_source_button = self.view.button(cx, ids!(view_source_button));
        let jump_to_related_button = self.view.button(cx, ids!(jump_to_related_button));
        // let report_button = self.view.button(cx, ids!(report_button));
        let delete_button = self.view.button(cx, ids!(delete_button));

        // Determine which buttons should be shown.
        // Note that `copy_text_button` is always enabled.
        let show_retry = details.abilities.contains(MessageAbilities::CanRetrySend);
        let show_divider_after_retry = show_retry;
        let show_react = details.abilities.contains(MessageAbilities::CanReact);
        let show_reply_to = details.abilities.contains(MessageAbilities::CanReplyTo);
        let show_reply_in_thread = details.abilities.contains(MessageAbilities::CanReplyInThread);
        let show_divider_after_react_reply = show_react || show_reply_to;
        let show_edit = details.abilities.contains(MessageAbilities::CanEdit);
        let show_pin: bool;
        let show_copy_text = true;
        let show_copy_html = details.abilities.contains(MessageAbilities::HasHtml);
        let show_copy_link = details.abilities.contains(MessageAbilities::HasEventId);
        let show_view_source = details.abilities.contains(MessageAbilities::HasEventId);
        let show_jump_to_related = details.related_event_id.is_some();
        // let show_report = true;
        let show_cancel_send = details.abilities.contains(MessageAbilities::CanCancelSend);
        let show_delete = show_cancel_send || details.abilities.contains(MessageAbilities::CanDelete);
        let show_divider_before_report_delete = show_delete; // || show_report;

        // Actually set the buttons' visibility.
        retry_send_button.set_visible(cx, show_retry);
        self.view.view(cx, ids!(divider_after_retry)).set_visible(cx, show_divider_after_retry);
        self.view.view(cx, ids!(react_view)).set_visible(cx, show_react);
        react_button.set_visible(cx, show_react);
        reply_button.set_visible(cx, show_reply_to);
        reply_in_thread_button.set_visible(cx, show_reply_in_thread);
        self.view.view(cx, ids!(divider_after_react_reply)).set_visible(cx, show_divider_after_react_reply);
        edit_button.set_visible(cx, show_edit);
        if details.abilities.contains(MessageAbilities::CanPin) {
            pin_button.set_text(cx, "Pin Message");
            show_pin = true;
        } else if details.abilities.contains(MessageAbilities::CanUnpin) {
            pin_button.set_text(cx, "Unpin Message");
            show_pin = true;
        } else {
            show_pin = false;
        }
        pin_button.set_visible(cx, show_pin);
        copy_html_button.set_visible(cx, show_copy_html);
        copy_link_button.set_visible(cx, show_copy_link);
        view_source_button.set_visible(cx, show_view_source);
        jump_to_related_button.set_visible(cx, show_jump_to_related);
        self.view.view(cx, ids!(divider_before_report_delete)).set_visible(cx, show_divider_before_report_delete);
        // report_button.set_visible(cx, show_report);
        delete_button.set_text(cx, if show_cancel_send { "Cancel Sending" } else { "Delete" });
        delete_button.set_visible(cx, show_delete);

        // Reset the hover state of each button.
        retry_send_button.reset_hover(cx);
        react_button.reset_hover(cx);
        reply_button.reset_hover(cx);
        reply_in_thread_button.reset_hover(cx);
        edit_button.reset_hover(cx);
        pin_button.reset_hover(cx);
        copy_text_button.reset_hover(cx);
        copy_html_button.reset_hover(cx);
        copy_link_button.reset_hover(cx);
        view_source_button.reset_hover(cx);
        jump_to_related_button.reset_hover(cx);
        // report_button.reset_hover(cx);
        delete_button.reset_hover(cx);

        // Reset reaction input view stuff.
        self.view.view(cx, ids!(reaction_input_view)).set_visible(cx, false); // hide until the react_button is clicked
        self.text_input(cx, ids!(reaction_input_view.reaction_text_input)).set_text(cx, "");

        self.redraw(cx);

        let num_visible_buttons =
            show_retry as usize
            + show_react as usize
            + show_reply_to as usize
            + show_reply_in_thread as usize
            + show_edit as usize
            + show_pin as usize
            + show_copy_text as usize
            + show_copy_html as usize
            + show_copy_link as usize
            + show_view_source as usize
            + show_jump_to_related as usize
            // + show_report as usize
            + show_delete as usize;
        let num_visible_dividers =
            show_divider_after_retry as usize
            + show_divider_after_react_reply as usize
            + show_divider_before_report_delete as usize;

        expected_menu_size(num_visible_buttons, num_visible_dividers)
    }

    fn close(&mut self, cx: &mut Cx) {
        self.visible = false;
        self.details = None;
        cx.revert_key_focus();
        cx.unblock_scrolling();
        cx.action(ContextMenuClosed);
        cx.clear_all_hovers();
        self.redraw(cx);
    }
}

impl NewMessageContextMenuRef {
    /// See [`NewMessageContextMenu::is_currently_shown()`].
    pub fn is_currently_shown(&self, cx: &mut Cx) -> bool {
        let Some(inner) = self.borrow() else { return false };
        inner.is_currently_shown(cx)
    }

    /// See [`NewMessageContextMenu::show()`].
    pub fn show(&self, cx: &mut Cx, details: MessageDetails) -> DVec2 {
        let Some(mut inner) = self.borrow_mut() else { return DVec2::default()};
        inner.show(cx, details)
    }
}
