//! Wrapper around a `TextInput` that shows an auto-complete popup upon trigger characters.
//!
//! Currently we use it for:
//! 1. Showing members in a room (upon pressing '@')
//! 2. Showing known rooms and spaces (upon pressing '#')
//! 3. Showing slash commands (upon pressing '/')

use std::{collections::BTreeSet, sync::Arc};
use makepad_widgets::{text::selection::Cursor, *};
use makepad_widgets::makepad_platform::event::finger::TouchState;
use matrix_sdk::{
    room::RoomMember,
    ruma::{
        events::{room::message::RoomMessageEventContent, Mentions},
        OwnedRoomId, OwnedUserId,
    },
};
use crate::{
    home::rooms_list::RoomsListRef,
    shared::{mention_popup::{MentionItem, MentionablePopupRef}, slash_commands},
    sliding_sync::{submit_async_request, MatrixRequest},
    utils::{self, MatchQuality},
};

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.MentionableTextInput = #(MentionableTextInput::register_widget(vm)) {
        width: Fill,
        height: Fit
        flow: Down

        text_input := RobrixTextInput {
            is_multiline: true,
        }
    }
}

#[derive(Script, ScriptHook, Widget)]
pub struct MentionableTextInput {
    #[source] source: ScriptObjectRef,
    #[deref] view: View,

    #[rust] room_id: Option<OwnedRoomId>,
    #[rust] room_members: Option<Arc<Vec<RoomMember>>>,
    /// Cached room display name, refreshed on room change (avoids a per-keystroke lookup).
    #[rust] room_name: String,
    #[rust] can_notify_room: bool,

    #[rust] active_trigger: Option<ActiveTrigger>,
    #[rust] request_id: u64,

    /// A superset of possible mentions that might be in the current textinput.
    /// Mentions may have been deleted after adding them, so we have to check for them
    /// before sending the message in the textinput.
    #[rust] possible_mentions: Mentions,
}

impl Widget for MentionableTextInput {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        let uid = self.widget_uid();

        let popup_ref = self.popup_ref(cx);
        // Handle events/actions that are relevant to a currently-open mention popup.
        if popup_ref.is_open_for(uid) {
            // On a window resize, the textinput moved, so re-anchor the popup to the cursor.
            if let Event::Actions(actions) = event {
                if actions.iter().any(|a| matches!(a.as_widget_action().cast(), WindowAction::WindowGeomChange(_))) {
                    let anchor = self.popup_anchor(cx);
                    popup_ref.set_anchor(cx, uid, anchor);
                }
            }

            // When the mention popup is open, key presses like arrows, return, and escape should be forwarded
            // to it so it can handle them (instead of treating them as regular TextInput navigation).
            // Obviously we can't just give key focus to the popup because we still need to let
            // the user type characters into the text input so they can filter the matches in the popup.
            //
            // While we typically don't want to match on "raw" events (see the comments i added to the
            // various `Event` variants in Makepad), here we don't really have a choice because we're
            // handling events for a different widget and delivering events to it.
            let text_input_area = self.text_input_ref().area();
            if cx.has_key_focus(text_input_area) {
                if let Event::KeyDown(ke) = event {
                    match ke.key_code {
                        KeyCode::ArrowDown => {
                            popup_ref.move_focus(cx, 1);
                            return;
                        }
                        KeyCode::ArrowUp => {
                            popup_ref.move_focus(cx, -1);
                            return;
                        }
                        KeyCode::ReturnKey => {
                            if let Some(item) = popup_ref.focused_item() {
                                self.selection_made(cx, item);
                                return;
                            }
                        }
                        KeyCode::Escape => {
                            self.close_popup(cx);
                            return;
                        }
                        _ => {}
                    }
                }
            }

            /// Returns true if the tap/click location was outside of the text input or the mention popup.
            fn is_outside(cx: &mut Cx, pref: &MentionablePopupRef, input_area: &Area, loc: DVec2) -> bool {
                !pref.content_rect(cx).contains(loc) && !input_area.rect(cx).contains(loc)
            }

            // Dismiss the mention popup on a click/touch outside it, or upon a go-back gesture.
            let should_dismiss = event.back_pressed()
                || match event {
                    Event::MouseDown(e) => is_outside(cx, &popup_ref, &text_input_area, e.abs),
                    Event::TouchUpdate(e) => e.touches.iter().any(
                        |t| t.state == TouchState::Start && is_outside(cx, &popup_ref, &text_input_area, t.abs)
                    ),
                    _ => false,
                };
            if should_dismiss {
                self.close_popup(cx);
            }
        }

        self.view.handle_event(cx, event, scope);

        if let Event::Actions(actions) = event {
            for action in actions {
                // Handle updated matches for the current query, but only if it's ours.
                if let Some(results) = action.downcast_ref::<MentionMatches>() {
                    if results.owner == uid && results.request_id == self.request_id {
                        let empty = self.active_trigger.map_or("", |t| t.kind.empty_message());
                        popup_ref.set_results(cx, uid, results.items.clone(), false, empty);
                    }
                }
            }

            if popup_ref.is_open_for(uid) {
                if let Some(item) = popup_ref.clicked_item(actions) {
                    self.selection_made(cx, item);
                    return;
                }
            }

            let text_input = self.text_input_ref();
            if cx.has_key_focus(text_input.area()) && text_input.changed(actions).is_some() {
                self.refresh_popup(cx);
            }
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }

    fn text(&self) -> String {
        self.text_input_ref().text()
    }

    fn set_text(&mut self, cx: &mut Cx, text: &str) {
        self.text_input_ref().set_text(cx, text);
        if text.trim().is_empty() {
            self.possible_mentions = Mentions::new();
        }
        self.redraw(cx);
    }

    fn set_key_focus(&self, cx: &mut Cx) {
        self.text_input_ref().set_key_focus(cx);
    }
}

impl MentionableTextInput {
    fn text_input_ref(&self) -> TextInputRef {
        self.child_by_path(ids!(text_input)).as_text_input()
    }

    fn popup_ref(&self, cx: &mut Cx) -> MentionablePopupRef {
        if cx.has_global::<MentionablePopupRef>() {
            cx.get_global::<MentionablePopupRef>().clone()
        } else {
            MentionablePopupRef::default()
        }
    }

    /// Returns the rectangle of the current line of text, in absolute window coordinates.
    ///
    /// If the text input cursor isn't set, it returns the rectangle of the text input itself.
    fn popup_anchor(&self, cx: &mut Cx) -> Rect {
        self.text_input_ref()
            .cursor_rect_in_absolute(cx)
            .unwrap_or_else(|| self.text_input_ref().area().rect(cx))
    }

    /// Re-detects the active trigger (e.g., when text changed) and starts or closes the matching.
    fn refresh_popup(&mut self, cx: &mut Cx) {
        let text = self.text_input_ref().text();
        let cursor = self.text_input_ref().cursor().index;

        match detect_trigger(&text, cursor) {
            Some((kind, start_byte, query)) => {
                self.active_trigger = Some(ActiveTrigger { kind, start_byte });
                self.start_matching(cx, kind, &query);
            }
            None => self.close_popup(cx),
        }
    }

    fn start_matching(&mut self, cx: &mut Cx, kind: TriggerKind, query: &str) {
        let uid = self.widget_uid();
        let anchor = self.popup_anchor(cx);
        let trigger_start = self.active_trigger.map_or(0, |t| t.start_byte);
        let popup_ref = self.popup_ref(cx);
        popup_ref.show(cx, uid, anchor, trigger_start, kind.header(), kind.loading_message());

        match kind {
            TriggerKind::User => self.match_members(cx, &popup_ref, query),
            TriggerKind::Room => self.match_rooms(cx, &popup_ref, query),
            TriggerKind::Command => {
                let items = slash_commands::matching_commands(query)
                    .map(MentionItem::Command)
                    .collect();
                popup_ref.set_results(cx, uid, Arc::new(items), false, kind.empty_message());
            }
        }
    }

    fn match_members(&mut self, cx: &mut Cx, popup_ref: &MentionablePopupRef, query: &str) {
        let uid = self.widget_uid();
        self.request_id = self.request_id.wrapping_add(1);
        // Show the loading spinner while we wait for the member list / ranking.
        popup_ref.set_results(cx, uid, Arc::new(Vec::new()), true, TriggerKind::User.empty_message());

        let (Some(_), Some(members)) = (self.room_id.as_ref(), self.room_members.clone()) else {
            // Room members not yet available, just keep showing the loading spinner for now
            return;
        };
        let request_id = self.request_id;
        let query = query.to_string();
        let can_notify_room = self.can_notify_room;
        let room_name = self.room_name.clone();

        std::thread::spawn(move || {
            let current_user = crate::sliding_sync::current_user_id();
            let items = rank_members(&query, &members, can_notify_room, current_user, room_name);
            Cx::post_action(MentionMatches::new(request_id, uid, items));
        });
    }

    /// Submits a request for the background matrix worker task to match & rank rooms/space.
    fn match_rooms(&mut self, cx: &mut Cx, popup_ref: &MentionablePopupRef, query: &str) {
        let uid = self.widget_uid();
        self.request_id = self.request_id.wrapping_add(1);
        // Show the loading spinner while the worker task does the rooms/spaces ranking.
        popup_ref.set_results(cx, uid, Arc::new(Vec::new()), true, TriggerKind::Room.empty_message());

        submit_async_request(MatrixRequest::GetMatchingRooms {
            query: query.to_string(),
            request_id: self.request_id,
            owner: uid,
        });
    }

    /// The user selected the given `item`, so insert that item's text/link at the trigger location.
    fn selection_made(&mut self, cx: &mut Cx, item: MentionItem) {
        let Some(trigger) = self.active_trigger else {
            self.close_popup(cx);
            return;
        };

        let text_to_insert = match &item {
            MentionItem::User { user_id, display_name, .. } => {
                self.possible_mentions.user_ids.insert(user_id.clone());
                format!("[{}]({}) ", display_name, user_id.matrix_to_uri())
            }
            MentionItem::NotifyRoom { .. } => {
                self.possible_mentions.room = true;
                "@room ".to_string()
            }
            MentionItem::Room(candidate) => {
                // Prefer the room alias so we don't need the `via` servers list.
                let (label, uri) = match candidate.alias.as_ref() {
                    Some(alias) => (alias.to_string(), alias.matrix_to_uri()),
                    None => (candidate.room_name_id.to_string(), candidate.room_name_id.room_id().matrix_to_uri()),
                };
                format!("[{label}]({uri}) ")
            }
            MentionItem::Command(cmd) => format!("/{} ", cmd.name),
        };

        let text_input = self.text_input_ref();
        let text = text_input.text();
        let start = trigger.start_byte.min(text.len());
        if !text.is_char_boundary(start) {
            return;
        }
        // Replace the whole trigger and query substring, up until the next whitespace.
        let end = text[start..]
            .find(char::is_whitespace)
            .map_or(text.len(), |i| start + i);
        let new_text = utils::safe_replace_by_byte_indices(&text, start, end, &text_to_insert);

        text_input.set_text(cx, &new_text);
        text_input.set_cursor(
            cx,
            Cursor { index: start + text_to_insert.len(), prefer_next_row: false },
            false,
        );

        self.close_popup(cx);
        // give key focus back to the text input so the user can keep typing
        text_input.set_key_focus(cx);
        self.redraw(cx);
    }

    fn close_popup(&mut self, cx: &mut Cx) {
        let uid = self.widget_uid();
        self.active_trigger = None;
        // Invalidate any in-flight background match.
        self.request_id = self.request_id.wrapping_add(1);
        self.popup_ref(cx).hide(cx, uid);
        self.redraw(cx);
    }
}

impl MentionableTextInputRef {
    pub fn text_input_ref(&self) -> TextInputRef {
        self.borrow()
            .map(|inner| inner.text_input_ref())
            .unwrap_or_default()
    }

    /// Updates whether the user can `@room`. Refreshes an open `@` popup so the
    /// "Notify the entire room" entry appears or disappears accordingly.
    pub fn set_can_notify_room(&self, cx: &mut Cx, can_notify: bool) {
        let Some(mut inner) = self.borrow_mut() else { return };
        if inner.can_notify_room != can_notify {
            inner.can_notify_room = can_notify;
            if inner.active_trigger.is_some_and(|t| t.kind == TriggerKind::User) {
                inner.refresh_popup(cx);
            }
        }
    }

    /// Updates the room context the input matches against. The RoomScreen calls this on
    /// room change / member-list fetch, so we don't poll for these rare changes every event.
    pub fn set_room_context(
        &self,
        cx: &mut Cx,
        room_id: OwnedRoomId,
        room_members: Option<Arc<Vec<RoomMember>>>,
    ) {
        let Some(mut inner) = self.borrow_mut() else { return };
        let uid = inner.widget_uid();
        let room_changed = inner.room_id.as_ref() != Some(&room_id);
        if room_changed {
            inner.room_name = cx.has_global::<RoomsListRef>()
                .then(|| cx.get_global::<RoomsListRef>().get_room_name(&room_id))
                .flatten()
                .map(|n| n.to_string())
                .unwrap_or_default();
        }
        inner.room_id = Some(room_id);
        let members_arrived = inner.room_members.is_none() && room_members.is_some();
        inner.room_members = room_members;

        // The input is reused across rooms, so reset @room capability (re-fetched with
        // the new room's power levels) and close a popup left open in the old one.
        if room_changed {
            inner.can_notify_room = false;
            if inner.popup_ref(cx).is_open_for(uid) {
                inner.close_popup(cx);
            }
        }
        // Repopulate a "loading members" popup once the members arrive.
        if members_arrived && inner.active_trigger.is_some_and(|t| t.kind == TriggerKind::User) {
            inner.refresh_popup(cx);
        }
    }

    /// Returns a saved instance of this widget's state.
    pub fn save_state(&self) -> MentionableTextInputState {
        self.borrow().map_or_else(
            MentionableTextInputState::default,
            |inner| MentionableTextInputState {
                text_input_state: inner.text_input_ref().save_state(),
                possible_mentions: inner.possible_mentions.clone(),
            }
        )
    }

    pub fn restore_state(&self, cx: &mut Cx, state: MentionableTextInputState) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.text_input_ref().restore_state(cx, state.text_input_state);
        inner.possible_mentions = state.possible_mentions;
    }

    /// Clears the possible mentions, e.g., after we've sent the edit.
    pub fn clear_mentions(&self) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.possible_mentions = Mentions::new();
        }
    }

    /// Creates a message from the given entered text, handling slash commands and mentions.
    pub fn create_message_with_mentions(&self, entered_text: &str) -> RoomMessageEventContent {
        if let Some(message) = slash_commands::build_message_for_command(entered_text) {
            return message;
        }

        let message = RoomMessageEventContent::text_markdown(entered_text);
        match self.borrow() {
            Some(inner) => message.add_mentions(inner.real_mentions_in_markdown(entered_text)),
            None => message,
        }
    }

    /// Returns the mentions whose links still exist in the given `text`.
    pub fn get_mentions_in(&self, text: &str) -> Mentions {
        self.borrow().map_or_else(Mentions::new, |inner| inner.real_mentions_in_markdown(text))
    }
}

impl MentionableTextInput {
    /// The possible mentions whose link text is still present in `text`.
    fn real_mentions_in_markdown(&self, text: &str) -> Mentions {
        let mut user_ids = BTreeSet::new();
        for user_id in &self.possible_mentions.user_ids {
            // Match on the link's URI, not its label, so editing the displayed name
            // (while keeping the link) still counts as a mention.
            let by_uri = format!("]({})", user_id.matrix_to_uri());
            if text.contains(&by_uri) {
                user_ids.insert(user_id.clone());
            }
        }

        let mut mentions = Mentions::new();
        mentions.user_ids = user_ids;
        mentions.room = self.possible_mentions.room && contains_room_mention(text);
        mentions
    }
}


/// The saved state of a `MentionableTextInput`.
#[derive(Clone, Default)]
pub struct MentionableTextInputState {
    text_input_state: TextInputState,
    possible_mentions: Mentions,
}

/// Matched users or rooms/spaces, ranked on a background thread.
#[derive(Clone, Debug)]
pub struct MentionMatches {
    request_id: u64,
    owner: WidgetUid,
    items: Arc<Vec<MentionItem>>,
}
impl MentionMatches {
    pub fn new(request_id: u64, owner: WidgetUid, items: Vec<MentionItem>) -> Self {
        Self { request_id, owner, items: Arc::new(items) }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum TriggerKind {
    User,
    Room,
    Command,
}

impl TriggerKind {
    fn from_char(c: char) -> Option<Self> {
        match c {
            '@' => Some(TriggerKind::User),
            '#' => Some(TriggerKind::Room),
            '/' => Some(TriggerKind::Command),
            _ => None,
        }
    }

    fn header(self) -> &'static str {
        match self {
            TriggerKind::User => "Mention a user in this room",
            TriggerKind::Room => "Link to a room or space",
            TriggerKind::Command => "Special Commands",
        }
    }

    fn empty_message(self) -> &'static str {
        match self {
            TriggerKind::User => "No matching users",
            TriggerKind::Room => "No matching rooms or spaces",
            TriggerKind::Command => "No matching commands",
        }
    }

    fn loading_message(self) -> &'static str {
        match self {
            TriggerKind::User => "Loading user members…",
            TriggerKind::Room => "Loading rooms…",
            TriggerKind::Command => "Loading commands…",
        }
    }
}

#[derive(Clone, Copy)]
struct ActiveTrigger {
    kind: TriggerKind,
    /// The byte-wise index of the trigger character within the text.
    start_byte: usize,
}

/// Returns true if `text` contains a standalone `@room` word.
fn contains_room_mention(text: &str) -> bool {
    const ROOM_MENTION: &str = "@room";
    text.match_indices(ROOM_MENTION).any(|(i, _)| {
        let has_whitespace_before = text[..i].chars().next_back().is_none_or(|c| c.is_whitespace());
        let has_whitespace_after  = text[i + ROOM_MENTION.len()..].chars().next().is_none_or(|c| c.is_whitespace());
        has_whitespace_before && has_whitespace_after
    })
}

fn member_display_name(member: &RoomMember) -> &str {
    member.display_name().unwrap_or_else(|| member.user_id().as_str())
}

/// Ranks and builds all matching members.
///
/// Note: run this on a bg thread, as it can be computationally expensive.
fn rank_members(
    query: &str,
    members: &[RoomMember],
    can_notify_room: bool,
    current_user: Option<OwnedUserId>,
    room_name: String,
) -> Vec<MentionItem> {
    let query_lower = query.to_lowercase();
    let mut ranked: Vec<((MatchQuality, u8), String, usize)> = members
        .iter()
        .enumerate()
        .filter(|(_, m)| current_user.as_deref() != Some(m.user_id()))
        .filter_map(|(i, m)| {
            let display_lower = member_display_name(m).to_lowercase();
            let localpart_lower = m.user_id().localpart().to_lowercase();
            user_match_priority(&display_lower, &localpart_lower, &query_lower).map(|p| (p, display_lower, i))
        })
        .collect();
    ranked.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));

    let mut items = Vec::with_capacity(ranked.len() + 1);
    if can_notify_room && (query_lower.is_empty() || "room".starts_with(&query_lower)) {
        items.push(MentionItem::NotifyRoom { room_name });
    }
    for (_, _, i) in ranked {
        let member = &members[i];
        items.push(MentionItem::User {
            user_id: member.user_id().to_owned(),
            display_name: member_display_name(member).to_owned(),
            avatar_url: member.avatar_url().map(ToOwned::to_owned),
        });
    }
    items
}

fn user_match_priority(display_lower: &str, localpart_lower: &str, query_lc: &str) -> Option<(MatchQuality, u8)> {
    if query_lc.is_empty() {
        return Some((MatchQuality::Substring, u8::MAX));
    }
    [ (MatchQuality::of(display_lower, query_lc), 0u8), (MatchQuality::of(localpart_lower, query_lc), 1u8) ]
        .into_iter()
        .filter(|(q, _)| q.is_match())
        .min()
}

/// Finds the active trigger "token" that ends at the current cursor, if any.
///
/// Returns a tuple of: (the detected trigger, the trigger's byte location, the query string).
///
/// We only accept '@' and '#' if there's leading whitespace before it (or they're at the beginning),
/// and '/' only if it's at the beginning.
fn detect_trigger(text: &str, cursor_byte: usize) -> Option<(TriggerKind, usize, String)> {
    if cursor_byte == 0 {
        return None;
    }
    // Start of the whitespace-delimited token the cursor sits in.
    let token_start = text[..cursor_byte]
        .char_indices()
        .rev()
        .find(|(_, c)| c.is_whitespace())
        .map_or(0, |(i, c)| i + c.len_utf8());
    if token_start >= cursor_byte {
        return None; // cursor sits right after whitespace: empty token
    }

    // The trigger is the token's first char, so a second trigger char (like "@@" or "#@")
    // should be treated as part of the query text, not another trigger.
    let trigger_char = text[token_start..cursor_byte].chars().next()?;
    let kind = TriggerKind::from_char(trigger_char)?;
    if kind == TriggerKind::Command && token_start != 0 {
        return None;
    }

    let query = text[token_start + trigger_char.len_utf8()..cursor_byte].to_string();
    Some((kind, token_start, query))
}
