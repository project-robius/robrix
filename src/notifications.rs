//! System notifications for incoming messages and invites, via `robius-notifications`.
//!
//! The flow: we register a notification handler with the matrix-sdk `Client`,
//! which invokes it for every incoming event whose push-rule evaluation says
//! "notify" (so per-room modes like mute or mentions-only are respected
//! automatically, same as any other Matrix client). Each such event is fed
//! through a queue to one long-lived consumer task that shows it as a
//! per-room conversation notification, in arrival order. Interactions come
//! back from the OS: tapping opens the room, quick-replying sends a message,
//! and "Mark as read" sends a read receipt.
//!
//! Notifications are suppressed while the app is in the foreground with that
//! same room open, and any shown notification for a room is cleared as soon
//! as the user opens that room (here or on another device). Notifications
//! survive app restarts — like other messaging apps, they stick around until
//! the user addresses them — but a logout or an expired session clears them
//! (see [`on_session_ended`]).
//!
//! ## State design
//! Only three process-wide statics exist, following the same pattern as
//! `sliding_sync`'s `CLIENT`/`REQUEST_SENDER`: two cross-thread atomic flags,
//! and one [`Shared`] mutex for the little state that genuinely crosses
//! threads (UI thread, tokio, and OS notification callback threads).
//! Everything else — the catch-up watermark, dedupe ring, invite markers,
//! avatar file cache — is owned exclusively by the consumer task, lock-free;
//! the UI communicates with it via [`QueueItem`] control messages.

use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex, OnceLock,
    },
    time::{SystemTime, UNIX_EPOCH},
};

use makepad_widgets::{error, log, Cx};
use matrix_sdk::{Client, Room, RoomDisplayName, RoomState};
use matrix_sdk_base::deserialized_responses::RawAnySyncOrStrippedTimelineEvent;
use ruma::{
    api::client::receipt::create_receipt::v3::ReceiptType,
    events::{
        receipt::ReceiptThread,
        relation::Thread,
        room::encrypted::Relation as EncryptedRelation,
        room::message::{
            sanitize::remove_plain_reply_fallback, MessageType, Relation, RoomMessageEventContent,
        },
        AnySyncMessageLikeEvent, AnySyncTimelineEvent,
    },
    OwnedEventId, OwnedRoomId, OwnedUserId, RoomId, UserId,
};
use robius_notifications::{
    Action, Conversation, Interaction, InteractionKind, Notification, NotificationChannel,
    SettingsScope, Urgency,
};

use crate::{
    app::AppStateAction,
    room::BasicRoomDetails,
    settings::app_preferences::preferred_receipt_type,
    sliding_sync::{MatrixRequest, current_user_id, submit_async_request},
    utils::{RoomNameId, AVATAR_THUMBNAIL_FORMAT},
};

/// The notification channel (user-visible "category") for message notifications.
const MESSAGES_CHANNEL: (&str, &str) = ("messages", "Messages");
/// The channel for messages that mention/highlight the user, so they can be
/// tuned separately in the OS notification settings.
const MENTIONS_CHANNEL: (&str, &str) = ("mentions", "Mentions & keywords");
/// The notification channel for room invites.
const INVITES_CHANNEL: (&str, &str) = ("invites", "Invites");

/// Metadata keys we attach to notifications and read back from interactions.
const META_ROOM_ID: &str = "room_id";
const META_EVENT_ID: &str = "event_id";
const META_ROOM_NAME: &str = "room_name";
/// Which account the notification belongs to, so an interaction with a stale
/// notification can't act as a different (re-logged-in) account.
const META_USER_ID: &str = "user_id";
/// Set when the notified message lives in a thread, so a quick reply goes
/// back into that thread and tapping opens the thread's timeline.
const META_THREAD_ROOT: &str = "thread_root";

/// How many recently-notified event IDs to remember for dedup.
const RECENTLY_NOTIFIED_CAP: usize = 1024;

/// Whether system notifications are usable at all on this platform/build.
/// Cleared on unrecoverable errors, e.g. running unbundled on macOS.
static ENABLED: AtomicBool = AtomicBool::new(true);

/// Whether the app is currently foregrounded (window focused, on desktop);
/// mirrored from App lifecycle events so the consumer task can read it.
static IS_FOREGROUND: AtomicBool = AtomicBool::new(true);

/// The one shared-state static: everything that genuinely crosses threads.
/// Keep it small; per-notification bookkeeping belongs in [`Consumer`].
#[derive(Default)]
struct Shared {
    /// Handle to the matrix tokio runtime, for spawning interaction work.
    runtime: Option<tokio::runtime::Handle>,
    /// Feeds the consumer task; also carries UI control messages.
    queue: Option<tokio::sync::mpsc::UnboundedSender<QueueItem>>,
    /// The consumer task itself, aborted when a new session registers.
    consumer: Option<tokio::task::JoinHandle<()>>,
    /// The room currently open in the UI.
    current_room: Option<OwnedRoomId>,
    /// Interactions that arrived before the runtime was up (e.g. a quick
    /// reply on a notification that relaunched the app).
    pending_interactions: Vec<Interaction>,
    /// The channel each room's latest notification was posted under,
    /// so the per-room settings page opens the right category.
    notified_channels: HashMap<OwnedRoomId, &'static str>,
    /// Who the currently-registered session belongs to.
    session_user: Option<OwnedUserId>,
    /// Whether we've asked for notification permission in this app run.
    permission_requested: bool,
}

fn shared() -> &'static Mutex<Shared> {
    static SHARED: OnceLock<Mutex<Shared>> = OnceLock::new();
    SHARED.get_or_init(Mutex::default)
}

/// Work items for the consumer task.
enum QueueItem {
    /// A push-rule-matched incoming event to (maybe) show.
    Notification(matrix_sdk::sync::Notification, Room),
    /// The user opened this room in the UI.
    RoomOpened(OwnedRoomId),
    /// This room's pending invite was resolved (accepted/declined/rescinded).
    InviteResolved(OwnedRoomId),
    /// The notification ids still showing from a previous app run,
    /// reported by the OS shortly after registration.
    SeedActiveIds(Vec<String>),
}

fn enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

/// Disables all future notification attempts, logging the reason once.
fn disable(err: &robius_notifications::Error) {
    if ENABLED.swap(false, Ordering::Relaxed) {
        log!("System notifications are unavailable on this platform/build: {err}");
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Basic one-time notification setup; call this as early as possible at app
/// startup (before login), so that interactions that *launched* the app
/// (e.g., the user tapped a notification of a killed app) get delivered.
///
/// This does NOT request notification permission — that happens after a
/// successful login, in [`register_with_client`].
pub fn init_early() {
    // On Linux, notifications (and launcher badges) are attributed via the
    // app's .desktop file; "robrix" matches packaging/robrix.desktop.
    // (Not set on Windows: an unregistered AUMID would break toast
    // attribution there, whereas the crate's fallback works out of the box.)
    #[cfg(target_os = "linux")]
    robius_notifications::set_app_id("robrix");

    if let Err(e) = robius_notifications::set_interaction_handler(handle_interaction) {
        disable(&e);
    }
}

/// Registers the per-event notification handler with the given (logged-in)
/// client, and requests notification permission if this is the first login
/// of this app run. Must be called from within the matrix tokio runtime.
pub async fn register_with_client(client: &Client) {
    if !enabled() {
        return;
    }
    let new_user = client.session_meta().map(|meta| meta.user_id.clone());

    // Only notify for events newer than the newest already-synced event: the
    // rest is catch-up traffic the user has likely already seen elsewhere.
    // Comparing server timestamps to server timestamps sidesteps local clock
    // skew; a fresh login (empty store) falls back to the local clock.
    let mut since_ms = 0u64;
    for room in client.rooms() {
        if let Some(ts) = room.latest_event_timestamp() {
            let ts: u64 = ts.0.into();
            since_ms = since_ms.max(ts + 1);
        }
    }
    if since_ms == 0 {
        since_ms = now_ms();
    }

    // One long-lived consumer shows notifications in event order.
    let (sender, receiver) = tokio::sync::mpsc::unbounded_channel::<QueueItem>();
    let consumer = tokio::spawn(Consumer::new(since_ms).run(receiver));

    let (account_switched, request_permission) = {
        let mut shared = shared().lock().unwrap();
        let switched = shared
            .session_user
            .as_ref()
            .is_some_and(|prev| Some(prev) != new_user.as_ref());
        shared.session_user = new_user;
        shared.runtime = Some(tokio::runtime::Handle::current());
        shared.queue = Some(sender);
        // The previous session's consumer (if any) must not keep showing its
        // queued events under this session.
        if let Some(previous) = shared.consumer.replace(consumer) {
            previous.abort();
        }
        if switched {
            shared.notified_channels.clear();
        }
        let request = !shared.permission_requested;
        shared.permission_requested = true;
        (switched, request)
    };

    // A different account logged in: the previous account's notifications
    // must not linger (their interactions would act as the wrong user).
    // A same-user restart keeps them — they're still unaddressed.
    if account_switched {
        clear_all_notifications();
    }

    // Ask for notification permission now that the user is logged in
    // (once per app run; afterwards the OS just reports the standing state).
    if request_permission {
        if let Err(e) = robius_notifications::request_permission(|granted| match granted {
            Ok(true) => log!("Notification permission granted."),
            Ok(false) => {
                log!("Notification permission denied; system notifications won't be shown.")
            }
            Err(e) => log!("Failed to determine notification permission: {e}"),
        }) {
            disable(&e);
        }
    }

    client
        .register_notification_handler(|notification, room, _client| async move {
            // Hand off to the consumer task; never block the sync loop.
            if let Some(sender) = shared().lock().unwrap().queue.as_ref() {
                let _ = sender.send(QueueItem::Notification(notification, room));
            }
        })
        .await;

    // Let the consumer know which notifications survived from a previous run,
    // so e.g. a still-pending invite doesn't re-alert.
    let _ = robius_notifications::active_notification_ids(|ids| {
        if let (Ok(ids), Some(sender)) = (ids, shared().lock().unwrap().queue.as_ref()) {
            let _ = sender.send(QueueItem::SeedActiveIds(ids));
        }
    });

    // Deliver any interactions that arrived before we were ready
    // (e.g. a quick reply on a notification that relaunched the app).
    // The runtime handle was stored above, before this drain, so a concurrent
    // handle_interaction either sees the runtime or lands in this drain.
    let pending = std::mem::take(&mut shared().lock().unwrap().pending_interactions);
    for interaction in pending {
        handle_interaction(interaction);
    }

    // Surface eventual failures of queued sends (e.g. notification quick
    // replies) to the user; the send queue retries transient errors itself.
    let send_queue_errors = client.send_queue().subscribe_errors();
    tokio::spawn(watch_send_queue_errors(send_queue_errors));

    // Baseline the app badge on this session's actual unread state.
    refresh_app_badge();
    log!("Registered system notification handler with the Matrix client.");
}

/// Clears all notification state for the current session: call when the user
/// logs out or the session's token expires. (Merely quitting and restarting
/// the app deliberately does NOT clear notifications — they stay up until
/// the user addresses them, like other messaging apps.)
pub async fn on_session_ended() {
    let consumer = {
        let mut shared = shared().lock().unwrap();
        shared.session_user = None;
        shared.current_room = None;
        shared.pending_interactions.clear();
        shared.notified_channels.clear();
        shared.queue = None;
        // Clearing the runtime re-arms the park-and-drain path in
        // `handle_interaction`, so an interaction racing this teardown waits
        // for the next login instead of being dropped.
        shared.runtime = None;
        shared.consumer.take()
    };
    // Stop the consumer *before* clearing: `abort()` only takes effect at an
    // await point, so a task already inside `show()` would otherwise land a
    // notification after we'd cancelled everything, with nothing left to
    // clean it up (notifications now survive restarts by design).
    if let Some(consumer) = consumer {
        consumer.abort();
        let _ = consumer.await;
    }
    let _ = robius_notifications::cancel_all();
    let _ = robius_notifications::set_app_badge(0);
}

/// Mirrors the app's foreground state; called from App lifecycle events.
pub fn set_app_foreground(is_foreground: bool) {
    IS_FOREGROUND.store(is_foreground, Ordering::Relaxed);
}

/// Mirrors which room is currently open in the UI; called from the App's
/// `RoomFocused`/`FocusNone` action handlers. Opening a room also clears any
/// notification shown for it, like reading a conversation does elsewhere.
pub fn set_current_room(room_id: Option<OwnedRoomId>) {
    let queue = {
        let mut shared = shared().lock().unwrap();
        shared.current_room = room_id.clone();
        shared.queue.clone()
    };
    if let Some(room_id) = room_id {
        clear_room_notification(&room_id);
        // Tell the consumer (e.g. to re-arm the room's invite marker).
        if let Some(queue) = queue {
            let _ = queue.send(QueueItem::RoomOpened(room_id));
        }
    }
}

/// Whether the app is currently foregrounded (window focused, on desktop).
///
/// Used to avoid acting as though the user has seen what's on screen —
/// e.g. read receipts shouldn't be sent for messages that scrolled by while
/// the app was in the background.
pub fn is_app_foreground() -> bool {
    IS_FOREGROUND.load(Ordering::Relaxed)
}

/// Notifies that a room's pending invite was resolved (accepted, declined,
/// or rescinded, here or on another device): removes its invite notification
/// and re-arms the room so a genuine future re-invite notifies again.
pub fn on_invite_resolved(room_id: &RoomId) {
    if !enabled() {
        return;
    }
    let _ = robius_notifications::cancel(&invite_notification_id(room_id));
    if let Some(queue) = shared().lock().unwrap().queue.as_ref() {
        let _ = queue.send(QueueItem::InviteResolved(room_id.to_owned()));
    }
}

/// Notifies that a room is gone from the user's view (left, kicked, banned,
/// or superseded by an upgrade): drops everything we're showing for it.
pub fn on_room_removed(room_id: &RoomId) {
    clear_room_notification(room_id);
    on_invite_resolved(room_id);
}

/// Whether the app is foregrounded with the given room open in the UI.
fn is_room_currently_open(room_id: &RoomId) -> bool {
    IS_FOREGROUND.load(Ordering::Relaxed)
        && shared().lock().unwrap().current_room.as_deref() == Some(room_id)
}

/// Opens the OS's notification settings for the given room's conversation
/// (falling back to this app's channel/app-level settings page where
/// per-conversation settings don't exist). Returns `false` if this platform
/// has no notification settings UI to open.
pub fn open_room_notification_settings(room_id: &RoomId) -> bool {
    // Use whichever channel this room's notifications actually posted under.
    let channel_id = shared()
        .lock()
        .unwrap()
        .notified_channels
        .get(room_id)
        .copied()
        .unwrap_or(MESSAGES_CHANNEL.0);
    robius_notifications::open_notification_settings(SettingsScope::Conversation {
        channel_id: channel_id.to_owned(),
        conversation_id: room_id.to_string(),
    })
    .is_ok()
}

/// Removes any shown notification for the given room and forgets its
/// accumulated notification history.
///
/// Also called when a room has been read on *another* device, so notifications
/// the user has already seen elsewhere disappear here too.
pub fn clear_room_notification(room_id: &RoomId) {
    if !enabled() {
        return;
    }
    let _ = robius_notifications::cancel(&message_notification_id(room_id));
    let _ = robius_notifications::cancel(&invite_notification_id(room_id));
    robius_notifications::clear_conversation_history(room_id.as_str());
    // Reading a room should also walk the app badge back down (to 0, eventually).
    refresh_app_badge();
}

/// Removes every notification we've ever shown and zeroes the app badge.
fn clear_all_notifications() {
    let _ = robius_notifications::cancel_all();
    let _ = robius_notifications::set_app_badge(0);
}

fn message_notification_id(room_id: &RoomId) -> String {
    format!("room:{room_id}")
}

fn invite_notification_id(room_id: &RoomId) -> String {
    format!("invite:{room_id}")
}

/// Recomputes the app icon badge from how many rooms have unread
/// notifications right now (0 clears it). No-op where badges don't exist.
fn refresh_app_badge() {
    let count = crate::sliding_sync::get_client()
        .map(|client| {
            client
                .joined_rooms()
                .iter()
                .filter(|room| room.num_unread_notifications() > 0)
                .count() as u32
        })
        .unwrap_or(0);
    let _ = robius_notifications::set_app_badge(count);
}

/// Reports queued sends that ultimately failed (the send queue already
/// retries transient errors on its own).
async fn watch_send_queue_errors(
    mut errors: tokio::sync::broadcast::Receiver<matrix_sdk::send_queue::SendQueueRoomError>,
) {
    while let Ok(report) = errors.recv().await {
        error!("A queued message failed to send in room {}: {:?}",
            report.room_id, report.error);
        crate::shared::popup_list::enqueue_popup_notification(
            "A queued message could not be sent. It will not be retried automatically.",
            crate::shared::popup_list::PopupKind::Error,
            None,
        );
    }
}

/// The consumer task's private, lock-free state. One instance lives inside
/// the task spawned by [`register_with_client`]; nothing else can touch it.
struct Consumer {
    /// Only events with a server timestamp at/after this are notified.
    since_ms: u64,
    /// Recently-notified event IDs, for deduping sliding-sync re-delivery.
    /// A *suppressed* event counts as handled too.
    recently_notified: Vec<OwnedEventId>,
    /// Rooms whose pending invite has already been notified.
    notified_invites: HashSet<OwnedRoomId>,
    /// On-disk avatar files already written this run, per room.
    avatar_files: HashMap<OwnedRoomId, PathBuf>,
}

impl Consumer {
    fn new(since_ms: u64) -> Self {
        Self {
            since_ms,
            recently_notified: Vec::new(),
            notified_invites: HashSet::new(),
            avatar_files: HashMap::new(),
        }
    }

    async fn run(mut self, mut receiver: tokio::sync::mpsc::UnboundedReceiver<QueueItem>) {
        while let Some(item) = receiver.recv().await {
            match item {
                QueueItem::Notification(notification, room) => {
                    self.handle_sdk_notification(notification, room).await;
                }
                QueueItem::RoomOpened(room_id) | QueueItem::InviteResolved(room_id) => {
                    // The invite is no longer pending, so a future re-invite
                    // (after leaving/declining) can notify again.
                    self.notified_invites.remove(&room_id);
                }
                QueueItem::SeedActiveIds(ids) => {
                    // Invite notifications that survived a previous app run:
                    // mark them notified so they don't re-alert this run.
                    for id in ids {
                        if let Some(room_id) = id.strip_prefix("invite:") {
                            if let Ok(room_id) = OwnedRoomId::try_from(room_id) {
                                self.notified_invites.insert(room_id);
                            }
                        }
                    }
                }
            }
        }
    }

    /// Handles one push-rule-matched event from the sync loop.
    async fn handle_sdk_notification(
        &mut self,
        notification: matrix_sdk::sync::Notification,
        room: Room,
    ) {
        if !enabled() {
            return;
        }
        // A "highlight" means this message mentions the user (or a keyword).
        let is_mention = notification.actions.iter().any(|a| a.is_highlight());
        match &notification.event {
            RawAnySyncOrStrippedTimelineEvent::Sync(raw) => {
                let Ok(event) = raw.deserialize() else {
                    return;
                };
                self.handle_message_event(event, room, is_mention).await;
            }
            // Stripped events are invite-state events: notify about the invite.
            RawAnySyncOrStrippedTimelineEvent::Stripped(_) => {
                self.handle_invite(room).await;
            }
        }
    }

    async fn handle_message_event(
        &mut self,
        event: AnySyncTimelineEvent,
        room: Room,
        is_mention: bool,
    ) {
        let AnySyncTimelineEvent::MessageLike(event) = event else {
            // State events (e.g. tombstones) aren't worth a system notification.
            return;
        };

        // Never notify for our own messages (e.g. echoes from other devices).
        let sender = event.sender().to_owned();
        if current_user_id().is_some_and(|me| me == sender) {
            return;
        }
        // Ignore events from before this run started (initial sync catch-up).
        let event_ts: u64 = event.origin_server_ts().0.into();
        if event_ts < self.since_ms {
            return;
        }
        // Skip events we've already handled: sliding sync may re-deliver an
        // event (e.g. across timeline windows), and a *suppressed* event
        // counts as handled, so re-delivery can't notify about a seen message.
        // Only final outcomes are recorded, so an event whose show() failed
        // transiently still gets another chance when it's re-delivered.
        let event_id = event.event_id().to_owned();
        if self.recently_notified.contains(&event_id) {
            return;
        }

        // The user is looking right at this room; no need to notify.
        let room_id = room.room_id().to_owned();
        if is_room_currently_open(&room_id) {
            self.mark_handled(event_id);
            return;
        }
        // A live message in this room means any pending-invite marker is stale.
        self.notified_invites.remove(&room_id);

        let Some(body) = event_preview(&event) else {
            // Nothing worth showing for this event type; don't revisit it.
            self.mark_handled(event_id);
            return;
        };
        let thread_root = thread_root_of(&event);
        let sender_name = sender_display_name(&room, &sender).await;
        let room_name_id = RoomNameId::from_room(&room).await;
        let room_name = room_name_id.display().into_owned();
        let is_direct = room.is_direct().await.unwrap_or(false);
        let icon = self.room_avatar_file(&room).await;

        let mut conversation = Conversation::new(room_id.as_str(), &room_name)
            .set_group_conversation(!is_direct);
        if let Some(icon) = icon {
            conversation = conversation.set_icon(icon);
        }
        // Mentions get their own channel (separately tunable in OS settings)
        // and a more prominent urgency where urgency is per-notification.
        let (channel, urgency) = if is_mention {
            (MENTIONS_CHANNEL, Urgency::Critical)
        } else {
            (MESSAGES_CHANNEL, Urgency::Normal)
        };

        // The fetches above take time (the avatar can hit the network); the
        // user may have opened this room in the meantime, e.g. via the very
        // notification this one would replace. Re-check before showing.
        if is_room_currently_open(&room_id) {
            self.mark_handled(event_id);
            return;
        }

        let mut notification = Notification::new()
            .set_id(message_notification_id(&room_id))
            .set_title(&sender_name)
            .set_body(&body)
            .set_channel(NotificationChannel::new(channel.0, channel.1).set_importance(urgency))
            .set_urgency(urgency)
            .set_conversation(conversation)
            // Show the event's own timestamp, not the moment we processed it.
            .set_timestamp(UNIX_EPOCH + std::time::Duration::from_millis(event_ts))
            .set_badge_count(unread_room_count(room.client()))
            .add_action(Action::reply("reply", "Reply").set_placeholder("Reply"))
            .add_action(Action::button("mark-read", "Mark as read"))
            .add_metadata(META_ROOM_ID, room_id.as_str())
            .add_metadata(META_EVENT_ID, event_id.as_str())
            .add_metadata(META_ROOM_NAME, &room_name)
            .add_metadata(
                META_USER_ID,
                current_user_id().as_ref().map(|u| u.as_str()).unwrap_or(""),
            );
        if let Some(thread_root) = &thread_root {
            notification = notification.add_metadata(META_THREAD_ROOT, thread_root.as_str());
        }

        let result = notification.show();
        if handle_show_result(result) {
            self.mark_handled(event_id);
            // Remember the channel so the settings page opens the right category.
            shared()
                .lock()
                .unwrap()
                .notified_channels
                .insert(room_id.clone(), channel.0);
            if is_room_currently_open(&room_id) {
                // The user opened the room in the split second we showed it.
                clear_room_notification(&room_id);
            }
        }
        // A failed show leaves the event unmarked, so a re-delivery retries it.
    }

    /// Records an event as finally dealt with (shown or deliberately skipped),
    /// so a sliding-sync re-delivery of it is ignored.
    fn mark_handled(&mut self, event_id: OwnedEventId) {
        if self.recently_notified.len() >= RECENTLY_NOTIFIED_CAP {
            self.recently_notified.remove(0);
        }
        self.recently_notified.push(event_id);
    }

    /// Shows a notification for a pending room invite (once per pending
    /// invite; the marker is dropped when the invite resolves, so a later
    /// re-invite in the same run notifies again).
    async fn handle_invite(&mut self, room: Room) {
        if room.state() != RoomState::Invited {
            return;
        }
        let room_id = room.room_id().to_owned();
        if self.notified_invites.contains(&room_id) {
            return;
        }

        let room_name_id = RoomNameId::from_room(&room).await;
        let room_name = room_name_id.display().into_owned();
        // The inviter's member event usually isn't part of an invite's stripped state,
        // so fall back to their user ID.
        let inviter = match room.invite_details().await {
            Ok(details) => Some(
                details.inviter
                    .and_then(|m| m.display_name().map(str::to_owned))
                    .unwrap_or_else(|| details.inviter_id.to_string())
            ),
            Err(e) => {
                error!("Couldn't determine who invited us to room {room_id}: {e}");
                None
            }
        };
        let body = match inviter {
            Some(inviter) => format!("{inviter} invited you to join"),
            None => "You've been invited to join".to_owned(),
        };

        let result = Notification::new()
            .set_id(invite_notification_id(&room_id))
            .set_title(&room_name)
            .set_body(&body)
            .set_channel(NotificationChannel::new(INVITES_CHANNEL.0, INVITES_CHANNEL.1))
            .add_metadata(META_ROOM_ID, room_id.as_str())
            .add_metadata(META_ROOM_NAME, &room_name)
            .add_metadata(
                META_USER_ID,
                current_user_id().as_ref().map(|u| u.as_str()).unwrap_or(""),
            )
            .show();
        if handle_show_result(result) {
            // Only mark it notified on success, so a transient failure can
            // retry on the next invite-state delivery.
            self.notified_invites.insert(room_id);
        }
    }

    /// Fetches the room's avatar (if any) and writes it to a per-run cache
    /// file, so the notification can use it as the conversation icon.
    async fn room_avatar_file(&mut self, room: &Room) -> Option<PathBuf> {
        let room_id = room.room_id().to_owned();
        if let Some(path) = self.avatar_files.get(&room_id) {
            return Some(path.clone());
        }

        // Bound the fetch: one slow avatar shouldn't delay the (serialized)
        // notifications behind it. Uncached rooms just show without an icon.
        let bytes = tokio::time::timeout(
            std::time::Duration::from_secs(3),
            room.avatar(AVATAR_THUMBNAIL_FORMAT.into()),
        )
        .await
        .ok()?
        .ok()??;
        let dir = crate::temp_storage::get_temp_dir_path().join("notification_avatars");
        std::fs::create_dir_all(&dir).ok()?;
        let path = dir.join(sanitize_filename::sanitize(room_id.as_str()));
        std::fs::write(&path, &bytes).ok()?;
        self.avatar_files.insert(room_id, path.clone());
        Some(path)
    }
}

/// Returns whether the notification was shown successfully.
fn handle_show_result(result: robius_notifications::Result<()>) -> bool {
    match result {
        Ok(()) => true,
        Err(
            e @ (robius_notifications::Error::NoAppBundle
            | robius_notifications::Error::Unsupported),
        ) => {
            disable(&e);
            false
        }
        Err(e) => {
            error!("Failed to show system notification: {e}");
            false
        }
    }
}

/// How many rooms currently have unread notifications, for the app icon
/// badge. This event may not be counted yet, so the result is at least 1.
fn unread_room_count(client: Client) -> u32 {
    let count = client
        .joined_rooms()
        .iter()
        .filter(|room| room.num_unread_notifications() > 0)
        .count();
    (count.max(1)).min(u32::MAX as usize) as u32
}

/// The root event of the thread this message belongs to, if any.
fn thread_root_of(event: &AnySyncMessageLikeEvent) -> Option<OwnedEventId> {
    match event {
        AnySyncMessageLikeEvent::RoomMessage(ev) => {
            match ev.as_original()?.content.relates_to.as_ref()? {
                Relation::Thread(thread) => Some(thread.event_id.clone()),
                _ => None,
            }
        }
        // `m.relates_to` stays in the clear on encrypted events, so a
        // still-undecrypted message still knows which thread it's in.
        AnySyncMessageLikeEvent::RoomEncrypted(ev) => {
            match ev.as_original()?.content.relates_to.as_ref()? {
                EncryptedRelation::Thread(thread) => Some(thread.event_id.clone()),
                _ => None,
            }
        }
        _ => None,
    }
}

/// A short human-readable preview of a message-like event, or `None` for
/// event types that don't warrant a notification.
fn event_preview(event: &AnySyncMessageLikeEvent) -> Option<String> {
    match event {
        AnySyncMessageLikeEvent::RoomMessage(ev) => {
            let original = ev.as_original()?;
            // Rich replies (and thread replies from older clients) prefix the
            // body with a quote of the replied-to message; the notification
            // should show what was actually said, like the in-app preview does.
            let strip = |body: &str| remove_plain_reply_fallback(body).to_owned();
            Some(match &original.content.msgtype {
                MessageType::Text(c) => strip(&c.body),
                MessageType::Notice(c) => strip(&c.body),
                MessageType::Emote(c) => format!("* {}", strip(&c.body)),
                MessageType::Image(_) => "Sent an image".to_owned(),
                MessageType::Video(_) => "Sent a video".to_owned(),
                MessageType::Audio(_) => "Sent an audio message".to_owned(),
                MessageType::File(_) => "Sent a file".to_owned(),
                MessageType::Location(_) => "Shared a location".to_owned(),
                MessageType::VerificationRequest(_) => "Sent a verification request".to_owned(),
                _ => "Sent a message".to_owned(),
            })
        }
        // An encrypted event that couldn't be decrypted (yet).
        AnySyncMessageLikeEvent::RoomEncrypted(_) => Some("New message".to_owned()),
        AnySyncMessageLikeEvent::Sticker(_) => Some("Sent a sticker".to_owned()),
        AnySyncMessageLikeEvent::CallInvite(_) => Some("Incoming call".to_owned()),
        _ => None,
    }
}

/// The sender's display name, falling back to their user ID's localpart.
async fn sender_display_name(room: &Room, sender: &UserId) -> String {
    if let Ok(Some(member)) = room.get_member_no_sync(sender).await {
        if let Some(name) = member.display_name() {
            return name.to_owned();
        }
    }
    sender.localpart().to_owned()
}

/// Handles the user's interaction with one of our notifications.
/// Runs on an arbitrary OS/platform callback thread.
fn handle_interaction(interaction: Interaction) {
    // Interactions needing the Matrix client may arrive before the runtime is
    // up (e.g. a quick reply on a notification that relaunched the app):
    // park them, and register_with_client re-delivers them once ready.
    // One lock covers the check and the push, so a concurrent registration
    // (which stores the runtime and then drains pending, under the same lock)
    // can't slip in between.
    let needs_runtime = matches!(interaction.kind, InteractionKind::Reply { .. })
        || matches!(&interaction.kind, InteractionKind::Action { id } if id == "mark-read");
    let runtime = {
        let mut shared = shared().lock().unwrap();
        if needs_runtime && shared.runtime.is_none() {
            shared.pending_interactions.push(interaction);
            return;
        }
        shared.runtime.clone()
    };

    // Never act on a notification shown for a different account (e.g. one
    // that survived from before a logout + re-login).
    let stamped_user = interaction
        .metadata
        .iter()
        .find(|(k, _)| k == META_USER_ID)
        .map(|(_, v)| v.clone());
    if let (Some(stamped), Some(current)) = (&stamped_user, current_user_id()) {
        if !stamped.is_empty() && stamped != current.as_str() {
            log!("Ignoring notification interaction stamped for a different account.");
            return;
        }
    }

    let metadata = |key: &str| {
        interaction
            .metadata
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.clone())
    };
    let Some(room_id) = metadata(META_ROOM_ID).and_then(|id| OwnedRoomId::try_from(id).ok())
    else {
        return;
    };

    let thread_root = metadata(META_THREAD_ROOT)
        .and_then(|id| OwnedEventId::try_from(id).ok());

    match interaction.kind {
        // Tapping the notification opens the room (or the thread) in the app.
        InteractionKind::Activated => {
            navigate_to_room_from_notification(metadata(META_ROOM_NAME), room_id, thread_root);
        }
        // A quick reply sends a plain text message to the room.
        InteractionKind::Reply { text, .. } => {
            let text = text.trim().to_owned();
            if text.is_empty() {
                return;
            }
            spawn_on(runtime, async move {
                let Some(room) = room_for_id(&room_id) else {
                    return;
                };
                let mut content = RoomMessageEventContent::text_plain(text);
                // A reply to a threaded message belongs in that thread, not
                // in the room's main timeline.
                if let Some(thread_root) = thread_root {
                    content.relates_to = Some(Relation::Thread(Thread::without_fallback(
                        thread_root,
                    )));
                }
                // The send queue persists the message, retries after
                // reconnects, and gives the open timeline a proper local echo.
                match room.send_queue().send(content.into()).await {
                    Ok(_) => log!("Sent quick reply to room {room_id}."),
                    Err(e) => {
                        error!("Failed to send quick reply to room {room_id}: {e}");
                        crate::shared::popup_list::enqueue_popup_notification(
                            "Failed to send your reply from the notification.",
                            crate::shared::popup_list::PopupKind::Error,
                            None,
                        );
                    }
                }
            });
        }
        // On platforms without inline reply (e.g. most Linux daemons), the
        // Reply action degrades to a plain button: open the room instead.
        InteractionKind::Action { ref id } if id == "reply" => {
            navigate_to_room_from_notification(metadata(META_ROOM_NAME), room_id, thread_root);
        }
        // "Mark as read" marks the whole room read, since it also dismisses
        // the room's notifications. This matches the room context menu.
        InteractionKind::Action { id } if id == "mark-read" => {
            clear_room_notification(&room_id);
            submit_async_request(MatrixRequest::MarkRoomAsRead {
                room_id,
                receipt_type: preferred_receipt_type(),
            });
        }
        _ => {}
    }
}

/// Posts the navigation action that opens the given room in the UI, or the
/// thread the notified message lives in.
fn navigate_to_room_from_notification(
    room_name: Option<String>,
    room_id: OwnedRoomId,
    thread_root: Option<OwnedEventId>,
) {
    let room_name_id = match room_name {
        Some(name) if !name.is_empty() => RoomNameId::new(RoomDisplayName::Named(name), room_id),
        _ => RoomNameId::empty(room_id),
    };
    Cx::post_action(AppStateAction::NavigateToRoom {
        room_to_close: None,
        destination_room: BasicRoomDetails::Name(room_name_id),
        // The main timeline hides threaded events, so a thread message is
        // only visible if we open its thread.
        thread_root_event_id: thread_root,
    });
}

fn room_for_id(room_id: &RoomId) -> Option<Room> {
    let room = crate::sliding_sync::get_client()?.get_room(room_id);
    if room.is_none() {
        error!("Notification interaction targets unknown room {room_id}.");
    }
    room
}

/// Spawns async work on the matrix runtime, if it's up.
fn spawn_on(
    runtime: Option<tokio::runtime::Handle>,
    fut: impl std::future::Future<Output = ()> + Send + 'static,
) {
    match runtime {
        Some(handle) => {
            handle.spawn(fut);
        }
        None => error!("Ignoring notification interaction: Matrix runtime not started yet."),
    }
}
