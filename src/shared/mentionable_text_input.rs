//! MentionableTextInput component provides text input with @mention capabilities.
//!
//! Can be used in any context where user mentions are needed (message input, editing).
//!
//! # Architecture Overview
//!
//! This component uses a **state machine** pattern combined with **background thread execution**
//! to provide responsive @mention search functionality even in large rooms.
//!
//! ## State Machine
//!
//! The search functionality is driven by [`MentionSearchState`], which has four states:
//!
//! ```text
//! ┌──────────────────────────────────────────────────────────────────────────┐
//! │                         State Transitions                                │
//! │                                                                          │
//! │   ┌──────┐    user types @    ┌───────────────────┐                      │
//! │   │ Idle │ ─────────────────► │ WaitingForMembers │ (if no cached data)  │
//! │   └──────┘                    └─────────┬─────────┘                      │
//! │      ▲                                  │                                │
//! │      │                                  │ members loaded                 │
//! │      │                                  ▼                                │
//! │      │         ┌─────────────────────────────────────┐                   │
//! │      │         │            Searching                │                   │
//! │      │         │  - receiver: channel for results    │                   │
//! │      │         │  - accumulated_results: Vec<usize>  │                   │
//! │      │         │  - cancel_token: Arc<AtomicBool>    │                   │
//! │      │         └──────────────┬──────────────────────┘                   │
//! │      │                        │                                          │
//! │      │    ┌───────────────────┼───────────────────┐                      │
//! │      │    │                   │                   │                      │
//! │      │    ▼                   ▼                   ▼                      │
//! │      │  search           user selects         user presses               │
//! │      │  completes        a mention            ESC                        │
//! │      │    │                   │                   │                      │
//! │      │    │                   │                   ▼                      │
//! │      │    │                   │           ┌───────────────┐              │
//! │      │    │                   │           │ JustCancelled │              │
//! │      │    │                   │           └───────┬───────┘              │
//! │      │    │                   │                   │                      │
//! │      └────┴───────────────────┴───────────────────┘                      │
//! │                         reset to Idle                                    │
//! └──────────────────────────────────────────────────────────────────────────┘
//! ```
//!
//! - **Idle**: Default state, no active search
//! - **WaitingForMembers**: Triggered @ detected, waiting for room member data to load
//! - **Searching**: Background search task running, receiving streaming results via channel
//! - **JustCancelled**: ESC pressed, prevents immediate re-trigger on next keystroke
//!
//! ## Background Thread Execution
//!
//! To keep the UI responsive during searches in large rooms, the actual member search
//! is offloaded to a background thread via [`cpu_worker::spawn_cpu_job`]:
//!
//! ```text
//! ┌─────────────────────┐         ┌─────────────────────┐
//! │    UI Thread        │         │   Background Thread │
//! │                     │         │                     │
//! │  update_user_list() │         │                     │
//! │         │           │         │                     │
//! │         ▼           │         │                     │
//! │  spawn_cpu_job() ───┼────────►│  SearchRoomMembers  │
//! │         │           │         │         │           │
//! │         ▼           │         │         ▼           │
//! │  cx.new_next_frame()│         │  search members...  │
//! │         │           │         │         │           │
//! │         ▼           │  MPSC   │         ▼           │
//! │  check_search_      │◄────────┼─ send batch (10)    │
//! │  channel()          │ Channel │         │           │
//! │         │           │         │         ▼           │
//! │         ▼           │         │  send batch (10)    │
//! │  update UI with     │◄────────┼─        │           │
//! │  streaming results  │         │         ▼           │
//! │                     │         │  send completion    │
//! └─────────────────────┘         └─────────────────────┘
//! ```
//!
//! Key features:
//! - Results are streamed in batches of 10 for progressive UI updates
//! - Cancellation is supported via `Arc<AtomicBool>` token
//! - Each search has a unique `search_id` to ignore stale results
//!
//! ## Focus Management
//!
//! The component handles complex focus scenarios:
//! - `pending_popup_cleanup`: Defers popup closure when focus is lost during search
//! - `pending_draw_focus_restore`: Retries focus restoration in draw_walk until successful
//!
//! ## Key Components
//!
//! - [`SearchResult`]: Result type sent through the channel from background thread
//! - [`MentionSearchState`]: State machine enum managing search lifecycle
//! - [`MentionableTextInputAction`]: Actions for external communication (power levels, member updates)
//!
use crate::app::AppState;
use crate::avatar_cache::*;
use crate::i18n::{AppLanguage, tr_key};
use crate::shared::avatar::AvatarWidgetRefExt;
use crate::shared::bouncing_dots::BouncingDotsWidgetRefExt;
use crate::shared::styles::COLOR_UNKNOWN_ROOM_AVATAR;
use crate::utils;
use crate::cpu_worker::{self, CpuJob, SearchRoomMembersJob};
use crate::sliding_sync::{submit_async_request, MatrixRequest};

use makepad_widgets::{makepad_draw::text::selection::Cursor, *};
use matrix_sdk::ruma::{
    events::{room::message::RoomMessageEventContent, Mentions},
    OwnedRoomId, OwnedUserId,
};
use matrix_sdk::RoomMemberships;
use unicode_segmentation::UnicodeSegmentation;
use crate::home::room_screen::RoomScreenProps;
use crate::shared::command_text_input::CommandTextInput;
use crate::LivePtr;

// Channel types for member search communication
use std::sync::{mpsc::Receiver, Arc};
use std::sync::atomic::{AtomicBool, Ordering};

/// Result type for member search channel communication
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub search_id: u64,
    pub results: Vec<usize>, // indices in members vec
    pub is_complete: bool,
    pub search_text: Arc<String>,
}

/// State machine for mention search functionality
#[derive(Debug, Default)]
enum MentionSearchState {
    /// Not in search mode
    #[default]
    Idle,

    /// Waiting for room members data to be loaded
    WaitingForMembers {
        trigger_position: usize,
        pending_search_text: String,
    },

    /// Actively searching with background task
    Searching {
        trigger_position: usize,
        search_text: String,
        receiver: Receiver<SearchResult>,
        accumulated_results: Vec<usize>,
        search_id: u64,
        cancel_token: Arc<std::sync::atomic::AtomicBool>,
    },

    /// Search was just cancelled (prevents immediate re-trigger)
    JustCancelled,
}

// Default is derived above; Idle is marked as the default variant

// Constants for mention popup sizing and search behavior.
// MAX_DISPLAY_ITEMS: total items loaded into the scrollable list.
// MAX_SCROLL_HEIGHT: maximum pixel height of the scroll viewport.
const DESKTOP_MAX_DISPLAY_ITEMS: usize = 30;
const MOBILE_MAX_DISPLAY_ITEMS: usize = 15;
const DESKTOP_MAX_SCROLL_HEIGHT: f64 = 360.0; // ~10 user items
const MOBILE_MAX_SCROLL_HEIGHT: f64 = 216.0;  // ~6 user items
const SEARCH_BUFFER_MULTIPLIER: usize = 2;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PopupStatusItemKind {
    Loading,
    NoMatches,
}

fn popup_status_item_is_selectable(_kind: PopupStatusItemKind) -> bool {
    false
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum PopupMode {
    #[default]
    None,
    Mention,
    SlashCommand,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SlashCommand {
    command: &'static str,
    description_key: &'static str,
    needs_args: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ParsedSlashCommand {
    pub command: String,
    pub target_localpart: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TrackedVisibleMention {
    user_id: OwnedUserId,
    visible_text: String,
    start: usize,
    end: usize,
}

const MENTION_POPUP_HEADER_TEXT: &str = "Users in this Room";
const SLASH_COMMANDS: &[SlashCommand] = &[
    SlashCommand {
        command: "/createbot",
        description_key: "slash_command.createbot.description",
        needs_args: true,
    },
    SlashCommand {
        command: "/deletebot",
        description_key: "slash_command.deletebot.description",
        needs_args: true,
    },
    SlashCommand {
        command: "/listbots",
        description_key: "slash_command.listbots.description",
        needs_args: false,
    },
    SlashCommand {
        command: "/bothelp",
        description_key: "slash_command.bothelp.description",
        needs_args: false,
    },
];

/// agent-chat demo workflow commands. Unlike the bot commands above, robrix2 does
/// NOT handle these on submit — they are plain text that the `wf_coordinator` agent
/// interprets. They exist only as a `/` autocomplete convenience, and are offered
/// only when a `wf_coordinator` agent is present in the room (see the workflow gate
/// in `update_slash_command_list`). `needs_args` is unused for these (they always
/// take the insert path in `on_slash_command_selected`).
///
/// Gated behind the `agent_chat` Cargo feature: compiled in only for agent-chat
/// builds, and even then activated only via the runtime Settings toggle.
#[cfg(feature = "agent_chat")]
const WORKFLOW_SLASH_COMMANDS: &[SlashCommand] = &[
    SlashCommand {
        command: "/create-issue",
        description_key: "slash_command.create_issue.description",
        needs_args: true,
    },
    SlashCommand {
        command: "/go",
        description_key: "slash_command.go.description",
        needs_args: true,
    },
    SlashCommand {
        command: "/review",
        description_key: "slash_command.review.description",
        needs_args: true,
    },
    SlashCommand {
        command: "/status",
        description_key: "slash_command.status.description",
        needs_args: false,
    },
];

pub(crate) fn is_management_bot_room(
    app_service_enabled: bool,
    is_direct_room: bool,
    has_persisted_management_binding: bool,
    bound_bot_user_id: Option<&OwnedUserId>,
    resolved_parent_bot_user_id: Option<&OwnedUserId>,
    _known_bot_user_ids: &[OwnedUserId],
) -> bool {
    if !app_service_enabled {
        return false;
    }

    let Some(bound_bot_user_id) = bound_bot_user_id else {
        return false;
    };

    if !is_direct_room && !has_persisted_management_binding {
        return false;
    }

    resolved_parent_bot_user_id.is_some_and(|resolved_parent_bot_user_id|
        bound_bot_user_id == resolved_parent_bot_user_id
    )
}

/// True if `name` looks like a workflow **coordinator** agent — bare `coordinator`
/// or `<team>_coordinator` (e.g. `wf_coordinator`, `alpha_coordinator`). Used to detect
/// agent-chat workflow rooms for ANY parallel team, regardless of team prefix. Matched
/// against both display name and MXID localpart (`ac_<team>_coordinator` ends with
/// `_coordinator`, so the `ac_` prefix is irrelevant).
#[cfg(feature = "agent_chat")]
fn name_is_workflow_coordinator(name: &str) -> bool {
    name == "coordinator" || name.ends_with("_coordinator")
}

fn bot_command_popup_enabled(
    app_service_enabled: bool,
    is_direct_room: bool,
    has_persisted_management_binding: bool,
    bound_bot_user_id: Option<&OwnedUserId>,
    resolved_parent_bot_user_id: Option<&OwnedUserId>,
    known_bot_user_ids: &[OwnedUserId],
) -> bool {
    is_management_bot_room(
        app_service_enabled,
        is_direct_room,
        has_persisted_management_binding,
        bound_bot_user_id,
        resolved_parent_bot_user_id,
        known_bot_user_ids,
    )
}

fn find_slash_command_trigger_position(text: &str, cursor_pos: usize) -> Option<usize> {
    if cursor_pos == 0 || cursor_pos > text.len() {
        return None;
    }

    let current_segment = text.get(..cursor_pos)?;
    let line_start = current_segment.rfind('\n').map(|idx| idx + 1).unwrap_or(0);
    let line = text.get(line_start..cursor_pos)?;

    // The command being typed is the last whitespace-delimited token before the cursor.
    let token_start = line.rfind(char::is_whitespace).map_or(0, |idx| idx + 1);
    let token = &line[token_start..];
    if !token.starts_with('/') {
        return None;
    }

    // Trigger when the command is at the start of the line (`/cmd`) OR follows only a
    // leading run of @mentions — the demo pattern `@wf_coordinator /create-issue`.
    // Anything else before the command (plain words, paths like `see /tmp`) must NOT
    // trigger the popup. (Mentions with spaces in their display text aren't supported as
    // a prefix here; type the command at line start in that case.)
    let prefix = &line[..token_start];
    if !prefix.split_whitespace().all(|word| word.starts_with('@')) {
        return None;
    }

    Some(line_start + token_start)
}

/// Prefix-filter a slash-command set by the typed query (the `/` and surrounding
/// whitespace are ignored). Lets the popup combine the bot command set and the
/// workflow command set depending on which are enabled for the room.
fn matching_slash_commands_in(commands: &[SlashCommand], search_text: &str) -> Vec<SlashCommand> {
    let query = search_text.trim().trim_start_matches('/').to_ascii_lowercase();
    commands
        .iter()
        .copied()
        .filter(|command| {
            command
                .command
                .trim_start_matches('/')
                .to_ascii_lowercase()
                .starts_with(&query)
        })
        .collect()
}

pub(crate) fn classify_known_slash_command_for_submission(text: &str) -> Option<SlashCommand> {
    let first_token = text.split_whitespace().next()?;
    SLASH_COMMANDS
        .iter()
        .copied()
        .find(|command| command.command == first_token)
}

pub(crate) fn parse_command_with_at_suffix(input: &str) -> Option<ParsedSlashCommand> {
    let trimmed = input.trim_start();
    let first_token = trimmed.split_whitespace().next()?;
    if !first_token.starts_with('/') || first_token.len() <= 1 {
        return None;
    }

    let Some(at_idx) = first_token.find('@') else {
        return Some(ParsedSlashCommand {
            command: first_token.to_owned(),
            target_localpart: None,
        });
    };

    if at_idx <= 1 {
        return None;
    }

    let command = first_token[..at_idx].to_owned();
    let suffix = &first_token[at_idx + 1..];
    let target_localpart = suffix
        .split(':')
        .next()
        .map(str::trim)
        .filter(|localpart| !localpart.is_empty())
        .map(ToOwned::to_owned);

    Some(ParsedSlashCommand {
        command,
        target_localpart,
    })
}

pub(crate) fn normalize_command_with_at_suffix_for_send(input: &str) -> String {
    let Some(parsed) = parse_command_with_at_suffix(input) else {
        return input.to_owned();
    };

    if parsed.target_localpart.is_none() {
        return input.to_owned();
    }

    let trimmed = input.trim_start();
    let first_token_end = trimmed
        .find(char::is_whitespace)
        .unwrap_or(trimmed.len());
    let suffix = &trimmed[first_token_end..];
    format!("{}{}", parsed.command, suffix)
}

fn primary_submit_modifiers() -> KeyModifiers {
    #[cfg(any(target_os = "ios", target_os = "macos", target_os = "tvos"))]
    {
        KeyModifiers {
            logo: true,
            ..Default::default()
        }
    }
    #[cfg(not(any(target_os = "ios", target_os = "macos", target_os = "tvos")))]
    {
        KeyModifiers {
            control: true,
            ..Default::default()
        }
    }
}

fn member_list_ready_for_mentions(member_count: usize, sync_pending: bool) -> bool {
    member_count > 0 && !sync_pending
}

fn member_data_change_requires_popup_refresh(
    previous_member_count: usize,
    current_member_count: usize,
    previous_sync_pending: bool,
    current_sync_pending: bool,
    search_state: &MentionSearchState,
) -> bool {
    let member_count_changed =
        current_member_count > 0 && current_member_count != previous_member_count;
    let sync_just_completed = previous_sync_pending && !current_sync_pending;

    (member_count_changed || sync_just_completed)
        && matches!(
            search_state,
            MentionSearchState::WaitingForMembers { .. } | MentionSearchState::Searching { .. }
        )
}

fn build_user_mention_insertion(
    display_name: Option<&str>,
    user_id: &OwnedUserId,
) -> String {
    let visible_text = display_name
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(|name| format!("@{name}"))
        .unwrap_or_else(|| format!("@{}", user_id.localpart()));
    format!("{visible_text} ")
}

fn apply_text_replacement_preserving_mentions(
    current_text: &str,
    start_idx: usize,
    head: usize,
    replacement_text: &str,
    tracked_visible_mentions: &mut Vec<TrackedVisibleMention>,
) -> (String, usize) {
    let new_text =
        utils::safe_replace_by_byte_indices(current_text, start_idx, head, replacement_text);
    *tracked_visible_mentions = reconcile_visible_mentions_after_text_change(
        current_text,
        &new_text,
        tracked_visible_mentions,
    );
    tracked_visible_mentions.sort_by_key(|mention| mention.start);

    let cursor = start_idx + replacement_text.len();
    (new_text, cursor)
}

fn apply_user_mention_selection(
    current_text: &str,
    start_idx: usize,
    head: usize,
    display_name: Option<&str>,
    user_id: &OwnedUserId,
    tracked_visible_mentions: &mut Vec<TrackedVisibleMention>,
) -> (String, usize) {
    let mention_to_insert = build_user_mention_insertion(display_name, user_id);
    let (new_text, cursor) = apply_text_replacement_preserving_mentions(
        current_text,
        start_idx,
        head,
        &mention_to_insert,
        tracked_visible_mentions,
    );

    let visible_text = mention_to_insert.trim_end().to_owned();
    tracked_visible_mentions.push(TrackedVisibleMention {
        user_id: user_id.clone(),
        visible_text: visible_text.clone(),
        start: start_idx,
        end: start_idx + visible_text.len(),
    });
    tracked_visible_mentions.sort_by_key(|mention| mention.start);

    (new_text, cursor)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TextChangeWindow {
    changed_start: usize,
    old_changed_end: usize,
    new_changed_end: usize,
    delta: isize,
}

fn compute_text_change_window(old_text: &str, new_text: &str) -> TextChangeWindow {
    let mut changed_start = 0;
    let mut old_iter = old_text.chars();
    let mut new_iter = new_text.chars();

    while let (Some(old_ch), Some(new_ch)) = (old_iter.next(), new_iter.next()) {
        if old_ch != new_ch {
            break;
        }
        changed_start += old_ch.len_utf8();
    }

    let mut old_suffix_len = 0;
    let mut new_suffix_len = 0;
    let mut old_suffix_iter = old_text[changed_start..].chars().rev();
    let mut new_suffix_iter = new_text[changed_start..].chars().rev();

    while old_text.len() - old_suffix_len > changed_start
        && new_text.len() - new_suffix_len > changed_start
    {
        match (old_suffix_iter.next(), new_suffix_iter.next()) {
            (Some(old_ch), Some(new_ch)) if old_ch == new_ch => {
                old_suffix_len += old_ch.len_utf8();
                new_suffix_len += new_ch.len_utf8();
            }
            _ => break,
        }
    }

    let old_changed_end = old_text.len() - old_suffix_len;
    let new_changed_end = new_text.len() - new_suffix_len;

    TextChangeWindow {
        changed_start,
        old_changed_end,
        new_changed_end,
        delta: new_changed_end as isize - old_changed_end as isize,
    }
}

fn reconcile_visible_mentions_after_text_change(
    old_text: &str,
    new_text: &str,
    tracked_visible_mentions: &[TrackedVisibleMention],
) -> Vec<TrackedVisibleMention> {
    let change = compute_text_change_window(old_text, new_text);

    tracked_visible_mentions
        .iter()
        .filter_map(|mention| {
            if mention.end <= change.changed_start {
                Some(mention.clone())
            } else if mention.start >= change.old_changed_end {
                let start = mention.start.saturating_add_signed(change.delta);
                let end = mention.end.saturating_add_signed(change.delta);
                Some(TrackedVisibleMention {
                    user_id: mention.user_id.clone(),
                    visible_text: mention.visible_text.clone(),
                    start,
                    end,
                })
            } else {
                None
            }
        })
        .collect()
}

fn reset_visible_mention_tracking_for_programmatic_text_set(
    tracked_visible_mentions: &mut Vec<TrackedVisibleMention>,
    possible_room_mention: &mut bool,
) {
    tracked_visible_mentions.clear();
    *possible_room_mention = false;
}

#[derive(Clone, Debug)]
struct ResolvedOutgoingMentionContent {
    markdown_text: String,
    html_text: String,
    mentions: Mentions,
}

fn markdown_escape_visible_mention_label(label: &str) -> String {
    let mut escaped = String::with_capacity(label.len());
    for ch in label.chars() {
        match ch {
            '\\' | '[' | ']' | '(' | ')' | '*' | '_' | '`' | '~' | '!' | '#' | '+' | '-' | '.' | '>' => {
                escaped.push('\\');
            }
            _ => {}
        }
        escaped.push(ch);
    }
    escaped
}

fn contains_standalone_room_mention(text: &str) -> bool {
    let Some(mut search_start) = text.find("@room") else {
        return false;
    };

    loop {
        let before = text[..search_start].chars().next_back();
        let after = text[search_start + "@room".len()..].chars().next();

        let starts_token = before.is_none_or(|ch| ch.is_whitespace() || ch.is_ascii_punctuation());
        let ends_token = after.is_none_or(|ch| ch.is_whitespace() || ch.is_ascii_punctuation());

        if starts_token && ends_token {
            return true;
        }

        let next_search_from = search_start + "@room".len();
        let Some(relative_next) = text[next_search_from..].find("@room") else {
            return false;
        };
        search_start = next_search_from + relative_next;
    }
}

fn resolve_visible_mentions_for_send(
    text: &str,
    tracked_visible_mentions: &[TrackedVisibleMention],
    possible_room_mention: bool,
) -> ResolvedOutgoingMentionContent {
    let mut mentions = Mentions::new();
    mentions.room = possible_room_mention && contains_standalone_room_mention(text);

    let mut markdown_text = String::with_capacity(text.len());
    let mut html_text = String::with_capacity(text.len());
    let mut cursor = 0;

    let mut tracked_mentions = tracked_visible_mentions.iter().collect::<Vec<_>>();
    tracked_mentions.sort_by_key(|mention| mention.start);

    for mention in tracked_mentions {
        if mention.start < cursor || mention.start >= mention.end || mention.end > text.len() {
            continue;
        }

        let Some(visible_slice) = text.get(mention.start..mention.end) else {
            continue;
        };

        if visible_slice != mention.visible_text {
            continue;
        }

        let Some(prefix) = text.get(cursor..mention.start) else {
            continue;
        };

        markdown_text.push_str(prefix);
        html_text.push_str(prefix);

        let matrix_uri = mention.user_id.matrix_to_uri().to_string();
        let escaped_label = markdown_escape_visible_mention_label(&mention.visible_text);
        markdown_text.push_str(&format!("[{escaped_label}]({matrix_uri})"));
        html_text.push_str(&format!(
            "<a href=\"{}\">{}</a>",
            htmlize::escape_attribute(&matrix_uri),
            htmlize::escape_text(&mention.visible_text),
        ));

        mentions.user_ids.insert(mention.user_id.clone());
        cursor = mention.end;
    }

    if let Some(suffix) = text.get(cursor..) {
        markdown_text.push_str(suffix);
        html_text.push_str(suffix);
    }

    ResolvedOutgoingMentionContent { markdown_text, html_text, mentions }
}

fn finalize_popup_selection(cx: &mut Cx, input: &mut MentionableTextInput) {
    input.cancel_active_search();
    input.close_mention_popup(cx);
    input.search_state = MentionSearchState::JustCancelled;
    input.pending_popup_cleanup = false;
    input.pending_draw_focus_restore = true;
}

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    let FOCUS_HOVER_COLOR = #C
    let KEYBOARD_FOCUS_OR_COLOR_HOVER = #x1C274C

    // Template for user list items in the mention dropdown
    mod.widgets.UserListItem = View {
        width: Fill
        height: 36
        margin: Inset{left: 3 right: 3}
        padding: Inset{left: 10 right: 10 top: 4 bottom: 4}
        cursor: MouseCursor.Hand
        show_bg: true
        draw_bg +: {
            color: (COLOR_PRIMARY)
            border_radius: 4.0
            selected: instance(0.0)

            pixel: fn() {
                let sdf = Sdf2d.viewport(self.pos * self.rect_size)
                sdf.box(0. 0. self.rect_size.x self.rect_size.y self.border_radius)
                let highlight = #x1E90FF30
                sdf.fill(Pal.premul(self.color.mix(highlight self.selected)))
                return sdf.result
            }
        }

        animator: Animator {
            highlight: {
                default: @off
                off: AnimatorState {
                    from: { all: Forward { duration: 0.12 } }
                    apply: { draw_bg: { selected: 0.0 } }
                }
                on: AnimatorState {
                    from: { all: Forward { duration: 0.08 } }
                    apply: { draw_bg: { selected: 1.0 } }
                }
            }
        }

        flow: Right
        spacing: 8.0
        align: Align{y: 0.5}

        avatar := Avatar {
            width: 26
            height: 26
        }

        username := Label {
            height: Fit
            draw_text +: {
                color: #222
                text_style: BOLD_TEXT {font_size: 13.0}
            }
        }

        filler := FillerX {}

        user_id := Label {
            height: Fit
            draw_text +: {
                color: #aaa
                text_style: REGULAR_TEXT {font_size: 10.0}
            }
        }
    }

    // Template for the @room mention list item
    mod.widgets.RoomMentionListItem = View {
        width: Fill
        height: 40
        margin: Inset{left: 4 right: 4}
        padding: Inset{left: 10 right: 10 top: 6 bottom: 6}
        cursor: MouseCursor.Hand
        show_bg: true
        draw_bg +: {
            color: (COLOR_PRIMARY)
            border_radius: 4.0
            selected: instance(0.0)

            pixel: fn() {
                let sdf = Sdf2d.viewport(self.pos * self.rect_size)
                sdf.box(0. 0. self.rect_size.x self.rect_size.y self.border_radius)
                let highlight = #x1E90FF30
                sdf.fill(Pal.premul(self.color.mix(highlight self.selected)))
                return sdf.result
            }
        }

        animator: Animator {
            highlight: {
                default: @off
                off: AnimatorState {
                    from: { all: Forward { duration: 0.12 } }
                    apply: { draw_bg: { selected: 0.0 } }
                }
                on: AnimatorState {
                    from: { all: Forward { duration: 0.08 } }
                    apply: { draw_bg: { selected: 1.0 } }
                }
            }
        }

        flow: Right
        spacing: 10.0
        align: Align{y: 0.5}

        room_avatar := Avatar {
            width: 28
            height: 28
        }

        room_mention := Label {
            height: Fit
            draw_text +: {
                color: #222
                text_style: BOLD_TEXT {font_size: 13.0}
            }
            text: "Notify the entire room"
        }

        room_user_id := Label {
            height: Fit
            draw_text +: {
                color: #aaa
                text_style: REGULAR_TEXT {font_size: 10.0}
            }
            text: "@room"
        }
    }

    mod.widgets.SlashCommandListItem = View {
        width: Fill
        height: Fit
        margin: Inset{left: 4 right: 4}
        padding: Inset{left: 12 right: 12 top: 8 bottom: 8}
        cursor: MouseCursor.Hand
        show_bg: true
        draw_bg +: {
            color: (COLOR_PRIMARY)
            border_radius: 4.0
            selected: instance(0.0)

            pixel: fn() {
                let sdf = Sdf2d.viewport(self.pos * self.rect_size)
                sdf.box(0. 0. self.rect_size.x self.rect_size.y self.border_radius)
                let highlight = #x1E90FF30
                sdf.fill(Pal.premul(self.color.mix(highlight self.selected)))
                return sdf.result
            }
        }

        animator: Animator {
            highlight: {
                default: @off
                off: AnimatorState {
                    from: { all: Forward { duration: 0.12 } }
                    apply: { draw_bg: { selected: 0.0 } }
                }
                on: AnimatorState {
                    from: { all: Forward { duration: 0.08 } }
                    apply: { draw_bg: { selected: 1.0 } }
                }
            }
        }

        flow: Down
        spacing: 2.0

        command_name := Label {
            height: Fit
            draw_text +: {
                color: (COLOR_ACTIVE_PRIMARY)
                text_style: BOLD_TEXT {font_size: 11.0}
            }
        }

        description := Label {
            width: Fill
            height: Fit
            draw_text +: {
                color: #666
                text_style: REGULAR_TEXT {font_size: 10.0}
            }
        }
    }

    // Non-selectable group label shown above each slash-command section when more than
    // one command set is active (e.g. bot commands + workflow commands in the same room).
    mod.widgets.SlashCommandSectionHeader = View {
        width: Fill
        height: Fit
        margin: Inset{left: 4 right: 4 top: 6 bottom: 0}
        padding: Inset{left: 12 right: 12 top: 2 bottom: 2}
        section_label := Label {
            height: Fit
            draw_text +: {
                color: #999
                text_style: BOLD_TEXT {font_size: 9.0}
            }
        }
    }

    // Template for loading indicator when members are being fetched
    mod.widgets.LoadingIndicator = View {
        width: Fill
        height: 48
        margin: Inset{left: 4 right: 4}
        padding: Inset{left: 8 right: 8 top: 8 bottom: 8}
        flow: Right
        spacing: 8.0
        align: Align{x: 0.0 y: 0.5}
        show_bg: true
        draw_bg +: {
            color: (COLOR_PRIMARY)
        }

        loading_text := Label {
            height: Fit
            draw_text +: {
                color: #666
                text_style: REGULAR_TEXT {font_size: 14.0}
            }
            text: "Loading members"
        }

        loading_animation := BouncingDots {
            width: 60
            height: 24
            draw_bg +: {
                color: (COLOR_ROBRIX_PURPLE)
                dot_radius: 2.0
            }
        }
    }

    // Template for no matches indicator when no users match the search
    mod.widgets.NoMatchesIndicator = View {
        width: Fill
        height: 48
        margin: Inset{left: 4 right: 4}
        padding: Inset{left: 8 right: 8 top: 8 bottom: 8}
        flow: Right
        spacing: 8.0
        align: Align{x: 0.0 y: 0.5}
        show_bg: true
        draw_bg +: {
            color: (COLOR_PRIMARY)
        }

        no_matches_text := Label {
            height: Fit
            draw_text +: {
                color: #666
                text_style: REGULAR_TEXT {font_size: 14.0}
            }
            text: "No matching users found"
        }
    }

    mod.widgets.MentionableTextInput = #(MentionableTextInput::register_widget(vm)) {
        ..mod.widgets.CommandTextInput
        width: Fill
        height: Fit
        trigger: "@"
        inline_search: true

        color_focus: (KEYBOARD_FOCUS_OR_COLOR_HOVER)
        color_hover: (FOCUS_HOVER_COLOR)

        popup +: {
            spacing: 0.0
            padding: Inset{top: 0 bottom: 6 left: 0 right: 0}

            draw_bg +: {
                color: (COLOR_PRIMARY)
                border_radius: 6.0
                border_size: 1.0
                border_color: #ddd
                shadow_color: #0003
                shadow_radius: 12.0
                shadow_offset: vec2(0.0 2.0)
            }
            header_view +: {
                margin: Inset{left: 0 right: 0 top: 0 bottom: 2}
                padding: Inset{left: 12 right: 12 top: 8 bottom: 8}
                draw_bg +: {
                    color: (COLOR_ROBRIX_PURPLE)
                    border_radius: 6.0
                }
                header_label +: {
                    draw_text +: {
                        color: #fff
                        text_style: REGULAR_TEXT {font_size: 11.0}
                    }
                    text: "Users in this Room"
                }
            }

            // height below is the DSL default; Rust dynamically adjusts it
            // per platform via set_list_scroll_height() and DESKTOP/MOBILE_MAX_SCROLL_HEIGHT.
            list_scroll +: {
                height: 360
                list +: {
                    height: Fit
                    spacing: 0.0
                    padding: Inset{top: 2 bottom: 2 left: 0 right: 0}
                }
            }
        }

        persistent +: {
            top +: { height: 0 }
            bottom +: { height: 0 }
            center +: {
                text_input := RobrixTextInput {
                    empty_text: "Start typing..."
                    is_multiline: true,
                }
            }
        }

        // Template for user list items in the mention popup
        user_list_item: mod.widgets.UserListItem {}
        room_mention_list_item: mod.widgets.RoomMentionListItem {}
        slash_command_list_item: mod.widgets.SlashCommandListItem {}
        slash_command_section_header: mod.widgets.SlashCommandSectionHeader {}
        loading_indicator: mod.widgets.LoadingIndicator {}
        no_matches_indicator: mod.widgets.NoMatchesIndicator {}
    }
}

// /// A special string used to denote the start of a mention within
// /// the actual text being edited.
// /// This is used to help easily locate and distinguish actual mentions
// /// from normal `@` characters.
// const MENTION_START_STRING: &str = "\u{8288}@\u{8288}";

#[derive(Debug)]
pub enum MentionableTextInputAction {
    /// Notifies the MentionableTextInput about updated power levels for the room.
    PowerLevelsUpdated {
        room_id: OwnedRoomId,
        can_notify_room: bool,
    },
    /// Notifies the MentionableTextInput that room members have been loaded.
    RoomMembersLoaded {
        room_id: OwnedRoomId,
        /// Whether member sync is still in progress
        sync_in_progress: bool,
        /// Whether we currently have cached members
        has_members: bool,
    },
}

/// Widget that extends CommandTextInput with @mention capabilities
#[derive(Script, ScriptHook, Widget)]
pub struct MentionableTextInput {
    /// Base command text input
    #[deref]
    cmd_text_input: CommandTextInput,
    /// Template for user list items
    #[live]
    user_list_item: Option<LivePtr>,
    /// Template for the @room mention list item
    #[live]
    room_mention_list_item: Option<LivePtr>,
    /// Template for slash command list items
    #[live]
    slash_command_list_item: Option<LivePtr>,
    /// Template for a non-selectable slash-command section header
    #[live]
    slash_command_section_header: Option<LivePtr>,
    /// Template for loading indicator
    #[live]
    loading_indicator: Option<LivePtr>,
    /// Template for no matches indicator
    #[live]
    no_matches_indicator: Option<LivePtr>,
    #[rust]
    tracked_visible_mentions: Vec<TrackedVisibleMention>,
    /// Last text value seen by the widget, used to reconcile tracked mention spans.
    #[rust]
    last_text: String,
    /// Indicates if the `@room` option was explicitly selected.
    #[rust]
    possible_room_mention: bool,
    /// Whether the current user can notify everyone in the room (@room mention)
    #[rust]
    can_notify_room: bool,
    /// Current state of the mention search functionality
    #[rust]
    search_state: MentionSearchState,
    /// Last search text to avoid duplicate searches
    #[rust]
    last_search_text: Option<String>,
    /// Next identifier for submitted search jobs
    #[rust]
    next_search_id: u64,
    /// Whether the background search task has pending results
    #[rust]
    search_results_pending: bool,
    /// Active loading indicator widget while we wait for members/results
    #[rust]
    loading_indicator_ref: Option<WidgetRef>,
    /// Cached text analysis to avoid repeated grapheme parsing
    /// Format: (text, graphemes_as_strings, byte_positions)
    #[rust]
    cached_text_analysis: Option<(String, Vec<String>, Vec<usize>)>,
    /// Last known member count - used ONLY for change detection (not rendering)
    /// Rendering always uses props as source of truth
    #[rust]
    last_member_count: usize,
    /// Last known sync pending state - used ONLY for change detection (not rendering)
    #[rust]
    last_sync_pending: bool,
    /// Whether a deferred popup cleanup is pending after focus loss
    #[rust]
    pending_popup_cleanup: bool,
    /// Whether focus should be restored in the next draw_walk cycle
    #[rust]
    pending_draw_focus_restore: bool,
    /// Which kind of popup content is currently active.
    #[rust]
    active_popup_mode: PopupMode,
}

impl Widget for MentionableTextInput {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        // Handle ESC key early before passing to child widgets
        if self.is_searching() {
            if let Event::KeyUp(key_event) = event {
                if key_event.key_code == KeyCode::Escape {
                    self.cancel_active_search();
                    self.search_state = MentionSearchState::JustCancelled;

                    // UI cleanup only - do NOT call close_mention_popup() as it resets
                    // state to Idle via reset_search_state(), losing the JustCancelled marker
                    let popup = self.cmd_text_input.view(cx, ids!(popup));
                    popup.set_visible(cx, false);
                    self.cmd_text_input.clear_items(cx);
                    self.loading_indicator_ref = None; // Clear loading indicator
                    self.pending_popup_cleanup = false; // Prevent next frame from triggering cleanup

                    self.redraw(cx);
                    return; // Don't process other events
                }
            }
        }

        if self.is_slash_command_popup_active() {
            if let Event::KeyDown(key_event) = event {
                if key_event.key_code == KeyCode::Escape {
                    self.close_mention_popup(cx);
                    self.pending_draw_focus_restore = true;
                    self.redraw(cx);
                    return;
                }
            }
        }

        // Intercept Cmd/Ctrl+Return to emit Returned action (send message)
        // before the multiline TextInput turns it into a newline.
        if let Event::KeyDown(KeyEvent {
            key_code: KeyCode::ReturnKey,
            modifiers,
            ..
        }) = event {
            // When the autocomplete popup (mention OR slash command) is open AND a row
            // is focused, a plain Enter must SELECT that row — which cmd_text_input
            // .handle_event does below — instead of sending the message. Without this
            // guard the send intercept fires first and the popup item can never be picked
            // with Enter. We also require a focused selectable item so that a popup that
            // is merely open-but-empty (loading / no matches) still lets Enter send.
            // A Cmd/Ctrl modifier always force-sends, even with the popup open.
            let popup_selecting = self.cmd_text_input.view(cx, ids!(popup)).visible()
                && self.cmd_text_input.keyboard_focus_index().is_some();
            let force_send = modifiers.logo || modifiers.control;
            if !(popup_selecting && !force_send) {
                let send_on_enter = scope
                    .data
                    .get::<crate::app::AppState>()
                    .map(|app_state| app_state.app_prefs.send_on_enter)
                    .unwrap_or(true);
                let should_submit = force_send
                    || send_on_enter && !modifiers.shift && !modifiers.alt;
                if should_submit {
                    let text_input = self.cmd_text_input.text_input(cx, ids!(text_input));
                    let uid = text_input.widget_uid();
                    let text = text_input.text();
                    cx.widget_action(uid, makepad_widgets::text_input::TextInputAction::Returned(text, *modifiers));
                    return;
                }
            }
        }

        self.cmd_text_input.handle_event(cx, event, scope);

        // Best practice: Always check Scope first to get current context
        // Scope represents the current widget context as passed down from parents
        let (scope_room_id, scope_member_count, scope_sync_pending) = {
            let room_props = scope
                .props
                .get::<RoomScreenProps>()
                .expect("RoomScreenProps should be available in scope for MentionableTextInput");
            let member_count = room_props
                .room_members
                .as_ref()
                .map(|members| members.len())
                .unwrap_or(0);
            (
                room_props.room_name_id.room_id().clone(),
                member_count,
                room_props.room_members_sync_pending,
            )
        };

        self.refresh_popup_for_member_change(
            cx,
            scope,
            scope_member_count,
            scope_sync_pending,
        );

        // Check search channel on every frame if we're searching
        if let MentionSearchState::Searching { .. } = &self.search_state {
            if let Event::NextFrame(_) = event {
                // Only continue requesting frames if we're still waiting for results
                if self.check_search_channel(cx, scope) {
                    cx.new_next_frame();
                }
            }
        }
        // Handle deferred cleanup after focus loss
        if let Event::NextFrame(_) = event {
            if self.pending_popup_cleanup {
                let text_input_ref = self.cmd_text_input.text_input_ref();
                let text_input_area = text_input_ref.area();
                self.pending_popup_cleanup = false;

                // Only close if input still doesn't have focus and we're not actively searching
                let has_focus = cx.has_key_focus(text_input_area);

                // If user refocused or is actively typing/searching, don't cleanup
                if !has_focus && !self.is_searching() {
                    self.close_mention_popup(cx);
                }
            }
        }

        if let Event::Actions(actions) = event {
            let text_input_ref = self.cmd_text_input.text_input_ref();
            let text_input_uid = text_input_ref.widget_uid();
            let text_input_area = text_input_ref.area();
            let has_focus = cx.has_key_focus(text_input_area);

            // Handle item selection from mention popup
            if let Some(selected) = self.cmd_text_input.item_selected(actions) {
                self.on_popup_item_selected(cx, scope, selected);
            }

            // Handle build items request
            if self.cmd_text_input.should_build_items(actions) {
                if has_focus {
                    let search_text = self.cmd_text_input.search_text().to_lowercase();
                    self.update_user_list(cx, &search_text, scope);
                } else if self.cmd_text_input.view(cx, ids!(popup)).visible() {
                    self.close_mention_popup(cx);
                }
            }

            // Process all actions
            for action in actions {
                // Handle TextInput changes
                if let Some(widget_action) = action.as_widget_action() {
                    if widget_action.widget_uid == text_input_uid {
                        if let TextInputAction::Changed(text) = widget_action.cast() {
                            if has_focus {
                                self.handle_text_change(cx, scope, text.to_owned());
                            }
                            continue; // Continue processing other actions
                        }
                    }
                }

                // Handle MentionableTextInputAction actions
                if let Some(action) = action.downcast_ref::<MentionableTextInputAction>() {
                    match action {
                        MentionableTextInputAction::PowerLevelsUpdated {
                            room_id,
                            can_notify_room,
                        } => {
                            if &scope_room_id != room_id {
                                continue;
                            }
                            log!("PowerLevelsUpdated received: room_id={}, can_notify_room={}, scope_room_id={}",
                                 room_id, can_notify_room, scope_room_id);

                            if self.can_notify_room != *can_notify_room {
                                log!("Updating can_notify_room from {} to {}", self.can_notify_room, can_notify_room);
                                self.can_notify_room = *can_notify_room;
                                if self.is_searching() && has_focus {
                                    let search_text =
                                        self.cmd_text_input.search_text().to_lowercase();
                                    self.update_user_list(cx, &search_text, scope);
                                } else {
                                    self.cmd_text_input.redraw(cx);
                                }
                            }
                        }
                        MentionableTextInputAction::RoomMembersLoaded {
                            room_id,
                            sync_in_progress,
                            has_members,
                        } => {
                            if &scope_room_id != room_id {
                                continue;
                            }

                            // CRITICAL: Use locally stored previous state for change detection
                            // (not from props, which is already the new state in the same frame)
                            let previous_member_count = self.last_member_count;
                            let was_sync_pending = self.last_sync_pending;

                            // Current state: read fresh props to avoid stale snapshot from handle_event entry
                            let current_member_count = scope
                                .props
                                .get::<RoomScreenProps>()
                                .map(|p| p.room_members.as_ref().map(|m| m.len()).unwrap_or(0))
                                .unwrap_or(scope_member_count);
                            let current_sync_pending = *sync_in_progress;

                            // Detect actual changes
                            let member_count_changed = current_member_count != previous_member_count
                                && current_member_count > 0
                                && previous_member_count > 0;
                            let sync_just_completed = !current_sync_pending && was_sync_pending;

                            // Update local state for next comparison
                            self.last_member_count = current_member_count;
                            self.last_sync_pending = current_sync_pending;

                            // Skip processing if search was cancelled by ESC
                            // This prevents async callbacks from reopening the popup
                            if matches!(self.search_state, MentionSearchState::JustCancelled) {
                                continue;
                            }

                            if *has_members {
                                // CRITICAL FIX: Use saved state instead of reading from text input
                                // Reading from text input causes race condition (text may be empty when members arrive)
                                // Extract needed values first to avoid borrow checker issues
                                let action = match &self.search_state {
                                    MentionSearchState::WaitingForMembers {
                                        pending_search_text,
                                        ..
                                    } => Some((true, pending_search_text.clone())),
                                    MentionSearchState::Searching { search_text, .. } => {
                                        Some((false, search_text.clone()))
                                    }
                                    _ => None,
                                };

                                if let Some((is_waiting, search_text)) = action {
                                    let member_set_updated = member_count_changed
                                        && matches!(self.search_state, MentionSearchState::Searching { .. });

                                    if is_waiting {
                                        self.last_search_text = None;
                                        self.update_user_list(cx, &search_text, scope);
                                    } else {
                                        // Already in Searching state
                                        // Check if remote sync just completed or member set changed - need to re-search with full member list
                                        if member_set_updated || sync_just_completed {
                                            self.last_search_text = None;
                                            self.update_user_list(cx, &search_text, scope);
                                        } else {
                                            self.update_ui_with_results(cx, scope, &search_text);
                                        }
                                    }
                                } else {
                                    // Not in WaitingForMembers or Searching state
                                    // Check if remote sync just completed - if so, refresh UI if there's an active mention trigger
                                    if sync_just_completed {
                                        let text = self.cmd_text_input.text_input_ref().text();
                                        let cursor_pos = self.cmd_text_input.text_input_ref()
                                            .borrow()
                                            .map_or(0, |p| p.cursor().index);

                                        if let Some(_trigger_pos) = self.find_mention_trigger_position(&text, cursor_pos) {
                                            let search_text = self.cmd_text_input.search_text().to_lowercase();
                                            self.last_search_text = None;
                                            self.update_user_list(cx, &search_text, scope);
                                        }
                                    }
                                }
                            } else if self.is_searching() {
                                // Still no members returned yet; keep showing loading indicator.
                                self.cmd_text_input.clear_items(cx);
                                self.show_loading_indicator(cx);
                                let popup = self.cmd_text_input.view(cx, ids!(popup));
                                popup.set_visible(cx, true);
                                // Only restore focus if input currently has focus
                                let text_input_area = self.cmd_text_input.text_input_ref().area();
                                if cx.has_key_focus(text_input_area) {
                                    self.cmd_text_input.text_input_ref().set_key_focus(cx);
                                }
                            }
                        }
                    }
                }
            }

            // Close popup and clean up search state if focus is lost while searching
            // This prevents background search tasks from continuing when user is no longer interested
            if !has_focus && self.is_searching() {
                let popup = self.cmd_text_input.view(cx, ids!(popup));
                popup.set_visible(cx, false);
                self.pending_popup_cleanup = true;
                // Guarantee cleanup executes even if search completes and stops requesting frames
                cx.new_next_frame();
            } else if !has_focus && self.is_slash_command_popup_active() {
                // Defer the close by one frame: when open_slash_command_popup
                // is invoked from a button click, set_key_focus is still
                // pending here, so closing immediately kills the popup we
                // just opened. The NextFrame handler re-checks has_focus, and
                // by then focus has committed — so the popup survives.
                self.pending_popup_cleanup = true;
                cx.new_next_frame();
            }
        }

    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        let result = self.cmd_text_input.draw_walk(cx, scope, walk);

        // Restore focus after all child drawing is complete.
        // This retries until focus is successfully restored, handling cases where
        // finger_up events might steal focus after our initial restoration attempt.
        if self.pending_draw_focus_restore {
            let text_input_ref = self.cmd_text_input.text_input_ref();
            text_input_ref.set_key_focus(cx);
            if let Some(mut ti) = text_input_ref.borrow_mut() {
                ti.reset_blink_timer(cx);
            }
            // Check if we successfully got focus
            let area = text_input_ref.area();
            if cx.has_key_focus(area) {
                // Successfully restored focus, clear the flag
                self.pending_draw_focus_restore = false;
            } else {
                // Focus restoration failed (likely due to finger_up event stealing focus)
                // Keep the flag true and request another frame to retry
                cx.new_next_frame();
            }
        }

        result
    }
}

impl MentionableTextInput {
    fn current_app_language(scope: &mut Scope) -> AppLanguage {
        scope
            .data
            .get::<AppState>()
            .map(|app_state| app_state.app_language)
            .unwrap_or_default()
    }

    fn set_popup_header_text(&mut self, cx: &mut Cx, text: &str) {
        self.cmd_text_input
            .view(cx, ids!(popup.header_view))
            .set_visible(cx, true);
        self.cmd_text_input
            .label(cx, ids!(popup.header_view.header_label))
            .set_text(cx, text);
    }

    fn set_popup_header_for_mentions(&mut self, cx: &mut Cx) {
        self.set_popup_header_text(cx, MENTION_POPUP_HEADER_TEXT);
    }

    fn set_popup_header_for_slash_commands(&mut self, cx: &mut Cx, scope: &mut Scope, header_key: &str) {
        let app_language = Self::current_app_language(scope);
        self.set_popup_header_text(cx, tr_key(app_language, header_key));
    }

    fn active_search_text(&self) -> Option<String> {
        match &self.search_state {
            MentionSearchState::WaitingForMembers {
                pending_search_text,
                ..
            } => Some(pending_search_text.clone()),
            MentionSearchState::Searching { search_text, .. } => Some(search_text.clone()),
            _ => None,
        }
    }

    fn add_popup_status_item(
        &mut self,
        cx: &Cx,
        widget: WidgetRef,
        item_kind: PopupStatusItemKind,
    ) {
        if popup_status_item_is_selectable(item_kind) {
            self.cmd_text_input.add_item(cx, widget);
        } else {
            self.cmd_text_input.add_unselectable_item(cx, widget);
        }
    }

    fn refresh_popup_for_member_change(
        &mut self,
        cx: &mut Cx,
        scope: &mut Scope,
        current_member_count: usize,
        current_sync_pending: bool,
    ) {
        let previous_member_count = self.last_member_count;
        let previous_sync_pending = self.last_sync_pending;
        self.last_member_count = current_member_count;
        self.last_sync_pending = current_sync_pending;

        if !member_data_change_requires_popup_refresh(
            previous_member_count,
            current_member_count,
            previous_sync_pending,
            current_sync_pending,
            &self.search_state,
        ) {
            return;
        }

        if self.pending_popup_cleanup {
            return;
        }

        let text_input_area = self.cmd_text_input.text_input_ref().area();
        if !cx.has_key_focus(text_input_area) {
            return;
        }

        let Some(search_text) = self.active_search_text() else {
            return;
        };

        self.last_search_text = None;
        self.update_user_list(cx, &search_text, scope);
    }

    /// Check if currently in any form of search mode
    fn is_searching(&self) -> bool {
        matches!(
            self.search_state,
            MentionSearchState::WaitingForMembers { .. } | MentionSearchState::Searching { .. }
        )
    }

    fn is_slash_command_popup_active(&self) -> bool {
        self.active_popup_mode == PopupMode::SlashCommand
    }

    /// Generate the next unique identifier for a background search job.
    fn allocate_search_id(&mut self) -> u64 {
        if self.next_search_id == 0 {
            self.next_search_id = 1;
        }
        let id = self.next_search_id;
        self.next_search_id = self.next_search_id.wrapping_add(1);
        if self.next_search_id == 0 {
            self.next_search_id = 1;
        }
        id
    }

    /// Get the current trigger position if in search mode
    fn get_trigger_position(&self) -> Option<usize> {
        match &self.search_state {
            MentionSearchState::WaitingForMembers {
                trigger_position, ..
            }
            | MentionSearchState::Searching {
                trigger_position, ..
            } => Some(*trigger_position),
            _ => None,
        }
    }

    /// Check if search was just cancelled
    fn is_just_cancelled(&self) -> bool {
        matches!(self.search_state, MentionSearchState::JustCancelled)
    }

    /// Sets the scroll container's viewport height.
    fn set_list_scroll_height(&self, cx: &Cx, height: f64) {
        let scroll_view = self.cmd_text_input.list_scroll_view(cx);
        if let Some(mut inner) = scroll_view.borrow_mut() {
            inner.walk.height = Size::Fixed(height);
        }
    }

    /// Tries to add the `@room` mention item to the list of selectable popup mentions.
    ///
    /// Returns true if @room item was added to the list and will be displayed in the popup.
    fn try_add_room_mention_item(
        &mut self,
        cx: &mut Cx,
        search_text: &str,
        room_props: &RoomScreenProps,
        _is_desktop: bool,
    ) -> bool {
        // Don't show @room option in direct messages
        if room_props.is_direct_room {
            return false;
        }
        if !self.can_notify_room || !("@room".contains(search_text) || search_text.is_empty()) {
            return false;
        }

        let Some(ptr) = self.room_mention_list_item else {
            return false;
        };
        let room_mention_item = crate::widget_ref_from_live_ptr(cx, Some(ptr));
        let mut room_avatar_shown = false;

        let avatar_ref = room_mention_item.avatar(cx, ids!(room_avatar));

        // Get room avatar fallback text from room name (with automatic ID fallback)
        let room_label = room_props.room_name_id.to_string();
        let room_name_first_char = room_label
            .graphemes(true)
            .next()
            .map(|s| s.to_uppercase())
            .filter(|s| s != "@" && s.chars().all(|c| c.is_alphabetic()))
            .unwrap_or_default();

        if let Some(avatar_url) = &room_props.room_avatar_url {
            match get_or_fetch_avatar(cx, avatar_url) {
                AvatarCacheEntry::Loaded(avatar_data) => {
                    // Display room avatar
                    let result = avatar_ref.show_image(cx, None, |cx, img| {
                        utils::load_png_or_jpg(&img, cx, &avatar_data)
                    });
                    if result.is_ok() {
                        room_avatar_shown = true;
                    }
                }
                AvatarCacheEntry::Requested => {
                    avatar_ref.show_text(
                        cx,
                        Some(COLOR_UNKNOWN_ROOM_AVATAR),
                        None,
                        &room_name_first_char,
                    );
                    room_avatar_shown = true;
                }
                AvatarCacheEntry::Failed => {
                    // Failed to load room avatar - will use fallback text
                }
            }
        }

        // If unable to display room avatar, show first character of room name
        if !room_avatar_shown {
            avatar_ref.show_text(
                cx,
                Some(COLOR_UNKNOWN_ROOM_AVATAR),
                None,
                &room_name_first_char,
            );
        }

        // Layout is set in the DSL template (defaults to desktop layout).
        // TODO: add mobile-specific layout when adaptive layout is implemented.

        self.cmd_text_input.add_item(cx, room_mention_item);
        true
    }

    /// Add user mention items to the list from search results
    /// Returns the number of items added
    fn add_user_mention_items_from_results(
        &mut self,
        cx: &mut Cx,
        results: &[usize],
        user_items_limit: usize,
        _is_desktop: bool,
        room_props: &RoomScreenProps,
    ) -> usize {
        let mut items_added = 0;

        // Get the actual members vec from room_props
        let Some(members) = &room_props.room_members else {
            return 0;
        };

        for (index, &member_idx) in results.iter().take(user_items_limit).enumerate() {
            // Get the actual member from the index
            let Some(member) = members.get(member_idx) else {
                continue;
            };

            // Get display name from member, with better fallback
            // Trim whitespace and filter out empty/whitespace-only names
            let display_name = member.display_name()
                .map(|name| name.trim())  // Remove leading/trailing whitespace
                .filter(|name| !name.is_empty())  // Filter out empty or whitespace-only names
                .unwrap_or_else(|| member.user_id().localpart())
                .to_owned();

            // Log warning for extreme cases where we still have no displayable text
            #[cfg(debug_assertions)]
            if display_name.is_empty() {
                log!(
                    "Warning: Member {} has no displayable name (empty display_name and localpart)",
                    member.user_id()
                );
            }

            let Some(user_list_item_ptr) = self.user_list_item else {
                // user_list_item_ptr is None
                continue;
            };
            let item = crate::widget_ref_from_live_ptr(cx, Some(user_list_item_ptr));

            item.label(cx, ids!(username)).set_text(cx, &display_name);

            // Use the full user ID string
            let user_id_str = member.user_id().as_str();
            item.label(cx, ids!(user_id)).set_text(cx, user_id_str);

            // Layout is set in the DSL template (defaults to desktop layout).
            // TODO: add mobile-specific layout when adaptive layout is implemented.

            let avatar = item.avatar(cx, ids!(avatar));
            if let Some(mxc_uri) = member.avatar_url() {
                match get_or_fetch_avatar(cx, &mxc_uri.to_owned()) {
                    AvatarCacheEntry::Loaded(avatar_data) => {
                        let _ = avatar.show_image(cx, None, |cx, img| {
                            utils::load_png_or_jpg(&img, cx, &avatar_data)
                        });
                    }
                    AvatarCacheEntry::Requested | AvatarCacheEntry::Failed => {
                        avatar.show_text(cx, None, None, &display_name);
                    }
                }
            } else {
                avatar.show_text(cx, None, None, &display_name);
            }

            self.cmd_text_input.add_item(cx, item.clone());
            items_added += 1;

            // Set keyboard focus to the first item
            if index == 0 {
                self.cmd_text_input.set_keyboard_focus_index(0);
            }
        }

        items_added
    }

    fn add_slash_command_items(
        &mut self,
        cx: &mut Cx,
        app_language: AppLanguage,
        commands: &[SlashCommand],
    ) -> usize {
        let Some(item_ptr) = self.slash_command_list_item else {
            return 0;
        };

        let mut items_added = 0;
        for command in commands {
            let item = crate::widget_ref_from_live_ptr(cx, Some(item_ptr));
            item.label(cx, ids!(command_name))
                .set_text(cx, command.command);
            item.label(cx, ids!(description))
                .set_text(cx, tr_key(app_language, command.description_key));
            self.cmd_text_input.add_item(cx, item);
            items_added += 1;
        }

        items_added
    }

    /// Add a non-selectable group label (e.g. "Bot Commands" / "Workflow Commands") above
    /// a slash-command section. Not added to the selectable set, so keyboard nav skips it.
    fn add_slash_command_section_header(&mut self, cx: &mut Cx, app_language: AppLanguage, title_key: &str) {
        let Some(ptr) = self.slash_command_section_header else {
            return;
        };
        let item = crate::widget_ref_from_live_ptr(cx, Some(ptr));
        item.label(cx, ids!(section_label))
            .set_text(cx, tr_key(app_language, title_key));
        self.cmd_text_input.add_unselectable_item(cx, item);
    }

    fn update_slash_command_list(&mut self, cx: &mut Cx, scope: &mut Scope, search_text: &str) {
        let room_props = scope
            .props
            .get::<RoomScreenProps>()
            .expect("RoomScreenProps should be available in scope for MentionableTextInput");

        let bot_enabled = bot_command_popup_enabled(
            room_props.app_service_enabled,
            room_props.is_direct_room,
            room_props.has_persisted_management_binding,
            room_props.bound_bot_user_id.as_ref(),
            room_props.resolved_parent_bot_user_id.as_ref(),
            &room_props.known_bot_user_ids,
        );
        // agent-chat demo: offer the workflow `/` commands when a coordinator agent is
        // in the room (robrix2 has no built-in "agent-chat room" concept). Match ANY
        // team's coordinator — `wf_coordinator`, `alpha_coordinator`, … — on display name
        // OR localpart, so it works for multiple parallel teams and regardless of the
        // `ac_` MXID prefix / friendly display name.
        //
        // Double-gated: the `agent_chat` Cargo feature (compile-time) AND the runtime
        // Settings toggle (Preferences → "Enable agent-chat support"). Without both, the
        // workflow commands are never offered (and the code isn't even compiled in).
        #[cfg(feature = "agent_chat")]
        let workflow_enabled = cx
            .global::<crate::settings::app_preferences::AppPreferencesGlobal>()
            .0
            .agent_chat_enabled
            && room_props.room_members.as_ref().is_some_and(|members| {
                members.iter().any(|member| {
                    member.display_name().is_some_and(name_is_workflow_coordinator)
                        || name_is_workflow_coordinator(member.user_id().localpart())
                })
            });
        #[cfg(not(feature = "agent_chat"))]
        let workflow_enabled = false;
        if !bot_enabled && !workflow_enabled {
            if self.is_slash_command_popup_active() {
                self.close_mention_popup(cx);
            }
            return;
        }

        self.cancel_active_search();
        self.search_state = MentionSearchState::Idle;
        self.last_search_text = None;
        self.loading_indicator_ref = None;
        self.active_popup_mode = PopupMode::SlashCommand;

        self.cmd_text_input.clear_items(cx);
        self.cmd_text_input.reset_list_scroll(cx);

        // Filter each enabled command set separately so they render as labelled,
        // visually-separated sections (a room like octos-public has BOTH Octos bots and
        // the wf_coordinator agent, so both sets are active).
        let bot_matches = if bot_enabled {
            matching_slash_commands_in(SLASH_COMMANDS, search_text)
        } else {
            Vec::new()
        };
        #[cfg(feature = "agent_chat")]
        let workflow_matches = if workflow_enabled {
            matching_slash_commands_in(WORKFLOW_SLASH_COMMANDS, search_text)
        } else {
            Vec::new()
        };
        #[cfg(not(feature = "agent_chat"))]
        let workflow_matches: Vec<SlashCommand> = Vec::new();
        if bot_matches.is_empty() && workflow_matches.is_empty() {
            self.close_mention_popup(cx);
            return;
        }

        // In-list section headers appear only when BOTH groups are present; with a single
        // group the popup's top header already labels it (no redundant section row).
        let show_sections = !bot_matches.is_empty() && !workflow_matches.is_empty();
        let header_key = if show_sections {
            "slash_command.combined_header"
        } else if !bot_matches.is_empty() {
            "slash_command.header"
        } else {
            "slash_command.workflow_header"
        };
        self.set_popup_header_for_slash_commands(cx, scope, header_key);

        let app_language = Self::current_app_language(scope);
        let mut items_added = 0usize;
        let mut section_rows = 0usize;
        if !bot_matches.is_empty() {
            if show_sections {
                self.add_slash_command_section_header(cx, app_language, "slash_command.header");
                section_rows += 1;
            }
            items_added += self.add_slash_command_items(cx, app_language, &bot_matches);
        }
        if !workflow_matches.is_empty() {
            if show_sections {
                self.add_slash_command_section_header(cx, app_language, "slash_command.workflow_header");
                section_rows += 1;
            }
            items_added += self.add_slash_command_items(cx, app_language, &workflow_matches);
        }

        const SLASH_COMMAND_ITEM_HEIGHT: f64 = 48.0;
        const SLASH_SECTION_HEADER_HEIGHT: f64 = 24.0;
        const LIST_PADDING: f64 = 4.0;
        let max_scroll_height = if cx.display_context.is_desktop() {
            DESKTOP_MAX_SCROLL_HEIGHT
        } else {
            MOBILE_MAX_SCROLL_HEIGHT
        };
        let content_height = (items_added as f64 * SLASH_COMMAND_ITEM_HEIGHT)
            + (section_rows as f64 * SLASH_SECTION_HEADER_HEIGHT)
            + LIST_PADDING;
        self.set_list_scroll_height(cx, content_height.min(max_scroll_height));

        let popup = self.cmd_text_input.view(cx, ids!(popup));
        popup.set_visible(cx, items_added > 0);
        let text_input_area = self.cmd_text_input.text_input_ref().area();
        if cx.has_key_focus(text_input_area) {
            self.cmd_text_input.text_input_ref().set_key_focus(cx);
        }

        self.redraw(cx);
    }

    fn emit_primary_submit_action(&self, cx: &mut Cx, text: String) {
        let text_input = self.cmd_text_input.text_input(cx, ids!(text_input));
        cx.widget_action(
            text_input.widget_uid(),
            makepad_widgets::text_input::TextInputAction::Returned(
                text,
                primary_submit_modifiers(),
            ),
        );
    }

    fn open_slash_command_popup(&mut self, cx: &mut Cx, scope: &mut Scope) {
        let text_input_ref = self.cmd_text_input.text_input_ref();
        self.set_input_text_preserving_mentions(cx, "/");
        text_input_ref.set_cursor(
            cx,
            Cursor {
                index: 1,
                prefer_next_row: false,
            },
            false,
        );
        self.update_slash_command_list(cx, scope, "/");
        text_input_ref.set_key_focus(cx);
        // The button-click event flow races with focus commit: when the user
        // clicks bot_menu_button, focus lives on the button (not the text
        // input), so the Event::Actions block below sees `has_focus == false`
        // for the text input even though we just requested focus. Belt-and-
        // suspenders: draw_walk will keep retrying focus restore until it
        // sticks.
        self.pending_draw_focus_restore = true;
    }

    /// Update popup visibility and layout based on current state
    fn update_popup_visibility(&mut self, cx: &mut Cx, scope: &mut Scope, has_items: bool) {
        let popup = self.cmd_text_input.view(cx, ids!(popup));

        // Get current state from props
        let room_props = scope
            .props
            .get::<RoomScreenProps>()
            .expect("RoomScreenProps should be available in scope");
        let members_sync_pending = room_props.room_members_sync_pending;
        let members_available = room_props
            .room_members
            .as_ref()
            .is_some_and(|m| !m.is_empty());

        match &self.search_state {
            MentionSearchState::Idle | MentionSearchState::JustCancelled => {
                // Not in search mode, hide popup
                
                popup.set_visible(cx, false);
            }
            MentionSearchState::WaitingForMembers { .. } => {
                // Waiting for room members to be loaded
                self.show_loading_indicator(cx);
                popup.set_visible(cx, true);
                // Only restore focus if input currently has focus
                let text_input_area = self.cmd_text_input.text_input_ref().area();
                if cx.has_key_focus(text_input_area) {
                    self.cmd_text_input.text_input_ref().set_key_focus(cx);
                }
            }
            MentionSearchState::Searching {
                accumulated_results,
                ..
            } => {
                if has_items {
                    // We have search results to display
                    popup.set_visible(cx, true);
                    // Only restore focus if input currently has focus
                    let text_input_area = self.cmd_text_input.text_input_ref().area();
                    if cx.has_key_focus(text_input_area) {
                        self.cmd_text_input.text_input_ref().set_key_focus(cx);
                    }
                } else if accumulated_results.is_empty() {
                    if members_sync_pending || self.search_results_pending {
                        // Still fetching either member list or background search results.
                        self.show_loading_indicator(cx);
                    } else if members_available {
                        // Search completed with no results even though we have members.
                        self.show_no_matches_indicator(cx);
                    } else {
                        // No members available yet.
                        self.show_loading_indicator(cx);
                    }
                    popup.set_visible(cx, true);
                    // Only restore focus if input currently has focus
                    let text_input_area = self.cmd_text_input.text_input_ref().area();
                    if cx.has_key_focus(text_input_area) {
                        self.cmd_text_input.text_input_ref().set_key_focus(cx);
                    }
                } else {
                    // Has accumulated results but no items (should not happen)
                    popup.set_visible(cx, true);
                    // Only restore focus if input currently has focus
                    let text_input_area = self.cmd_text_input.text_input_ref().area();
                    if cx.has_key_focus(text_input_area) {
                        self.cmd_text_input.text_input_ref().set_key_focus(cx);
                    }
                }
            }
        }
    }

    /// Handles item selection from mention popup (either user or @room)
    fn on_user_selected(&mut self, cx: &mut Cx, _scope: &mut Scope, selected: WidgetRef) {
        // Note: We receive scope as parameter but don't use it in this method
        // This is good practice to maintain signature consistency with other methods
        // and allow for future scope-based enhancements

        let text_input_ref = self.cmd_text_input.text_input_ref();
        let current_text = text_input_ref.text();
        let head = text_input_ref.borrow().map_or(0, |p| p.cursor().index);

        if let Some(start_idx) = self.get_trigger_position() {
            let room_mention_label = selected.label(cx, ids!(room_mention));
            let room_mention_text = room_mention_label.text();
            let room_user_id_text = selected.label(cx, ids!(room_user_id)).text();

            let is_room_mention =
                { room_mention_text == "Notify the entire room" && room_user_id_text == "@room" };

            let mention_to_insert = if is_room_mention {
                // Always set to true, don't reset previously selected @room mentions
                self.possible_room_mention = true;
                "@room ".to_string()
            } else {
                // User selected a specific user
                let username = selected.label(cx, ids!(username)).text();
                let user_id_str = selected.label(cx, ids!(user_id)).text();
                let Ok(user_id): Result<OwnedUserId, _> = user_id_str.clone().try_into() else {
                    // Invalid user ID format - skip selection
                    return;
                };
                let (new_text, new_pos) = apply_user_mention_selection(
                    &current_text,
                    start_idx,
                    head,
                    Some(&username),
                    &user_id,
                    &mut self.tracked_visible_mentions,
                );
                self.set_input_text_preserving_mentions(cx, &new_text);
                text_input_ref.set_cursor(
                    cx,
                    Cursor {
                        index: new_pos,
                        prefer_next_row: false,
                    },
                    false,
                );
                finalize_popup_selection(cx, self);
                return;
            };

            let (new_text, new_pos) = apply_text_replacement_preserving_mentions(
                &current_text,
                start_idx,
                head,
                &mention_to_insert,
                &mut self.tracked_visible_mentions,
            );

            self.set_input_text_preserving_mentions(cx, &new_text);
            text_input_ref.set_cursor(
                cx,
                Cursor {
                    index: new_pos,
                    prefer_next_row: false,
                },
                false,
            );
        }
        finalize_popup_selection(cx, self);
    }

    fn on_slash_command_selected(&mut self, cx: &mut Cx, selected: WidgetRef) {
        let command = selected.label(cx, ids!(command_name)).text();
        if command.is_empty() {
            return;
        }

        if classify_known_slash_command_for_submission(&command)
            .is_some_and(|slash_command| !slash_command.needs_args)
        {
            self.close_mention_popup(cx);
            self.emit_primary_submit_action(cx, command);
            self.pending_draw_focus_restore = true;
            return;
        }

        let text_input_ref = self.cmd_text_input.text_input_ref();
        let current_text = text_input_ref.text();
        let head = text_input_ref.borrow().map_or(0, |p| p.cursor().index);

        if let Some(start_idx) = find_slash_command_trigger_position(&current_text, head) {
            let command_to_insert = format!("{command} ");
            let (new_text, new_pos) = apply_text_replacement_preserving_mentions(
                &current_text,
                start_idx,
                head,
                &command_to_insert,
                &mut self.tracked_visible_mentions,
            );

            self.set_input_text_preserving_mentions(cx, &new_text);
            text_input_ref.set_cursor(
                cx,
                Cursor {
                    index: new_pos,
                    prefer_next_row: false,
                },
                false,
            );
        }

        self.close_mention_popup(cx);
        self.pending_draw_focus_restore = true;
    }

    fn on_popup_item_selected(&mut self, cx: &mut Cx, scope: &mut Scope, selected: WidgetRef) {
        match self.active_popup_mode {
            PopupMode::Mention => self.on_user_selected(cx, scope, selected),
            PopupMode::SlashCommand => self.on_slash_command_selected(cx, selected),
            PopupMode::None => {}
        }
    }

    /// Core text change handler that manages mention context
    fn handle_text_change(&mut self, cx: &mut Cx, scope: &mut Scope, text: String) {
        let previous_text = std::mem::replace(&mut self.last_text, text.clone());
        let tracked_visible_mentions = std::mem::take(&mut self.tracked_visible_mentions);
        self.tracked_visible_mentions = reconcile_visible_mentions_after_text_change(
            &previous_text,
            &text,
            &tracked_visible_mentions,
        );

        // If search was just cancelled, clear the flag and don't re-trigger search
        if self.is_just_cancelled() {
            self.search_state = MentionSearchState::Idle;
        }

        // Check if text is empty or contains only whitespace
        let trimmed_text = text.trim();
        if trimmed_text.is_empty() {
            reset_visible_mention_tracking_for_programmatic_text_set(
                &mut self.tracked_visible_mentions,
                &mut self.possible_room_mention,
            );
            if self.is_searching() || self.is_slash_command_popup_active() {
                self.close_mention_popup(cx);
            }
            return;
        }

        if self.is_just_cancelled() {
            return;
        }

        let cursor_pos = self
            .cmd_text_input
            .text_input_ref()
            .borrow()
            .map_or(0, |p| p.cursor().index);

        // Check if we're currently searching and the @ symbol was deleted
        if self.active_popup_mode == PopupMode::Mention {
            if let Some(start_pos) = self.get_trigger_position() {
                // Check if the @ symbol at the start position still exists
                if start_pos >= text.len()
                    || text.get(start_pos..start_pos + 1).is_some_and(|c| c != "@")
                {
                    // The @ symbol was deleted, stop searching
                    self.close_mention_popup(cx);
                    return;
                }
            }
        }

        if self.active_popup_mode == PopupMode::SlashCommand
            && find_slash_command_trigger_position(&text, cursor_pos).is_none()
        {
            self.close_mention_popup(cx);
        }

        // Look for trigger position for @ menu
        if let Some(trigger_pos) = self.find_mention_trigger_position(&text, cursor_pos) {
            let search_text =
                utils::safe_substring_by_byte_indices(&text, trigger_pos + 1, cursor_pos);

            // Check if this is a continuation of existing search or a new one
            let is_new_search = self.get_trigger_position() != Some(trigger_pos);

            if is_new_search {
                // This is a new @ mention, reset everything
                self.last_search_text = None;
            } else {
                // User is editing existing mention, don't reset search state
                // This allows smooth deletion/modification of search text
                // But clear last_search_text if the new text is different to trigger search
                if self.last_search_text.as_ref() != Some(&search_text) {
                    self.last_search_text = None;
                }
            }

            // Ensure header view is visible to prevent header disappearing during consecutive @mentions
            let popup = self.cmd_text_input.view(cx, ids!(popup));
            let header_view = self.cmd_text_input.view(cx, ids!(popup.header_view));
            header_view.set_visible(cx, true);

            // Transition to appropriate state and update user list
            // update_user_list will handle state transition properly
            self.update_user_list(cx, &search_text, scope);

            popup.set_visible(cx, true);

            // Immediately check for results instead of waiting for next frame
            self.check_search_channel(cx, scope);

            // Redraw to ensure UI updates are visible
            self.redraw(cx);
        } else if let Some(trigger_pos) = find_slash_command_trigger_position(&text, cursor_pos) {
            let search_text =
                utils::safe_substring_by_byte_indices(&text, trigger_pos + 1, cursor_pos);
            self.update_slash_command_list(cx, scope, &search_text);
        } else if self.is_searching() || self.is_slash_command_popup_active() {
            self.close_mention_popup(cx);
        }
    }

    /// Check the search channel for new results
    /// Returns true if we should continue checking for more results
    fn check_search_channel(&mut self, cx: &mut Cx, scope: &mut Scope) -> bool {
        // Only check if we're in Searching state
        let mut is_complete = false;
        let mut search_text: Option<Arc<String>> = None;
        let mut any_results = false;
        let mut should_update_ui = false;
        let mut new_results = Vec::new();

        // Process all available results from the channel
        if let MentionSearchState::Searching {
            receiver,
            accumulated_results,
            search_id,
            ..
        } = &mut self.search_state
        {
            while let Ok(result) = receiver.try_recv() {
                if result.search_id != *search_id {
                    continue;
                }

                any_results = true;
                search_text = Some(result.search_text.clone());
                is_complete = result.is_complete;

                // Collect results
                if !result.results.is_empty() {
                    new_results.extend(result.results);
                    should_update_ui = true;
                }
            }

            if !new_results.is_empty() {
                accumulated_results.extend(new_results);
            }
        } else {
            return false;
        }

        // Update UI immediately if we got new results
        if should_update_ui {
            if matches!(
                &self.search_state,
                MentionSearchState::Searching { accumulated_results, .. }
                if !accumulated_results.is_empty()
            ) {
                // Results are already sorted in member_search.rs and indices are unique
                let query = search_text
                    .as_ref()
                    .map(|s| s.as_str())
                    .unwrap_or_default();
                self.update_ui_with_results(cx, scope, query);
            }
        }

        // Handle completion
        if is_complete {
            self.search_results_pending = false;
            // Search is complete - get results for final UI update
            if matches!(
                &self.search_state,
                MentionSearchState::Searching { accumulated_results, .. }
                if accumulated_results.is_empty()
            ) {
                // No user results, but still update UI (may show @room)
                let query = search_text
                    .as_ref()
                    .map(|s| s.as_str())
                    .unwrap_or_default();
                self.update_ui_with_results(cx, scope, query);
            }

            // Don't change state here - let update_ui_with_results handle it
        } else if !any_results {
            // No results received yet - check if channel is still open
            let disconnected =
                if let MentionSearchState::Searching { receiver, .. } = &self.search_state {
                    matches!(
                        receiver.try_recv(),
                        Err(std::sync::mpsc::TryRecvError::Disconnected)
                    )
                } else {
                    false
                };

            if disconnected {
                // Channel was closed - search completed or failed
                self.search_results_pending = false;
                self.handle_search_channel_closed(cx, scope);
                // Stop checking - channel is closed, no more results will arrive
                return false;
            }
        }

        // Return whether we should continue checking for results
        !is_complete && matches!(self.search_state, MentionSearchState::Searching { .. })
    }

    /// Common UI update logic for both streaming and non-streaming results
    fn update_ui_with_results(&mut self, cx: &mut Cx, scope: &mut Scope, search_text: &str) {
        let room_props = scope
            .props
            .get::<RoomScreenProps>()
            .expect("RoomScreenProps should be available in scope for MentionableTextInput");

        // If we're in Searching state, we have local data - always show results
        // Don't wait for remote sync to complete
        // Remote sync will trigger update when it completes (if data changed)
        self.cmd_text_input.clear_items(cx);
        self.loading_indicator_ref = None;

        let is_desktop = cx.display_context.is_desktop();
        let max_display_items: usize = if is_desktop {
            DESKTOP_MAX_DISPLAY_ITEMS
        } else {
            MOBILE_MAX_DISPLAY_ITEMS
        };
        let mut items_added = 0;

        // 4. Try to add @room mention item
        let has_room_item = self.try_add_room_mention_item(cx, search_text, room_props, is_desktop);
        if has_room_item {
            items_added += 1;
        }

        // Get accumulated results from current state
        let results_to_display = if let MentionSearchState::Searching {
            accumulated_results,
            ..
        } = &self.search_state
        {
            accumulated_results.clone()
        } else {
            Vec::new()
        };

        // Add user mention items using the results
        if !results_to_display.is_empty() {
            let user_items_limit = max_display_items.saturating_sub(has_room_item as usize);
            let user_items_added = self.add_user_mention_items_from_results(
                cx,
                &results_to_display,
                user_items_limit,
                is_desktop,
                room_props,
            );
            items_added += user_items_added;
        }

        // If remote sync is still in progress, add loading indicator after results
        // This gives visual feedback that more members may be loading
        // IMPORTANT: Don't call show_loading_indicator here as it calls clear_items()
        // which would remove the user list we just added
        if room_props.room_members_sync_pending {
            // Add loading indicator widget without clearing existing items
            if let Some(ptr) = self.loading_indicator {
                let loading_item = crate::widget_ref_from_live_ptr(cx, Some(ptr));
                self.add_popup_status_item(cx, loading_item.clone(), PopupStatusItemKind::Loading);
                self.loading_indicator_ref = Some(loading_item.clone());

                // Start the loading animation
                loading_item
                    .bouncing_dots(cx, ids!(loading_animation))
                    .start_animation(cx);
                cx.new_next_frame();

                items_added += 1;
            }
        }

        // Dynamically adjust the scroll container height based on item count.
        // ScrollYView needs a fixed height viewport. We default to 360px in the DSL,
        // but shrink it when there are fewer items to keep the popup compact.
        {
            const USER_ITEM_HEIGHT: f64 = 36.0;
            const ROOM_ITEM_HEIGHT: f64 = 40.0;
            const STATUS_ITEM_HEIGHT: f64 = 48.0;
            const LIST_PADDING: f64 = 4.0; // top: 2 + bottom: 2

            let max_scroll_height = if is_desktop {
                DESKTOP_MAX_SCROLL_HEIGHT
            } else {
                MOBILE_MAX_SCROLL_HEIGHT
            };

            // Estimate content height from items added
            let content_height = if items_added == 0 {
                0.0
            } else {
                let user_count = items_added.saturating_sub(has_room_item as usize)
                    .saturating_sub(if room_props.room_members_sync_pending { 1 } else { 0 });
                let room_count = has_room_item as usize;
                let loading_count = if room_props.room_members_sync_pending { 1 } else { 0 };
                (user_count as f64 * USER_ITEM_HEIGHT)
                    + (room_count as f64 * ROOM_ITEM_HEIGHT)
                    + (loading_count as f64 * STATUS_ITEM_HEIGHT)
                    + LIST_PADDING
            };

            let scroll_height = content_height.min(max_scroll_height).max(0.0);
            self.set_list_scroll_height(cx, scroll_height);
        }

        // Update popup visibility based on whether we have items
        self.update_popup_visibility(cx, scope, items_added > 0);

        // Force immediate redraw to ensure UI updates are visible
        self.redraw(cx);
    }

    /// Updates the mention suggestion list based on search
    fn update_user_list(&mut self, cx: &mut Cx, search_text: &str, scope: &mut Scope) {
        // Get room_props to read real-time member state from props (single source of truth)
        let room_props = scope
            .props
            .get::<RoomScreenProps>()
            .expect("RoomScreenProps should be available in scope for MentionableTextInput");

        self.active_popup_mode = PopupMode::Mention;
        self.set_popup_header_for_mentions(cx);

        // Get trigger position from current state (if in searching mode)
        let trigger_pos = match &self.search_state {
            MentionSearchState::WaitingForMembers {
                trigger_position, ..
            }
            | MentionSearchState::Searching {
                trigger_position, ..
            } => *trigger_position,
            _ => {
                // Not in searching mode, need to determine trigger position
                if let Some(pos) = self.find_mention_trigger_position(
                    &self.cmd_text_input.text_input_ref().text(),
                    self.cmd_text_input
                        .text_input_ref()
                        .borrow()
                        .map_or(0, |p| p.cursor().index),
                ) {
                    pos
                } else {
                    return;
                }
            }
        };

        // Skip if search text hasn't changed AND we're already in Searching state
        // Don't skip if we're in WaitingForMembers - need to transition to Searching
        if self.last_search_text.as_deref() == Some(search_text) {
            if matches!(self.search_state, MentionSearchState::Searching { .. }) {
                return; // Already searching with same text, skip
            }
            // In WaitingForMembers with same text -> need to start search now that members arrived
        }

        self.last_search_text = Some(search_text.to_string());

        // Reset scroll to top for a new search round
        self.cmd_text_input.reset_list_scroll(cx);

        let is_desktop = cx.display_context.is_desktop();
        let max_display_items = if is_desktop {
            DESKTOP_MAX_DISPLAY_ITEMS
        } else {
            MOBILE_MAX_DISPLAY_ITEMS
        };
        let members_sync_pending = room_props.room_members_sync_pending;
        let cached_member_count = room_props
            .room_members
            .as_ref()
            .map(|members| members.len())
            .unwrap_or(0);

        let cached_members = match &room_props.room_members {
            Some(members)
                if member_list_ready_for_mentions(cached_member_count, members_sync_pending) =>
            {
                // Members available, continue to search
                members.clone()
            }
            _ => {
                let already_waiting = matches!(
                    self.search_state,
                    MentionSearchState::WaitingForMembers { .. }
                );
                let needs_local_member_fetch = cached_member_count == 0 && !members_sync_pending;

                self.cancel_active_search();

                if !already_waiting && needs_local_member_fetch {
                    submit_async_request(MatrixRequest::GetRoomMembers {
                        timeline_kind: crate::sliding_sync::TimelineKind::MainRoom { room_id: room_props.room_name_id.room_id().clone() },
                        memberships: RoomMemberships::JOIN,
                        local_only: true,
                    });
                }

                self.search_state = MentionSearchState::WaitingForMembers {
                    trigger_position: trigger_pos,
                    pending_search_text: search_text.to_string(),
                };

                // Clear old items before showing loading indicator
                self.cmd_text_input.clear_items(cx);
                self.show_loading_indicator(cx);
                // Request next frame to check when members are loaded
                cx.new_next_frame();
                return; // Don't submit search request yet
            }
        };

        // We have cached members, ensure popup is visible
        let popup = self.cmd_text_input.view(cx, ids!(popup));
        let header_view = self.cmd_text_input.view(cx, ids!(popup.header_view));
        header_view.set_visible(cx, true);
        popup.set_visible(cx, true);
        // Only restore focus if input currently has focus
        let text_input_area = self.cmd_text_input.text_input_ref().area();
        if cx.has_key_focus(text_input_area) {
            self.cmd_text_input.text_input_ref().set_key_focus(cx);
        }

        // Create a new channel for this search
        let (sender, receiver) = std::sync::mpsc::channel();

        // Prepare background search job parameters
        let search_text_clone = search_text.to_string();
        let max_results = max_display_items * SEARCH_BUFFER_MULTIPLIER;
        let search_id = self.allocate_search_id();

        // Transition to Searching state with new receiver
        self.cancel_active_search();
        let cancel_token = Arc::new(AtomicBool::new(false));
        self.search_state = MentionSearchState::Searching {
            trigger_position: trigger_pos,
            search_text: search_text.to_string(),
            receiver,
            accumulated_results: Vec::new(),
            search_id,
            cancel_token: cancel_token.clone(),
        };
        self.search_results_pending = true;

        let precomputed_sort = room_props.room_members_sort.clone();
        let cancel_token_for_job = cancel_token.clone();
        cpu_worker::spawn_cpu_job(cx, CpuJob::SearchRoomMembers(SearchRoomMembersJob {
            members: cached_members,
            search_text: search_text_clone,
            max_results,
            sender,
            search_id,
            precomputed_sort,
            cancel_token: Some(cancel_token_for_job),
        }));

        // Request next frame to check the channel
        cx.new_next_frame();

        // Try to check immediately for faster response
        self.check_search_channel(cx, scope);
    }

    /// Detects valid mention trigger positions in text
    fn find_mention_trigger_position(&mut self, text: &str, cursor_pos: usize) -> Option<usize> {
        if cursor_pos == 0 {
            return None;
        }

        // Ensure cache is up-to-date (rebuild only if text changed)
        let needs_rebuild = self.cached_text_analysis.as_ref()
            .is_none_or(|(cached_text, _, _)| cached_text != text);
        if needs_rebuild {
            let graphemes_owned: Vec<String> = text.graphemes(true).map(|s| s.to_string()).collect();
            let positions = utils::build_grapheme_byte_positions(text);
            self.cached_text_analysis = Some((text.to_string(), graphemes_owned, positions));
        }

        // Borrow directly from cache — no clone needed
        let (_, text_graphemes_owned, byte_positions) = self.cached_text_analysis.as_ref().unwrap();
        let text_graphemes: Vec<&str> = text_graphemes_owned.iter().map(|s| s.as_str()).collect();

        // Use utility function to convert byte position to grapheme index
        let cursor_grapheme_idx = utils::byte_index_to_grapheme_index(text, cursor_pos);

        // Simple logic: trigger when cursor is immediately after @ symbol
        // Only trigger if @ is preceded by whitespace or beginning of text
        if cursor_grapheme_idx > 0 && text_graphemes.get(cursor_grapheme_idx - 1) == Some(&"@") {
            let is_preceded_by_whitespace_or_start = cursor_grapheme_idx == 1
                || (cursor_grapheme_idx > 1
                    && text_graphemes
                        .get(cursor_grapheme_idx - 2)
                        .is_some_and(|g| g.trim().is_empty()));
            if is_preceded_by_whitespace_or_start {
                if let Some(&byte_pos) = byte_positions.get(cursor_grapheme_idx - 1) {
                    return Some(byte_pos);
                }
            }
        }

        // Find the last @ symbol before the cursor for search continuation
        // Only continue if we're already in search mode
        if self.is_searching() {
            let last_at_pos = text_graphemes.get(..cursor_grapheme_idx).and_then(|slice| {
                slice
                    .iter()
                    .enumerate()
                    .filter(|(_, g)| **g == "@")
                    .map(|(i, _)| i)
                    .next_back()
            });

            if let Some(at_idx) = last_at_pos {
                // Get the byte position of this @ symbol
                let &at_byte_pos = byte_positions.get(at_idx)?;

                // Extract the text after the @ symbol up to the cursor position
                let mention_text = text_graphemes
                    .get(at_idx + 1..cursor_grapheme_idx)
                    .unwrap_or(&[]);

                // Only trigger if this looks like an ongoing mention (contains only alphanumeric and basic chars)
                if self.is_valid_mention_text(mention_text) {
                    return Some(at_byte_pos);
                }
            }
        }

        None
    }

    /// Simple validation for mention text
    fn is_valid_mention_text(&self, graphemes: &[&str]) -> bool {
        // Allow empty text (for @)
        if graphemes.is_empty() {
            return true;
        }

        // Check if it contains newline characters
        !graphemes.iter().any(|g| g.contains('\n'))
    }

    /// Shows the loading indicator when waiting for initial members to be loaded
    fn show_loading_indicator(&mut self, cx: &mut Cx) {
        // Rebuild the popup body every time. Mention search can request loading twice
        // within the same actions batch (`should_build_items` and `Changed`), and the
        // second pass may have already cleared the list before we get here again.
        self.cmd_text_input.clear_items(cx);

        let loading_item = if let Some(existing_indicator) = self.loading_indicator_ref.clone() {
            existing_indicator
        } else {
            let Some(ptr) = self.loading_indicator else {
                return;
            };
            crate::widget_ref_from_live_ptr(cx, Some(ptr))
        };

        self.add_popup_status_item(cx, loading_item.clone(), PopupStatusItemKind::Loading);
        self.loading_indicator_ref = Some(loading_item.clone());
        loading_item.bouncing_dots(cx, ids!(loading_animation)).start_animation(cx);
        cx.new_next_frame();

        // Setup popup dimensions for loading state
        let popup = self.cmd_text_input.view(cx, ids!(popup));
        let header_view = self.cmd_text_input.view(cx, ids!(popup.header_view));

        // Ensure header is visible
        header_view.set_visible(cx, true);

        // Set scroll container height to fit the loading indicator (48px + 4px padding)
        self.set_list_scroll_height(cx, 52.0);

        popup.set_visible(cx, true);

        // Maintain text input focus only if it currently has focus
        let text_input_area = self.cmd_text_input.text_input_ref().area();
        if self.is_searching() && cx.has_key_focus(text_input_area) {
            self.cmd_text_input.text_input_ref().set_key_focus(cx);
        }
    }

    /// Shows the no matches indicator when no users match the search
    fn show_no_matches_indicator(&mut self, cx: &mut Cx) {
        // Clear any existing items
        self.cmd_text_input.clear_items(cx);

        // Create no matches indicator widget
        let Some(ptr) = self.no_matches_indicator else {
            return;
        };
        let no_matches_item = crate::widget_ref_from_live_ptr(cx, Some(ptr));

        // Add the no matches indicator to the popup
        self.add_popup_status_item(cx, no_matches_item, PopupStatusItemKind::NoMatches);
        self.loading_indicator_ref = None;

        // Setup popup dimensions for no matches state
        let header_view = self.cmd_text_input.view(cx, ids!(popup.header_view));

        // Ensure header is visible
        header_view.set_visible(cx, true);

        // Set scroll container height to fit the no-matches indicator (48px + 4px padding)
        self.set_list_scroll_height(cx, 52.0);

        // Maintain text input focus so user can continue typing, but only if currently focused
        let text_input_area = self.cmd_text_input.text_input_ref().area();
        if self.is_searching() && cx.has_key_focus(text_input_area) {
            self.cmd_text_input.text_input_ref().set_key_focus(cx);
        }
    }

    /// Check if mention search is currently active
    pub fn is_mention_searching(&self) -> bool {
        self.is_searching()
    }

    /// Check if ESC was handled by mention popup
    pub fn handled_escape(&self) -> bool {
        self.is_just_cancelled()
    }

    /// Handle search channel closed event
    fn handle_search_channel_closed(&mut self, cx: &mut Cx, scope: &mut Scope) {
        // Get accumulated results before changing state
        let has_results = if let MentionSearchState::Searching {
            accumulated_results,
            ..
        } = &self.search_state
        {
            !accumulated_results.is_empty()
        } else {
            false
        };

        // If no results were shown, show empty state
        if !has_results {
            self.update_ui_with_results(cx, scope, "");
        }

        // Keep searching state but mark search as complete
        // The state will be reset when user types or closes popup
    }

    fn cancel_active_search(&mut self) {
        match &self.search_state {
            MentionSearchState::Searching { cancel_token, .. } => {
                cancel_token.store(true, Ordering::Relaxed);
            }
            MentionSearchState::WaitingForMembers { .. } => {
                // WaitingForMembers has no cancel_token, but we need to mark as cancelled.
                // The state will be set to JustCancelled by the caller, which prevents
                // RoomMembersLoaded from reopening the popup.
            }
            _ => {}
        }
        self.search_results_pending = false;
    }

    /// Reset all search-related state
    fn reset_search_state(&mut self, cx: &Cx) {
        self.cancel_active_search();

        // Reset to idle state
        self.search_state = MentionSearchState::Idle;

        // Reset last search text to allow new searches
        self.last_search_text = None;
        self.search_results_pending = false;
        self.loading_indicator_ref = None;
        self.active_popup_mode = PopupMode::None;

        // Reset change detection state
        self.last_member_count = 0;
        self.last_sync_pending = false;
        self.pending_popup_cleanup = false;

        // Clear list items
        self.cmd_text_input.clear_items(cx);
    }

    /// Cleanup helper for closing mention popup
    fn close_mention_popup(&mut self, cx: &mut Cx) {
        // Reset all search-related state
        self.reset_search_state(cx);

        // Get popup and header view references
        let popup = self.cmd_text_input.view(cx, ids!(popup));
        let header_view = self.cmd_text_input.view(cx, ids!(popup.header_view));

        // Force hide header view - necessary when handling deletion operations
        // When backspace-deleting mentions, we want to completely hide the header
        header_view.set_visible(cx, false);

        // Hide the entire popup
        popup.set_visible(cx, false);

        // Reset popup height
        

        // Ensure header view is reset to visible next time it's triggered
        // This will happen before update_user_list is called in handle_text_change

        // Note: Do NOT call request_text_input_focus() here.
        // Focus restoration is handled solely via `pending_draw_focus_restore` in draw_walk
        // to avoid race conditions between multiple focus mechanisms.
        self.cmd_text_input.redraw(cx);
    }

    /// Returns the current text content
    pub fn text(&self) -> String {
        self.cmd_text_input.text_input_ref().text()
    }

    /// Sets the text content
    pub fn set_text(&mut self, cx: &mut Cx, text: &str) {
        reset_visible_mention_tracking_for_programmatic_text_set(
            &mut self.tracked_visible_mentions,
            &mut self.possible_room_mention,
        );
        self.set_input_text_preserving_mentions(cx, text);
    }

    /// Sets whether the current user can notify the entire room (@room mention)
    pub fn set_can_notify_room(&mut self, can_notify: bool) {
        self.can_notify_room = can_notify;
    }

    /// Gets whether the current user can notify the entire room (@room mention)
    pub fn can_notify_room(&self) -> bool {
        self.can_notify_room
    }

    fn set_input_text_preserving_mentions(&mut self, cx: &mut Cx, text: &str) {
        self.cmd_text_input.text_input_ref().set_text(cx, text);
        self.last_text = text.to_owned();
        self.cmd_text_input.redraw(cx);
    }
}

impl MentionableTextInputRef {
    pub fn text(&self) -> String {
        self.borrow().map_or_else(String::new, |inner| inner.text())
    }

    /// Returns a reference to the inner `TextInput` widget.
    pub fn text_input_ref(&self) -> TextInputRef {
        self.borrow()
            .map(|inner| inner.cmd_text_input.text_input_ref())
            .unwrap_or_default()
    }

    /// Check if mention search is currently active
    pub fn is_mention_searching(&self) -> bool {
        self.borrow()
            .is_some_and(|inner| inner.is_mention_searching())
    }

    /// Check if ESC was handled by mention popup
    pub fn handled_escape(&self) -> bool {
        self.borrow().is_some_and(|inner| inner.handled_escape())
    }

    pub fn set_text(&self, cx: &mut Cx, text: &str) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_text(cx, text);
        }
    }

    pub fn open_slash_command_popup(&self, cx: &mut Cx, scope: &mut Scope) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.open_slash_command_popup(cx, scope);
        }
    }

    /// Sets whether the current user can notify the entire room (@room mention)
    pub fn set_can_notify_room(&self, can_notify: bool) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_can_notify_room(can_notify);
        }
    }

    /// Gets whether the current user can notify the entire room (@room mention)
    pub fn can_notify_room(&self) -> bool {
        self.borrow().is_some_and(|inner| inner.can_notify_room())
    }

    /// Processes entered text and creates a message with mentions based on detected message type.
    /// This method handles /html, /plain prefixes and defaults to markdown.
    pub fn create_message_with_mentions(&self, entered_text: &str) -> RoomMessageEventContent {
        let Some(inner) = self.borrow() else {
            return create_message_with_tracked_mentions(entered_text, &[], false);
        };

        create_message_with_tracked_mentions(
            entered_text,
            &inner.tracked_visible_mentions,
            inner.possible_room_mention,
        )
    }

    pub fn create_message_with_mentions_for_submission(
        &self,
        entered_text: &str,
    ) -> RoomMessageEventContent {
        let Some(inner) = self.borrow() else {
            let normalized_text = normalize_command_with_at_suffix_for_send(entered_text);
            return create_message_with_tracked_mentions(&normalized_text, &[], false);
        };

        let normalized_text = normalize_command_with_at_suffix_for_send(entered_text);
        if normalized_text == entered_text {
            return create_message_with_tracked_mentions(
                entered_text,
                &inner.tracked_visible_mentions,
                inner.possible_room_mention,
            );
        }

        let adjusted_mentions = reconcile_visible_mentions_after_text_change(
            entered_text,
            &normalized_text,
            &inner.tracked_visible_mentions,
        );
        create_message_with_tracked_mentions(
            &normalized_text,
            &adjusted_mentions,
            inner.possible_room_mention,
        )
    }
}

fn create_message_with_tracked_mentions(
    entered_text: &str,
    tracked_visible_mentions: &[TrackedVisibleMention],
    possible_room_mention: bool,
) -> RoomMessageEventContent {
    if let Some(html_text) = entered_text.strip_prefix("/html") {
        let resolved = resolve_visible_mentions_for_send(
            html_text,
            tracked_visible_mentions,
            possible_room_mention,
        );
        let message = RoomMessageEventContent::text_html(html_text, resolved.html_text);
        message.add_mentions(resolved.mentions)
    } else if let Some(plain_text) = entered_text.strip_prefix("/plain") {
        // Plain text messages don't support mentions
        RoomMessageEventContent::text_plain(plain_text)
    } else {
        let resolved = resolve_visible_mentions_for_send(
            entered_text,
            tracked_visible_mentions,
            possible_room_mention,
        );
        let message = RoomMessageEventContent::text_markdown(resolved.markdown_text);
        message.add_mentions(resolved.mentions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use matrix_sdk::ruma::events::room::message::MessageType;

    #[test]
    fn popup_status_items_are_never_selectable() {
        assert!(!popup_status_item_is_selectable(PopupStatusItemKind::Loading));
        assert!(!popup_status_item_is_selectable(PopupStatusItemKind::NoMatches));
    }

    #[test]
    fn member_list_waits_while_sync_is_pending() {
        assert!(!member_list_ready_for_mentions(1, true));
        assert!(!member_list_ready_for_mentions(0, false));
        assert!(member_list_ready_for_mentions(3, false));
    }

    #[test]
    fn member_count_change_refreshes_active_search() {
        assert!(member_data_change_requires_popup_refresh(
            2,
            5,
            false,
            false,
            &MentionSearchState::Searching {
                trigger_position: 0,
                search_text: "al".to_owned(),
                receiver: std::sync::mpsc::channel().1,
                accumulated_results: Vec::new(),
                search_id: 1,
                cancel_token: Arc::new(AtomicBool::new(false)),
            },
        ));
    }

    #[test]
    fn member_count_change_refreshes_waiting_search_when_members_arrive() {
        assert!(member_data_change_requires_popup_refresh(
            0,
            3,
            false,
            true,
            &MentionSearchState::WaitingForMembers {
                trigger_position: 0,
                pending_search_text: "al".to_owned(),
            },
        ));
    }

    #[test]
    fn unchanged_member_count_does_not_refresh_popup() {
        assert!(!member_data_change_requires_popup_refresh(
            3,
            3,
            false,
            false,
            &MentionSearchState::Searching {
                trigger_position: 0,
                search_text: "al".to_owned(),
                receiver: std::sync::mpsc::channel().1,
                accumulated_results: Vec::new(),
                search_id: 1,
                cancel_token: Arc::new(AtomicBool::new(false)),
            },
        ));
    }

    #[test]
    fn sync_completion_refreshes_popup_even_without_member_count_change() {
        assert!(member_data_change_requires_popup_refresh(
            1,
            1,
            true,
            false,
            &MentionSearchState::WaitingForMembers {
                trigger_position: 0,
                pending_search_text: "al".to_owned(),
            },
        ));
    }

    #[test]
    fn slash_command_popup_requires_management_bot_room() {
        let parent_bot = OwnedUserId::try_from("@octosbot:example.com").unwrap();
        let child_bot = OwnedUserId::try_from("@octosbot_child:example.com").unwrap();
        let mismatched_parent = OwnedUserId::try_from("@bot:example.com").unwrap();

        assert!(bot_command_popup_enabled(
            true,
            true,
            false,
            Some(&parent_bot),
            Some(&parent_bot),
            &[],
        ));
        assert!(!bot_command_popup_enabled(
            true,
            false,
            false,
            Some(&parent_bot),
            Some(&parent_bot),
            &[],
        ));
        assert!(!bot_command_popup_enabled(
            true,
            false,
            true,
            Some(&child_bot),
            Some(&parent_bot),
            std::slice::from_ref(&child_bot),
        ));
        assert!(!bot_command_popup_enabled(
            true,
            false,
            false,
            None,
            Some(&parent_bot),
            &[],
        ));
        assert!(!bot_command_popup_enabled(
            true,
            false,
            true,
            Some(&parent_bot),
            Some(&mismatched_parent),
            &[],
        ));
        assert!(!bot_command_popup_enabled(
            true,
            false,
            false,
            Some(&parent_bot),
            Some(&mismatched_parent),
            &[],
        ));
        assert!(!bot_command_popup_enabled(
            false,
            false,
            true,
            Some(&parent_bot),
            Some(&parent_bot),
            &[],
        ));
    }

    #[test]
    fn slash_command_trigger_is_found_at_input_start() {
        assert_eq!(find_slash_command_trigger_position("/li", "/li".len()), Some(0));
    }

    #[test]
    fn slash_command_trigger_is_found_after_newline() {
        let text = "hello\n/list";
        assert_eq!(
            find_slash_command_trigger_position(text, text.len()),
            Some("hello\n".len())
        );
    }

    #[test]
    fn slash_command_trigger_is_not_found_mid_line() {
        let text = "hello /list";
        assert_eq!(find_slash_command_trigger_position(text, text.len()), None);
    }

    #[test]
    fn slash_command_trigger_is_found_after_leading_mention() {
        // The demo pattern: @-mention the coordinator, then type the command.
        let text = "@wf_coordinator /cre";
        assert_eq!(
            find_slash_command_trigger_position(text, text.len()),
            Some("@wf_coordinator ".len())
        );
        let text2 = "@wf_coordinator @wf_reviewer /st";
        assert_eq!(
            find_slash_command_trigger_position(text2, text2.len()),
            Some("@wf_coordinator @wf_reviewer ".len())
        );
    }

    #[test]
    fn slash_command_trigger_rejected_when_plain_word_precedes() {
        // A non-mention word before the command must NOT trigger (e.g. a file path).
        let text = "@wf_coordinator see /tmp";
        assert_eq!(find_slash_command_trigger_position(text, text.len()), None);
    }

    #[cfg(feature = "agent_chat")]
    #[test]
    fn workflow_coordinator_name_matches_any_team() {
        // The `/` workflow popup must enable for ANY team's coordinator, not just wf_.
        assert!(name_is_workflow_coordinator("coordinator"));
        assert!(name_is_workflow_coordinator("wf_coordinator"));
        assert!(name_is_workflow_coordinator("alpha_coordinator"));
        assert!(name_is_workflow_coordinator("ac_beta_coordinator")); // MXID localpart form
        assert!(!name_is_workflow_coordinator("wf_implementer"));
        assert!(!name_is_workflow_coordinator("coordinatorx"));
        assert!(!name_is_workflow_coordinator("alex"));
    }

    #[test]
    fn slash_commands_filter_by_prefix_without_leading_slash() {
        let commands = matching_slash_commands_in(SLASH_COMMANDS, "li");
        assert_eq!(commands, vec![SlashCommand {
            command: "/listbots",
            description_key: "slash_command.listbots.description",
            needs_args: false,
        }]);
    }

    #[test]
    fn slash_commands_return_empty_for_unknown_prefix() {
        assert!(matching_slash_commands_in(SLASH_COMMANDS, "zzzznotacommand").is_empty());
    }

    #[test]
    fn classify_known_slash_command_for_submission_matches_first_token() {
        assert_eq!(
            classify_known_slash_command_for_submission("/listbots"),
            Some(SlashCommand {
                command: "/listbots",
                description_key: "slash_command.listbots.description",
                needs_args: false,
            })
        );
        assert_eq!(
            classify_known_slash_command_for_submission("/createbot weather Weather Bot"),
            Some(SlashCommand {
                command: "/createbot",
                description_key: "slash_command.createbot.description",
                needs_args: true,
            })
        );
        assert_eq!(classify_known_slash_command_for_submission("/unknown arg"), None);
    }

    #[test]
    fn test_parse_command_at_localpart() {
        assert_eq!(
            parse_command_with_at_suffix("/listbots@octosbot_weather"),
            Some(ParsedSlashCommand {
                command: "/listbots".to_owned(),
                target_localpart: Some("octosbot_weather".to_owned()),
            })
        );
    }

    #[test]
    fn test_parse_command_at_full_mxid() {
        assert_eq!(
            parse_command_with_at_suffix("/listbots@octosbot:127.0.0.1:8128"),
            Some(ParsedSlashCommand {
                command: "/listbots".to_owned(),
                target_localpart: Some("octosbot".to_owned()),
            })
        );
    }

    #[test]
    fn test_parse_bare_command_no_target() {
        assert_eq!(
            parse_command_with_at_suffix("/listbots"),
            Some(ParsedSlashCommand {
                command: "/listbots".to_owned(),
                target_localpart: None,
            })
        );
    }

    #[test]
    fn test_parse_command_at_with_args() {
        assert_eq!(
            parse_command_with_at_suffix("/createbot@octosbot weather Weather Bot"),
            Some(ParsedSlashCommand {
                command: "/createbot".to_owned(),
                target_localpart: Some("octosbot".to_owned()),
            })
        );
    }

    #[test]
    fn test_parser_rejects_bare_mention() {
        assert_eq!(parse_command_with_at_suffix("@octosbot hello"), None);
    }

    #[test]
    fn test_parser_rejects_space_before_at() {
        assert_eq!(
            parse_command_with_at_suffix("/listbots @octosbot"),
            Some(ParsedSlashCommand {
                command: "/listbots".to_owned(),
                target_localpart: None,
            })
        );
    }

    #[test]
    fn test_parameterized_command_at_bot_normalizes() {
        assert_eq!(
            normalize_command_with_at_suffix_for_send("/createbot@octosbot weather Weather Bot"),
            "/createbot weather Weather Bot".to_owned(),
        );
    }

    #[test]
    fn test_input_box_preserves_at_bot_during_typing() {
        let visible_text = "/listbots@octosbot";

        assert_eq!(visible_text, "/listbots@octosbot");
        assert_eq!(
            normalize_command_with_at_suffix_for_send(visible_text),
            "/listbots".to_owned(),
        );
    }

    #[test]
    fn test_editing_inside_visible_mention_clears_tracking_for_that_mention() {
        let alice: OwnedUserId = "@alice:example.com".try_into().expect("valid user id");
        let bob: OwnedUserId = "@bob:example.com".try_into().expect("valid user id");
        let old_text = "hello @Alice and @Bob";
        let new_text = "hello @Alicia and @Bob";
        let tracked_mentions = vec![
            TrackedVisibleMention {
                user_id: alice.clone(),
                visible_text: "@Alice".to_owned(),
                start: "hello ".len(),
                end: "hello @Alice".len(),
            },
            TrackedVisibleMention {
                user_id: bob.clone(),
                visible_text: "@Bob".to_owned(),
                start: "hello @Alice and ".len(),
                end: "hello @Alice and @Bob".len(),
            },
        ];

        let reconciled = reconcile_visible_mentions_after_text_change(
            old_text,
            new_text,
            &tracked_mentions,
        );

        assert_eq!(reconciled.len(), 1);
        let tracked = &reconciled[0];
        assert_eq!(tracked.user_id, bob);
        assert_eq!(tracked.visible_text, "@Bob");
        assert_eq!(tracked.start, "hello @Alicia and ".len());
        assert_eq!(tracked.end, "hello @Alicia and @Bob".len());
    }

    #[test]
    fn test_duplicate_display_name_mentions_preserve_distinct_user_ids() {
        let first: OwnedUserId = "@sam:example.com".try_into().expect("valid user id");
        let second: OwnedUserId = "@sam:other.example".try_into().expect("valid user id");
        let old_text = "@Sam and @Sam";
        let new_text = "yo @Sam and @Sam";
        let tracked_mentions = vec![
            TrackedVisibleMention {
                user_id: first.clone(),
                visible_text: "@Sam".to_owned(),
                start: 0,
                end: "@Sam".len(),
            },
            TrackedVisibleMention {
                user_id: second.clone(),
                visible_text: "@Sam".to_owned(),
                start: "@Sam and ".len(),
                end: "@Sam and @Sam".len(),
            },
        ];

        let reconciled = reconcile_visible_mentions_after_text_change(
            old_text,
            new_text,
            &tracked_mentions,
        );

        assert_eq!(reconciled.len(), 2);
        assert_eq!(reconciled[0].user_id, first);
        assert_eq!(reconciled[0].visible_text, "@Sam");
        assert_eq!(reconciled[0].start, "yo ".len());
        assert_eq!(reconciled[0].end, "yo @Sam".len());
        assert_eq!(reconciled[1].user_id, second);
        assert_eq!(reconciled[1].visible_text, "@Sam");
        assert_eq!(reconciled[1].start, "yo @Sam and ".len());
        assert_eq!(reconciled[1].end, "yo @Sam and @Sam".len());
    }

    #[test]
    fn test_visible_mention_spans_shift_when_edit_happens_before_them() {
        let bob: OwnedUserId = "@bob:example.com".try_into().expect("valid user id");
        let old_text = "hello @Bob";
        let new_text = "hello brave @Bob";
        let tracked_mentions = vec![TrackedVisibleMention {
            user_id: bob.clone(),
            visible_text: "@Bob".to_owned(),
            start: "hello ".len(),
            end: "hello @Bob".len(),
        }];

        let reconciled = reconcile_visible_mentions_after_text_change(
            old_text,
            new_text,
            &tracked_mentions,
        );

        assert_eq!(reconciled.len(), 1);
        let tracked = &reconciled[0];
        assert_eq!(tracked.user_id, bob);
        assert_eq!(tracked.visible_text, "@Bob");
        assert_eq!(tracked.start, "hello brave ".len());
        assert_eq!(tracked.end, "hello brave @Bob".len());
    }

    #[test]
    fn test_programmatic_set_text_clears_stale_tracked_mention_state() {
        let alice: OwnedUserId = "@alice:example.com".try_into().expect("valid user id");
        let mut tracked_mentions = vec![TrackedVisibleMention {
            user_id: alice,
            visible_text: "@Alice".to_owned(),
            start: 0,
            end: "@Alice".len(),
        }];
        let mut possible_room_mention = true;

        reset_visible_mention_tracking_for_programmatic_text_set(
            &mut tracked_mentions,
            &mut possible_room_mention,
        );

        assert!(tracked_mentions.is_empty());
        assert!(!possible_room_mention);
    }

    #[test]
    fn test_inserting_mention_before_existing_mention_shifts_older_span_correctly() {
        let alice: OwnedUserId = "@alice:example.com".try_into().expect("valid user id");
        let bob: OwnedUserId = "@bob:example.com".try_into().expect("valid user id");
        let current_text = "@al @Bob";
        let start_idx = 0;
        let head = "@al".len();
        let mut tracked_mentions = vec![TrackedVisibleMention {
            user_id: bob.clone(),
            visible_text: "@Bob".to_owned(),
            start: "@al ".len(),
            end: "@al @Bob".len(),
        }];

        let (new_text, cursor) = apply_user_mention_selection(
            current_text,
            start_idx,
            head,
            Some("Alice"),
            &alice,
            &mut tracked_mentions,
        );

        assert_eq!(new_text, "@Alice  @Bob");
        assert_eq!(cursor, "@Alice ".len());
        assert_eq!(tracked_mentions.len(), 2);
        assert_eq!(tracked_mentions[0].user_id, alice);
        assert_eq!(tracked_mentions[0].start, 0);
        assert_eq!(tracked_mentions[0].end, "@Alice".len());
        assert_eq!(tracked_mentions[1].user_id, bob);
        assert_eq!(tracked_mentions[1].start, "@Alice  ".len());
        assert_eq!(tracked_mentions[1].end, "@Alice  @Bob".len());
    }

    #[test]
    fn test_inserting_room_mention_before_existing_mention_shifts_older_span_correctly() {
        let bob: OwnedUserId = "@bob:example.com".try_into().expect("valid user id");
        let current_text = "@ro @Bob";
        let start_idx = 0;
        let head = "@ro".len();
        let mut tracked_mentions = vec![TrackedVisibleMention {
            user_id: bob.clone(),
            visible_text: "@Bob".to_owned(),
            start: "@ro ".len(),
            end: "@ro @Bob".len(),
        }];

        let (new_text, cursor) = apply_text_replacement_preserving_mentions(
            current_text,
            start_idx,
            head,
            "@room ",
            &mut tracked_mentions,
        );

        assert_eq!(new_text, "@room  @Bob");
        assert_eq!(cursor, "@room ".len());
        assert_eq!(tracked_mentions.len(), 1);
        assert_eq!(tracked_mentions[0].user_id, bob);
        assert_eq!(tracked_mentions[0].start, "@room  ".len());
        assert_eq!(tracked_mentions[0].end, "@room  @Bob".len());
    }

    #[test]
    fn test_selecting_user_mention_inserts_visible_display_name() {
        let user_id: OwnedUserId = "@alice:example.com".try_into().expect("valid user id");
        let current_text = "hello @al";
        let start_idx = "hello ".len();
        let head = current_text.len();
        let mut tracked_mentions = Vec::new();
        let (new_text, cursor) = apply_user_mention_selection(
            current_text,
            start_idx,
            head,
            Some("Alice"),
            &user_id,
            &mut tracked_mentions,
        );

        assert_eq!(new_text, "hello @Alice ");
        assert!(!new_text.contains("matrix.to"));
        assert_eq!(cursor, "hello @Alice ".len());
        assert_eq!(tracked_mentions.len(), 1);
        let tracked = &tracked_mentions[0];
        assert_eq!(tracked.user_id, user_id);
        assert_eq!(tracked.visible_text, "@Alice");
        assert_eq!(tracked.start, start_idx);
        assert_eq!(tracked.end, start_idx + tracked.visible_text.len());
    }

    #[test]
    fn test_selecting_user_mention_without_display_name_falls_back_to_localpart() {
        let user_id: OwnedUserId = "@octosbot:127.0.0.1:8128".try_into().expect("valid user id");
        let current_text = "ping @oc";
        let start_idx = "ping ".len();
        let head = current_text.len();
        let mut tracked_mentions = Vec::new();
        let (new_text, cursor) = apply_user_mention_selection(
            current_text,
            start_idx,
            head,
            None,
            &user_id,
            &mut tracked_mentions,
        );

        assert_eq!(new_text, "ping @octosbot ");
        assert!(!new_text.contains("matrix.to"));
        assert_eq!(cursor, "ping @octosbot ".len());
        assert_eq!(tracked_mentions.len(), 1);
        let tracked = &tracked_mentions[0];
        assert_eq!(tracked.user_id, user_id);
        assert_eq!(tracked.visible_text, "@octosbot");
        assert_eq!(tracked.start, start_idx);
        assert_eq!(tracked.end, start_idx + tracked.visible_text.len());
    }

    #[test]
    fn test_create_message_with_visible_mentions_emits_matrix_links_and_mentions() {
        let alice: OwnedUserId = "@alice:example.com".try_into().expect("valid user id");
        let sam_one: OwnedUserId = "@sam:example.com".try_into().expect("valid user id");
        let sam_two: OwnedUserId = "@sam:other.example".try_into().expect("valid user id");
        let entered_text = "Hello @Alice and @Sam and @Sam";
        let tracked_mentions = vec![
            TrackedVisibleMention {
                user_id: alice.clone(),
                visible_text: "@Alice".to_owned(),
                start: "Hello ".len(),
                end: "Hello @Alice".len(),
            },
            TrackedVisibleMention {
                user_id: sam_one.clone(),
                visible_text: "@Sam".to_owned(),
                start: "Hello @Alice and ".len(),
                end: "Hello @Alice and @Sam".len(),
            },
            TrackedVisibleMention {
                user_id: sam_two.clone(),
                visible_text: "@Sam".to_owned(),
                start: "Hello @Alice and @Sam and ".len(),
                end: "Hello @Alice and @Sam and @Sam".len(),
            },
        ];

        let message =
            create_message_with_tracked_mentions(entered_text, &tracked_mentions, false);

        assert_eq!(
            message.msgtype.body(),
            format!(
                "Hello [@Alice]({}) and [@Sam]({}) and [@Sam]({})",
                alice.matrix_to_uri(),
                sam_one.matrix_to_uri(),
                sam_two.matrix_to_uri(),
            )
        );
        let mentions = message.mentions.expect("markdown send should include mentions");
        assert_eq!(mentions.user_ids, [alice, sam_one, sam_two].into());
        assert!(!mentions.room);
    }

    #[test]
    fn test_html_message_with_visible_mentions_emits_anchor_and_mentions() {
        let alice: OwnedUserId = "@alice:example.com".try_into().expect("valid user id");
        let entered_text = "/html<p>Hello @Alice</p>";
        let tracked_mentions = vec![TrackedVisibleMention {
            user_id: alice.clone(),
            visible_text: "@Alice".to_owned(),
            start: "<p>Hello ".len(),
            end: "<p>Hello @Alice".len(),
        }];

        let message =
            create_message_with_tracked_mentions(entered_text, &tracked_mentions, false);

        assert_eq!(message.msgtype.body(), "<p>Hello @Alice</p>");
        let MessageType::Text(text_content) = &message.msgtype else {
            panic!("expected text message");
        };
        let formatted = text_content
            .formatted
            .as_ref()
            .expect("/html send should include formatted html");
        assert_eq!(
            formatted.body,
            format!(
                "<p>Hello <a href=\"{}\">@Alice</a></p>",
                alice.matrix_to_uri(),
            )
        );
        let mentions = message.mentions.expect("html send should include mentions");
        assert_eq!(mentions.user_ids, [alice].into());
        assert!(!mentions.room);
    }

    #[test]
    fn test_plain_mode_visible_mentions_remain_plain_text_without_mentions() {
        let alice: OwnedUserId = "@alice:example.com".try_into().expect("valid user id");
        let entered_text = "/plainHello @Alice";
        let tracked_mentions = vec![TrackedVisibleMention {
            user_id: alice,
            visible_text: "@Alice".to_owned(),
            start: "Hello ".len(),
            end: "Hello @Alice".len(),
        }];

        let message =
            create_message_with_tracked_mentions(entered_text, &tracked_mentions, false);

        assert_eq!(message.msgtype.body(), "Hello @Alice");
        let MessageType::Text(text_content) = &message.msgtype else {
            panic!("expected text message");
        };
        assert!(text_content.formatted.is_none());
        assert!(message.mentions.is_none());
    }

    #[test]
    fn test_markdown_visible_mentions_escape_markdown_metacharacters_in_label() {
        let weird: OwnedUserId = "@weird:example.com".try_into().expect("valid user id");
        let entered_text = r"Hello @A[ice]\*";
        let tracked_mentions = vec![TrackedVisibleMention {
            user_id: weird.clone(),
            visible_text: r"@A[ice]\*".to_owned(),
            start: "Hello ".len(),
            end: entered_text.len(),
        }];

        let message =
            create_message_with_tracked_mentions(entered_text, &tracked_mentions, false);

        assert_eq!(
            message.msgtype.body(),
            format!(
                r"Hello [@A\[ice\]\\\*]({})",
                weird.matrix_to_uri(),
            )
        );
        let mentions = message.mentions.expect("markdown send should include mentions");
        assert_eq!(mentions.user_ids, [weird].into());
    }

    #[test]
    fn test_roommate_does_not_count_as_room_mention() {
        let resolved = resolve_visible_mentions_for_send("@roommate hi", &[], true);
        assert!(!resolved.mentions.room);
    }
}
