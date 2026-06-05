//! The public room directory browser screen.
//!
//! Reached from the compass button in the RoomsListHeader. Lets the user search
//! their homeserver's public room directory and join rooms directly from the list.

use std::collections::HashSet;

use makepad_widgets::*;
use matrix_sdk::ruma::OwnedRoomId;

use crate::{
    avatar_cache::{self, AvatarCacheEntry},
    home::invite_screen::JoinRoomResultAction,
    shared::avatar::AvatarWidgetExt,
    sliding_sync::{
        DirectoryRoomKind, MatrixRequest, PublicDirectoryAction, PublicRoomDirectoryEntry,
        submit_async_request,
    },
    utils,
};

const PAGE_LIMIT: u64 = 20;

/// Delay (seconds) after the user stops typing before firing a search request.
const SEARCH_DEBOUNCE_SECS: f64 = 0.3;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*


    mod.widgets.DirectoryRoomEntry = #(DirectoryRoomEntry::register_widget(vm)) {
        ..mod.widgets.View
        width: Fill, height: Fit
        padding: Inset{top: 10, bottom: 10, left: 10, right: 10}
        flow: Right
        align: Align{y: 0.5}
        spacing: 12
        show_bg: true
        draw_bg +: { color: (COLOR_PRIMARY) }

        avatar := Avatar {
            width: 48, height: 48
        }

        text_column := View {
            width: Fill, height: Fit
            flow: Down
            spacing: 2

            name_label := Label {
                width: Fill, height: Fit
                draw_text +: {
                    color: #x0
                    text_style: TITLE_TEXT { font_size: 12 }
                }
                text: ""
            }

            topic_label := Label {
                width: Fill, height: Fit
                draw_text +: {
                    color: (MESSAGE_TEXT_COLOR)
                    text_style: MESSAGE_TEXT_STYLE { font_size: 10 }
                }
                text: ""
            }

            meta_label := Label {
                width: Fill, height: Fit
                draw_text +: {
                    color: (MESSAGE_TEXT_COLOR)
                    text_style: MESSAGE_TEXT_STYLE { font_size: 9 }
                }
                text: ""
            }
        }

        join_button := RobrixPositiveIconButton {
            width: Fit, height: Fit
            padding: Inset{top: 8, bottom: 8, left: 12, right: 12}
            draw_icon.svg: (ICON_JOIN_ROOM)
            icon_walk: Walk{width: 14, height: 14}
            text: "Join"
        }
    }


    mod.widgets.DirectoryScreen = #(DirectoryScreen::register_widget(vm)) {
        ..mod.widgets.View
        width: Fill, height: Fill
        flow: Down
        padding: Inset{top: 6, left: 12, right: 12, bottom: 0}
        spacing: 8
        show_bg: true
        draw_bg +: { color: (COLOR_PRIMARY) }

        title := Label {
            width: Fill, height: Fit
            margin: Inset{top: 4, bottom: 4, left: 4}
            draw_text +: {
                color: #x0
                text_style: TITLE_TEXT { font_size: 16 }
            }
            text: "Public Room Directory"
        }

        search_row := View {
            width: Fill, height: Fit
            flow: Right
            align: Align{y: 0.5}
            spacing: 6

            search_input := RobrixTextInput {
                width: Fill, height: 36
                padding: Inset{left: 12, right: 12, top: 9, bottom: 0}
                empty_text: "Search public rooms..."
            }
        }

        error_banner := View {
            visible: false
            width: Fill, height: Fit
            padding: Inset{left: 10, right: 10, top: 6, bottom: 6}
            margin: Inset{top: 0, bottom: 4}
            show_bg: true
            draw_bg +: { color: (COLOR_FG_DANGER_RED) }

            error_label := Label {
                width: Fill, height: Fit
                draw_text +: {
                    color: #xfff
                    text_style: MESSAGE_TEXT_STYLE { font_size: 10 }
                }
                text: ""
            }
        }

        room_list := PortalList {
            width: Fill, height: Fill
            keep_invisible: false,
            max_pull_down: 0.0,
            auto_tail: false,
            flow: Down

            room_entry := mod.widgets.DirectoryRoomEntry {}

            loading_entry := View {
                width: Fill, height: 60
                flow: Right
                align: Align{x: 0.5, y: 0.5}
                LoadingSpinner {
                    width: 24, height: 24
                    draw_bg +: {
                        color: (COLOR_ACTIVE_PRIMARY)
                        border_size: 3.0
                    }
                }
            }

            status_entry := View {
                width: Fill, height: Fit
                padding: Inset{top: 20, bottom: 20, left: 10, right: 10}
                flow: Right
                align: Align{x: 0.5, y: 0.5}
                status_label := Label {
                    width: Fit, height: Fit
                    draw_text +: {
                        color: (MESSAGE_TEXT_COLOR)
                        text_style: MESSAGE_TEXT_STYLE { font_size: 11 }
                    }
                    text: ""
                }
            }
        }
    }
}


#[derive(Script, ScriptHook, Widget)]
pub struct DirectoryRoomEntry {
    #[deref] view: View,
    #[rust] room_id: Option<OwnedRoomId>,
    #[rust(false)] is_joining: bool,
    #[rust(false)] is_joined: bool,
}

#[derive(Clone, Default, Debug)]
pub enum DirectoryEntryAction {
    JoinClicked(OwnedRoomId),
    #[default]
    None,
}

impl Widget for DirectoryRoomEntry {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        let uid = self.widget_uid();
        if let Event::Actions(actions) = event {
            let join_button = self.view.button(cx, ids!(join_button));
            if join_button.clicked(actions) {
                if let Some(rid) = &self.room_id {
                    log!("[public_directory] DirectoryRoomEntry::join clicked: room_id={rid}");
                    if !self.is_joining && !self.is_joined {
                        cx.widget_action(uid, DirectoryEntryAction::JoinClicked(rid.clone()));
                    }
                }
            }
        }
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl DirectoryRoomEntry {
    pub fn populate(
        &mut self,
        cx: &mut Cx,
        entry: &PublicRoomDirectoryEntry,
        is_joining: bool,
        is_joined: bool,
    ) {
        self.room_id = Some(entry.room_id.clone());
        self.is_joining = is_joining;
        self.is_joined = is_joined;

        self.view
            .label(cx, ids!(text_column.name_label))
            .set_text(cx, &entry.display_name);

        let topic_text = entry
            .topic
            .as_deref()
            .map(collapse_to_single_line)
            .unwrap_or_default();
        self.view
            .label(cx, ids!(text_column.topic_label))
            .set_text(cx, &topic_text);

        let mut meta = String::new();
        if let Some(alias) = &entry.canonical_alias {
            meta.push_str(alias);
            meta.push_str("  ·  ");
        }
        meta.push_str(&format!("{} members", entry.num_joined_members));
        self.view
            .label(cx, ids!(text_column.meta_label))
            .set_text(cx, &meta);

        let avatar = self.view.avatar(cx, ids!(avatar));
        let mut drew_image = false;
        if let Some(uri) = &entry.avatar_uri {
            if let AvatarCacheEntry::Loaded(data) = avatar_cache::get_or_fetch_avatar(cx, uri) {
                let res = avatar.show_image(cx, None, |cx, img| {
                    utils::load_png_or_jpg(&img, cx, &data)
                });
                drew_image = res.is_ok();
            }
        }
        if !drew_image {
            avatar.show_text(cx, None, None, entry.display_name.as_str());
        }

        let join_button = self.view.button(cx, ids!(join_button));
        if is_joined {
            join_button.set_text(cx, "Joined");
            join_button.set_enabled(cx, false);
        } else if is_joining {
            join_button.set_text(cx, "Joining…");
            join_button.set_enabled(cx, false);
        } else {
            join_button.set_text(cx, "Join");
            join_button.set_enabled(cx, true);
        }
    }
}


#[derive(Script, ScriptHook, Widget)]
pub struct DirectoryScreen {
    #[deref] view: View,
    #[rust(0u64)] query_id: u64,
    #[rust] search_text: String,
    #[rust] rooms: Vec<PublicRoomDirectoryEntry>,
    #[rust] next_batch: Option<String>,
    #[rust(false)] is_loading: bool,
    #[rust(true)] needs_initial_fetch: bool,
    #[rust] last_error: Option<String>,
    #[rust] pending_joins: HashSet<OwnedRoomId>,
    #[rust] joined_rooms: HashSet<OwnedRoomId>,
    #[rust(Timer::empty())] search_debounce_timer: Timer,
    #[rust] pending_search_text: String,
}

impl Widget for DirectoryScreen {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        // Debounce timer fired: run the pending search if the text actually
        // differs from what we last queried.
        if let Event::Timer(te) = event {
            if self.search_debounce_timer.is_timer(te).is_some() {
                self.search_debounce_timer = Timer::empty();
                let pending = std::mem::take(&mut self.pending_search_text);
                log!(
                    "[public_directory] debounce fired: pending={pending:?} current={:?}",
                    self.search_text,
                );
                if pending != self.search_text {
                    self.start_fresh_query(cx, pending);
                }
            }
        }

        if let Event::Actions(actions) = event {
            let search_input = self.view.text_input(cx, ids!(search_input));
            if let Some(text) = search_input.changed(actions) {
                log!("[public_directory] search input changed: text={text:?}");
                cx.stop_timer(self.search_debounce_timer);
                self.pending_search_text = text;
                self.search_debounce_timer = cx.start_timeout(SEARCH_DEBOUNCE_SECS);
            }
            if let Some((text, _)) = search_input.returned(actions) {
                log!("[public_directory] search submitted (Enter): text={text:?}");
                cx.stop_timer(self.search_debounce_timer);
                self.search_debounce_timer = Timer::empty();
                self.pending_search_text.clear();
                self.start_fresh_query(cx, text);
            }

            for action in actions {
                if let Some(dir_action) = action.downcast_ref::<PublicDirectoryAction>() {
                    self.handle_public_directory_action(cx, dir_action);
                    continue;
                }
                if let Some(jra) = action.downcast_ref::<JoinRoomResultAction>() {
                    self.handle_join_result(cx, jra);
                    continue;
                }
                if let DirectoryEntryAction::JoinClicked(rid) =
                    action.as_widget_action().cast()
                {
                    log!("[public_directory] JoinClicked received: room_id={rid}");
                    if !self.pending_joins.contains(&rid) && !self.joined_rooms.contains(&rid) {
                        self.pending_joins.insert(rid.clone());
                        submit_async_request(MatrixRequest::JoinRoom { room_id: rid });
                        self.view.redraw(cx);
                    }
                    continue;
                }
            }
        }

        // Detect bottom-reach for pagination.
        if self.next_batch.is_some() && !self.is_loading {
            let list = self.view.portal_list(cx, ids!(room_list));
            if list.is_at_end() {
                self.submit_fetch(false);
            }
        }

        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        if self.needs_initial_fetch {
            self.needs_initial_fetch = false;
            self.start_fresh_query(cx, String::new());
        }

        let banner_visible = self.last_error.is_some();
        self.view
            .view(cx, ids!(error_banner))
            .set_visible(cx, banner_visible);
        if banner_visible {
            self.view
                .label(cx, ids!(error_banner.error_label))
                .set_text(cx, self.last_error.as_deref().unwrap_or(""));
        }

        while let Some(widget_to_draw) = self.view.draw_walk(cx, scope, walk).step() {
            let plist = widget_to_draw.as_portal_list();
            let Some(mut list) = plist.borrow_mut() else { continue };

            let n = self.rooms.len();
            let has_more = self.next_batch.is_some() || self.is_loading;
            let show_empty_status = n == 0 && !self.is_loading;
            let total = if show_empty_status {
                1
            } else {
                n + if has_more { 1 } else { 0 }
            };
            list.set_item_range(cx, 0, total);

            while let Some(item_id) = list.next_visible_item(cx) {
                let item = if show_empty_status && item_id == 0 {
                    let item = list.item(cx, item_id, id!(status_entry));
                    let msg = if self.search_text.trim().is_empty() {
                        "No public rooms found."
                    } else {
                        "No rooms match your search."
                    };
                    item.child_by_path(ids!(status_label))
                        .as_label()
                        .set_text(cx, msg);
                    item
                } else if item_id < n {
                    let entry = self.rooms[item_id].clone();
                    let is_joining = self.pending_joins.contains(&entry.room_id);
                    let is_joined = self.joined_rooms.contains(&entry.room_id);
                    let item = list.item(cx, item_id, id!(room_entry));
                    if let Some(mut inner) = item.borrow_mut::<DirectoryRoomEntry>() {
                        inner.populate(cx, &entry, is_joining, is_joined);
                    }
                    item
                } else if has_more && item_id == n {
                    list.item(cx, item_id, id!(loading_entry))
                } else {
                    continue;
                };
                item.draw_all(cx, scope);
            }
        }

        DrawStep::done()
    }
}

impl DirectoryScreen {
    fn start_fresh_query(&mut self, cx: &mut Cx, text: String) {
        self.search_text = text;
        self.query_id = self.query_id.wrapping_add(1);
        self.rooms.clear();
        self.next_batch = None;
        self.last_error = None;
        self.is_loading = false;
        log!(
            "[public_directory] start_fresh_query: query_id={} search_text={:?}",
            self.query_id,
            self.search_text,
        );
        self.submit_fetch(true);
        self.view.redraw(cx);
    }

    fn submit_fetch(&mut self, is_first_page: bool) {
        if self.is_loading {
            log!(
                "[public_directory] submit_fetch skipped (already loading): \
                 query_id={} is_first_page={is_first_page}",
                self.query_id,
            );
            return;
        }
        self.is_loading = true;
        log!(
            "[public_directory] search_id={} submit_fetch: query_id={} is_first_page={is_first_page} \
             since={:?} limit={}",
            self.search_text,
            self.query_id,
            self.next_batch,
            PAGE_LIMIT,
        );
        submit_async_request(MatrixRequest::FetchPublicDirectoryPage {
            search_term: self.search_text.clone(),
            kind: DirectoryRoomKind::Rooms,
            since: if is_first_page { None } else { self.next_batch.clone() },
            limit: Some(PAGE_LIMIT),
            query_id: self.query_id,
        });
    }

    fn handle_public_directory_action(&mut self, cx: &mut Cx, action: &PublicDirectoryAction) {
        match action {
            PublicDirectoryAction::Page {
                query_id,
                is_first_page,
                rooms,
                next_batch,
            } => {
                if *query_id != self.query_id {
                    return;
                }
                self.is_loading = false;
                if *is_first_page {
                    self.rooms.clear();
                }
                self.rooms.extend(rooms.iter().cloned());
                self.next_batch = next_batch.clone();
                self.last_error = None;
                self.view.redraw(cx);
            }
            PublicDirectoryAction::Failed {
                query_id,
                is_first_page: _,
                error,
            } => {
                if *query_id != self.query_id {
                    return;
                }
                self.is_loading = false;
                self.last_error = Some(error.clone());
                self.view.redraw(cx);
            }
        }
    }

    fn handle_join_result(&mut self, cx: &mut Cx, action: &JoinRoomResultAction) {
        match action {
            JoinRoomResultAction::Joined { room_id } => {
                if self.pending_joins.remove(room_id) {
                    self.joined_rooms.insert(room_id.clone());
                    self.view.redraw(cx);
                }
            }
            JoinRoomResultAction::Failed { room_id, error: _ } => {
                if self.pending_joins.remove(room_id) {
                    self.view.redraw(cx);
                }
            }
        }
    }
}


fn collapse_to_single_line(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_ws = false;
    for c in s.chars() {
        if c.is_whitespace() {
            if !prev_ws {
                out.push(' ');
                prev_ws = true;
            }
        } else {
            out.push(c);
            prev_ws = false;
        }
    }
    out.trim().to_string()
}
