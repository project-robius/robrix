//! Global message search results widget.
//!
//! Mounted inside the room-filter modal (see `src/app.rs` →
//! `room_filter_modal_inner.search_results_scroll.search_results`).
//! Becomes populated when the user clicks the "Search in all rooms"
//! button next to the search input.
//!
//! Renders the [`GlobalSearchHit`]s returned by
//! [`MatrixRequest::SearchAllMessages`] grouped by `room_id`, with one
//! section header per room (room display name + hit count) followed by
//! that room's hits sorted most-recent-first.
//!
//! Click on a hit row emits [`GlobalMessageSearchUiAction::JumpToEvent`]
//! `{ room_id, event_id }`; the app handler closes the search modal,
//! opens the target room, and dispatches the existing scroll-to-event
//! mechanism.

use std::collections::BTreeMap;

use makepad_widgets::*;
use matrix_sdk::ruma::{
    MilliSecondsSinceUnixEpoch, OwnedEventId, OwnedRoomId,
};

use crate::home::rooms_list::RoomsListRef;
use crate::sliding_sync::GlobalSearchHit;
use crate::utils::unix_time_millis_to_datetime;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    // ---------- Section header (one per room) ----------
    mod.widgets.GlobalSearchSectionHeader = #(GlobalSearchSectionHeader::register_widget(vm)) {
        width: Fill,
        height: Fit,
        flow: Right,
        spacing: 8,
        padding: Inset{top: 10, bottom: 4, left: 6, right: 6}
        align: Align{y: 0.5}

        room_name_label := Label {
            width: Fill,
            draw_text +: {
                color: (COLOR_TEXT)
                text_style: BOLD_TEXT {font_size: 11}
            }
            text: ""
        }
        hit_count_label := Label {
            width: Fit,
            draw_text +: {
                color: (COLOR_TEXT_INPUT_IDLE)
                text_style: REGULAR_TEXT {font_size: 10}
            }
            text: ""
        }
    }

    // ---------- One hit row ----------
    mod.widgets.GlobalSearchHitRow = #(GlobalSearchHitRow::register_widget(vm)) {
        width: Fill,
        height: Fit,
        flow: Down,
        padding: Inset{top: 6, bottom: 6, left: 14, right: 8}
        cursor: MouseCursor.Hand

        // sender + timestamp row
        meta_row := View {
            width: Fill,
            height: Fit,
            flow: Right,
            spacing: 6,
            align: Align{y: 0.5}

            sender_label := Label {
                width: Fit,
                draw_text +: {
                    color: (COLOR_TEXT)
                    text_style: BOLD_TEXT {font_size: 10}
                }
                text: ""
            }
            ts_label := Label {
                width: Fill,
                draw_text +: {
                    color: (COLOR_TEXT_INPUT_IDLE)
                    text_style: REGULAR_TEXT {font_size: 9}
                }
                text: ""
            }
        }

        body_label := Label {
            width: Fill,
            flow: Flow.Right{wrap: true}
            draw_text +: {
                color: (COLOR_TEXT)
                text_style: REGULAR_TEXT {font_size: 10}
            }
            text: ""
        }
    }

    // ---------- Top-level widget ----------
    mod.widgets.GlobalMessageSearch = #(GlobalMessageSearch::register_widget(vm)) {
        width: Fill,
        height: Fit,
        flow: Down,
        spacing: 2,

        status_label := Label {
            width: Fill,
            height: Fit,
            margin: Inset{left: 4, top: 6, bottom: 2}
            visible: false
            draw_text +: {
                color: (COLOR_TEXT_INPUT_IDLE)
                text_style: REGULAR_TEXT {font_size: 10}
            }
            text: ""
        }

        // Vertically scrollable region for results. Bounded height so it
        // doesn't push the load-more button off-screen when results are
        // large.
        results_scroll := ScrollYView {
            width: Fill,
            height: 360,
            visible: false

            results_list := PortalList {
                width: Fill,
                height: Fill,
                flow: Down,
                max_pull_down: 0.0,

                section_header := mod.widgets.GlobalSearchSectionHeader {}
                hit_row := mod.widgets.GlobalSearchHitRow {}
            }
        }

        load_more_button := RobrixIconButton {
            width: Fill,
            height: Fit,
            margin: Inset{top: 4, bottom: 4}
            text: "Load more results"
            visible: false
        }
    }
}

// ============================================================================
// Section header widget
// ============================================================================

#[derive(Script, ScriptHook, Widget)]
pub struct GlobalSearchSectionHeader {
    #[deref] view: View,
}

impl Widget for GlobalSearchSectionHeader {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
    }
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl GlobalSearchSectionHeader {
    pub fn set_header(&mut self, cx: &mut Cx, room_display: &str, hit_count: usize) {
        self.label(cx, ids!(room_name_label)).set_text(cx, room_display);
        let count_text = if hit_count == 1 {
            "1 match".to_string()
        } else {
            format!("{hit_count} matches")
        };
        self.label(cx, ids!(hit_count_label)).set_text(cx, &count_text);
    }
}

// ============================================================================
// Hit row widget
// ============================================================================

#[derive(Script, ScriptHook, Widget)]
pub struct GlobalSearchHitRow {
    #[deref] view: View,
    /// Bound on each draw via `set_hit`. `None` before binding.
    #[rust] event_id: Option<OwnedEventId>,
    #[rust] room_id: Option<OwnedRoomId>,
}

impl Widget for GlobalSearchHitRow {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);

        let (Some(event_id), Some(room_id)) =
            (self.event_id.as_ref(), self.room_id.as_ref())
        else { return };
        let area = self.view.area();
        if let Hit::FingerUp(fe) = event.hits(cx, area) {
            if fe.is_over && fe.is_primary_hit() && fe.was_tap() {
                cx.widget_action(
                    self.widget_uid(),
                    GlobalMessageSearchUiAction::JumpToEvent {
                        room_id: room_id.clone(),
                        event_id: event_id.clone(),
                    },
                );
            }
        }
    }
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl GlobalSearchHitRow {
    pub fn set_hit(&mut self, cx: &mut Cx, hit: &GlobalSearchHit) {
        self.event_id = Some(hit.event_id.clone());
        self.room_id = Some(hit.room_id.clone());
        let sender = hit.sender_display_name
            .clone()
            .unwrap_or_else(|| hit.sender_user_id.to_string());
        self.label(cx, ids!(sender_label)).set_text(cx, &sender);
        self.label(cx, ids!(ts_label)).set_text(cx, &format_timestamp(hit.timestamp));
        self.label(cx, ids!(body_label)).set_text(cx, &truncate_body(&hit.body));
    }
}

// ============================================================================
// Actions
// ============================================================================

#[derive(Clone, Debug, Default)]
pub enum GlobalMessageSearchUiAction {
    /// Result row was clicked; navigate to that event in that room.
    JumpToEvent {
        room_id: OwnedRoomId,
        event_id: OwnedEventId,
    },
    /// "Load more results" was clicked; the parent should submit a
    /// follow-up `SearchAllMessages` request with the stored
    /// `next_batch` token.
    LoadMoreClicked,
    #[default]
    None,
}

impl ActionDefaultRef for GlobalMessageSearchUiAction {
    fn default_ref() -> &'static Self {
        static DEFAULT: GlobalMessageSearchUiAction = GlobalMessageSearchUiAction::None;
        &DEFAULT
    }
}

// ============================================================================
// Top-level widget
// ============================================================================

/// Status of the global-search widget. Drives the visible affordances
/// (status label, results list, load-more button) in `refresh_display`.
#[derive(Clone, Debug)]
enum Status {
    /// No query has been searched yet. Everything hidden.
    Idle,
    /// A search is in flight.
    Loading { query: String },
    /// Results received and `hits` is non-empty.
    Results { total: u64, has_more: bool },
    /// Search succeeded but produced zero hits.
    Empty,
    /// Request failed.
    Failed { error: String },
}

/// A single render-list entry. Flattens `(section header, hit, hit, ..., section header, hit, ...)`
/// into one indexable list so PortalList can drive it.
#[derive(Clone, Debug)]
enum RenderItem {
    Header { room_id: OwnedRoomId, count: usize },
    Hit { hit_index: usize },
}

#[derive(Script, ScriptHook, Widget)]
pub struct GlobalMessageSearch {
    #[deref] view: View,

    #[rust] hits: Vec<GlobalSearchHit>,
    /// Flattened render plan (built whenever `hits` changes).
    #[rust] render_items: Vec<RenderItem>,
    #[rust(Status::Idle)] status: Status,
    #[rust] next_batch: Option<String>,
    /// The query string of the last completed (or in-flight) search.
    /// Used by the parent when re-submitting a paginated follow-up.
    #[rust] last_query: String,
}

impl Widget for GlobalMessageSearch {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);

        if let Event::Actions(actions) = event {
            if self.button(cx, ids!(load_more_button)).clicked(actions) {
                cx.widget_action(
                    self.widget_uid(),
                    GlobalMessageSearchUiAction::LoadMoreClicked,
                );
            }
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.refresh_display(cx);

        while let Some(widget) = self.view.draw_walk(cx, scope, walk).step() {
            let portal_list_ref = widget.as_portal_list();
            let Some(mut list) = portal_list_ref.borrow_mut() else { continue };

            list.set_item_range(cx, 0, self.render_items.len());
            while let Some(idx) = list.next_visible_item(cx) {
                let Some(item) = self.render_items.get(idx).cloned() else { continue };
                match item {
                    RenderItem::Header { room_id, count } => {
                        let widget_item = list.item(cx, idx, id!(section_header));
                        if let Some(mut hdr) =
                            widget_item.as_global_search_section_header().borrow_mut()
                        {
                            let display = resolve_room_display(cx, &room_id);
                            hdr.set_header(cx, &display, count);
                        }
                        widget_item.draw_all(cx, &mut Scope::empty());
                    }
                    RenderItem::Hit { hit_index } => {
                        let Some(hit) = self.hits.get(hit_index) else { continue };
                        let widget_item = list.item(cx, idx, id!(hit_row));
                        if let Some(mut row) = widget_item.as_global_search_hit_row().borrow_mut() {
                            row.set_hit(cx, hit);
                        }
                        widget_item.draw_all(cx, &mut Scope::empty());
                    }
                }
            }
        }
        DrawStep::done()
    }
}

impl GlobalMessageSearch {
    /// Mark the widget as having a search in-flight. Hides results,
    /// shows a "Searching..." status.
    pub fn set_loading(&mut self, cx: &mut Cx, query: String) {
        self.status = Status::Loading { query: query.clone() };
        self.last_query = query;
        self.redraw(cx);
    }

    /// Replace results with a freshly-arrived first page.
    pub fn set_results(
        &mut self,
        cx: &mut Cx,
        query: String,
        hits: Vec<GlobalSearchHit>,
        total_count: u64,
        next_batch: Option<String>,
    ) {
        self.last_query = query;
        self.hits = hits;
        self.next_batch = next_batch.clone();
        self.render_items = build_render_items(&self.hits);
        self.status = if self.hits.is_empty() {
            Status::Empty
        } else {
            Status::Results {
                total: total_count.max(self.hits.len() as u64),
                has_more: next_batch.is_some(),
            }
        };
        let list = self.portal_list(cx, ids!(results_list));
        list.set_first_id_and_scroll(0, 0.0);
        self.redraw(cx);
    }

    /// Append a follow-up page (pagination).
    pub fn append_results(
        &mut self,
        cx: &mut Cx,
        mut hits: Vec<GlobalSearchHit>,
        total_count: u64,
        next_batch: Option<String>,
    ) {
        self.hits.append(&mut hits);
        self.next_batch = next_batch.clone();
        self.render_items = build_render_items(&self.hits);
        self.status = Status::Results {
            total: total_count.max(self.hits.len() as u64),
            has_more: next_batch.is_some(),
        };
        self.redraw(cx);
    }

    /// Surface a failed request.
    pub fn set_error(&mut self, cx: &mut Cx, error: String) {
        self.status = Status::Failed { error };
        self.redraw(cx);
    }

    /// Reset to the idle state — used when the user clears the search
    /// input or closes the modal.
    pub fn clear(&mut self, cx: &mut Cx) {
        self.hits.clear();
        self.render_items.clear();
        self.next_batch = None;
        self.status = Status::Idle;
        self.last_query.clear();
        self.redraw(cx);
    }

    /// Get the current `next_batch` pagination token, if any.
    pub fn next_batch(&self) -> Option<&str> {
        self.next_batch.as_deref()
    }

    /// Get the query string that produced the current results.
    pub fn last_query(&self) -> &str {
        &self.last_query
    }

    fn refresh_display(&mut self, cx: &mut Cx) {
        let (status_text, status_visible, results_visible, load_more_visible) =
            match &self.status {
                Status::Idle => (String::new(), false, false, false),
                Status::Loading { query } => (
                    format!("Searching for \u{201C}{}\u{201D}…", truncate_for_display(query, 40)),
                    true, !self.hits.is_empty(), false,
                ),
                Status::Empty => (
                    format!(
                        "No messages match \u{201C}{}\u{201D}.",
                        truncate_for_display(&self.last_query, 40),
                    ),
                    true, false, false,
                ),
                Status::Results { total, has_more } => {
                    let by_room = group_count(&self.hits);
                    let txt = if *has_more {
                        format!("Showing {} of {} matches in {} room(s)",
                            self.hits.len(), total, by_room)
                    } else if *total == 1 {
                        "1 match".to_string()
                    } else {
                        format!("{total} matches in {by_room} room(s)")
                    };
                    (txt, true, true, *has_more)
                }
                Status::Failed { error } => (
                    format!("Search failed: {error}"),
                    true, !self.hits.is_empty(), false,
                ),
            };

        let status_label = self.label(cx, ids!(status_label));
        status_label.set_visible(cx, status_visible);
        if status_visible {
            status_label.set_text(cx, &status_text);
        }
        self.view(cx, ids!(results_scroll)).set_visible(cx, results_visible);
        self.button(cx, ids!(load_more_button)).set_visible(cx, load_more_visible);
    }
}

// ============================================================================
// Helpers (pure, easy to unit-test)
// ============================================================================

/// Group hits by `room_id`, sort sections by most-recent hit timestamp
/// descending, and flatten into a `RenderItem` list. Hits within each
/// section keep their original (server-returned) order, which is already
/// most-recent-first since `OrderBy::Recent`.
fn build_render_items(hits: &[GlobalSearchHit]) -> Vec<RenderItem> {
    if hits.is_empty() {
        return Vec::new();
    }
    // Walk once to bucket hits by room while preserving incoming order.
    let mut buckets: BTreeMap<OwnedRoomId, Vec<usize>> = BTreeMap::new();
    for (i, hit) in hits.iter().enumerate() {
        buckets.entry(hit.room_id.clone()).or_default().push(i);
    }
    // Sort the buckets by the most-recent hit timestamp in each (desc).
    let mut sections: Vec<(OwnedRoomId, Vec<usize>, MilliSecondsSinceUnixEpoch)> = buckets
        .into_iter()
        .map(|(room_id, indices)| {
            let recent_ts = indices.iter()
                .map(|&i| hits[i].timestamp)
                .max()
                .unwrap_or_else(|| MilliSecondsSinceUnixEpoch(matrix_sdk::ruma::UInt::default()));
            (room_id, indices, recent_ts)
        })
        .collect();
    sections.sort_by(|a, b| b.2.0.cmp(&a.2.0));

    // Flatten into render plan.
    let mut out = Vec::with_capacity(hits.len() + sections.len());
    for (room_id, indices, _) in sections {
        out.push(RenderItem::Header { room_id, count: indices.len() });
        for idx in indices {
            out.push(RenderItem::Hit { hit_index: idx });
        }
    }
    out
}

/// Distinct-room count across the current hit list.
fn group_count(hits: &[GlobalSearchHit]) -> usize {
    use std::collections::HashSet;
    let rooms: HashSet<&OwnedRoomId> = hits.iter().map(|h| &h.room_id).collect();
    rooms.len()
}

/// Resolve a room's display name via the cached `RoomsListRef` global.
/// Falls back to the bare room_id (e.g. `!abc:example.org`) when the
/// room isn't yet known to the local sidebar cache — better than
/// rendering nothing.
fn resolve_room_display(cx: &mut Cx, room_id: &OwnedRoomId) -> String {
    if cx.has_global::<RoomsListRef>() {
        if let Some(name_id) = cx.get_global::<RoomsListRef>().get_room_name(room_id) {
            let display = name_id.display_name().to_string();
            if !display.is_empty() && display != "Empty" {
                return display;
            }
        }
    }
    room_id.to_string()
}

/// Format a Matrix `origin_server_ts` for display in a result row.
/// Returns a short `YYYY-MM-DD HH:MM` form when possible, falling back
/// to the raw ms count if conversion fails.
fn format_timestamp(ts: MilliSecondsSinceUnixEpoch) -> String {
    if let Some(dt) = unix_time_millis_to_datetime(ts) {
        dt.format("%Y-%m-%d %H:%M").to_string()
    } else {
        format!("{}", u64::from(ts.0))
    }
}

/// Truncate a message body for display in a result row preview. Picks
/// the first ~140 chars and trims to the nearest whitespace boundary
/// to avoid mid-word cuts. Inserts an ellipsis when truncated.
fn truncate_body(body: &str) -> String {
    const MAX: usize = 140;
    let trimmed = body.replace('\n', " ");
    if trimmed.chars().count() <= MAX {
        return trimmed;
    }
    let take: String = trimmed.chars().take(MAX).collect();
    let last_ws = take.rfind(|c: char| c.is_whitespace());
    let cut = last_ws.map(|i| &take[..i]).unwrap_or(&take);
    format!("{cut}…")
}

/// Trim a string for display in status text. Same shape as the per-room
/// pane's helper but kept local to keep the dependency surface small.
fn truncate_for_display(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let take: String = s.chars().take(max).collect();
    format!("{take}…")
}

#[cfg(test)]
mod tests {
    use super::*;
    use matrix_sdk::ruma::{event_id, user_id, UInt};

    fn make_hit(room: &str, ts: u64, body: &str) -> GlobalSearchHit {
        GlobalSearchHit {
            event_id: event_id!("$evt:example.org").to_owned(),
            room_id: <&matrix_sdk::ruma::RoomId>::try_from(room).unwrap().to_owned(),
            sender_user_id: user_id!("@alice:example.org").to_owned(),
            sender_display_name: None,
            body: body.to_string(),
            timestamp: MilliSecondsSinceUnixEpoch(UInt::new(ts).unwrap()),
        }
    }

    #[test]
    fn build_render_items_groups_by_room_and_sorts_sections_by_recency() {
        // room A: oldest activity; room B: newest
        let hits = vec![
            make_hit("!a:example.org", 100, "hello"),
            make_hit("!a:example.org", 200, "world"),
            make_hit("!b:example.org", 1000, "fresh"),
        ];
        let items = build_render_items(&hits);
        // Room B (timestamp 1000) should come first.
        match &items[0] {
            RenderItem::Header { room_id, count } => {
                assert_eq!(room_id.as_str(), "!b:example.org");
                assert_eq!(*count, 1);
            }
            other => panic!("expected first item to be section header, got {:?}", other),
        }
        match &items[1] {
            RenderItem::Hit { hit_index } => assert_eq!(*hit_index, 2),
            other => panic!("expected hit, got {:?}", other),
        }
        // Then Room A header with 2 hits.
        match &items[2] {
            RenderItem::Header { room_id, count } => {
                assert_eq!(room_id.as_str(), "!a:example.org");
                assert_eq!(*count, 2);
            }
            other => panic!("expected section header, got {:?}", other),
        }
    }

    #[test]
    fn build_render_items_empty_input_returns_empty() {
        assert!(build_render_items(&[]).is_empty());
    }

    #[test]
    fn group_count_counts_distinct_rooms() {
        let hits = vec![
            make_hit("!a:example.org", 1, "x"),
            make_hit("!a:example.org", 2, "y"),
            make_hit("!b:example.org", 3, "z"),
        ];
        assert_eq!(group_count(&hits), 2);
    }

    #[test]
    fn truncate_body_caps_at_140_chars_with_ellipsis() {
        let long = "a".repeat(300);
        let out = truncate_body(&long);
        assert!(out.ends_with('…'));
        assert!(out.chars().count() <= 141);
    }

    #[test]
    fn truncate_body_short_input_unchanged() {
        assert_eq!(truncate_body("hello"), "hello");
    }
}
