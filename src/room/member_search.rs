//! Room member search functionality for @mentions
//!
//! This module provides efficient searching of room members with streaming results
//! to support responsive UI when users type @mentions.

use std::collections::BinaryHeap;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc::Sender,
    Arc,
};
use matrix_sdk::{room::{RoomMember, RoomMemberRole}, ruma::OwnedUserId};
use unicode_segmentation::UnicodeSegmentation;
use crate::shared::mentionable_text_input::SearchResult;
use crate::sliding_sync::current_user_id;
use makepad_widgets::log;

const BATCH_SIZE: usize = 10; // Number of results per streamed batch

/// Pre-computed member sort key for fast empty search
#[derive(Debug, Clone)]
pub struct MemberSortKey {
    /// Power level rank: 0=Admin, 1=Moderator, 2=User
    pub power_rank: u8,
    /// Name category: 0=Alphabetic, 1=Numeric, 2=Symbols
    pub name_category: u8,
    /// Normalized lowercase name for sorting
    pub sort_key: String,
}

/// Pre-computed sorted indices and keys for room members
#[derive(Debug, Clone)]
pub struct PrecomputedMemberSort {
    /// Sorted indices into the members array
    pub sorted_indices: Vec<usize>,
    /// Pre-computed sort keys (parallel to original members array)
    pub member_keys: Vec<MemberSortKey>,
}

/// Pre-compute sort keys and indices for room members
/// This is called once when members are fetched, avoiding repeated computation
pub fn precompute_member_sort(members: &[RoomMember]) -> PrecomputedMemberSort {
    let current_user_id = current_user_id();
    let mut member_keys = Vec::with_capacity(members.len());
    let mut sortable_members = Vec::with_capacity(members.len());

    for (index, member) in members.iter().enumerate() {
        // Skip current user
        if let Some(ref current_id) = current_user_id {
            if member.user_id() == current_id {
                // Add placeholder for current user to maintain index alignment
                member_keys.push(MemberSortKey {
                    power_rank: 255, // Will be filtered out
                    name_category: 255,
                    sort_key: String::new(),
                });
                continue;
            }
        }

        // Get power level rank
        let power_rank = role_to_rank(member.suggested_role_for_power_level());

        // Get normalized display name
        let raw_name = member
            .display_name()
            .map(|n| n.trim())
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| member.user_id().localpart());

        // Generate sort key by stripping leading non-alphanumeric
        let stripped = raw_name.trim_start_matches(|c: char| !c.is_alphanumeric());
        let sort_key = if stripped.is_empty() {
            // Name is all symbols, use original
            if raw_name.is_ascii() {
                raw_name.to_ascii_lowercase()
            } else {
                raw_name.to_lowercase()
            }
        } else {
            // Use stripped version for sorting
            if stripped.is_ascii() {
                stripped.to_ascii_lowercase()
            } else {
                stripped.to_lowercase()
            }
        };

        // Determine name category based on stripped name for consistency
        // This makes "!!!alice" categorized as alphabetic, not symbols
        let name_category = if !stripped.is_empty() {
            // Use first char of stripped name
            match stripped.chars().next() {
                Some(c) if c.is_alphabetic() => 0,
                Some(c) if c.is_numeric() => 1,
                _ => 2,
            }
        } else {
            // Name is all symbols, use original first char
            match raw_name.chars().next() {
                Some(c) if c.is_alphabetic() => 0, // Shouldn't happen if stripped is empty
                Some(c) if c.is_numeric() => 1,    // Shouldn't happen if stripped is empty
                _ => 2,                            // Symbols
            }
        };

        let key = MemberSortKey {
            power_rank,
            name_category,
            sort_key: sort_key.clone(),
        };

        member_keys.push(key.clone());
        sortable_members.push((power_rank, name_category, sort_key, index));
    }

    // Sort all valid members
    sortable_members.sort_by(|a, b| match a.0.cmp(&b.0) {
        std::cmp::Ordering::Equal => match a.1.cmp(&b.1) {
            std::cmp::Ordering::Equal => a.2.cmp(&b.2),
            other => other,
        },
        other => other,
    });

    // Extract sorted indices
    let sorted_indices: Vec<usize> = sortable_members
        .into_iter()
        .map(|(_, _, _, idx)| idx)
        .collect();

    PrecomputedMemberSort {
        sorted_indices,
        member_keys,
    }
}

/// Maps a member role to a sortable rank (lower value = higher priority)
fn role_to_rank(role: RoomMemberRole) -> u8 {
    match role {
        RoomMemberRole::Administrator | RoomMemberRole::Creator => 0,
        RoomMemberRole::Moderator => 1,
        RoomMemberRole::User => 2,
    }
}

fn is_cancelled(token: &Option<Arc<AtomicBool>>) -> bool {
    token
        .as_ref()
        .map(|flag| flag.load(Ordering::Relaxed))
        .unwrap_or(false)
}

fn send_search_update(
    sender: &Sender<SearchResult>,
    cancel_token: &Option<Arc<AtomicBool>>,
    search_id: u64,
    search_text: &Arc<String>,
    results: Vec<usize>,
    is_complete: bool,
) -> bool {
    if is_cancelled(cancel_token) {
        return false;
    }

    let search_result = SearchResult {
        search_id,
        results,
        is_complete,
        search_text: Arc::clone(search_text),
    };

    if sender.send(search_result).is_err() {
        log!("Failed to send search results - receiver dropped");
        return false;
    }

    true
}

fn stream_index_batches(
    indices: &[usize],
    sender: &Sender<SearchResult>,
    cancel_token: &Option<Arc<AtomicBool>>,
    search_id: u64,
    search_text: &Arc<String>,
) -> bool {
    if indices.is_empty() {
        return send_search_update(sender, cancel_token, search_id, search_text, Vec::new(), true);
    }

    let mut start = 0;
    while start < indices.len() {
        let end = (start + BATCH_SIZE).min(indices.len());
        let batch = indices[start..end].to_vec();
        start = end;
        let is_last = start >= indices.len();

        if !send_search_update(sender, cancel_token, search_id, search_text, batch, is_last) {
            return false;
        }
    }

    true
}

fn compute_empty_search_indices(
    members: &[RoomMember],
    max_results: usize,
    current_user_id: Option<&OwnedUserId>,
    precomputed_sort: Option<&PrecomputedMemberSort>,
    cancel_token: &Option<Arc<AtomicBool>>,
) -> Option<Vec<usize>> {
    if is_cancelled(cancel_token) {
        return None;
    }

    if let Some(sort_data) = precomputed_sort {
        let mut indices: Vec<usize> = sort_data
            .sorted_indices
            .iter()
            .take(max_results)
            .copied()
            .collect();

        if max_results == 0 {
            indices.clear();
        }

        return Some(indices);
    }

    let mut valid_members: Vec<(u8, u8, usize)> = Vec::with_capacity(members.len());

    for (index, member) in members.iter().enumerate() {
        if is_cancelled(cancel_token) {
            return None;
        }

        if current_user_id.is_some_and(|id| member.user_id() == id) {
            continue;
        }

        let power_rank = role_to_rank(member.suggested_role_for_power_level());

        let raw_name = member
            .display_name()
            .map(|n| n.trim())
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| member.user_id().localpart());

        let stripped = raw_name.trim_start_matches(|c: char| !c.is_alphanumeric());
        let name_category = if !stripped.is_empty() {
            match stripped.chars().next() {
                Some(c) if c.is_alphabetic() => 0,
                Some(c) if c.is_numeric() => 1,
                _ => 2,
            }
        } else {
            2
        };

        valid_members.push((power_rank, name_category, index));
    }

    if is_cancelled(cancel_token) {
        return None;
    }

    valid_members.sort_by(|a, b| match a.0.cmp(&b.0) {
        std::cmp::Ordering::Equal => match a.1.cmp(&b.1) {
            std::cmp::Ordering::Equal => {
                let name_a = members[a.2]
                    .display_name()
                    .map(|n| n.trim())
                    .filter(|n| !n.is_empty())
                    .unwrap_or_else(|| members[a.2].user_id().localpart());
                let name_b = members[b.2]
                    .display_name()
                    .map(|n| n.trim())
                    .filter(|n| !n.is_empty())
                    .unwrap_or_else(|| members[b.2].user_id().localpart());

                name_a
                    .chars()
                    .map(|c| c.to_ascii_lowercase())
                    .cmp(name_b.chars().map(|c| c.to_ascii_lowercase()))
            }
            other => other,
        },
        other => other,
    });

    if is_cancelled(cancel_token) {
        return None;
    }

    valid_members.truncate(max_results);

    Some(valid_members.into_iter().map(|(_, _, idx)| idx).collect())
}

fn compute_non_empty_search_indices(
    members: &[RoomMember],
    search_text: &str,
    max_results: usize,
    current_user_id: Option<&OwnedUserId>,
    precomputed_sort: Option<&PrecomputedMemberSort>,
    cancel_token: &Option<Arc<AtomicBool>>,
) -> Option<Vec<usize>> {
    if is_cancelled(cancel_token) {
        return None;
    }

    let mut top_matches: BinaryHeap<(u8, usize)> = BinaryHeap::with_capacity(max_results);
    let mut high_priority_count = 0;
    let mut best_priority_seen = u8::MAX;

    for (index, member) in members.iter().enumerate() {
        if is_cancelled(cancel_token) {
            return None;
        }

        if current_user_id.is_some_and(|id| member.user_id() == id) {
            continue;
        }

        if let Some(priority) = match_member_with_priority(member, search_text) {
            if priority <= 3 {
                high_priority_count += 1;
            }
            best_priority_seen = best_priority_seen.min(priority);

            if top_matches.len() < max_results {
                top_matches.push((priority, index));
            } else if let Some(&(worst_priority, _)) = top_matches.peek() {
                if priority < worst_priority {
                    top_matches.pop();
                    top_matches.push((priority, index));
                }
            }

            if max_results > 0
                && high_priority_count >= max_results * 2
                && top_matches.len() == max_results
                && best_priority_seen == 0
            {
                break;
            }
        }
    }

    if is_cancelled(cancel_token) {
        return None;
    }

    let mut all_matches: Vec<(u8, usize)> = top_matches.into_iter().collect();

    all_matches.sort_by(|(priority_a, idx_a), (priority_b, idx_b)| {
        match priority_a.cmp(priority_b) {
            std::cmp::Ordering::Equal => {
                if let Some(sort_data) = precomputed_sort {
                    let key_a = &sort_data.member_keys[*idx_a];
                    let key_b = &sort_data.member_keys[*idx_b];

                    match key_a.power_rank.cmp(&key_b.power_rank) {
                        std::cmp::Ordering::Equal => match key_a.name_category.cmp(&key_b.name_category) {
                            std::cmp::Ordering::Equal => key_a.sort_key.cmp(&key_b.sort_key),
                            other => other,
                        },
                        other => other,
                    }
                } else {
                    let member_a = &members[*idx_a];
                    let member_b = &members[*idx_b];

                    let power_a = role_to_rank(member_a.suggested_role_for_power_level());
                    let power_b = role_to_rank(member_b.suggested_role_for_power_level());

                    match power_a.cmp(&power_b) {
                        std::cmp::Ordering::Equal => {
                            let name_a = member_a
                                .display_name()
                                .map(|n| n.trim())
                                .filter(|n| !n.is_empty())
                                .unwrap_or_else(|| member_a.user_id().localpart());
                            let name_b = member_b
                                .display_name()
                                .map(|n| n.trim())
                                .filter(|n| !n.is_empty())
                                .unwrap_or_else(|| member_b.user_id().localpart());

                            if name_a.is_ascii() && name_b.is_ascii() {
                                name_a
                                    .chars()
                                    .map(|c| c.to_ascii_lowercase())
                                    .cmp(name_b.chars().map(|c| c.to_ascii_lowercase()))
                            } else {
                                name_a.to_lowercase().cmp(&name_b.to_lowercase())
                            }
                        }
                        other => other,
                    }
                }
            }
            other => other,
        }
    });

    if is_cancelled(cancel_token) {
        return None;
    }

    Some(all_matches.into_iter().map(|(_, idx)| idx).collect())
}

/// Search room members with optional pre-computed sort data
pub fn search_room_members_streaming_with_sort(
    members: Arc<Vec<RoomMember>>,
    search_text: String,
    max_results: usize,
    sender: Sender<SearchResult>,
    search_id: u64,
    precomputed_sort: Option<Arc<PrecomputedMemberSort>>,
    cancel_token: Option<Arc<AtomicBool>>,
) {
    let current_user_id = current_user_id();

    if is_cancelled(&cancel_token) {
        return;
    }

    let search_text_arc = Arc::new(search_text);
    let search_query = search_text_arc.as_str();
    let precomputed_ref = precomputed_sort.as_deref();
    let cancel_ref = &cancel_token;
    let members_slice = members.as_ref();

    let results = if search_query.is_empty() {
        match compute_empty_search_indices(
            members_slice,
            max_results,
            current_user_id.as_ref(),
            precomputed_ref,
            cancel_ref,
        ) {
            Some(indices) => indices,
            None => return,
        }
    } else {
        match compute_non_empty_search_indices(
            members_slice,
            search_query,
            max_results,
            current_user_id.as_ref(),
            precomputed_ref,
            cancel_ref,
        ) {
            Some(indices) => indices,
            None => return,
        }
    };

    let _ = stream_index_batches(&results, &sender, cancel_ref, search_id, &search_text_arc);
}


/// Check if search_text appears after a word boundary in text
/// Word boundaries include: punctuation, symbols, and other non-alphanumeric characters
/// For ASCII text, also supports case-insensitive matching
fn check_word_boundary_match(text: &str, search_text: &str, case_insensitive: bool) -> bool {
    if search_text.is_empty() {
        return false;
    }

    if case_insensitive && search_text.is_ascii() {
        let search_len = search_text.len();
        for (index, _) in text.char_indices() {
            if index == 0 || index + search_len > text.len() {
                continue;
            }
            if substring_eq_ignore_ascii_case(text, index, search_text) {
                if let Some(prev_char) = text[..index].chars().last() {
                    if !prev_char.is_alphanumeric() {
                        return true;
                    }
                }
            }
        }
        false
    } else {
        for (index, _) in text.match_indices(search_text) {
            if index == 0 {
                continue; // Already handled by starts_with checks
            }

            if let Some(prev_char) = text[..index].chars().last() {
                if !prev_char.is_alphanumeric() {
                    return true;
                }
            }
        }
        false
    }
}

/// Check if a string starts with another string based on grapheme clusters
///
/// ## What are Grapheme Clusters?
///
/// A grapheme cluster is what users perceive as a single "character". This is NOT about
/// phonetics/pronunciation, but about visual representation. Examples:
///
/// - "👨‍👩‍👧‍👦" (family emoji) looks like 1 character but is actually 7 Unicode code points
/// - "é" might be 1 precomposed character or 2 characters (e + ´ combining accent)
/// - "🇺🇸" (flag) is 2 regional indicator symbols that combine into 1 visual character
///
/// ## Why is this needed?
///
/// Standard string operations like `starts_with()` work on bytes or chars, which can
/// break these multi-codepoint characters. For @mentions, users expect:
/// - Typing "👨‍👩‍👧‍👦" should match a username starting with that family emoji
/// - Typing "é" should match whether the username uses precomposed or decomposed form
///
/// ## When is this function called?
///
/// This function is ONLY used when the search text contains complex Unicode characters
/// (when grapheme count != char count). For regular ASCII or simple Unicode, the
/// standard `starts_with()` is used for better performance.
///
/// ## Performance Note
///
/// This function is intentionally not called for common cases (ASCII usernames,
/// simple Chinese characters) to avoid the overhead of grapheme segmentation.
fn grapheme_starts_with(haystack: &str, needle: &str, case_insensitive: bool) -> bool {
    if needle.is_empty() {
        return true;
    }

    let haystack_graphemes: Vec<&str> = haystack.graphemes(true).collect();
    let needle_graphemes: Vec<&str> = needle.graphemes(true).collect();

    if needle_graphemes.len() > haystack_graphemes.len() {
        return false;
    }

    for i in 0..needle_graphemes.len() {
        let h_grapheme = haystack_graphemes[i];
        let n_grapheme = needle_graphemes[i];

        let grapheme_matches = if case_insensitive && h_grapheme.is_ascii() && n_grapheme.is_ascii()
        {
            h_grapheme.to_lowercase() == n_grapheme.to_lowercase()
        } else {
            h_grapheme == n_grapheme
        };

        if !grapheme_matches {
            return false;
        }
    }

    true
}

/// Match a member against search text and return priority if matched
/// Returns None if no match, Some(priority) if matched (lower priority = better match)
///
/// Follows Matrix official recommendations for matching order:
/// 1. Exact display name match
/// 2. Exact user ID match
/// 3. Display name starts with search text
/// 4. User ID starts with search text
/// 5. Display name contains search text (at word boundary)
/// 6. User ID contains search text
fn match_member_with_priority(member: &RoomMember, search_text: &str) -> Option<u8> {
    if search_text.is_empty() {
        return Some(10);
    }

    let display_name = member.display_name();
    let user_id = member.user_id().as_str();
    let localpart = member.user_id().localpart();
    let case_insensitive = search_text.is_ascii();
    let search_without_at = search_text.strip_prefix('@').unwrap_or(search_text);
    let search_has_at = search_without_at.len() != search_text.len();

    for matcher in MATCHERS {
        if let Some(priority) = reducer(
            matcher,
            search_text,
            display_name,
            user_id,
            localpart,
            case_insensitive,
            search_has_at,
        ) {
            return Some(priority);
        }
    }

    if !case_insensitive && search_text.graphemes(true).count() != search_text.chars().count() {
        if let Some(display) = display_name {
            if grapheme_starts_with(display, search_text, false) {
                return Some(8);
            }
        }
    }

    None
}

#[derive(Copy, Clone)]
struct Matcher {
    priority: u8,
    func: fn(
        search_text: &str,
        display_name: Option<&str>,
        user_id: &str,
        localpart: &str,
        case_insensitive: bool,
        search_has_at: bool,
    ) -> bool,
}

const MATCHERS: &[Matcher] = &[
    Matcher {
        priority: 0,
        func: |search_text, display_name, _, _, _, _| display_name == Some(search_text),
    },
    Matcher {
        priority: 1,
        func: |search_text, display_name, _, _, case_insensitive, _| {
            case_insensitive && display_name.is_some_and(|d| d.eq_ignore_ascii_case(search_text))
        },
    },
    Matcher {
        priority: 2,
        func: |search_text, _, user_id, _, _, search_has_at| {
            user_id == search_text
                || (!search_has_at
                    && user_id.starts_with('@')
                    && user_id.strip_prefix('@') == Some(search_text))
        },
    },
    Matcher {
        priority: 3,
        func: |search_text, _, user_id, _, case_insensitive, search_has_at| {
            case_insensitive
                && (user_id.eq_ignore_ascii_case(search_text)
                    || (!search_has_at
                        && user_id.starts_with('@')
                        && user_id.strip_prefix('@').is_some_and(|id| {
                            id.eq_ignore_ascii_case(search_text)
                        })))
        },
    },
    Matcher {
        priority: 4,
        func: |search_text, display_name, _, _, _, _| {
            display_name.is_some_and(|d| d.starts_with(search_text))
        },
    },
    Matcher {
        priority: 5,
        func: |search_text, display_name, _, _, case_insensitive, _| {
            case_insensitive
                && display_name.is_some_and(|d| starts_with_ignore_ascii_case(d, search_text))
        },
    },
    Matcher {
        priority: 6,
        func: |search_text, _, user_id, localpart, _, search_has_at| {
            user_id.starts_with(search_text)
                || (!search_has_at
                    && user_id.starts_with('@')
                    && user_id.strip_prefix('@').is_some_and(|id| id.starts_with(search_text)))
                || localpart.starts_with(search_text)
        },
    },
    Matcher {
        priority: 7,
        func: |search_text, _, user_id, localpart, case_insensitive, search_has_at| {
            case_insensitive
                && (starts_with_ignore_ascii_case(user_id, search_text)
                    || starts_with_ignore_ascii_case(localpart, search_text)
                    || (!search_has_at
                        && user_id.starts_with('@')
                        && user_id.strip_prefix('@').is_some_and(|id| {
                            starts_with_ignore_ascii_case(id, search_text)
                        })))
        },
    },
    Matcher {
        priority: 8,
        func: |search_text, display_name, _, _, case_insensitive, _| {
            display_name.is_some_and(|display| {
                check_word_boundary_match(display, search_text, case_insensitive)
                    || display.contains(search_text)
                    || (case_insensitive
                        && contains_ignore_ascii_case(display, search_text))
            })
        },
    },
    Matcher {
        priority: 9,
        func: |search_text, _, user_id, localpart, case_insensitive, _| {
            if case_insensitive {
                contains_ignore_ascii_case(user_id, search_text)
                    || contains_ignore_ascii_case(localpart, search_text)
            } else {
                user_id.contains(search_text) || localpart.contains(search_text)
            }
        },
    },
];

fn reducer(
    matcher: &Matcher,
    search_text: &str,
    display_name: Option<&str>,
    user_id: &str,
    localpart: &str,
    case_insensitive: bool,
    search_has_at: bool,
) -> Option<u8> {
    if (matcher.func)(
        search_text,
        display_name,
        user_id,
        localpart,
        case_insensitive,
        search_has_at,
    ) {
        Some(matcher.priority)
    } else {
        None
    }
}

/// Returns true if the `haystack` starts with `needle` ignoring ASCII case.
fn starts_with_ignore_ascii_case(haystack: &str, needle: &str) -> bool {
    haystack
        .get(..needle.len())
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case(needle))
}

/// Returns true if the `haystack` contains `needle` ignoring ASCII case.
fn contains_ignore_ascii_case(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return true;
    }
    if !needle.is_ascii() {
        return haystack.contains(needle);
    }
    let needle_len = needle.len();
    for (index, _) in haystack.char_indices() {
        if index + needle_len > haystack.len() {
            break;
        }
        if substring_eq_ignore_ascii_case(haystack, index, needle) {
            return true;
        }
    }
    false
}

fn substring_eq_ignore_ascii_case(haystack: &str, start: usize, needle: &str) -> bool {
    haystack
        .get(start..start.saturating_add(needle.len()))
        .is_some_and(|segment| segment.eq_ignore_ascii_case(needle))
}

// typos:disable
#[cfg(test)]
mod tests {
    use super::*;
    use matrix_sdk::room::RoomMemberRole;
    use std::sync::mpsc::channel;

    #[test]
    fn test_send_search_update_respects_cancellation() {
        let (tx, rx) = channel();
        let cancel = Some(Arc::new(AtomicBool::new(true)));
        let query = Arc::new("query".to_owned());

        let result = send_search_update(&tx, &cancel, 1, &query, vec![1], false);

        assert!(!result);
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn test_stream_index_batches_emits_completion() {
        let (tx, rx) = channel();
        let cancel = None;
        let query = Arc::new("abc".to_owned());

        assert!(stream_index_batches(&[1, 2], &tx, &cancel, 7, &query));

        let message = rx.recv().expect("expected batched result");
        assert_eq!(message.results, vec![1, 2]);
        assert!(message.is_complete);
        assert_eq!(message.search_id, 7);
        assert_eq!(message.search_text.as_str(), "abc");
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn test_stream_index_batches_cancelled_before_send() {
        let (tx, rx) = channel();
        let cancel = Some(Arc::new(AtomicBool::new(true)));
        let query = Arc::new("abc".to_owned());

        assert!(!stream_index_batches(&[1, 2], &tx, &cancel, 3, &query));
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn test_role_to_rank() {
        // Verify that admin < moderator < user in terms of rank
        assert_eq!(role_to_rank(RoomMemberRole::Administrator), 0);
        assert_eq!(role_to_rank(RoomMemberRole::Moderator), 1);
        assert_eq!(role_to_rank(RoomMemberRole::User), 2);

        // Verify ordering
        assert!(
            role_to_rank(RoomMemberRole::Administrator) < role_to_rank(RoomMemberRole::Moderator)
        );
        assert!(role_to_rank(RoomMemberRole::Moderator) < role_to_rank(RoomMemberRole::User));
    }

    #[test]
    fn test_top_k_selection_correctness() {
        use std::collections::BinaryHeap;

        // Simulate Top-K selection with mixed priorities
        let test_data = vec![
            (5, "user5"),  // priority 5
            (1, "user1"),  // priority 1 (better)
            (3, "user3"),  // priority 3
            (0, "user0"),  // priority 0 (best)
            (8, "user8"),  // priority 8 (worst)
            (2, "user2"),  // priority 2
            (4, "user4"),  // priority 4
            (1, "user1b"), // priority 1 (tie)
        ];

        let max_results = 3;
        let mut top_matches: BinaryHeap<(u8, &str)> = BinaryHeap::with_capacity(max_results);

        // Apply the same algorithm as in search
        for (priority, name) in test_data {
            if top_matches.len() < max_results {
                top_matches.push((priority, name));
            } else if let Some(&(worst_priority, _)) = top_matches.peek() {
                if priority < worst_priority {
                    top_matches.pop();
                    top_matches.push((priority, name));
                }
            }
        }

        // Extract and sort results
        let mut results: Vec<(u8, &str)> = top_matches.into_iter().collect();
        results.sort_by_key(|&(priority, _)| priority);

        // Verify we got the top 3 with lowest priorities
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].0, 0); // Best priority
        assert_eq!(results[1].0, 1); // Second best
        assert_eq!(results[2].0, 1); // Tied second best

        // Verify the worst candidates were excluded
        assert!(!results.iter().any(|&(p, _)| p >= 4));
    }

    #[test]
    fn test_word_boundary_case_insensitive() {
        // Test case-insensitive word boundary matching for ASCII
        assert!(check_word_boundary_match("Hello, Alice", "alice", true));
        assert!(check_word_boundary_match("@BOB is here", "bob", true));
        assert!(check_word_boundary_match("Meet CHARLIE!", "charlie", true));
        assert!(check_word_boundary_match("user:David", "david", true));

        // Should not match in middle of word (case-insensitive)
        assert!(!check_word_boundary_match("AliceSmith", "lice", true));
        assert!(!check_word_boundary_match("BOBCAT", "cat", true));

        // Test case-sensitive mode
        assert!(check_word_boundary_match("Hello, alice", "alice", false));
        assert!(!check_word_boundary_match("Hello, Alice", "alice", false));

        // Test with mixed case in search text
        assert!(check_word_boundary_match("Hello, Alice", "Alice", true));
        assert!(check_word_boundary_match("Hello, Alice", "Alice", false));
    }

    #[test]
    fn test_name_category_with_stripped_prefix() {
        // Helper to determine name category (matching the actual implementation)
        fn get_name_category(raw_name: &str) -> u8 {
            let stripped = raw_name.trim_start_matches(|c: char| !c.is_alphanumeric());
            if !stripped.is_empty() {
                match stripped.chars().next() {
                    Some(c) if c.is_alphabetic() => 0,
                    Some(c) if c.is_numeric() => 1,
                    _ => 2,
                }
            } else {
                2 // All symbols
            }
        }

        // Test normal names
        assert_eq!(get_name_category("alice"), 0); // Alphabetic
        assert_eq!(get_name_category("123user"), 1); // Numeric
        assert_eq!(get_name_category("@#$%"), 2); // All symbols

        // Test names with symbol prefixes
        assert_eq!(get_name_category("!!!alice"), 0); // Should be alphabetic after stripping
        assert_eq!(get_name_category("@bob"), 0); // Should be alphabetic after stripping
        assert_eq!(get_name_category("___123"), 1); // Should be numeric after stripping
        assert_eq!(get_name_category("#$%alice"), 0); // Should be alphabetic after stripping

        // Test edge cases
        assert_eq!(get_name_category(""), 2); // Empty -> symbols
        assert_eq!(get_name_category("!!!"), 2); // All symbols -> symbols
    }

    #[test]
    fn test_grapheme_starts_with_basic() {
        // Basic ASCII cases
        assert!(grapheme_starts_with("hello", "hel", false));
        assert!(grapheme_starts_with("hello", "hello", false));
        assert!(!grapheme_starts_with("hello", "llo", false));
        assert!(grapheme_starts_with("hello", "", false));
        assert!(!grapheme_starts_with("hi", "hello", false));
    }

    #[test]
    fn test_grapheme_starts_with_case_sensitivity() {
        // Case-insensitive for ASCII
        assert!(grapheme_starts_with("Hello", "hel", true));
        assert!(grapheme_starts_with("HELLO", "hel", true));
        assert!(!grapheme_starts_with("Hello", "hel", false));

        // Case-insensitive only works for ASCII
        assert!(!grapheme_starts_with("Привет", "прив", true)); // Russian
    }

    #[test]
    fn test_grapheme_starts_with_emojis() {
        // Family emoji (multiple code points appearing as single character)
        let family = "👨‍👩‍👧‍👦"; // 7 code points, 1 grapheme
        assert!(grapheme_starts_with("👨‍👩‍👧‍👦 Smith Family", "👨‍👩‍👧‍👦", false));
        assert!(grapheme_starts_with(family, family, false));

        // Flag emojis (regional indicators)
        assert!(grapheme_starts_with("🇺🇸 USA", "🇺🇸", false));
        assert!(grapheme_starts_with("🇯🇵 Japan", "🇯🇵", false));

        // Skin tone modifiers
        assert!(grapheme_starts_with("👋🏽 Hello", "👋🏽", false));
        assert!(!grapheme_starts_with("👋🏽 Hello", "👋", false)); // Different without modifier

        // Complex emoji sequences
        assert!(grapheme_starts_with("🧑‍💻 Developer", "🧑‍💻", false));
    }

    #[test]
    fn test_grapheme_starts_with_combining_characters() {
        // Precomposed vs decomposed forms
        let precomposed = "café"; // é as single character (U+00E9)
        let decomposed = "cafe\u{0301}"; // e + combining acute accent (U+0065 + U+0301)

        // Both should work
        assert!(grapheme_starts_with(precomposed, "caf", false));
        assert!(grapheme_starts_with(decomposed, "caf", false));

        // Other combining characters
        assert!(grapheme_starts_with("naïve", "naï", false)); // ï with diaeresis
        assert!(grapheme_starts_with("piñata", "piñ", false)); // ñ with tilde
    }

    #[test]
    fn test_grapheme_starts_with_various_scripts() {
        // Chinese
        assert!(grapheme_starts_with("张三", "张", false));

        // Japanese (Hiragana + Kanji)
        assert!(grapheme_starts_with("こんにちは", "こん", false));
        assert!(grapheme_starts_with("日本語", "日本", false));

        // Korean
        assert!(grapheme_starts_with("안녕하세요", "안녕", false));

        // Arabic (RTL)
        assert!(grapheme_starts_with("مرحبا", "مر", false));

        // Hindi with complex ligatures
        assert!(grapheme_starts_with("नमस्ते", "नम", false));

        // Thai with combining marks
        assert!(grapheme_starts_with("สวัสดี", "สวั", false));
    }

    #[test]
    fn test_grapheme_starts_with_zero_width_joiners() {
        // Zero-width joiner sequences
        let zwj_sequence = "👨‍⚕️"; // Man + ZWJ + Medical symbol
        assert!(grapheme_starts_with("👨‍⚕️ Dr. Smith", zwj_sequence, false));

        // Gender-neutral sequences
        assert!(grapheme_starts_with("🧑‍🎓 Student", "🧑‍🎓", false));
    }

    #[test]
    fn test_grapheme_starts_with_edge_cases() {
        // Empty strings
        assert!(grapheme_starts_with("", "", false));
        assert!(!grapheme_starts_with("", "a", false));

        // Single grapheme vs multiple
        assert!(grapheme_starts_with("a", "a", false));
        assert!(!grapheme_starts_with("a", "ab", false));

        // Whitespace handling
        assert!(grapheme_starts_with("  hello", "  ", false));
        assert!(grapheme_starts_with("\nhello", "\n", false));
    }

    #[test]
    fn test_word_boundary_match() {
        // Test case-sensitive word boundary scenarios
        assert!(check_word_boundary_match("Hello,alice", "alice", false));
        assert!(check_word_boundary_match("(bob) is here", "bob", false));
        assert!(check_word_boundary_match("user:charlie", "charlie", false));
        assert!(check_word_boundary_match("@david!", "david", false));
        assert!(check_word_boundary_match("eve.smith", "smith", false));
        assert!(check_word_boundary_match("frank-jones", "jones", false));

        // Test case-insensitive matching (ASCII)
        assert!(check_word_boundary_match("Hello,Alice", "alice", true));
        assert!(check_word_boundary_match("(Bob) is here", "bob", true));
        assert!(check_word_boundary_match("USER:Charlie", "charlie", true));
        assert!(check_word_boundary_match("@DAVID!", "david", true));

        // Should not match in the middle of a word
        assert!(!check_word_boundary_match("alice123", "lice", false));
        assert!(!check_word_boundary_match("bobcat", "cat", false));
        assert!(!check_word_boundary_match("Alice123", "lice", true));
        assert!(!check_word_boundary_match("BobCat", "cat", true));

        // Edge cases
        assert!(!check_word_boundary_match("test", "test", false)); // Starts with (handled elsewhere)
        assert!(!check_word_boundary_match("", "test", false)); // Empty text
    }

    #[test]
    fn test_smart_sort_key_generation() {
        // Helper function to simulate sort key generation
        fn generate_sort_key(raw_name: &str) -> (u8, String) {
            let stripped = raw_name.trim_start_matches(|c: char| !c.is_alphanumeric());
            let sort_key = if stripped.is_empty() {
                raw_name.to_lowercase()
            } else {
                stripped.to_lowercase()
            };

            // Three-tier ranking: alphabetic (0), numeric (1), symbols (2)
            let rank = match raw_name.chars().next() {
                Some(c) if c.is_alphabetic() => 0,
                Some(c) if c.is_numeric() => 1,
                _ => 2,
            };

            (rank, sort_key)
        }

        // Test alphabetic names get rank 0
        assert_eq!(generate_sort_key("alice"), (0, "alice".to_string()));
        assert_eq!(generate_sort_key("Bob"), (0, "bob".to_string()));
        assert_eq!(generate_sort_key("张三"), (0, "张三".to_string()));

        // Test numeric names get rank 1
        assert_eq!(generate_sort_key("0user"), (1, "0user".to_string()));
        assert_eq!(generate_sort_key("123abc"), (1, "123abc".to_string()));
        assert_eq!(generate_sort_key("999test"), (1, "999test".to_string()));

        // Test symbol-prefixed names get rank 2 but sort by stripped version
        assert_eq!(generate_sort_key("!!!alice"), (2, "alice".to_string()));
        assert_eq!(generate_sort_key("@bob"), (2, "bob".to_string()));
        assert_eq!(generate_sort_key("___charlie"), (2, "charlie".to_string()));

        // Test pure symbol names
        assert_eq!(generate_sort_key("!!!"), (2, "!!!".to_string()));
        assert_eq!(generate_sort_key("@@@"), (2, "@@@".to_string()));

        // Test ordering: alphabetic -> numeric -> symbols
        let mut names = vec![
            ("!!!alice", generate_sort_key("!!!alice")),
            ("0user", generate_sort_key("0user")),
            ("alice", generate_sort_key("alice")),
            ("123test", generate_sort_key("123test")),
            ("@bob", generate_sort_key("@bob")),
            ("bob", generate_sort_key("bob")),
        ];

        // Sort by (rank, sort_key)
        names.sort_by(|a, b| match a.1.0.cmp(&b.1.0) {
            std::cmp::Ordering::Equal => a.1.1.cmp(&b.1.1),
            other => other,
        });

        // Verify order: alice, bob, 0user, 123test, !!!alice, @bob
        assert_eq!(names[0].0, "alice");
        assert_eq!(names[1].0, "bob");
        assert_eq!(names[2].0, "0user");
        assert_eq!(names[3].0, "123test");
        assert_eq!(names[4].0, "!!!alice");
        assert_eq!(names[5].0, "@bob");
    }

    #[test]
    fn test_role_to_rank_mapping() {
        assert_eq!(role_to_rank(RoomMemberRole::Administrator), 0);
        assert_eq!(role_to_rank(RoomMemberRole::Moderator), 1);
        assert_eq!(role_to_rank(RoomMemberRole::User), 2);
    }

    #[test]
    fn test_top_k_heap_selection_priorities() {
        // Simulate the heap logic used in non-empty search: keep K smallest priorities
        fn top_k(items: &[(u8, usize)], k: usize) -> Vec<(u8, usize)> {
            use std::collections::BinaryHeap;
            let mut heap: BinaryHeap<(u8, usize)> = BinaryHeap::with_capacity(k);
            for &(p, idx) in items {
                if heap.len() < k {
                    heap.push((p, idx));
                } else if let Some(&(worst_p, _)) = heap.peek() {
                    if p < worst_p {
                        let _ = heap.pop();
                        heap.push((p, idx));
                    }
                }
            }
            let mut out: Vec<(u8, usize)> = heap.into_iter().collect();
            out.sort_by_key(|(p, _)| *p);
            out
        }

        let items = vec![
            (9, 0),
            (3, 1),
            (5, 2),
            (1, 3),
            (2, 4),
            (7, 5),
            (0, 6),
            (4, 7),
            (6, 8),
            (8, 9),
        ];

        // K = 3 should return priorities [0, 1, 2]
        let k3 = top_k(&items, 3);
        let priorities: Vec<u8> = k3.into_iter().map(|(p, _)| p).collect();
        assert_eq!(priorities, vec![0, 1, 2]);

        // K = 5 should return priorities [0, 1, 2, 3, 4]
        let k5 = top_k(&items, 5);
        let priorities: Vec<u8> = k5.into_iter().map(|(p, _)| p).collect();
        assert_eq!(priorities, vec![0, 1, 2, 3, 4]);
    }

    #[test]
    fn test_when_grapheme_search_is_used() {
        // This test demonstrates when grapheme_starts_with is actually called
        // in the user_matches_search function

        // Regular ASCII - grapheme count == char count
        assert_eq!("hello".graphemes(true).count(), "hello".chars().count());

        // Family emoji - grapheme count != char count
        assert_ne!("👨‍👩‍👧‍👦".graphemes(true).count(), "👨‍👩‍👧‍👦".chars().count());
        assert_eq!("👨‍👩‍👧‍👦".graphemes(true).count(), 1);
        assert_eq!("👨‍👩‍👧‍👦".chars().count(), 7);

        // Combining character - grapheme count != char count
        // Using actual decomposed form: e (U+0065) + combining acute accent (U+0301)
        let decomposed = "e\u{0301}"; // e + combining acute accent
        assert_ne!(
            decomposed.graphemes(true).count(),
            decomposed.chars().count()
        );
        assert_eq!(decomposed.graphemes(true).count(), 1); // Shows as 1 grapheme
        assert_eq!(decomposed.chars().count(), 2); // But is 2 chars

        // Simple Chinese - grapheme count == char count
        assert_eq!("你好".graphemes(true).count(), "你好".chars().count());
    }
}
