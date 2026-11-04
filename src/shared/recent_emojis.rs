//! `RecentEmojis`: the emoji that the user picked recently, which the emoji picker
//! shows first and the `:shortcode:` autocomplete ranks higher.
//!
//! We combine two lists of picks:
//! 1. our own record of the emoji that the user picked in Robrix, which we save in the `AppState`.
//! 2. the account's `m.recent_emoji` (and Element's older `io.element.recent_emoji`) account data,
//!    which other clients also update.

use std::time::Duration;
use emojis::{Emoji, SkinTone};
use futures_util::StreamExt;
use makepad_widgets::{Cx, error};
use matrix_sdk::{
    Client, HttpError,
    ruma::{OwnedUserId, UInt, api::error::ErrorKind, events::{GlobalAccountDataEvent, recent_emoji::RecentEmojiEventContent}},
};
use matrix_sdk_base::recent_emojis::RecentEmojisContent;
use matrix_sdk_ui::sync_service::State;
use rustc_hash::{FxHashMap, FxHashSet};
use serde::{Deserialize, Serialize};
use tokio::{runtime::Handle, sync::mpsc::UnboundedSender, task::JoinHandle};
use crate::{
    shared::emoji_catalog::{can_draw, without_skin_tone},
    sliding_sync::{MatrixRequest, get_client, get_sync_service, submit_async_request},
};

/// How long we wait to try adding a pick to the account data again, if it failed while syncing still seemed to work.
const RETRY_DELAY: Duration = Duration::from_secs(5);

/// Our own record of the user's emoji picks, which we save as part of the `AppState`.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RecentEmojis {
    /// Each emoji that the user picked in Robrix and how many times,
    /// from most to least recently picked.
    #[serde(default, deserialize_with = "crate::utils::deserialize_or_default")]
    picked: Vec<(String, u64)>,
    /// The skin tone that the user chose for emoji that have one, if they chose one.
    #[serde(default, deserialize_with = "crate::utils::deserialize_or_default")]
    skin_tone: Option<SkinTone>,
}

/// The recent emoji of the logged-in user, shared by every emoji picker and text input.
#[derive(Default)]
struct RecentEmojisGlobal {
    user_id: Option<OwnedUserId>,
    saved: RecentEmojis,
    /// The account's recent emoji, from most to least recently used.
    from_account_data: Vec<(String, u64)>,
    /// The picks that the account's recent emoji don't count yet, from most to least recent,
    /// along with the count that the account will have for each one once it does.
    pending_picks: Vec<(String, u64)>,
}

impl RecentEmojisGlobal {
    /// Returns the recent emoji of the given user, first clearing any other user's.
    fn get_or_reset_for_user<'a>(cx: &'a mut Cx, user_id: &OwnedUserId) -> &'a mut Self {
        let global = cx.global::<Self>();
        // A different user logged in, so the old user's picks no longer apply.
        if global.user_id.as_ref() != Some(user_id) {
            *global = Self { user_id: Some(user_id.clone()), ..Default::default() };
        }
        global
    }
}

/// An action that the matrix worker posts about the given user's recent emoji.
///
/// This is NOT a widget action.
#[derive(Debug)]
pub enum RecentEmojisAction {
    /// The account's recent emoji are now the given list, from most to least recently used.
    Updated { user_id: OwnedUserId, recent_emoji: Vec<(String, u64)> },
    /// We couldn't add the given emoji to the account's recent emoji.
    PickNotSaved { user_id: OwnedUserId, emoji: String },
}

/// Records that the user just picked the given emoji, both in our own record and in their account data.
pub fn record_emoji_pick(cx: &mut Cx, emoji: &str) {
    let global = cx.global::<RecentEmojisGlobal>();
    let picked_count = global.saved.picked.iter()
        .position(|(recent, _)| recent == emoji)
        .map_or(0, |index| global.saved.picked.remove(index).1);
    global.saved.picked.insert(0, (emoji.to_owned(), picked_count + 1));
    let account_count = global.pending_picks.iter()
        .chain(&global.from_account_data)
        .find_map(|(recent, count)| (recent == emoji).then_some(*count))
        .unwrap_or(0);
    global.pending_picks.retain(|(pending, _)| pending != emoji);
    global.pending_picks.insert(0, (emoji.to_owned(), account_count + 1));
    submit_async_request(MatrixRequest::AddRecentEmoji { emoji: emoji.to_owned() });
}

/// Returns the user's recently picked emoji that we can draw, from most to least recently picked,
/// along with how many times they picked each one.
pub fn get_recent_emojis(cx: &mut Cx) -> Vec<(&'static Emoji, u64)> {
    let global = cx.global::<RecentEmojisGlobal>();
    let picked_counts: FxHashMap<&str, u64> = global.saved.picked.iter()
        .map(|(emoji, count)| (emoji.as_str(), *count))
        .collect();
    // Our pending picks are newer than any of the account's emoji, while our other picks
    // that the account doesn't have (e.g., ones it no longer lists) come after all of its emoji.
    let account_emojis = global.pending_picks.iter()
        .chain(&global.from_account_data)
        .map(|(emoji, count)| (emoji, (*count).max(picked_counts.get(emoji.as_str()).copied().unwrap_or(0))));
    let mut seen = FxHashSet::default();
    account_emojis
        .chain(global.saved.picked.iter().map(|(emoji, count)| (emoji, *count)))
        .filter_map(|(emoji, count)| Some((emojis::get(emoji).filter(|emoji| can_draw(emoji))?, count)))
        .filter(|(emoji, _)| seen.insert(emoji.as_str()))
        .collect()
}

/// Returns up to `max` of the user's most recently picked emoji, with the ones they picked most often first.
pub fn get_frequently_used_emojis(cx: &mut Cx, max: usize) -> Vec<&'static Emoji> {
    let mut recent_emojis = get_recent_emojis(cx);
    recent_emojis.truncate(max);
    // Like Element, we take the most recently used ones and show the most used of those first.
    recent_emojis.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
    recent_emojis.into_iter().map(|(emoji, _)| emoji).collect()
}

/// Returns how many times the user picked each emoji, in any skin tone.
///
/// The key is the emoji in its default skin tone, and the value is that count.
pub fn get_pick_counts(cx: &mut Cx) -> FxHashMap<&'static str, u64> {
    let mut pick_counts = FxHashMap::default();
    for (emoji, count) in get_recent_emojis(cx) {
        *pick_counts.entry(without_skin_tone(emoji).as_str()).or_default() += count;
    }
    pick_counts
}

pub fn get_skin_tone(cx: &mut Cx) -> SkinTone {
    cx.global::<RecentEmojisGlobal>().saved.skin_tone.unwrap_or(SkinTone::Default)
}

pub fn set_skin_tone(cx: &mut Cx, skin_tone: SkinTone) {
    cx.global::<RecentEmojisGlobal>().saved.skin_tone = Some(skin_tone);
}

/// Returns our own record of the given user's picks, to be saved in the `AppState`.
pub fn save_recent_emojis(cx: &mut Cx, user_id: &OwnedUserId) -> RecentEmojis {
    let global = cx.global::<RecentEmojisGlobal>();
    if global.user_id.as_ref() == Some(user_id) {
        global.saved.clone()
    } else {
        RecentEmojis::default()
    }
}

/// Restores our own record of the given user's picks, as loaded from the `AppState`.
pub fn restore_recent_emojis(cx: &mut Cx, user_id: &OwnedUserId, saved: RecentEmojis) {
    RecentEmojisGlobal::get_or_reset_for_user(cx, user_id).saved = saved;
}

pub fn handle_recent_emojis_action(cx: &mut Cx, action: &RecentEmojisAction) {
    match action {
        RecentEmojisAction::Updated { user_id, recent_emoji } => {
            let global = RecentEmojisGlobal::get_or_reset_for_user(cx, user_id);
            // A pick is no longer pending once the account counts it.
            global.pending_picks.retain(|(pending, expected_count)| {
                !recent_emoji.iter().any(|(emoji, count)| emoji == pending && count >= expected_count)
            });
            global.from_account_data = recent_emoji.clone();
        }
        RecentEmojisAction::PickNotSaved { user_id, emoji } => {
            let global = cx.global::<RecentEmojisGlobal>();
            if global.user_id.as_ref() == Some(user_id) {
                global.pending_picks.retain(|(pending, _)| pending != emoji);
            }
        }
    }
}

pub fn clear_recent_emojis(cx: &mut Cx) {
    *cx.global::<RecentEmojisGlobal>() = RecentEmojisGlobal::default();
}

/// Posts the account's recent emoji now and again every time they change.
///
/// This function spawns a task that captures a strong `Client` ref,
/// so the caller should abort+await it upon logout to ensure the Client gets dropped.
pub fn handle_recent_emojis_subscriber(client: Client) -> JoinHandle<()> {
    let observer = client.observe_events::<GlobalAccountDataEvent<RecentEmojiEventContent>, ()>();
    let legacy_observer = client.observe_events::<GlobalAccountDataEvent<RecentEmojisContent>, ()>();
    Handle::current().spawn(async move {
        let Some(user_id) = client.user_id().map(ToOwned::to_owned) else { return };
        let mut updates = observer.subscribe();
        let mut legacy_updates = legacy_observer.subscribe();
        // A restored session doesn't get unchanged account data synced again, so we read it from the store first.
        let account = client.account();
        let mut recent_emoji = account.account_data::<RecentEmojiEventContent>().await
            .ok()
            .flatten()
            .and_then(|raw| raw.deserialize().ok())
            .map(|content| get_recent_emoji_of(&content))
            .unwrap_or_default();
        let mut legacy_recent_emoji = account.account_data::<RecentEmojisContent>().await
            .ok()
            .flatten()
            .and_then(|raw| raw.deserialize().ok())
            .map(|content| content.recent_emoji)
            .unwrap_or_default();
        let get_count = |list: &[(String, UInt)], emoji: &str| list.iter()
            .find_map(|(recent, count)| (recent == emoji).then(|| u64::from(*count)))
            .unwrap_or(0);
        loop {
            // Like Element, we list the ones in `m.recent_emoji` first, then the ones only in the older
            // `io.element.recent_emoji`, each with the higher of its two counts.
            let merged_recent_emoji = recent_emoji.iter()
                .chain(legacy_recent_emoji.iter().filter(|(emoji, _)| !recent_emoji.iter().any(|(recent, _)| recent == emoji)))
                .map(|(emoji, _)| (emoji.clone(), get_count(&recent_emoji, emoji).max(get_count(&legacy_recent_emoji, emoji))))
                .collect();
            Cx::post_action(RecentEmojisAction::Updated { user_id: user_id.clone(), recent_emoji: merged_recent_emoji });
            tokio::select! {
                Some((event, ())) = updates.next() => recent_emoji = get_recent_emoji_of(&event.content),
                Some((event, ())) = legacy_updates.next() => legacy_recent_emoji = event.content.recent_emoji,
                else => break,
            }
        }
    })
}

fn get_recent_emoji_of(content: &RecentEmojiEventContent) -> Vec<(String, UInt)> {
    content.recent_emoji.iter().map(|recent| (recent.emoji.clone(), recent.total)).collect()
}

/// Spawns the task that adds each emoji the user picks to their account data, one at a time and in order,
/// and returns the sender for the user ID and emoji of each pick.
pub fn spawn_recent_emoji_writer() -> UnboundedSender<(OwnedUserId, String)> {
    let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel::<(OwnedUserId, String)>();
    Handle::current().spawn(async move {
        while let Some((user_id, emoji)) = receiver.recv().await {
            // Each list counts this pick only once, even if we have to try again.
            let (mut is_in_recent_emoji, mut is_in_legacy_recent_emoji) = (false, false);
            loop {
                // Skip picks that a previous account made before it logged out.
                let Some(client) = get_client().filter(|client| client.user_id() == Some(&user_id)) else { break };
                let account = client.account();
                let result: matrix_sdk::Result<()> = async {
                    if !is_in_recent_emoji {
                        let mut content = account.fetch_account_data_static::<RecentEmojiEventContent>().await?
                            .and_then(|raw| raw.deserialize().ok())
                            .unwrap_or_else(|| RecentEmojiEventContent::new(Vec::new()));
                        content.increment_emoji_total(&emoji);
                        account.set_account_data(content).await?;
                        is_in_recent_emoji = true;
                    }
                    if !is_in_legacy_recent_emoji {
                        // Element X and other clients that use the SDK only read the older list.
                        match account.add_recent_emoji(&emoji).await {
                            // The SDK can only add to an existing list, so we create the account's first one ourselves.
                            Err(e) if matches!(e.client_api_error_kind(), Some(ErrorKind::NotFound)) => {
                                account.set_account_data(RecentEmojisContent::new(vec![(emoji.clone(), UInt::from(1u8))])).await?;
                            }
                            other => other?,
                        }
                        is_in_legacy_recent_emoji = true;
                    }
                    Ok(())
                }.await;
                match result {
                    Ok(()) => break,
                    // We're offline, so we try again once we're syncing again, before adding any later picks.
                    Err(matrix_sdk::Error::Http(e)) if matches!(*e, HttpError::Reqwest(_)) => {
                        let Some(sync_service) = get_sync_service() else { break };
                        let mut state = sync_service.state();
                        if matches!(state.get(), State::Running) {
                            tokio::time::sleep(RETRY_DELAY).await;
                        } else {
                            while let Some(new_state) = state.next().await {
                                if matches!(new_state, State::Running) {
                                    break;
                                }
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to add {emoji} to the account's recent emoji: {e}");
                        Cx::post_action(RecentEmojisAction::PickNotSaved { user_id, emoji });
                        break;
                    }
                }
            }
        }
    });
    sender
}
