//! A list of a room's threads, most recently active first, which subscribes to them itself
//! so it can be docked within a RoomScreen or popped out. Clicking a thread opens it.

use std::{borrow::Cow, sync::Arc};

use makepad_widgets::*;
use matrix_sdk::ruma::OwnedRoomId;
use matrix_sdk_ui::{sync_service::State as SyncServiceState, timeline::TimelineDetails};

use crate::{
    app::SelectedRoom,
    event_preview::text_preview_of_thread_reply,
    home::{rooms_list::RoomsListAction, rooms_list_header::RoomsListHeaderAction},
    profile::user_profile_cache::{self, CachedName},
    shared::{html_or_plaintext::HtmlOrPlaintextWidgetRefExt, list_rows::status_row},
    sliding_sync::{MatrixRequest, RoomDataKind, submit_async_request},
    threads_list_sync::ThreadListItem,
    utils::RoomNameId,
};
use super::message_list::{AVATAR_SIZE, MessageListRowAction, MessageListState, ROW_SPACING, RowMessage, SavedMessageList};

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.ThreadsList = #(ThreadsList::register_widget(vm)) {
        width: Fill, height: Fill
        flow: Down, spacing: 6

        threads_count_label := mod.widgets.MessageListCountLabel {}

        threads_list := mod.widgets.MessageListPortalList {
            // A thread's root message, and beneath it, how many replies it has and a preview of its latest reply.
            thread_row := mod.widgets.MessageListRow {
                // The divider at the bottom takes the place of the bottom padding, so it's evenly spaced between threads.
                padding +: { bottom: 0 }
                reply_summary := View {
                    width: Fill, height: Fit
                    margin: Inset{top: 4}
                    flow: Right, spacing: 5
                    // The icon is centered beneath the avatar on the reply count's line,
                    // and the count starts where the message text above it does.
                    View {
                        width: Fit, height: Fit
                        flow: Right, spacing: #(ROW_SPACING)
                        align: Align{y: 0.5}
                        View {
                            width: #(AVATAR_SIZE), height: Fit
                            flow: Down
                            align: Align{x: 0.5}
                            Icon {
                                width: Fit, height: Fit
                                // It overhangs the count's line rather than pushing the count down.
                                icon_walk: Walk{width: 20, height: 20, margin: Inset{top: -3, bottom: -3}}
                                draw_icon +: {
                                    svg: (ICON_REPLY_IN_THREAD)
                                    color: (COLOR_THREAD_SUMMARY_REPLY_COUNT)
                                }
                            }
                        }
                        reply_count := Label {
                            width: Fit, height: Fit
                            padding: 0
                            draw_text +: {
                                color: (COLOR_THREAD_SUMMARY_REPLY_COUNT)
                                text_style: USERNAME_TEXT_STYLE { font_size: (MESSAGE_FONT_SIZE), line_spacing: (MESSAGE_TEXT_LINE_SPACING) }
                            }
                        }
                    }
                    // A bit smaller than the reply count beside it, with a small top margin so the two share a baseline.
                    latest_reply := mod.widgets.MessagePreview {
                        margin: Inset{top: 1.3}
                        latest_message +: {
                            html_view +: { html +: {
                                max_lines: 2
                                font_size: #(LATEST_REPLY_FONT_SIZE)
                                text_style_normal +: { font_size: #(LATEST_REPLY_FONT_SIZE), line_spacing: (MESSAGE_TEXT_LINE_SPACING) }
                                text_style_italic +: { font_size: #(LATEST_REPLY_FONT_SIZE), line_spacing: (MESSAGE_TEXT_LINE_SPACING) }
                                text_style_bold +: { font_size: #(LATEST_REPLY_FONT_SIZE), line_spacing: (MESSAGE_TEXT_LINE_SPACING) }
                                text_style_bold_italic +: { font_size: #(LATEST_REPLY_FONT_SIZE), line_spacing: (MESSAGE_TEXT_LINE_SPACING) }
                                text_style_fixed +: { font_size: #(LATEST_REPLY_FONT_SIZE), line_spacing: (MESSAGE_TEXT_LINE_SPACING) }
                            } }
                            plaintext_view +: { pt_label +: { max_lines: 2 } }
                        }
                    }
                }
                divider := LineH {
                    height: 1
                    margin: Inset{top: 8}
                }
            }
            loading_row := mod.widgets.ListLoadingRow {}
            empty_row := mod.widgets.ListEmptyRow {}
        }
    }
}

/// The font size of a thread's latest reply, a bit smaller than the reply count beside it.
const LATEST_REPLY_FONT_SIZE: f64 = 10.0;

/// A room's threads, posted by the worker whenever they change (see [`RoomDataKind::Threads`]).
///
/// This is NOT a widget action.
#[derive(Debug)]
pub enum ThreadsListAction {
    Updated {
        room_id: OwnedRoomId,
        /// The room's threads that have been loaded so far, most recently active first.
        threads: Arc<Vec<Arc<ThreadListItem>>>,
        /// Whether all of the room's threads have been loaded.
        end_reached: bool,
    },
    Failed {
        room_id: OwnedRoomId,
        error: String,
    },
}

/// The state of a [`ThreadsList`] that is saved and restored.
#[derive(Default)]
pub struct SavedThreadsList {
    pub(super) list: SavedMessageList,
    pub(super) threads: Option<Arc<Vec<Arc<ThreadListItem>>>>,
    was_end_reached: bool,
}

#[derive(Script, ScriptHook, Widget)]
pub struct ThreadsList {
    #[deref] view: View,

    #[rust(MessageListState::new(RoomDataKind::Threads))] state: MessageListState,
    /// The threads posted by the worker, or `None` until they first arrive.
    #[rust] threads: Option<Arc<Vec<Arc<ThreadListItem>>>>,
    #[rust] end_reached: bool,
    #[rust] error: Option<String>,
    /// Whether we've asked the worker for more threads and are waiting for them.
    #[rust] is_paginating: bool,
}

impl Widget for ThreadsList {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        // A list that hasn't been given a room yet has nothing to do.
        if self.state.room_name_id().is_none() { return }

        self.view.handle_event(cx, event, scope);
        let list_ref = self.threads_list();
        if self.state.handle_event(cx, event, &list_ref) {
            self.redraw(cx);
        }

        let Event::Actions(actions) = event else { return };
        for action in actions {
            match action.downcast_ref() {
                Some(ThreadsListAction::Updated { room_id, threads, end_reached }) if self.state.is_showing_room(room_id) => {
                    self.is_paginating = false;
                    self.set_threads(cx, threads, *end_reached);
                }
                Some(ThreadsListAction::Failed { room_id, error }) if self.state.is_showing_room(room_id) => {
                    error!("Failed to load the threads of room {room_id}: {error}");
                    self.is_paginating = false;
                    self.error = Some(error.clone());
                    self.redraw(cx);
                }
                _ => {}
            }
            // Retry loading threads that failed to load while we were offline:
            // clearing the error brings back the loading row, which asks for them again.
            if self.error.is_some()
                && let Some(RoomsListHeaderAction::StateUpdate(state)) = action.downcast_ref()
                && !matches!(state, SyncServiceState::Offline)
            {
                self.error = None;
                self.state.subscribe();
                self.redraw(cx);
            }
        }

        for (index, row) in list_ref.items_with_actions(actions) {
            if let MessageListRowAction::Clicked = actions.find_widget_action(row.widget_uid()).cast()
                && let Some(thread) = self.threads.as_ref().and_then(|t| t.get(index))
                && let Some(room_name_id) = self.state.room_name_id().cloned()
            {
                cx.widget_action(
                    self.widget_uid(),
                    RoomsListAction::Selected(SelectedRoom::Thread {
                        room_name_id,
                        thread_root_event_id: thread.root_event.event_id.clone(),
                    }),
                );
            }
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        let status = self.get_status();
        let num_threads = self.threads.as_ref().map_or(0, |t| t.len());
        // While there are more threads to load, a row at the end of the list shows that they're loading.
        let has_end_row = status.is_none() && !self.end_reached;
        while let Some(item) = self.view.draw_walk(cx, scope, walk).step() {
            let list_ref = item.as_portal_list();
            let Some(mut list) = list_ref.borrow_mut() else { continue };
            let count = if status.is_some() { 1 } else { num_threads + usize::from(has_end_row) };
            self.state.prepare_list_for_draw(cx, &mut list, count);
            while let Some(index) = list.next_visible_item(cx) {
                if index >= count { continue; }
                let row = if let Some((text, is_loading)) = &status {
                    // A fresh worker loads the first page itself, so this only asks again after an error,
                    // or after a page came back empty with more to load.
                    if *is_loading && self.threads.is_some() {
                        self.paginate();
                    }
                    status_row(cx, &mut list, index, *is_loading, text)
                } else if index == num_threads {
                    if let Some(error) = self.error.as_ref() {
                        status_row(cx, &mut list, index, false, &format!("Failed to load more threads: {error}"))
                    } else {
                        self.paginate();
                        status_row(cx, &mut list, index, true, "Loading more threads...")
                    }
                } else {
                    let Some(thread) = self.threads.as_ref().and_then(|t| t.get(index)) else { continue };
                    let root = &thread.root_event;
                    let message = RowMessage {
                        sender: &root.sender,
                        sender_profile: &root.sender_profile,
                        timestamp: root.timestamp,
                        content: root.content.as_ref(),
                    };
                    let (row, is_new_content) = self.state.populate_message_row(cx, &mut list, index, id!(thread_row), message);
                    if is_new_content {
                        row.child_by_path(ids!(reply_count)).set_text(cx, &format!("({})", thread.num_replies));
                        let latest_reply = row.child_by_path(ids!(latest_reply));
                        latest_reply.set_visible(cx, thread.latest_event.is_some());
                        if let Some(latest) = thread.latest_event.as_ref()
                            && let Some(room_id) = self.state.room_name_id().map(|r| r.room_id().clone())
                        {
                            // Like the root's sender, the reply's sender is named by their room profile if it's known.
                            let cached_name = match &latest.sender_profile {
                                TimelineDetails::Ready(profile) => CachedName::FoundInRoom(profile.display_name.clone()),
                                _ => user_profile_cache::get_user_display_name_for_room(cx, latest.sender.clone(), Some(&room_id), true),
                            };
                            if !cached_name.was_found() {
                                self.state.mark_row_as_waiting_on_profile(index);
                            }
                            let sender_name = cached_name.as_deref().unwrap_or(latest.sender.as_str());
                            latest_reply.child_by_path(ids!(latest_message)).as_html_or_plaintext().show_html(
                                cx,
                                text_preview_of_thread_reply(&latest.sender, sender_name, latest.content.as_ref()),
                            );
                        }
                    }
                    row
                };
                row.draw_all(cx, scope);
            }
        }
        DrawStep::done()
    }
}

impl ThreadsList {
    fn threads_list(&self) -> PortalListRef {
        self.view.child(id!(threads_list)).as_portal_list()
    }

    /// Asks the worker to load more of our room's threads, unless we're already waiting for some.
    fn paginate(&mut self) {
        if self.is_paginating { return }
        let Some(room_name_id) = self.state.room_name_id() else { return };
        submit_async_request(MatrixRequest::PaginateThreadsList { room_id: room_name_id.room_id().clone() });
        self.is_paginating = true;
    }

    /// Shows the threads for the given room.
    ///
    /// Call this again with the same room whenever its name changes.
    fn set_room(&mut self, cx: &mut Cx, room_name_id: &RoomNameId) {
        if !self.state.is_showing_room(room_name_id.room_id()) {
            self.reset(cx);
        }
        self.state.set_room(room_name_id);
    }

    fn set_threads(&mut self, cx: &mut Cx, threads: &Arc<Vec<Arc<ThreadListItem>>>, end_reached: bool) {
        if self.threads.as_ref().is_some_and(|t| Arc::ptr_eq(t, threads))
            && self.end_reached == end_reached
        {
            // don't do anything if nothing changed
            return;
        }
        let old_threads = self.threads.replace(threads.clone());
        self.end_reached = end_reached;
        self.error = None;
        self.update_threads_count(cx);
        self.state.handle_new_messages(cx, &self.threads_list(), old_threads.as_deref(), threads, end_reached);
        self.redraw(cx);
    }

    /// Clears this list and unsubscribes from its room's threads,
    /// such that it shows nothing until `set_room()` is called.
    fn reset(&mut self, cx: &mut Cx) {
        self.state.reset(cx, &self.threads_list());
        self.threads = None;
        self.end_reached = false;
        self.error = None;
        self.is_paginating = false;
        self.update_threads_count(cx);
        self.redraw(cx);
    }

    /// Returns the status text to show instead of thread rows, and whether it's a loading status.
    fn get_status(&self) -> Option<(Cow<'static, str>, bool)> {
        if self.threads.as_ref().is_some_and(|t| !t.is_empty()) {
            None
        } else if let Some(error) = self.error.as_ref() {
            Some((format!("Failed to load threads: {error}").into(), false))
        } else if self.end_reached {
            Some(("This room has no threads.".into(), false))
        } else {
            Some(("Loading threads...".into(), true))
        }
    }

    /// Shows how many threads have been loaded, and whether there are more to load.
    fn update_threads_count(&mut self, cx: &mut Cx) {
        let num_loaded = self.threads.as_ref().map_or(0, |t| t.len());
        let text = match (num_loaded, self.end_reached) {
            (0, _) => String::new(),
            (1, true) => String::from("1 thread"),
            (n, true) => format!("{n} threads"),
            (n, false) => format!("{n}+ threads"),
        };
        self.view.child(id!(threads_count_label)).set_text(cx, &text);
    }
}

impl ThreadsListRef {
    /// See [`ThreadsList::set_room()`].
    pub fn set_room(&self, cx: &mut Cx, room_name_id: &RoomNameId) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_room(cx, room_name_id);
        }
    }

    /// See [`ThreadsList::reset()`].
    pub fn reset(&self, cx: &mut Cx) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.reset(cx);
        }
    }

    /// Saves and returns this list's state, e.g., to be restored if it's shown again later.
    ///
    /// This includes our subscription to this room's threads too,
    /// so you should only call this right before this list is hidden or destroyed.
    pub fn save_state(&self) -> SavedThreadsList {
        let Some(mut inner) = self.borrow_mut() else { return SavedThreadsList::default() };
        let list = inner.threads_list();
        SavedThreadsList {
            list: inner.state.save_state(&list),
            threads: inner.threads.clone(),
            was_end_reached: inner.end_reached,
        }
    }

    /// Restores the saved list of threads for the given room without loading them from scratch again.
    pub fn restore_state(&self, cx: &mut Cx, room_name_id: &RoomNameId, saved: SavedThreadsList) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.reset(cx);
        inner.state.restore_state(room_name_id, saved.list);
        if let Some(threads) = saved.threads {
            inner.set_threads(cx, &threads, saved.was_end_reached);
        }
    }
}
