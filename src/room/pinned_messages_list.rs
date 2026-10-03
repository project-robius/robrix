//! A list of a room's pinned messages, most recently pinned first.
//!
//! The list subscribes to its room's pinned messages itself, so it can be shown anywhere,
//! e.g., docked within a RoomScreen or popped out into its own screen.
//! Clicking a message asks the list's host to jump to it in the timeline that contains it.

use std::{borrow::Cow, cell::RefCell, sync::Arc};

use makepad_widgets::*;
use matrix_sdk::ruma::{OwnedEventId, OwnedRoomId};
use matrix_sdk_ui::timeline::TimelineItem;
use matrix_sdk_ui::sync_service::State as SyncServiceState;

use crate::{
    app::ConfirmDeleteAction,
    home::rooms_list_header::RoomsListHeaderAction,
    room::pane_dock::FRAME_PADDING,
    shared::{confirmation_modal::ConfirmationModalContent, list_rows::status_row},
    sliding_sync::{MatrixRequest, RoomDataKind, TimelineKind, submit_async_request},
    utils::RoomNameId,
};
use super::{
    message_list::{MessageListRowAction, MessageListState, ROW_BUTTON_SIZE, RowMessage, SavedMessageList},
    room_action_bar::RoomActionTooltip,
};

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.PinnedMessagesList = #(PinnedMessagesList::register_widget(vm)) {
        width: Fill, height: Fill
        flow: Down, spacing: 6
        align: Align{x: 0.5}

        pinned_count_label := mod.widgets.MessageListCountLabel {}

        pinned_list := mod.widgets.MessageListPortalList {
            pinned_row := mod.widgets.MessageListRow {
                message +: {
                    // Shown by default, since hiding a row's button before its first draw costs nothing, unlike showing it.
                    button_view +: {
                        visible: true
                        unpin_button := RobrixNeutralIconButton {
                            width: #(ROW_BUTTON_SIZE), height: #(ROW_BUTTON_SIZE)
                            padding: 0, spacing: 0, margin: 0
                            align: Align{x: 0.5, y: 0.5}
                            draw_icon.svg: (ICON_UNPIN)
                        }
                    }
                }
            }
            loading_row := mod.widgets.ListLoadingRow {}
            empty_row := mod.widgets.ListEmptyRow {}
        }

        unpin_all_button := RobrixNegativeIconButton {
            visible: false
            margin +: {right: #(FRAME_PADDING)}
            padding: Inset{top: 10, right: 12, bottom: 10, left: 12}
            spacing: 6
            icon_walk: Walk{width: 14, height: 14}
            draw_icon.svg: (ICON_UNPIN)
            text: "Unpin all"
        }
    }
}

/// A room's pinned messages, as posted by the worker whenever they change,
/// see [`RoomDataKind::PinnedMessages`].
///
/// This is NOT a widget action.
#[derive(Debug)]
pub enum PinnedMessagesAction {
    Updated {
        room_id: OwnedRoomId,
        /// The room's pinned messages that could be loaded, most recently pinned first.
        messages: Arc<Vec<Arc<TimelineItem>>>,
        /// How many messages the room has pinned, including any that aren't loaded.
        num_pinned: usize,
        /// Whether the current user is allowed to unpin messages in this room.
        can_unpin: bool,
    },
    Failed {
        room_id: OwnedRoomId,
        error: String,
    },
}

/// Widget actions emitted by a [`PinnedMessagesList`].
#[derive(Clone, Debug, Default)]
pub enum PinnedMessagesListAction {
    /// The user clicked on the given pinned message, which is in the given timeline of the given room.
    MessageClicked {
        room_name_id: RoomNameId,
        timeline_kind: TimelineKind,
        event_id: OwnedEventId,
        /// What to call this message while searching the timeline for it.
        description: String,
    },
    #[default]
    None,
}

/// The state of a [`PinnedMessagesList`] that is saved and restored.
#[derive(Default)]
pub struct SavedPinnedMessagesList {
    pub(super) list: SavedMessageList,
    pub(super) messages: Option<Arc<Vec<Arc<TimelineItem>>>>,
    num_pinned: usize,
    can_unpin: bool,
}

#[derive(Script, ScriptHook, Widget)]
pub struct PinnedMessagesList {
    #[deref] view: View,

    #[rust(MessageListState::new(RoomDataKind::PinnedMessages))] state: MessageListState,
    /// The pinned messages posted by the worker, or `None` until they first arrive.
    #[rust] messages: Option<Arc<Vec<Arc<TimelineItem>>>>,
    #[rust] num_pinned: usize,
    #[rust] can_unpin: bool,
    #[rust] error: Option<String>,
    /// Shows the "Unpin" tooltip of the unpin button in each row.
    #[rust] tooltip: RoomActionTooltip,
}

impl Widget for PinnedMessagesList {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        // A list that hasn't been given a room yet has nothing to do.
        if self.state.room_name_id().is_none() { return }

        // This must come before the rows handle the event, else they'd claim the hover first.
        let list_ref = self.pinned_list();
        if let Some(list) = list_ref.borrow() {
            let unpin_buttons = list.items().values()
                .filter(|item| item.template == id!(pinned_row))
                .map(|item| (item.widget.child(id!(message)).child(id!(button_view)).child(id!(unpin_button)), "Unpin"));
            self.tooltip.handle_event(cx, event, unpin_buttons, TooltipPosition::Top);
        }

        self.view.handle_event(cx, event, scope);
        if self.state.handle_event(cx, event, &list_ref) {
            self.redraw(cx);
        }

        let Event::Actions(actions) = event else { return };
        for action in actions {
            match action.downcast_ref() {
                Some(PinnedMessagesAction::Updated { room_id, messages, num_pinned, can_unpin }) if self.state.is_showing_room(room_id) => {
                    self.set_messages(cx, messages, *num_pinned, *can_unpin);
                }
                Some(PinnedMessagesAction::Failed { room_id, error }) if self.state.is_showing_room(room_id) => {
                    error!("Failed to load the pinned messages of room {room_id}: {error}");
                    self.error = Some(error.clone());
                    self.redraw(cx);
                }
                _ => {}
            }
            // Retry loading pinned messages that failed to load while we were offline.
            if self.error.is_some()
                && let Some(RoomsListHeaderAction::StateUpdate(state)) = action.downcast_ref()
                && !matches!(state, SyncServiceState::Offline)
            {
                self.state.subscribe();
            }
        }

        for (index, row) in list_ref.items_with_actions(actions) {
            let Some(event) = self.messages.as_ref()
                .and_then(|m| m.get(index))
                .and_then(|item| item.as_event())
            else { continue };
            let Some(event_id) = event.event_id() else { continue };
            let Some(room_name_id) = self.state.room_name_id().cloned() else { continue };
            let room_id = room_name_id.room_id().clone();

            let unpin_button = row.child(id!(message)).child(id!(button_view)).child(id!(unpin_button)).as_button();
            if let Some(modifiers) = unpin_button.clicked_modifiers(actions) {
                self.tooltip.hide(cx);
                if modifiers.shift {
                    submit_async_request(MatrixRequest::PinEvent { room_id, event_id: event_id.to_owned(), pin: false });
                } else {
                    confirm_unpin_message(cx, room_id, event_id.to_owned(), true);
                }
            }
            else if let MessageListRowAction::Clicked = actions.find_widget_action(row.widget_uid()).cast() {
                // The main timeline doesn't show thread replies, so a reply must be shown in its thread.
                let timeline_kind = match event.content().thread_root() {
                    Some(thread_root_event_id) => TimelineKind::Thread { room_id, thread_root_event_id },
                    None => TimelineKind::MainRoom { room_id },
                };
                let sender = row.child_by_path(ids!(sender)).text();
                cx.widget_action(
                    self.widget_uid(),
                    PinnedMessagesListAction::MessageClicked {
                        room_name_id,
                        timeline_kind,
                        event_id: event_id.to_owned(),
                        description: format!("the pinned message from {sender}"),
                    },
                );
            }
        }

        if self.view.child(id!(unpin_all_button)).as_button().clicked(actions)
            && let Some(room_name_id) = self.state.room_name_id()
        {
            let room_id = room_name_id.room_id().clone();
            let content = ConfirmationModalContent {
                title_text: "Unpin All Messages".into(),
                body_text: format!("Are you sure you want to unpin every pinned message in {room_name_id}?").into(),
                accept_button_text: Some("Unpin All".into()),
                on_accept_clicked: Some(Box::new(move |_cx| {
                    submit_async_request(MatrixRequest::UnpinAllEvents { room_id });
                })),
                ..Default::default()
            };
            cx.action(ConfirmDeleteAction::Show(RefCell::new(Some(content))));
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        let status = self.get_status();
        while let Some(item) = self.view.draw_walk(cx, scope, walk).step() {
            let list_ref = item.as_portal_list();
            let Some(mut list) = list_ref.borrow_mut() else { continue };
            let count = if status.is_some() { 1 } else { self.messages.as_ref().map_or(0, |m| m.len()) };
            self.state.prepare_list_for_draw(cx, &mut list, count);
            while let Some(index) = list.next_visible_item(cx) {
                if index >= count { continue; }
                let row = if let Some((text, is_loading)) = &status {
                    status_row(cx, &mut list, index, *is_loading, text)
                } else {
                    let Some(event) = self.messages.as_ref()
                        .and_then(|m| m.get(index))
                        .and_then(|item| item.as_event())
                    else { continue };
                    let message = RowMessage {
                        sender: event.sender(),
                        sender_profile: event.sender_profile(),
                        timestamp: event.timestamp(),
                        content: Some(event.content()),
                    };
                    let (row, is_new_content) = self.state.populate_message_row(cx, &mut list, index, id!(pinned_row), message);
                    if is_new_content {
                        row.child(id!(message)).child(id!(button_view)).set_visible(cx, self.can_unpin);
                    }
                    row
                };
                row.draw_all(cx, scope);
            }
        }
        DrawStep::done()
    }
}

impl PinnedMessagesList {
    fn pinned_list(&self) -> PortalListRef {
        self.view.child(id!(pinned_list)).as_portal_list()
    }

    /// Shows the pinned messages for the given room.
    ///
    /// Call this again with the same room whenever its name changes.
    fn set_room(&mut self, cx: &mut Cx, room_name_id: &RoomNameId) {
        if !self.state.is_showing_room(room_name_id.room_id()) {
            self.reset(cx);
        }
        self.state.set_room(room_name_id);
    }

    fn set_messages(&mut self, cx: &mut Cx, messages: &Arc<Vec<Arc<TimelineItem>>>, num_pinned: usize, can_unpin: bool) {
        if self.messages.as_ref().is_some_and(|m| Arc::ptr_eq(m, messages))
            && self.can_unpin == can_unpin
        {
            return;
        }
        let old_messages = self.messages.replace(messages.clone());
        let is_can_unpin_changed = self.can_unpin != can_unpin;
        self.num_pinned = num_pinned;
        self.can_unpin = can_unpin;
        self.error = None;
        self.tooltip.hide(cx);
        self.update_pinned_count(cx);
        // Every row shows whether the message can be unpinned,
        // so a change to that user power level needs to change all rows.
        self.state.handle_new_messages(
            cx,
            &self.pinned_list(),
            old_messages.as_deref().filter(|_| !is_can_unpin_changed),
            messages,
            true,
        );
        self.redraw(cx);
    }

    /// Clears this list and unsubscribes from its room's pinned messages,
    /// such that it shows nothing until `set_room()` is called.
    fn reset(&mut self, cx: &mut Cx) {
        self.state.reset(cx, &self.pinned_list());
        self.messages = None;
        self.num_pinned = 0;
        self.can_unpin = false;
        self.error = None;
        self.tooltip.hide(cx);
        self.update_pinned_count(cx);
        self.redraw(cx);
    }

    /// Returns the status text to show instead of message rows, and whether it's a loading status.
    fn get_status(&self) -> Option<(Cow<'static, str>, bool)> {
        if self.messages.as_ref().is_some_and(|m| !m.is_empty()) {
            None
        } else if let Some(error) = self.error.as_ref() {
            Some((format!("Failed to load pinned messages: {error}").into(), false))
        } else if self.messages.is_some() && self.num_pinned == 0 {
            Some(("This room has no pinned messages.".into(), false))
        } else {
            Some(("Loading pinned messages...".into(), true))
        }
    }

    /// Shows how many messages are pinned, and whether they can all be unpinned.
    fn update_pinned_count(&mut self, cx: &mut Cx) {
        self.view.child(id!(unpin_all_button)).set_visible(cx, self.can_unpin && self.num_pinned > 0);
        let num_loaded = self.messages.as_ref().map_or(0, |m| m.len());
        let text = match (num_loaded, self.num_pinned) {
            (0, _) => String::new(),
            (1, 1) => String::from("1 pinned message"),
            (n, total) if n == total => format!("{n} pinned messages"),
            (n, total) => format!("{n} of {total} pinned messages"),
        };
        self.view.child(id!(pinned_count_label)).set_text(cx, &text);
    }
}

/// Asks the user to confirm unpinning the given message, and then unpins it.
///
/// If `show_shift_tip` is `true`, this also says that holding Shift skips this prompt.
pub fn confirm_unpin_message(cx: &mut Cx, room_id: OwnedRoomId, event_id: OwnedEventId, show_shift_tip: bool) {
    let body_text = if show_shift_tip {
        "Are you sure you want to unpin this message?\n\n\
        Tip: hold Shift when clicking the unpin button to bypass this prompt."
    } else {
        "Are you sure you want to unpin this message?"
    };
    let content = ConfirmationModalContent {
        title_text: "Unpin Message".into(),
        body_text: body_text.into(),
        accept_button_text: Some("Unpin".into()),
        on_accept_clicked: Some(Box::new(move |_cx| {
            submit_async_request(MatrixRequest::PinEvent { room_id, event_id, pin: false });
        })),
        ..Default::default()
    };
    cx.action(ConfirmDeleteAction::Show(RefCell::new(Some(content))));
}

impl PinnedMessagesListRef {
    /// See [`PinnedMessagesList::set_room()`].
    pub fn set_room(&self, cx: &mut Cx, room_name_id: &RoomNameId) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_room(cx, room_name_id);
        }
    }

    /// See [`PinnedMessagesList::reset()`].
    pub fn reset(&self, cx: &mut Cx) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.reset(cx);
        }
    }

    /// Saves and returns this list's state, e.g., to be restored when its room's timeline is shown again.
    pub fn save_state(&self) -> SavedPinnedMessagesList {
        let Some(mut inner) = self.borrow_mut() else { return SavedPinnedMessagesList::default() };
        let list = inner.pinned_list();
        SavedPinnedMessagesList {
            list: inner.state.save_state(&list),
            messages: inner.messages.clone(),
            num_pinned: inner.num_pinned,
            can_unpin: inner.can_unpin,
        }
    }

    /// Restores the given room's pinned messages from a saved snapshot without loading them again.
    pub fn restore_state(&self, cx: &mut Cx, room_name_id: &RoomNameId, saved: SavedPinnedMessagesList) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.reset(cx);
        inner.state.restore_state(room_name_id, saved.list);
        if let Some(messages) = saved.messages {
            inner.set_messages(cx, &messages, saved.num_pinned, saved.can_unpin);
        }
    }
}
