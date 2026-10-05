//! A background task that loads a room's threads and keeps them up to date
//! while the UI shows its threads list (or keeps that list's saved state).

use std::{cmp::Reverse, sync::Arc};
use eyeball_im::VectorDiff;
use futures_util::{future::{BoxFuture, Fuse, FusedFuture, FutureExt, join_all}, stream::FuturesUnordered, StreamExt};
use hashbrown::{HashMap, HashSet, hash_map::Entry};
use makepad_widgets::{error, warning, Cx};
use matrix_sdk_base::apply_redaction;
use matrix_sdk::{
    check_validity_of_replacement_events, deserialized_responses::{EncryptionInfo, TimelineEvent, TimelineEventKind, UnsignedDecryptionResult, UnsignedEventLocation}, event_cache::{EventsOrigin, RoomEventCacheUpdate, TimelineVectorDiffs}, room::ListThreadsOptions, ruma::{
        events::{relation::RelationType, AnySyncTimelineEvent}, serde::Raw, MilliSecondsSinceUnixEpoch, OwnedEventId, OwnedUserId, UInt
    }, serde_helpers::{extract_redaction_target, extract_relation}, Error, Room
};
use matrix_sdk_ui::timeline::{Profile, TimelineDetails, TimelineItemContent};
use tokio::sync::{broadcast, Notify};
use crate::room::threads_list::ThreadsListAction;

/// A page (chunk) of a room's threads, and the token to load the next page.
struct ThreadsPage {
    threads_chunk: Vec<ThreadListItem>,
    /// The token the page was loaded from, which is `None` for the first page.
    from: Option<String>,
    /// The token to load the next page; `None` if this is the final page.
    token_next_page: Option<String>,
}

/// Watches the given room's threads for changes to the root message or latest reply.
///
/// Loads another page of threads when `load_more_notifier` is notified,
/// and re-sends the existing thread list whenever they change or when
/// the given `resend_notifier` is notified.
pub async fn threads_list_subscriber_handler(
    room: Room,
    resend_notifier: Arc<Notify>,
    load_more_notifier: Arc<Notify>,
) {
    let room_id = room.room_id().to_owned();
    // Subscribe the room's event cache to get updates on new/edited threads
    // and thread replies.
    let (_event_cache_drop_handles, mut room_event_updates) = match async {
        let (room_event_cache, drop_handles) = room.event_cache().await?;
        let (_, subscriber) = room_event_cache.subscribe().await?;
        matrix_sdk::event_cache::Result::Ok((drop_handles, subscriber))
    }.await {
        Ok(pair) => pair,
        Err(error) => {
            error!("Failed to watch the event cache of room {room_id} for its threads: {error}");
            Cx::post_action(ThreadsListAction::Failed { room_id, error: error.to_string() });
            return;
        }
    };

    // The loaded threads, sorted by most recently active first.
    let mut threads: Arc<Vec<Arc<ThreadListItem>>> = Arc::default();
    let mut was_end_reached = false;
    let mut next_page_token = None;
    let mut next_page_future: Fuse<BoxFuture<'static, Result<ThreadsPage, Error>>> = Fuse::terminated();
    // Whether to reload the list once the page that's loading now is done, since we missed events meanwhile.
    let mut is_reload_pending = false;
    // Whether the latest sync redacted an event we couldn't place in a thread, like an older reply.
    // If it was in a loaded thread, the SDK's next summary update for that thread tells us which one.
    let mut has_redaction_in_unknown_thread = false;

    // The threads we're in the midst of fetching from the server.
    let mut fetch_states: HashMap<OwnedEventId, FetchState> = HashMap::new();
    let mut root_fetches: FuturesUnordered<BoxFuture<'static, FetchedThread>> = FuturesUnordered::new();

    // The number of replies we've counted ourselves for a given thread root.
    // The key is the event ID of a reply we counted ourselves,
    // and the value is that reply's thread root event ID.
    // This is here to ensure we don't double-count a reply,
    // and so we know which thread root to refetch if a counted reply gets redacted.
    let mut counted_replies: HashMap<OwnedEventId, OwnedEventId> = HashMap::new();

    let redaction_rules = room.clone_info().room_version_rules_or_default().redaction;
    load_more_notifier.notify_one();

    let mut should_post = false;
    loop {
        if should_post {
            Cx::post_action(ThreadsListAction::Updated {
                room_id: room_id.clone(),
                threads: threads.clone(),
                end_reached: was_end_reached,
            });
        }

        should_post = tokio::select! {
            page = &mut next_page_future => {
                let is_changed = match page {
                    Ok(ThreadsPage { from, threads_chunk, token_next_page }) => {
                        let list = Arc::make_mut(&mut threads);
                        for thread in threads_chunk {
                            merge_thread(list, thread, false);
                        }
                        list.sort_by_key(|t| Reverse(get_latest_activity(t)));
                        // A first page we reloaded after missing events doesn't move us on to the next page.
                        if !was_end_reached && from == next_page_token {
                            was_end_reached = token_next_page.is_none();
                            next_page_token = token_next_page;
                        }
                        true
                    }
                    Err(error) => {
                        error!("Failed to load the threads of room {room_id}: {error}");
                        Cx::post_action(ThreadsListAction::Failed { room_id: room_id.clone(), error: error.to_string() });
                        false
                    }
                };
                if is_reload_pending {
                    is_reload_pending = false;
                    next_page_future = load_threads_page(&room, None, Some(threads.len()));
                }
                is_changed
            }

            Some(fetched) = root_fetches.next(), if !root_fetches.is_empty() => {
                let index = threads.iter().position(|t| t.root_event.event_id == fetched.root_id);
                // If the thread changed since we loaded it, request to fetch it again.
                let is_outdated = fetch_states.remove(&fetched.root_id) == Some(FetchState::Outdated);
                if is_outdated {
                    request_thread_fetch(&room, fetched.root_id.clone(), &mut fetch_states, &mut root_fetches);
                }
                match fetched.thread {
                    Ok(Some(thread)) if !is_outdated || index.is_none() => {
                        let list = Arc::make_mut(&mut threads);
                        merge_thread(list, thread, true);
                        list.sort_by_key(|t| Reverse(get_latest_activity(t)));
                        true
                    }
                    // The root is no longer a thread, e.g., its only reply was redacted.
                    Ok(None) if !is_outdated => index.is_some_and(|index| {
                        Arc::make_mut(&mut threads).remove(index);
                        true
                    }),
                    Err(error) if !is_outdated => {
                        warning!("Failed to fetch the root of thread {} in room {room_id}: {error}", fetched.root_id);
                        false
                    }
                    _ => false,
                }
            },

            update = room_event_updates.recv() => {
                let (diffs, is_from_sync, missed_events) = match update {
                    Ok(RoomEventCacheUpdate::UpdateTimelineEvents(TimelineVectorDiffs { diffs, origin })) => {
                        has_redaction_in_unknown_thread = false;
                        let is_from_sync = matches!(origin, EventsOrigin::Sync);
                        // A sync that skipped some events (a "limited" sync) clears the cache,
                        // so only the newest events reach us.
                        let missed_events = is_from_sync && diffs.iter().any(|diff| matches!(diff, VectorDiff::Clear));
                        (diffs, is_from_sync, missed_events)
                    }
                    // A redacted event we couldn't place still changes its thread's summary, which tells us the thread.
                    Ok(RoomEventCacheUpdate::UpdateThreadSummary { thread_root, .. }) => {
                        if has_redaction_in_unknown_thread && threads.iter().any(|t| t.root_event.event_id == thread_root) {
                            request_thread_fetch(&room, thread_root, &mut fetch_states, &mut root_fetches);
                        }
                        (Vec::new(), false, false)
                    }
                    Ok(_) => (Vec::new(), false, false),
                    Err(broadcast::error::RecvError::Lagged(num_missed)) => {
                        warning!("Missed {num_missed} event cache updates while watching the threads of room {room_id}");
                        (Vec::new(), false, true)
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                };
                let mut is_changed = false;
                // Each new event, and whether it's an edit that we could only decrypt now.
                let mut new_events = Vec::new();
                let mut roots_to_fetch: HashSet<OwnedEventId> = HashSet::new();
                let is_undecrypted = |event: &TimelineEvent| matches!(event.kind, TimelineEventKind::UnableToDecrypt { .. });
                for diff in diffs {
                    match diff {
                        // A newer version of an event we show, e.g., once it's decrypted or redacted.
                        VectorDiff::Set { value, .. } => {
                            let Some(event_id) = value.event_id() else { continue };
                            let Some((index, is_root)) = find_thread_event(&threads, |shown| shown.event_id == event_id) else {
                                // An edit that couldn't be decrypted when it arrived is handled like a new one once it is.
                                if matches!(extract_relation(value.raw()), Some((RelationType::Replacement, _))) {
                                    new_events.push((value, true));
                                }
                                continue;
                            };
                            let Some(shown) = threads[index].get_root_or_latest_reply(is_root) else { continue };
                            // A root is also set whenever its thread summary changes, which we don't show,
                            // and the cache may still hold an undecrypted copy of an event we could decrypt.
                            if shown.original.raw().json().get() == value.raw().json().get()
                                || is_undecrypted(&value) && !is_undecrypted(&shown.original)
                            {
                                continue;
                            }
                            // We track the event's edits ourselves, except those we couldn't read while it was undecrypted.
                            let edit = match shown.edit.clone() {
                                Some(edit) => Some(edit),
                                None if is_undecrypted(&shown.original) => extract_bundled_edit(&room, &value).await,
                                None => None,
                            };
                            let Some(updated) = build_thread_list_item_event(&room, value, edit, Some(&shown.sender_profile)).await else { continue };
                            modify_thread(&mut threads, index, &mut fetch_states, |thread| thread.set_root_or_latest_reply(is_root, updated));
                            is_changed = true;
                        }
                        // Only a sync appends new events, as others are older ones, e.g., from back-pagination.
                        VectorDiff::Append { values } if is_from_sync => new_events.extend(values.into_iter().map(|event| (event, false))),
                        VectorDiff::PushBack { value } if is_from_sync => new_events.push((value, false)),
                        _ => {}
                    }
                }
                for (event, is_late_edit) in new_events {
                    if let Some(target_id) = extract_redaction_target(event.raw(), &redaction_rules) {
                        if let Some((index, is_root)) = find_thread_event(&threads, |shown| shown.event_id == target_id) {
                            // We only hear about the redaction of an event the event cache holds in memory,
                            // so we redact the event ourselves.
                            let is_redacted = if let Some(shown) = threads[index].get_root_or_latest_reply(is_root)
                                && let Some(redacted) = apply_redaction(shown.original.raw(), event.raw().cast_ref_unchecked(), &redaction_rules)
                                && let Some(updated) = build_thread_list_item_event(&room, TimelineEvent::from_plaintext(redacted), None, Some(&shown.sender_profile)).await
                            {
                                modify_thread(&mut threads, index, &mut fetch_states, |thread| thread.set_root_or_latest_reply(is_root, updated));
                                is_changed = true;
                                true
                            } else {
                                false
                            };
                            // A redacted reply also changes its thread's reply count and latest reply, which only the server knows.
                            if !is_root || !is_redacted {
                                roots_to_fetch.insert(threads[index].root_event.event_id.clone());
                            }
                        } else if let Some((index, is_root)) = find_thread_event(&threads, |shown| shown.edit.as_ref().is_some_and(|edit| edit.event_id == target_id)) {
                            // A redacted edit no longer applies, so we show its event as it was sent
                            // until the server tells us about any earlier edit.
                            if let Some(shown) = threads[index].get_root_or_latest_reply(is_root)
                                && let Some(updated) = build_thread_list_item_event(&room, shown.original.clone(), None, Some(&shown.sender_profile)).await
                            {
                                modify_thread(&mut threads, index, &mut fetch_states, |thread| thread.set_root_or_latest_reply(is_root, updated));
                                is_changed = true;
                            }
                            roots_to_fetch.insert(threads[index].root_event.event_id.clone());
                        } else if let Some(root_id) = counted_replies.remove(&target_id) {
                            roots_to_fetch.insert(root_id);
                        } else {
                            has_redaction_in_unknown_thread = true;
                        }
                        continue;
                    }
                    let Some((relation_type, related_id)) = extract_relation(event.raw()) else { continue };
                    if matches!(relation_type, RelationType::Replacement) {
                        let Some((index, is_root)) = find_thread_event(&threads, |shown| shown.event_id == related_id) else { continue };
                        let Some(shown) = threads[index].get_root_or_latest_reply(is_root) else { continue };
                        let Some(edit) = Edit::new(event.raw().clone(), event.encryption_info().cloned()) else { continue };
                        // Decrypting an edit late can reveal it after a newer edit was applied.
                        if is_late_edit && shown.edit.as_ref().is_some_and(|applied| applied.timestamp > edit.timestamp) { continue }
                        // An edit that isn't valid for its event (e.g., from another sender) is ignored.
                        let Some(updated) = build_thread_list_item_event(&room, shown.original.clone(), Some(edit), Some(&shown.sender_profile)).await
                            .filter(|updated| updated.edit.is_some())
                        else { continue };
                        modify_thread(&mut threads, index, &mut fetch_states, |thread| thread.set_root_or_latest_reply(is_root, updated));
                        is_changed = true;
                        continue;
                    }
                    if !matches!(relation_type, RelationType::Thread) { continue }
                    let root_id = related_id;
                    let Some(timestamp) = event.timestamp() else { continue };
                    match threads.iter().position(|t| t.root_event.event_id == root_id) {
                        Some(index) => {
                            let Some(event_id) = event.event_id() else { continue };
                            let thread = &threads[index];
                            let latest = thread.latest_event.as_ref();
                            let is_latest = latest.is_some_and(|latest| latest.event_id == event_id);
                            // A reply delivered again changes nothing, unlike the echo of our own reply,
                            // which has the server's timestamp instead of our clock's.
                            if is_latest && latest.is_some_and(|latest| latest.original.raw().json().get() == event.raw().json().get()) { continue }
                            // The server's summary already counts the replies up to its latest one.
                            let is_new = !is_latest && timestamp > thread.summary_timestamp && !counted_replies.contains_key(event_id);
                            let is_newer = is_latest || timestamp > get_latest_activity(thread);
                            if !is_new && !is_newer { continue }
                            // Timestamps can't always tell a new reply from one the server already counted,
                            // so the server confirms our count.
                            if is_new {
                                roots_to_fetch.insert(root_id.clone());
                            }
                            let edit = extract_bundled_edit(&room, &event).await;
                            let Some(reply) = build_thread_list_item_event(&room, event, edit, None).await else { continue };
                            modify_thread(&mut threads, index, &mut fetch_states, |thread| {
                                if is_new {
                                    counted_replies.insert(reply.event_id.clone(), root_id);
                                    thread.num_replies += 1;
                                }
                                if is_newer {
                                    thread.latest_event = Some(reply);
                                }
                            });
                            is_changed = true;
                        }
                        // A reply newer than every loaded thread's latest activity (or before any thread loads) is in
                        // a thread that just became active, as any thread on a page we haven't loaded yet is older.
                        None if was_end_reached || threads.last().is_none_or(|oldest| timestamp > get_latest_activity(oldest)) => {
                            roots_to_fetch.insert(root_id);
                        }
                        None => {}
                    }
                }
                for root_id in roots_to_fetch {
                    request_thread_fetch(&room, root_id, &mut fetch_states, &mut root_fetches);
                }
                // The threads that changed in the events we missed are back on the first page, with fresh summaries.
                // We reload as many threads as we had loaded, once any page that's loading now is done.
                if missed_events {
                    if next_page_future.is_terminated() {
                        next_page_future = load_threads_page(&room, None, Some(threads.len()));
                    } else {
                        is_reload_pending = true;
                    }
                }
                if is_changed {
                    Arc::make_mut(&mut threads).sort_by_key(|t| Reverse(get_latest_activity(t)));
                }
                is_changed
            }

            _ = load_more_notifier.notified() => {
                if next_page_future.is_terminated() && !was_end_reached {
                    next_page_future = load_threads_page(&room, next_page_token.clone(), None);
                }
                false
            }

            _ = resend_notifier.notified() => true,
        };
    }
}

/// A thread in a room's list of threads: its root message and the server's summary of its replies.
#[derive(Clone, Debug)]
pub struct ThreadListItem {
    pub root_event: ThreadListItemEvent,
    /// The latest reply in the thread, or `None` if it has no replies (or the latest couldn't be parsed).
    pub latest_event: Option<ThreadListItemEvent>,
    /// The number of replies in the thread, as counted by the server plus any that arrived since.
    pub num_replies: u32,
    /// When the server's summary of the thread was taken, i.e., when its latest reply (or its root) was sent.
    summary_timestamp: MilliSecondsSinceUnixEpoch,
}

impl ThreadListItem {
    /// Returns the thread's root (if `is_root`) or its latest reply.
    fn get_root_or_latest_reply(&self, is_root: bool) -> Option<&ThreadListItemEvent> {
        if is_root { Some(&self.root_event) } else { self.latest_event.as_ref() }
    }

    fn set_root_or_latest_reply(&mut self, is_root: bool, event: ThreadListItemEvent) {
        if is_root { self.root_event = event } else { self.latest_event = Some(event) }
    }
}

/// An event in a room's list of threads: a thread's root message, or its latest reply.
#[derive(Clone, Debug)]
pub struct ThreadListItemEvent {
    pub event_id: OwnedEventId,
    pub timestamp: MilliSecondsSinceUnixEpoch,
    pub sender: OwnedUserId,
    pub sender_profile: TimelineDetails<Profile>,
    /// The event's content with its latest edit applied, or `None` if it couldn't be parsed.
    pub content: Option<TimelineItemContent>,
    /// The event as we got it, which later edits and redactions apply to.
    original: TimelineEvent,
    /// The edit applied to the event's content, if any.
    edit: Option<Edit>,
}

/// An edit of an event, i.e., a replacement event that gives it new content.
#[derive(Clone, Debug)]
struct Edit {
    event_id: OwnedEventId,
    timestamp: MilliSecondsSinceUnixEpoch,
    raw: Raw<AnySyncTimelineEvent>,
    encryption_info: Option<Arc<EncryptionInfo>>,
}

impl Edit {
    fn new(raw: Raw<AnySyncTimelineEvent>, encryption_info: Option<Arc<EncryptionInfo>>) -> Option<Self> {
        let event_id = raw.get_field::<OwnedEventId>("event_id").ok()??;
        let timestamp = raw.get_field::<MilliSecondsSinceUnixEpoch>("origin_server_ts").ok()??;
        Some(Self { event_id, timestamp, raw, encryption_info })
    }
}

/// Extracts the edit the server bundled with the given event, if any.
async fn extract_bundled_edit(room: &Room, event: &TimelineEvent) -> Option<Edit> {
    let unsigned = event.raw().get_field::<Raw<serde_json::Value>>("unsigned").ok()??;
    let relations = unsigned.get_field::<Raw<serde_json::Value>>("m.relations").ok()??;
    let raw = relations.get_field::<Raw<AnySyncTimelineEvent>>("m.replace").ok()??;
    // The SDK decrypts an event's bundled edit along with it, but not the edit bundled with a thread's latest reply.
    if raw.get_field::<String>("type").ok()?.as_deref() == Some("m.room.encrypted") {
        let decrypted = room.decrypt_event(raw.cast_ref_unchecked(), None).await.ok()?;
        return Edit::new(decrypted.raw().clone(), decrypted.encryption_info().cloned());
    }
    let encryption_info = match &event.kind {
        TimelineEventKind::Decrypted(decrypted) => decrypted.unsigned_encryption_info.as_ref()
            .and_then(|infos| infos.get(&UnsignedEventLocation::RelationsReplace))
            .and_then(|result| match result {
                UnsignedDecryptionResult::Decrypted(info) => Some(info.clone()),
                _ => None,
            }),
        _ => None,
    };
    Edit::new(raw, encryption_info)
}

/// A thread's root that we fetched, along with the server's summary of the thread.
struct FetchedThread {
    root_id: OwnedEventId,
    /// The thread, or `Ok(None)` if its root isn't (or no longer is) a thread, or couldn't be parsed.
    thread: Result<Option<ThreadListItem>, Error>,
}

/// The state of a thread's fetch from the server, which is forgotten once the fetch completes.
#[derive(Clone, Copy, PartialEq, Eq)]
enum FetchState {
    InFlight,
    /// The thread changed since we asked, so the fetch might not include the change.
    Outdated,
}

/// Starts fetching the given thread from the server, or marks its fetch as out of date if one is already underway.
fn request_thread_fetch(
    room: &Room,
    root_id: OwnedEventId,
    fetch_states: &mut HashMap<OwnedEventId, FetchState>,
    root_fetches: &mut FuturesUnordered<BoxFuture<'static, FetchedThread>>,
) {
    match fetch_states.entry(root_id) {
        Entry::Occupied(mut entry) => { entry.insert(FetchState::Outdated); }
        Entry::Vacant(entry) => {
            let (room, root_id) = (room.clone(), entry.key().clone());
            root_fetches.push(async move {
                let thread = match room.event(&root_id, None).await {
                    Ok(root) => Ok(build_thread_list_item(&room, root, &HashMap::new()).await),
                    Err(error) => Err(error),
                };
                FetchedThread { root_id, thread }
            }.boxed());
            entry.insert(FetchState::InFlight);
        }
    }
}

/// Applies the given change to the loaded thread at the given index, and marks any fetch of that thread
/// as out of date, since the fetch might not include the change.
fn modify_thread(
    threads: &mut Arc<Vec<Arc<ThreadListItem>>>,
    index: usize,
    fetch_states: &mut HashMap<OwnedEventId, FetchState>,
    change: impl FnOnce(&mut ThreadListItem),
) {
    let thread = Arc::make_mut(&mut Arc::make_mut(threads)[index]);
    if let Some(state) = fetch_states.get_mut(&thread.root_event.event_id) {
        *state = FetchState::Outdated;
    }
    change(thread);
}

/// Returns the index of the loaded thread whose root or latest reply matches the given predicate,
/// and whether that's its root.
fn find_thread_event(
    threads: &[Arc<ThreadListItem>],
    is_match: impl Fn(&ThreadListItemEvent) -> bool,
) -> Option<(usize, bool)> {
    threads.iter().enumerate().find_map(|(index, thread)| {
        if is_match(&thread.root_event) {
            Some((index, true))
        } else {
            thread.latest_event.as_ref().is_some_and(&is_match).then_some((index, false))
        }
    })
}

/// Returns when the given thread was last active, i.e., when its latest reply (or its root) was sent.
fn get_latest_activity(thread: &ThreadListItem) -> MilliSecondsSinceUnixEpoch {
    thread.latest_event.as_ref().unwrap_or(&thread.root_event).timestamp
}

/// Adds the given thread to the given list, or updates the list's copy of it if that has an older summary.
/// A thread we fetched always replaces it, since we drop an outdated fetch of a loaded thread.
fn merge_thread(list: &mut Vec<Arc<ThreadListItem>>, thread: ThreadListItem, is_fetched: bool) {
    match list.iter_mut().find(|t| t.root_event.event_id == thread.root_event.event_id) {
        Some(loaded) if is_fetched || loaded.summary_timestamp < thread.summary_timestamp => *loaded = Arc::new(thread),
        Some(_) => {}
        None => list.push(Arc::new(thread)),
    }
}

/// Loads the page of the room's threads at the given token, or its first page if `None`,
/// with up to `limit` threads, or the server's default number of them.
fn load_threads_page(room: &Room, from: Option<String>, limit: Option<usize>) -> Fuse<BoxFuture<'static, Result<ThreadsPage, Error>>> {
    let room = room.clone();
    let limit = limit.filter(|limit| *limit > 0).map(|limit| UInt::try_from(limit).unwrap_or(UInt::MAX));
    async move {
        let thread_roots = room.list_threads(ListThreadsOptions { from: from.clone(), limit, ..Default::default() }).await?;
        // The same few people start most threads, so we load each sender's profile once per page.
        let room = &room;
        let senders: HashSet<OwnedUserId> = thread_roots.chunk.iter()
            .flat_map(|root| [root.sender(), root.bundled_latest_thread_event().and_then(|latest| latest.sender())])
            .flatten()
            .collect();
        let profiles: HashMap<OwnedUserId, TimelineDetails<Profile>> = join_all(senders.into_iter().map(|sender| async move {
            let profile = TimelineDetails::from_initial_value(Profile::load(room, &sender).await);
            (sender, profile)
        })).await.into_iter().collect();
        let threads = join_all(thread_roots.chunk.into_iter().map(|root| build_thread_list_item(room, root, &profiles))).await;
        Ok(ThreadsPage {
            from,
            threads_chunk: threads.into_iter().flatten().collect(),
            token_next_page: thread_roots.prev_batch_token,
        })
    }.boxed().fuse()
}

/// Returns the reply count and latest reply that the server bundled with the given thread root,
/// or `None` if it has no bundled thread, e.g., because all of its replies were redacted.
pub async fn get_bundled_thread_summary(room: &Room, root: &TimelineEvent) -> Option<(u32, Option<TimelineEvent>)> {
    let num_replies = root.thread_summary()?.num_replies;
    let mut latest_reply = root.bundled_latest_thread_event();
    // The SDK doesn't decrypt the latest reply bundled with a root it couldn't decrypt.
    if let Some(latest) = &latest_reply
        && matches!(latest.kind, TimelineEventKind::UnableToDecrypt { .. })
        && let Ok(decrypted) = room.decrypt_event(latest.raw().cast_ref_unchecked(), None).await
    {
        latest_reply = Some(decrypted);
    }
    Some((num_replies, latest_reply))
}

/// Builds a list item from the given thread root event and its bundled summary, or returns `None` if it isn't
/// a thread or couldn't be parsed. Senders' profiles are loaded unless given in `profiles`.
async fn build_thread_list_item(
    room: &Room,
    root: TimelineEvent,
    profiles: &HashMap<OwnedUserId, TimelineDetails<Profile>>,
) -> Option<ThreadListItem> {
    let (num_replies, latest_reply) = get_bundled_thread_summary(room, &root).await?;
    let latest_event = match latest_reply {
        Some(latest) => {
            let edit = extract_bundled_edit(room, &latest).await;
            let profile = latest.sender().and_then(|sender| profiles.get(&sender));
            build_thread_list_item_event(room, latest, edit, profile).await
        }
        None => None,
    };
    let edit = extract_bundled_edit(room, &root).await;
    let profile = root.sender().and_then(|sender| profiles.get(&sender));
    let root_event = build_thread_list_item_event(room, root, edit, profile).await?;
    let summary_timestamp = latest_event.as_ref().unwrap_or(&root_event).timestamp;
    Some(ThreadListItem { root_event, latest_event, num_replies, summary_timestamp })
}

/// Builds the given event (a thread's root or one of its replies) as it's shown in a list of threads,
/// with the given edit applied if it's valid for the event, and its sender's profile loaded unless given.
async fn build_thread_list_item_event(
    room: &Room,
    event: TimelineEvent,
    edit: Option<Edit>,
    sender_profile: Option<&TimelineDetails<Profile>>,
) -> Option<ThreadListItemEvent> {
    let event_id = event.event_id()?.to_owned();
    let timestamp = event.timestamp()?;
    let sender = event.sender()?;
    // An edit must come from the event's sender, and can't bring back a redacted event's content.
    let edited_event = edit.as_ref()
        .filter(|edit| check_validity_of_replacement_events(
            event.raw(),
            event.encryption_info().map(|info| &**info),
            &edit.raw,
            edit.encryption_info.as_deref(),
        ).is_ok())
        .filter(|_| !event.raw().deserialize().is_ok_and(|event| event.is_redacted()))
        .and_then(|edit| {
            let new_content = edit.raw.get_field::<serde_json::Value>("content").ok()??.get_mut("m.new_content")?.take();
            let mut json = event.raw().deserialize_as::<serde_json::Map<String, serde_json::Value>>().ok()?;
            json.insert(String::from("content"), new_content);
            Some(TimelineEvent::from_plaintext(Raw::new(&json).ok()?.cast_unchecked()))
        });
    // An edit of an event we can't decrypt yet is kept, to apply once we can.
    let is_undecrypted = matches!(event.kind, TimelineEventKind::UnableToDecrypt { .. });
    let edit = edit.filter(|edit| edited_event.is_some()
        || is_undecrypted && edit.raw.get_field::<OwnedUserId>("sender").ok().flatten().as_ref() == Some(&sender)
    );
    let sender_profile = match sender_profile {
        Some(profile) => profile.clone(),
        None => TimelineDetails::from_initial_value(Profile::load(room, &sender).await),
    };
    let content = TimelineItemContent::from_event(room, edited_event.unwrap_or_else(|| event.clone())).await;
    Some(ThreadListItemEvent { event_id, timestamp, sender, sender_profile, content, original: event, edit })
}
