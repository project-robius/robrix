//! The pieces shared by lists of a room's messages, e.g., its pinned messages or its threads.
//!
//! Each list shows its messages in a [`MessageListRow`], and keeps a [`MessageListState`].

use std::{collections::HashSet, sync::Arc};

use makepad_widgets::*;
use matrix_sdk::ruma::{MilliSecondsSinceUnixEpoch, OwnedRoomId, RoomId, UserId};
use matrix_sdk_ui::timeline::{Profile, TimelineDetails, TimelineItemContent};

use crate::{
    event_preview::{UNSUPPORTED_MESSAGE_PREVIEW, text_preview_of_timeline_item},
    profile::user_profile_cache::UserProfilesUpdated,
    room::pane_dock::FRAME_PADDING,
    shared::{avatar::AvatarWidgetRefExt, hover_highlight::handle_hover_hit_with_test, html_or_plaintext::HtmlOrPlaintextWidgetRefExt},
    sliding_sync::{MatrixRequest, RoomDataKind, TimelineEndpointsRecreated, TimelineKind, submit_async_request},
    utils::{self, RoomNameId},
};

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.MessageListTimestamp = Label {
        width: Fit, height: Fit
        padding: 0
        draw_text +: { color: (TIMESTAMP_TEXT_COLOR), text_style: TIMESTAMP_TEXT_STYLE { font_size: 7.5 } }
    }

    // A message: its sender's avatar and name, when it was sent, and a preview of it cut off after three lines.
    // A list can add more beneath the message, e.g., a thread's latest reply.
    mod.widgets.MessageListRow = #(MessageListRow::register_widget(vm)) {
        ..mod.widgets.RoundedView
        width: Fill, height: Fit
        flow: Down
        // The top padding matches the side padding, so a row button sits evenly in its corner.
        padding: Inset{top: #(ROW_PADDING), right: #(ROW_PADDING), bottom: 8, left: #(ROW_PADDING)}
        // Tapping a row shouldn't take key focus away from a text input.
        grab_key_focus: false
        show_bg: true
        draw_bg +: { color: #0000, border_radius: 5.0 }

        message := View {
            width: Fill, height: Fit
            flow: Right, spacing: #(ROW_SPACING)
            avatar := Avatar {
                width: #(AVATAR_SIZE), height: #(AVATAR_SIZE), margin: Inset{top: 2}
                text_view +: { text +: { draw_text +: { text_style: TITLE_TEXT { font_size: 12.5 } } } }
            }
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
                        draw_text +: { color: (COLOR_TEXT), text_style: USERNAME_TEXT_STYLE {} }
                    }
                    timestamp := mod.widgets.MessageListTimestamp { padding: Inset{top: 1} }
                }
                preview := mod.widgets.MessagePreview {
                    margin: Inset{top: 2.5}
                    latest_message +: {
                        // Sized like the timeline's messages.
                        html_view +: { html +: {
                            max_lines: 3
                            font_size: (MESSAGE_FONT_SIZE)
                            text_style_normal +: { font_size: (MESSAGE_FONT_SIZE), line_spacing: (MESSAGE_TEXT_LINE_SPACING) }
                            text_style_italic +: { font_size: (MESSAGE_FONT_SIZE), line_spacing: (MESSAGE_TEXT_LINE_SPACING) }
                            text_style_bold +: { font_size: (MESSAGE_FONT_SIZE), line_spacing: (MESSAGE_TEXT_LINE_SPACING) }
                            text_style_bold_italic +: { font_size: (MESSAGE_FONT_SIZE), line_spacing: (MESSAGE_TEXT_LINE_SPACING) }
                            text_style_fixed +: { font_size: (MESSAGE_FONT_SIZE), line_spacing: (MESSAGE_TEXT_LINE_SPACING) }
                        } }
                        plaintext_view +: { pt_label +: { max_lines: 3 } }
                    }
                }
                // Shown instead of the timestamp above when that would cut off the sender's name.
                bottom_timestamp := mod.widgets.MessageListTimestamp {
                    visible: false
                    margin: Inset{top: 2}
                }
            }
            // An optional button of size `ROW_BUTTON_SIZE` in the row's top-right corner, e.g., to unpin a message.
            button_view := View {
                visible: false
                width: Fit, height: Fit
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

    // How many messages a list has, shown above them.
    mod.widgets.MessageListCountLabel = Label {
        width: Fill, height: Fit
        margin: Inset{right: #(FRAME_PADDING)}
        padding: Inset{left: 4, right: 4}
        max_lines: 1, text_overflow: Ellipsis
        draw_text +: { color: #737373, text_style: REGULAR_TEXT {font_size: 8.5} }
        text: ""
    }

    mod.widgets.MessageListPortalList = PortalList {
        width: Fill, height: Fill
        flow: Down
        // The list has the right padding instead of the pane, so its scroll bar sits at the pane's edge.
        padding: Inset{right: #(FRAME_PADDING)}
        scroll_bar: ListScrollBar {}
        auto_tail: false
        keep_invisible: false
    }
}

/// The layout of a row, which it uses to tell whether its sender's name fits beside its timestamp.
const ROW_PADDING: f64 = 6.0;
pub const ROW_SPACING: f64 = 9.0;
pub const AVATAR_SIZE: f64 = 30.0;
const TITLE_SPACING: f64 = 3.0;
pub const ROW_BUTTON_SIZE: f64 = 35.0;

/// How often (in seconds) to refresh the relative timestamps of a list's rows,
/// which change at most once per minute, e.g., "Just now" to "1 min ago".
const TIMESTAMP_REFRESH_INTERVAL: f64 = 60.0;

/// Widget actions emitted by a [`MessageListRow`].
#[derive(Clone, Debug, Default)]
pub enum MessageListRowAction {
    Clicked,
    #[default]
    None,
}

/// A row in a list of messages, which handles clicks and hovers on the whole row
/// before its children, so that a click on a link in its preview still counts as a click on the row.
#[derive(Script, ScriptHook, Widget, Animator)]
pub struct MessageListRow {
    #[source] source: ScriptObjectRef,
    #[deref] view: View,
    #[apply_default] animator: Animator,
    /// When the message was sent, which is shown relative to now.
    #[rust] timestamp: Option<MilliSecondsSinceUnixEpoch>,
    /// The widths of the sender's name and the timestamp on one line.
    #[rust] sender_width: f64,
    #[rust] timestamp_width: f64,
    /// Whether the timestamp is shown beneath the preview, as it'd cut off the sender's name beside it.
    #[rust] is_timestamp_below: bool,
}

impl Widget for MessageListRow {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if self.animator_handle_event(cx, event).must_redraw() {
            self.redraw(cx);
        }
        let area = self.view.area();
        let claim_before = event.pointer_claimed_area();
        // Presses on the row's button are left for it to handle.
        let button_rect = utils::is_interactive_hit_event(event)
            .then(|| self.view.child(id!(message)).child(id!(button_view)))
            .filter(|button_view| button_view.visible())
            .map_or_else(Rect::default, |button_view| button_view.area().rect(cx));
        let hit = handle_hover_hit_with_test(self, cx, event, area, claim_before, false, |abs, rect, inset|
            Inset::rect_contains_with_inset(abs, rect, inset) && !button_rect.contains(abs)
        );
        match hit {
            Hit::FingerHoverIn(_) => cx.set_cursor(MouseCursor::Hand),
            Hit::FingerUp(fe) if fe.is_over && fe.is_primary_hit() && fe.was_tap() => {
                cx.widget_action(self.widget_uid(), MessageListRowAction::Clicked);
            }
            _ => {}
        }
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        // We move the timestamp to beneath the message preview if it wouldn't fit
        // on one line after the sender's full name.
        let button_width = if self.view.child(id!(message)).child(id!(button_view)).visible() {
            ROW_BUTTON_SIZE + ROW_SPACING
        } else {
            0.0
        };
        let title_width = cx.peek_walk_turtle(walk).size.x - ROW_PADDING * 2.0 - AVATAR_SIZE - ROW_SPACING - button_width;
        let is_timestamp_below = self.sender_width + TITLE_SPACING + self.timestamp_width > title_width;
        if is_timestamp_below != self.is_timestamp_below {
            self.is_timestamp_below = is_timestamp_below;
            self.view.child_by_path(ids!(title_row.timestamp)).set_visible(cx, !is_timestamp_below);
            self.view.child_by_path(ids!(bottom_timestamp)).set_visible(cx, is_timestamp_below);
        }
        self.view.draw_walk(cx, scope, walk)
    }
}

impl MessageListRow {
    /// Updates the timestamp showing how long ago the message was sent.
    fn refresh_timestamp(&mut self, cx: &mut Cx) {
        let text = self.timestamp.and_then(utils::relative_format);
        let text = text.as_deref().unwrap_or("");
        let label = self.view.child_by_path(ids!(title_row.timestamp)).as_label();
        label.set_text(cx, text);
        self.timestamp_width = label.borrow().map_or(0.0, |label| utils::unwrapped_text_width(cx, &label.draw_text, text));
        self.view.child_by_path(ids!(bottom_timestamp)).set_text(cx, text);
        self.redraw(cx);
    }
}

/// A message to show in a [`MessageListRow`].
pub struct RowMessage<'a> {
    pub sender: &'a UserId,
    pub sender_profile: &'a TimelineDetails<Profile>,
    pub timestamp: MilliSecondsSinceUnixEpoch,
    /// The message's content, or `None` if it couldn't be parsed.
    pub content: Option<&'a TimelineItemContent>,
}

/// The state of a list of messages that is saved/restored.
#[derive(Default)]
pub struct SavedMessageList {
    first_id_and_scroll: (usize, f64),
    pub(super) subscription: Option<RoomDataSubscription>,
}

/// A list's subscription to its room's messages, which auto-unsubscribes when dropped.
pub(super) struct RoomDataSubscription {
    room_id: OwnedRoomId,
    kind: RoomDataKind,
    id: WidgetUid,
}

impl RoomDataSubscription {
    fn new(room_id: OwnedRoomId, kind: RoomDataKind) -> Self {
        // Each subscription needs its own ID; a saved subscription can outlive the widget itself.
        let subscription = Self { room_id, kind, id: WidgetUid::new() };
        subscription.set_subscribed(true);
        subscription
    }

    /// Subscribes to or unsubscribes from the room's messages.
    ///
    /// Subscribing again is harmless, and makes the worker post the messages again.
    fn set_subscribed(&self, subscribe: bool) {
        submit_async_request(MatrixRequest::SubscribeToRoomData {
            room_id: self.room_id.clone(),
            kind: self.kind,
            subscriber: self.id,
            subscribe,
        });
    }
}

impl Drop for RoomDataSubscription {
    fn drop(&mut self) {
        self.set_subscribed(false);
    }
}

/// The state that each list of a room's messages keeps about its room, its subscription, and its rows.
///
/// The list subscribes to its room's messages itself, so it can be docked within a RoomScreen or popped out.
pub struct MessageListState {
    /// The kind of room data that this message list is showing and subscribing to.
    kind: RoomDataKind,
    room_name_id: Option<RoomNameId>,
    subscription: Option<RoomDataSubscription>,
    /// The room's main timeline, which avatars use to look up senders' room profiles.
    main_timeline_kind: Option<TimelineKind>,
    /// The scroll position to restore once enough messages have arrived to reach it.
    pending_scroll: Option<(usize, f64)>,
    /// Whether we've scrolled the list to the `pending_scroll` yet.
    ///
    /// If we can't reach it until we load more messages, the list will wait
    /// at its last row until we get more messages loaded.
    has_scrolled_to_pending: bool,
    /// The indices of the rows whose message is set.
    rows_with_content: HashSet<usize>,
    /// The indices of the rows that are completely populated, including their sender's profile.
    populated_rows: HashSet<usize>,
    /// Whether all rows were fully drawn, i.e., no senders' profiles or avatars are still being fetched.
    is_fully_drawn: bool,
    /// Triggers every [`TIMESTAMP_REFRESH_INTERVAL`] to keep relative timestamps correct.
    timestamp_refresh_timer: Timer,
}

impl MessageListState {
    pub fn new(kind: RoomDataKind) -> Self {
        Self {
            kind,
            room_name_id: None,
            subscription: None,
            main_timeline_kind: None,
            pending_scroll: None,
            has_scrolled_to_pending: false,
            rows_with_content: HashSet::new(),
            populated_rows: HashSet::new(),
            is_fully_drawn: true,
            timestamp_refresh_timer: Timer::empty(),
        }
    }

    pub fn room_name_id(&self) -> Option<&RoomNameId> {
        self.room_name_id.as_ref()
    }

    pub fn is_showing_room(&self, room_id: &RoomId) -> bool {
        self.room_name_id.as_ref().is_some_and(|r| r.room_id() == room_id)
    }

    /// Subscribes to our room's messages, or asks the worker to post them again if already subscribed.
    pub fn subscribe(&mut self) {
        let Some(room_name_id) = self.room_name_id.as_ref() else { return };
        if let Some(subscription) = self.subscription.as_ref()
            && subscription.room_id == *room_name_id.room_id()
        {
            subscription.set_subscribed(true);
        } else {
            self.subscription = Some(RoomDataSubscription::new(room_name_id.room_id().clone(), self.kind));
        }
    }

    /// Shows the messages of the given room, which must be reset first if it's a different room.
    ///
    /// Call this again with the same room whenever its name changes.
    pub fn set_room(&mut self, room_name_id: &RoomNameId) {
        self.main_timeline_kind = Some(TimelineKind::MainRoom { room_id: room_name_id.room_id().clone() });
        self.room_name_id = Some(room_name_id.clone());
        // Also re-subscribe so we get the latest datat feed.
        self.subscribe();
    }

    /// Unsubscribes from the room and resets the widget to its clean default state.
    pub fn reset(&mut self, cx: &mut Cx, list: &PortalListRef) {
        cx.stop_timer(self.timestamp_refresh_timer);
        *self = Self::new(self.kind);
        list.set_first_id_and_scroll(0, 0.0);
    }

    /// Refreshes the rows' relative timestamps once a minute, e.g., from "Just now" to "1 min ago",
    /// and subscribes again when the worker rebuilds the room's state, e.g., after a sync gap.
    ///
    /// Returns whether the list must be redrawn.
    pub fn handle_event(&mut self, cx: &mut Cx, event: &Event, list: &PortalListRef) -> bool {
        if self.timestamp_refresh_timer.is_event(event).is_some() {
            // Only the rows drawn last time need this, as they don't set their timestamps again when redrawn.
            if let Some(list) = list.borrow() {
                for item in list.items().values() {
                    if let Some(mut row) = item.widget.as_message_list_row().borrow_mut() {
                        row.refresh_timestamp(cx);
                    }
                }
            }
            self.timestamp_refresh_timer = cx.start_timeout(TIMESTAMP_REFRESH_INTERVAL);
        }
        let Event::Actions(actions) = event else { return false };
        let mut must_redraw = false;
        for action in actions {
            if let Some(TimelineEndpointsRecreated { room_id }) = action.downcast_ref()
                && self.is_showing_room(room_id)
            {
                self.subscribe();
            } else if !self.is_fully_drawn && action.downcast_ref::<UserProfilesUpdated>().is_some() {
                must_redraw = true;
            }
        }
        must_redraw
    }

    /// Call this whenever the worker posts a new list of messages, so the rows whose message changed
    /// (or all rows, if `old` is `None`) get populated again when they're next drawn.
    ///
    /// This also scrolls the list to any pending restored position once enough messages have loaded
    /// to reach it (or all of them have, if `is_complete`).
    pub fn handle_new_messages<T>(
        &mut self,
        cx: &mut Cx,
        list: &PortalListRef,
        old: Option<&Vec<Arc<T>>>,
        new: &[Arc<T>],
        is_complete: bool,
    ) {
        let is_row_unchanged = |index: &usize| {
            old.and_then(|old| old.get(*index)).zip(new.get(*index)).is_some_and(|(old, new)| Arc::ptr_eq(old, new))
        };
        self.rows_with_content.retain(is_row_unchanged);
        self.populated_rows.retain(is_row_unchanged);
        if new.is_empty() {
            cx.stop_timer(self.timestamp_refresh_timer);
            self.timestamp_refresh_timer = Timer::empty();
        } else if self.timestamp_refresh_timer.is_empty() {
            self.timestamp_refresh_timer = cx.start_timeout(TIMESTAMP_REFRESH_INTERVAL);
        }
        let Some((first_id, scroll)) = self.pending_scroll else { return };
        // While the restored position is beyond the loaded messages, the list sits at its last row,
        // so the user scrolling up from there means they don't want the position anymore.
        if self.has_scrolled_to_pending && !list.is_at_end() {
            self.pending_scroll = None;
            return;
        }
        list.set_first_id_and_scroll(first_id, scroll);
        self.has_scrolled_to_pending = true;
        if first_id < new.len() || is_complete {
            self.pending_scroll = None;
        }
    }

    /// Sets how many rows the given list has, right before we draw them.
    pub fn prepare_list_for_draw(&mut self, cx: &mut Cx, list: &mut PortalList, count: usize) {
        self.is_fully_drawn = true;
        list.set_item_range(cx, 0, count);
        // The list can shrink past its top row, e.g., when a restored position is beyond the messages loaded so far.
        // Its last row is then shown, which asks for more messages if there are any.
        if count > 0 && list.first_id() >= count {
            list.set_first_id_and_scroll(count - 1, 0.0);
        }
    }

    /// Marks the given row as waiting on a sender's profile, so we populate it again once that profile arrives.
    pub fn mark_row_as_waiting_on_profile(&mut self, index: usize) {
        self.populated_rows.remove(&index);
        self.is_fully_drawn = false;
    }

    /// Gets or creates the row at `index` of the given list, and shows the given message in it if the row is new
    /// or its message changed. Also returns whether the message was just set, so the caller can set the rest of the row.
    pub fn populate_message_row(
        &mut self,
        cx: &mut Cx,
        list: &mut PortalList,
        index: usize,
        template: LiveId,
        message: RowMessage,
    ) -> (WidgetRef, bool) {
        let (row, existed) = list.item_with_existed(cx, index, template);
        if !existed {
            self.rows_with_content.remove(&index);
            self.populated_rows.remove(&index);
        }
        // Like the timeline, only set a row's content when it's new or has changed.
        if self.populated_rows.contains(&index) { return (row, false) }
        let Some(main_timeline_kind) = self.main_timeline_kind.as_ref() else { return (row, false) };
        let (username, is_profile_drawn) = row.avatar(cx, ids!(avatar)).set_avatar_and_get_username(
            cx,
            main_timeline_kind,
            message.sender,
            Some(message.sender_profile),
            None,
            false,
        );
        if is_profile_drawn {
            self.populated_rows.insert(index);
        } else {
            self.is_fully_drawn = false;
        }
        // The preview may include the sender's name, so we set it again once their profile is known.
        if !self.rows_with_content.insert(index) && !is_profile_drawn { return (row, false) }

        let row_ref = row.as_message_list_row();
        let Some(mut inner) = row_ref.borrow_mut() else { return (row, false) };
        let label = inner.view.child_by_path(ids!(sender)).as_label();
        label.set_text(cx, &username);
        inner.sender_width = label.borrow().map_or(0.0, |label| utils::unwrapped_text_width(cx, &label.draw_text, &username));
        let preview = message.content.map_or_else(
            || String::from(UNSUPPORTED_MESSAGE_PREVIEW),
            |content| text_preview_of_timeline_item(content, message.sender, &username).format_under_username(&username, true),
        );
        inner.view.child_by_path(ids!(preview.latest_message)).as_html_or_plaintext().show_html(
            cx,
            utils::replace_linebreaks_separators(&preview, true),
        );
        inner.timestamp = Some(message.timestamp);
        inner.refresh_timestamp(cx);
        (row, true)
    }

    pub fn save_state(&mut self, list: &PortalListRef) -> SavedMessageList {
        SavedMessageList {
            first_id_and_scroll: self.pending_scroll.unwrap_or((list.first_id(), list.scroll_position())),
            subscription: self.subscription.take(),
        }
    }

    /// Restores the given room's messages and data subscription from the given saved state.
    pub fn restore_state(&mut self, room_name_id: &RoomNameId, saved: SavedMessageList) {
        self.pending_scroll = Some(saved.first_id_and_scroll);
        self.has_scrolled_to_pending = false;
        self.subscription = saved.subscription;
        self.set_room(room_name_id);
    }
}
