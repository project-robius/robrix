//! A list of a room's pinned messages, most recently pinned first.
//!
//! The list subscribes to its room's pinned messages itself, so it can be shown anywhere,
//! e.g., docked within a RoomScreen or popped out into its own screen.
//! Clicking a message asks the list's host to jump to it in the timeline that contains it.

use std::{borrow::Cow, cell::RefCell, collections::HashSet, sync::Arc};

use makepad_widgets::*;
use matrix_sdk::ruma::{OwnedEventId, OwnedRoomId};
use matrix_sdk_ui::timeline::TimelineItem;
use matrix_sdk_ui::sync_service::State as SyncServiceState;

use crate::{
    app::ConfirmDeleteAction,
    avatar_cache,
    event_preview::text_preview_of_timeline_item,
    home::rooms_list_header::RoomsListHeaderAction,
    profile::user_profile_cache,
    shared::{avatar::AvatarWidgetRefExt, confirmation_modal::ConfirmationModalContent, hover_highlight::handle_hover_hit_with_test, html_or_plaintext::HtmlOrPlaintextWidgetRefExt, list_rows::status_row},
    sliding_sync::{MatrixRequest, TimelineEndpointsRecreated, TimelineKind, submit_async_request},
    utils::{self, RoomNameId},
};
use super::room_action_bar::RoomActionTooltip;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.PinnedMessageTimestamp = Label {
        width: Fit, height: Fit
        padding: 0
        draw_text +: { color: (TIMESTAMP_TEXT_COLOR), text_style: TIMESTAMP_TEXT_STYLE { font_size: 7.5 } }
    }

    // A pinned message: its sender's avatar and name, when it was sent, and a preview of it.
    // It's only as tall as its content, with the preview cut off after three lines.
    mod.widgets.PinnedMessageRow = #(PinnedMessageRow::register_widget(vm)) {
        ..mod.widgets.RoundedView
        width: Fill, height: Fit
        flow: Right, spacing: #(ROW_SPACING)
        // The top padding matches the side padding, so the unpin button sits evenly in its corner.
        padding: Inset{top: #(ROW_PADDING), right: #(ROW_PADDING), bottom: 8, left: #(ROW_PADDING)}
        // Tapping a row shouldn't take key focus away from a text input.
        grab_key_focus: false
        show_bg: true
        draw_bg +: { color: #0000, border_radius: 5.0 }

        avatar := Avatar { width: #(AVATAR_SIZE), height: #(AVATAR_SIZE), margin: Inset{top: 2} }
        info := View {
            width: Fill, height: Fit
            margin: Inset{top: 2}
            flow: Down
            title_row := View {
                width: Fill, height: Fit
                flow: Right, spacing: #(TITLE_SPACING)
                sender := Label {
                    width: Fill, height: Fit
                    padding: 0
                    max_lines: 1, text_overflow: Ellipsis
                    draw_text +: { color: (COLOR_TEXT), text_style: USERNAME_TEXT_STYLE { font_size: 10. } }
                }
                timestamp := mod.widgets.PinnedMessageTimestamp { padding: Inset{top: 1} }
            }
            preview := mod.widgets.MessagePreview {
                margin: Inset{top: 2.5}
                latest_message +: {
                    html_view +: { html +: { max_lines: 3 } }
                    plaintext_view +: { pt_label +: { max_lines: 3 } }
                }
            }
            // Shown instead of the timestamp above when that would cut off the sender's name.
            bottom_timestamp := mod.widgets.PinnedMessageTimestamp {
                visible: false
                margin: Inset{top: 2}
            }
        }
        unpin_view := View {
            width: Fit, height: Fit
            unpin_button := RobrixNeutralIconButton {
                width: #(UNPIN_BUTTON_SIZE), height: #(UNPIN_BUTTON_SIZE)
                padding: 0, spacing: 0, margin: 0
                align: Align{x: 0.5, y: 0.5}
                draw_icon.svg: (ICON_UNPIN)
            }
        }

        animator: Animator {
            bg_hover: {
                default: @off
                off: AnimatorState{
                    redraw: true
                    from: {all: Snap}
                    apply: { draw_bg: { color: #0000 } }
                }
                on: AnimatorState{
                    redraw: true
                    from: {all: Snap}
                    apply: { draw_bg: { color: (COLOR_LIST_ROW_HOVER) } }
                }
            }
        }
    }

    mod.widgets.PinnedMessagesList = #(PinnedMessagesList::register_widget(vm)) {
        width: Fill, height: Fill
        flow: Down, spacing: 6
        align: Align{x: 0.5}

        pinned_count_label := Label {
            width: Fill, height: Fit
            padding: Inset{left: 4, right: 4}
            max_lines: 1, text_overflow: Ellipsis
            draw_text +: { color: #737373, text_style: REGULAR_TEXT {font_size: 8.5} }
            text: ""
        }

        pinned_list := PortalList {
            width: Fill, height: Fill
            flow: Down
            auto_tail: false
            keep_invisible: false

            pinned_row := mod.widgets.PinnedMessageRow {}
            loading_row := mod.widgets.ListLoadingRow {}
            empty_row := mod.widgets.ListEmptyRow {}
        }

        unpin_all_button := RobrixNegativeIconButton {
            visible: false
            padding: Inset{top: 10, right: 12, bottom: 10, left: 12}
            spacing: 6
            icon_walk: Walk{width: 14, height: 14}
            draw_icon.svg: (ICON_UNPIN)
            text: "Unpin all"
        }
    }
}

/// The layout of a pinned message row, which it uses to tell whether its sender's name fits beside its timestamp.
const ROW_PADDING: f64 = 6.0;
const ROW_SPACING: f64 = 9.0;
const AVATAR_SIZE: f64 = 30.0;
const TITLE_SPACING: f64 = 3.0;
const UNPIN_BUTTON_SIZE: f64 = 35.0;

/// How often (in seconds) to refresh the relative timestamps of pinned messages,
/// which change at most once per minute, e.g., "Just now" to "1 min ago".
const TIMESTAMP_REFRESH_INTERVAL: f64 = 60.0;

/// A room's pinned messages, as posted by the worker whenever they change,
/// see [`MatrixRequest::SubscribeToPinnedMessages`].
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

/// Widget actions emitted by a [`PinnedMessageRow`].
#[derive(Clone, Debug, Default)]
enum PinnedMessageRowAction {
    Clicked,
    #[default]
    None,
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

/// A row in a [`PinnedMessagesList`], which handles clicks and hovers on the whole row
/// before its children, so that a click on a link in its preview still jumps to its message.
#[derive(Script, ScriptHook, Widget, Animator)]
pub struct PinnedMessageRow {
    #[source] source: ScriptObjectRef,
    #[deref] view: View,
    #[apply_default] animator: Animator,
    /// The widths of the sender's name and the timestamp on one line.
    #[rust] sender_width: f64,
    #[rust] timestamp_width: f64,
    /// Whether the timestamp is shown beneath the preview, as it'd cut off the sender's name beside it.
    #[rust] is_timestamp_below: bool,
}

impl Widget for PinnedMessageRow {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if self.animator_handle_event(cx, event).must_redraw() {
            self.redraw(cx);
        }
        let area = self.view.area();
        let claim_before = event.pointer_claimed_area();
        // Presses on the unpin button are left for it to handle.
        let unpin_view = self.view.child(id!(unpin_view));
        let unpin_rect = if unpin_view.visible() && utils::is_interactive_hit_event(event) {
            unpin_view.child(id!(unpin_button)).area().rect(cx)
        } else {
            Rect::default()
        };
        let hit = handle_hover_hit_with_test(self, cx, event, area, claim_before, false, |abs, rect, inset|
            Inset::rect_contains_with_inset(abs, rect, inset) && !unpin_rect.contains(abs)
        );
        match hit {
            Hit::FingerHoverIn(_) => cx.set_cursor(MouseCursor::Hand),
            Hit::FingerUp(fe) if fe.is_over && fe.is_primary_hit() && fe.was_tap() => {
                cx.widget_action(self.widget_uid(), PinnedMessageRowAction::Clicked);
            }
            _ => {}
        }
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        // The timestamp goes beneath the preview when the sender's name wouldn't fit in full beside it.
        let unpin_width = if self.view.child(id!(unpin_view)).visible() { UNPIN_BUTTON_SIZE + ROW_SPACING } else { 0.0 };
        let title_width = cx.peek_walk_turtle(walk).size.x - ROW_PADDING * 2.0 - AVATAR_SIZE - ROW_SPACING - unpin_width;
        let is_timestamp_below = self.sender_width + TITLE_SPACING + self.timestamp_width > title_width;
        if is_timestamp_below != self.is_timestamp_below {
            self.is_timestamp_below = is_timestamp_below;
            self.view.child_by_path(ids!(title_row.timestamp)).set_visible(cx, !is_timestamp_below);
            self.view.child_by_path(ids!(bottom_timestamp)).set_visible(cx, is_timestamp_below);
        }
        self.view.draw_walk(cx, scope, walk)
    }
}

impl PinnedMessageRowRef {
    /// Shows the given name of the message's sender and when it was sent.
    fn set_title(&self, cx: &mut Cx, sender: &str, timestamp: &str) {
        let Some(mut inner) = self.borrow_mut() else { return };
        let label = inner.view.child_by_path(ids!(sender)).as_label();
        label.set_text(cx, sender);
        inner.sender_width = label.borrow().map_or(0.0, |label| utils::unwrapped_text_width(cx, &label.draw_text, sender));
        drop(inner);
        self.set_timestamp(cx, timestamp);
    }

    /// Shows when the message was sent, either beside its sender's name or beneath its preview.
    fn set_timestamp(&self, cx: &mut Cx, timestamp: &str) {
        let Some(mut inner) = self.borrow_mut() else { return };
        let label = inner.view.child_by_path(ids!(title_row.timestamp)).as_label();
        label.set_text(cx, timestamp);
        inner.timestamp_width = label.borrow().map_or(0.0, |label| utils::unwrapped_text_width(cx, &label.draw_text, timestamp));
        inner.view.child_by_path(ids!(bottom_timestamp)).set_text(cx, timestamp);
        inner.redraw(cx);
    }
}

/// The state of a [`PinnedMessagesList`] that is saved and restored along with its room's timeline.
#[derive(Clone, Default)]
pub struct SavedPinnedMessagesList {
    first_id_and_scroll: (usize, f64),
}

#[derive(Script, ScriptHook, Widget)]
pub struct PinnedMessagesList {
    #[deref] view: View,

    #[rust] room_name_id: Option<RoomNameId>,
    /// The room's main timeline, which avatars use to look up senders' room profiles.
    #[rust] main_timeline_kind: Option<TimelineKind>,
    /// The pinned messages posted by the worker, or `None` until they first arrive.
    #[rust] messages: Option<Arc<Vec<Arc<TimelineItem>>>>,
    #[rust] num_pinned: usize,
    #[rust] can_unpin: bool,
    #[rust] error: Option<String>,
    /// The scroll position to restore once messages have arrived.
    #[rust] pending_scroll: Option<(usize, f64)>,
    /// The indices of the rows whose sender name, timestamp, and preview are set.
    #[rust] rows_with_content: HashSet<usize>,
    /// The indices of the rows that are completely populated, including their sender's profile.
    #[rust] populated_rows: HashSet<usize>,
    /// Whether all rows were fully drawn, i.e., no senders' profiles or avatars are still being fetched.
    #[rust(true)] is_fully_drawn: bool,
    #[rust] timestamp_refresh_timer: Timer,
    /// Shows the "Unpin" tooltip of the unpin button in each row.
    #[rust] tooltip: RoomActionTooltip,
}

impl Drop for PinnedMessagesList {
    fn drop(&mut self) {
        self.set_subscribed(false);
    }
}

impl Widget for PinnedMessagesList {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        // A list that hasn't been given a room yet has nothing to do.
        if self.room_name_id.is_none() { return }

        // This must come before the rows handle the event, else they'd claim the hover first.
        let list_ref = self.pinned_list();
        if let Some(list) = list_ref.borrow() {
            let unpin_buttons = list.items().values()
                .filter(|item| item.template == id!(pinned_row))
                .map(|item| (item.widget.child(id!(unpin_view)).child(id!(unpin_button)), "Unpin"));
            self.tooltip.handle_event(cx, event, unpin_buttons, TooltipPosition::Top);
        }

        self.view.handle_event(cx, event, scope);

        if !self.is_fully_drawn && matches!(event, Event::Signal) {
            user_profile_cache::process_user_profile_updates(cx);
            avatar_cache::process_avatar_updates(cx);
            self.redraw(cx);
        }

        if self.timestamp_refresh_timer.is_event(event).is_some() {
            self.refresh_timestamps(cx);
        }

        let Event::Actions(actions) = event else { return };
        for action in actions {
            match action.downcast_ref() {
                Some(PinnedMessagesAction::Updated { room_id, messages, num_pinned, can_unpin }) if self.is_room(room_id) => {
                    if self.messages.as_ref().is_some_and(|m| Arc::ptr_eq(m, messages))
                        && self.can_unpin == *can_unpin
                    {
                        continue;
                    }
                    self.messages = Some(messages.clone());
                    self.num_pinned = *num_pinned;
                    self.can_unpin = *can_unpin;
                    self.error = None;
                    self.rows_with_content.clear();
                    self.populated_rows.clear();
                    self.tooltip.hide(cx);
                    self.update_pinned_count(cx);
                    if self.timestamp_refresh_timer.is_empty() {
                        self.timestamp_refresh_timer = cx.start_timeout(TIMESTAMP_REFRESH_INTERVAL);
                    }
                    if let Some((first_id, scroll)) = self.pending_scroll.take() {
                        self.pinned_list().set_first_id_and_scroll(first_id, scroll);
                    }
                    self.redraw(cx);
                }
                Some(PinnedMessagesAction::Failed { room_id, error }) if self.is_room(room_id) => {
                    error!("Failed to load the pinned messages of room {room_id}: {error}");
                    self.error = Some(error.clone());
                    self.redraw(cx);
                }
                _ => {}
            }
            // The worker drops our subscription when it rebuilds this room's state, e.g., after a sync gap.
            if let Some(TimelineEndpointsRecreated { room_id }) = action.downcast_ref()
                && self.is_room(room_id)
            {
                self.set_subscribed(true);
            }
            // Retry loading pinned messages that failed to load while we were offline.
            if self.error.is_some()
                && let Some(RoomsListHeaderAction::StateUpdate(state)) = action.downcast_ref()
                && !matches!(state, SyncServiceState::Offline)
            {
                self.set_subscribed(true);
            }
        }

        for (index, row) in self.pinned_list().items_with_actions(actions) {
            let Some(event) = self.messages.as_ref()
                .and_then(|m| m.get(index))
                .and_then(|item| item.as_event())
            else { continue };
            let Some(event_id) = event.event_id() else { continue };
            let Some(room_name_id) = self.room_name_id.clone() else { continue };
            let room_id = room_name_id.room_id().clone();

            let unpin_button = row.child(id!(unpin_view)).child(id!(unpin_button)).as_button();
            if let Some(modifiers) = unpin_button.clicked_modifiers(actions) {
                self.tooltip.hide(cx);
                if modifiers.shift {
                    submit_async_request(MatrixRequest::PinEvent { room_id, event_id: event_id.to_owned(), pin: false });
                } else {
                    confirm_unpin_message(cx, room_id, event_id.to_owned(), true);
                }
            }
            else if let PinnedMessageRowAction::Clicked = actions.find_widget_action(row.widget_uid()).cast() {
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
            && let Some(room_name_id) = self.room_name_id.as_ref()
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
        let status = self.status();
        let mut fully_drawn = true;
        while let Some(item) = self.view.draw_walk(cx, scope, walk).step() {
            let list_ref = item.as_portal_list();
            let Some(mut list) = list_ref.borrow_mut() else { continue };
            let count = if status.is_some() { 1 } else { self.messages.as_ref().map_or(0, |m| m.len()) };
            list.set_item_range(cx, 0, count);
            while let Some(index) = list.next_visible_item(cx) {
                if index >= count { continue; }
                let row = if let Some((text, is_loading)) = &status {
                    status_row(cx, &mut list, index, *is_loading, text)
                } else {
                    let Some(event) = self.messages.as_ref()
                        .and_then(|m| m.get(index))
                        .and_then(|item| item.as_event())
                    else { continue };
                    let (row, existed) = list.item_with_existed(cx, index, id!(pinned_row));
                    if !existed {
                        self.rows_with_content.remove(&index);
                        self.populated_rows.remove(&index);
                    }
                    // Like the timeline, only set a row's content when it's new or has changed.
                    if !self.populated_rows.contains(&index)
                        && let Some(main_timeline_kind) = self.main_timeline_kind.as_ref()
                    {
                        let (username, is_profile_drawn) = row.avatar(cx, ids!(avatar)).set_avatar_and_get_username(
                            cx,
                            main_timeline_kind,
                            event.sender(),
                            Some(event.sender_profile()),
                            None,
                            false,
                        );
                        // The preview may include the sender's name, so we set it again once their profile is known.
                        if is_profile_drawn || !self.rows_with_content.contains(&index) {
                            row.child(id!(unpin_view)).set_visible(cx, self.can_unpin);
                            row.as_pinned_message_row().set_title(
                                cx,
                                &username,
                                utils::relative_format(event.timestamp()).as_deref().unwrap_or(""),
                            );
                            let preview = text_preview_of_timeline_item(event.content(), event.sender(), &username)
                                .format_under_username(&username, true);
                            row.child_by_path(ids!(latest_message)).as_html_or_plaintext().show_html(
                                cx,
                                utils::replace_linebreaks_separators(&preview, true),
                            );
                            self.rows_with_content.insert(index);
                        }
                        if is_profile_drawn {
                            self.populated_rows.insert(index);
                        } else {
                            fully_drawn = false;
                        }
                    }
                    row
                };
                row.draw_all(cx, scope);
            }
        }
        self.is_fully_drawn = fully_drawn;
        DrawStep::done()
    }
}

impl PinnedMessagesList {
    fn pinned_list(&self) -> PortalListRef {
        self.view.child(id!(pinned_list)).as_portal_list()
    }

    fn is_room(&self, room_id: &OwnedRoomId) -> bool {
        self.room_name_id.as_ref().is_some_and(|r| r.room_id() == room_id)
    }

    /// Subscribes to (or unsubscribes from) our room's pinned messages.
    ///
    /// Subscribing again is harmless, and makes the worker post the messages again.
    fn set_subscribed(&self, subscribe: bool) {
        let Some(room_name_id) = self.room_name_id.as_ref() else { return };
        submit_async_request(MatrixRequest::SubscribeToPinnedMessages {
            room_id: room_name_id.room_id().clone(),
            subscriber: self.widget_uid(),
            subscribe,
        });
    }

    /// Shows the pinned messages of the given room.
    ///
    /// Call this again with the same room whenever its name changes.
    fn set_room(&mut self, cx: &mut Cx, room_name_id: &RoomNameId) {
        if !self.is_room(room_name_id.room_id()) {
            self.reset(cx);
            self.main_timeline_kind = Some(TimelineKind::MainRoom { room_id: room_name_id.room_id().clone() });
        }
        self.room_name_id = Some(room_name_id.clone());
        // Also re-subscribe to the same room, in case the worker rebuilt its state while we didn't see it.
        self.set_subscribed(true);
    }

    /// Clears this list and unsubscribes from its room's pinned messages,
    /// such that it shows nothing until `set_room()` is called.
    fn reset(&mut self, cx: &mut Cx) {
        self.set_subscribed(false);
        self.room_name_id = None;
        self.main_timeline_kind = None;
        self.messages = None;
        self.num_pinned = 0;
        self.can_unpin = false;
        self.error = None;
        self.pending_scroll = None;
        self.rows_with_content.clear();
        self.populated_rows.clear();
        cx.stop_timer(self.timestamp_refresh_timer);
        self.timestamp_refresh_timer = Timer::empty();
        self.tooltip.hide(cx);
        self.update_pinned_count(cx);
        self.pinned_list().set_first_id_and_scroll(0, 0.0);
        self.redraw(cx);
    }

    /// Updates the relative timestamps (e.g., "5 mins ago") of the rows that are shown,
    /// and then does so again after another `TIMESTAMP_REFRESH_INTERVAL`.
    fn refresh_timestamps(&mut self, cx: &mut Cx) {
        self.timestamp_refresh_timer = Timer::empty();
        let Some(messages) = self.messages.as_ref().filter(|m| !m.is_empty()) else { return };
        let list_ref = self.pinned_list();
        // These are the rows drawn last time, which don't set their timestamps again when redrawn.
        if let Some(list) = list_ref.borrow() {
            for (index, item) in list.items().iter().filter(|(_, item)| item.template == id!(pinned_row)) {
                if let Some(event) = messages.get(*index).and_then(|item| item.as_event()) {
                    item.widget.as_pinned_message_row().set_timestamp(
                        cx,
                        utils::relative_format(event.timestamp()).as_deref().unwrap_or(""),
                    );
                }
            }
        }
        self.timestamp_refresh_timer = cx.start_timeout(TIMESTAMP_REFRESH_INTERVAL);
    }

    /// Returns the status text to show instead of message rows, and whether it's a loading status.
    fn status(&self) -> Option<(Cow<'static, str>, bool)> {
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

    /// Returns this list's state, e.g., to be restored when its room's timeline is shown again.
    pub fn save_state(&self) -> SavedPinnedMessagesList {
        let Some(inner) = self.borrow() else { return SavedPinnedMessagesList::default() };
        let list = inner.pinned_list();
        SavedPinnedMessagesList {
            // Messages may not have arrived to apply the last restored position to.
            first_id_and_scroll: inner.pending_scroll.unwrap_or((list.first_id(), list.scroll_position())),
        }
    }

    /// Shows the given room's pinned messages, restoring the given saved state once they arrive.
    pub fn restore_state(&self, cx: &mut Cx, room_name_id: &RoomNameId, saved: SavedPinnedMessagesList) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.set_room(cx, room_name_id);
        inner.pending_scroll = Some(saved.first_id_and_scroll);
    }
}
