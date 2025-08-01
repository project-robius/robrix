//! Room member search functionality for @mentions
//!
//! This module provides efficient searching of room members with streaming results
//! to support responsive UI when users type @mentions.

use std::sync::Arc;
use std::sync::mpsc::Sender;
use std::collections::BinaryHeap;
use std::cmp::Reverse;
use matrix_sdk::room::RoomMember;
use unicode_segmentation::UnicodeSegmentation;
use crate::shared::mentionable_text_input::SearchResult;
use crate::sliding_sync::current_user_id;
use makepad_widgets::log;

/// Search room members in background thread with streaming support
pub fn search_room_members_streaming(
    members: Arc<Vec<RoomMember>>,
    search_text: String,
    max_results: usize,
    sender: Sender<SearchResult>,
) {
    // Get current user ID to filter out self-mentions
    let current_user_id = current_user_id();

    // Constants for batching
    const BATCH_SIZE: usize = 10;  // Send results in batches

    // For empty search, return all members (up to max_results)
    if search_text.is_empty() {
        let mut all_results = Vec::new();
        let mut sent_count = 0;

        for (index, member) in members.iter().enumerate() {
            if all_results.len() >= max_results {
                break;
            }

            // Skip the current user
            if let Some(ref current_id) = current_user_id {
                if member.user_id() == current_id {
                    continue;
                }
            }

            let display_name = member.display_name()
                .map(|d| d.to_owned())
                .unwrap_or_else(|| member.user_id().to_string());

            all_results.push((display_name, index));

            // Send in batches
            if all_results.len() >= sent_count + BATCH_SIZE {
                let batch_end = (sent_count + BATCH_SIZE).min(all_results.len());
                let batch: Vec<_> = all_results.get(sent_count..batch_end)
                    .map(|slice| slice.to_vec())
                    .unwrap_or_else(Vec::new);
                sent_count = batch_end;

                let search_result = SearchResult {
                    results: batch.clone(),
                    is_complete: false,
                    search_text: search_text.clone(),
                };
                // Sending batch of results
                if sender.send(search_result).is_err() {
                    // Failed to send search results - receiver dropped
                    return;
                }
            }
        }

        // Send any remaining results
        if sent_count < all_results.len() {
            let remaining: Vec<_> = all_results.get(sent_count..)
                .map(|slice| slice.to_vec())
                .unwrap_or_else(Vec::new);
            let search_result = SearchResult {
                results: remaining,
                is_complete: true,
                search_text: search_text.clone(),
            };
            if sender.send(search_result).is_err() {
                return;
            }
        } else {
            // Send completion signal
            let completion_result = SearchResult {
                results: Vec::new(),
                is_complete: true,
                search_text,
            };
            if sender.send(completion_result).is_err() {
                return;
            }
        }
        return;
    }

    // Use a min-heap to keep only the top max_results
    // We use Reverse to make BinaryHeap work as a min-heap
    let mut top_matches: BinaryHeap<Reverse<(u8, String, usize)>> = BinaryHeap::with_capacity(max_results);

    // Track if we have enough high-priority matches to stop early
    let mut high_priority_count = 0;

    for (index, member) in members.iter().enumerate() {
        // Skip the current user - users should not be able to mention themselves
        if let Some(ref current_id) = current_user_id {
            if member.user_id() == current_id {
                continue;
            }
        }

        // Check if this member matches the search text
        if user_matches_search(member, &search_text) {
            let display_name = member
                .display_name()
                .map(|d| d.to_owned())
                .unwrap_or_else(|| member.user_id().to_string());

            let priority = get_match_priority(member, &search_text);

            // Count high-priority matches (0-3 are exact or starts-with matches)
            if priority <= 3 {
                high_priority_count += 1;
            }

            // Add to heap - it automatically maintains top K elements
            if top_matches.len() < max_results {
                top_matches.push(Reverse((priority, display_name, index)));
            } else if let Some(&Reverse((worst_priority, _, _))) = top_matches.peek() {
                // Only add if this match is better than the worst in heap
                if priority < worst_priority {
                    top_matches.pop();
                    top_matches.push(Reverse((priority, display_name, index)));
                }
            }

            // Early exit: if we have enough high-priority matches, stop searching
            if high_priority_count >= max_results {
                break;
            }
        }
    }


    // Extract results from heap and sort them
    let mut all_matches: Vec<(u8, String, usize)> = top_matches
        .into_iter()
        .map(|Reverse(item)| item)
        .collect();
    all_matches.sort_by_key(|(priority, _, _)| *priority);

    // Send results in sorted batches
    let mut sent_count = 0;
    let total_results = all_matches.len();

    while sent_count < total_results {
        let batch_end = (sent_count + BATCH_SIZE).min(total_results);

        let batch: Vec<(String, usize)> = all_matches
            .get(sent_count..batch_end)
            .map(|slice| slice.iter()
                .map(|(_, name, idx)| (name.clone(), *idx))
                .collect())
            .unwrap_or_else(Vec::new);

        if batch.is_empty() {
            break; // Safety: prevent infinite loop
        }

        sent_count = batch_end;
        let is_last_batch = sent_count >= total_results;


        let search_result = SearchResult {
            results: batch.clone(),
            is_complete: is_last_batch,
            search_text: search_text.clone(),
        };

        // Sending search results
        if sender.send(search_result).is_err() {
            log!("Failed to send search results - receiver dropped");
            return;
        }
    }

    // If we didn't send any results, send completion signal
    if total_results == 0 {
        // No search results found, sending completion signal
        let completion_result = SearchResult {
            results: Vec::new(),
            is_complete: true,
            search_text,
        };
        if sender.send(completion_result).is_err() {
            // Failed to send completion signal - receiver dropped
        }
    }
}

/// Helper function to check if a string starts with another string based on graphemes
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

        let grapheme_matches = if case_insensitive && h_grapheme.is_ascii() && n_grapheme.is_ascii() {
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

/// Helper function to check if a user matches the search text
fn user_matches_search(member: &RoomMember, search_text: &str) -> bool {
    // Early return for empty search
    if search_text.is_empty() {
        return true;
    }

    // Determine if we should do case-insensitive search (only for pure ASCII text)
    let case_insensitive = search_text.is_ascii();

    // For ASCII searches, use simple string operations which are much faster
    if case_insensitive {
        let search_lower = search_text.to_lowercase();

        // Check display name
        if let Some(display_name) = member.display_name() {
            let display_lower = display_name.to_lowercase();
            if display_lower.starts_with(&search_lower) {
                return true;
            }
            // Check word boundary (simple version for performance)
            if display_lower.contains(&format!(" {}", search_lower)) {
                return true;
            }
            // Check if search text appears anywhere in display name
            if display_lower.contains(&search_lower) {
                return true;
            }
        }

        // Check localpart
        let localpart = member.user_id().localpart();
        let localpart_lower = localpart.to_lowercase();
        if localpart_lower.starts_with(&search_lower) {
            return true;
        }
        // Check if search text appears anywhere in localpart
        if localpart_lower.contains(&search_lower) {
            return true;
        }
    } else {
        // For non-ASCII text, try simple string operations first (much faster for most cases)
        // Check display name
        if let Some(display_name) = member.display_name() {
            // First try simple starts_with (works for most Chinese names)
            if display_name.starts_with(search_text) {
                return true;
            }
            // Check if it appears after a space (common word boundary)
            if display_name.contains(&format!(" {}", search_text)) {
                return true;
            }
            // Check if search text appears anywhere in display name
            if display_name.contains(search_text) {
                return true;
            }
            // Only fall back to grapheme search for complex cases
            // (e.g., when search text has combining characters or emojis)
            if search_text.graphemes(true).count() != search_text.chars().count() {
                if grapheme_starts_with(display_name, search_text, false) {
                    return true;
                }
            }
        }

        // Check localpart
        let localpart = member.user_id().localpart();
        if localpart.starts_with(search_text) {
            return true;
        }
        // Check if search text appears anywhere in localpart
        if localpart.contains(search_text) {
            return true;
        }
    }

    false
}


/// Helper function to determine match priority for sorting
/// Lower values = higher priority (better matches shown first)
fn get_match_priority(member: &RoomMember, search_text: &str) -> u8 {
    let display_name = member
        .display_name()
        .map(|n| n.to_string())
        .unwrap_or_else(|| member.user_id().to_string());

    let localpart = member.user_id().localpart();

    // Determine if we should do case-insensitive comparison
    let case_insensitive = search_text.is_ascii();

    // Cache lowercase conversions for ASCII text to avoid repeated allocations
    let search_lower = if case_insensitive { Some(search_text.to_lowercase()) } else { None };
    let display_lower = if case_insensitive { Some(display_name.to_lowercase()) } else { None };
    let localpart_lower = if case_insensitive { Some(localpart.to_lowercase()) } else { None };

    // Priority 0: Exact case-sensitive match (highest priority)
    if display_name == search_text || localpart == search_text {
        return 0;
    }

    // Priority 1: Exact match (case-insensitive for ASCII)
    if let (Some(ref search_l), Some(ref display_l), Some(ref localpart_l)) =
        (&search_lower, &display_lower, &localpart_lower) {
        if display_l == search_l || localpart_l == search_l {
            return 1;
        }
    }

    // Priority 2: Starts with search text (case-sensitive)
    if display_name.starts_with(search_text) || localpart.starts_with(search_text) {
        return 2;
    }

    // Priority 3: Starts with search text (case-insensitive for ASCII)
    if let (Some(ref search_l), Some(ref display_l)) = (&search_lower, &display_lower) {
        if display_l.starts_with(search_l) {
            return 3;
        }
    }

    // Priority 4: Localpart starts with search text (case-insensitive)
    if let (Some(ref search_l), Some(ref localpart_l)) = (&search_lower, &localpart_lower) {
        if localpart_l.starts_with(search_l) {
            return 4;
        }
    }

    // Priority 5: Display name contains search text at word boundary
    if display_name.contains(&format!(" {}", search_text)) {
        return 5;
    }

    // Priority 6: Display name contains search text anywhere (substring match)
    if let (Some(ref search_l), Some(ref display_l)) = (&search_lower, &display_lower) {
        if display_l.contains(search_l) {
            return 6;
        }
    } else if !case_insensitive && display_name.contains(search_text) {
        return 6;
    }

    // Priority 7: Localpart contains search text anywhere (substring match)
    if let (Some(ref search_l), Some(ref localpart_l)) = (&search_lower, &localpart_lower) {
        if localpart_l.contains(search_l) {
            return 7;
        }
    } else if !case_insensitive && localpart.contains(search_text) {
        return 7;
    }

    // Priority 8: Other matches (shouldn't happen with optimized search)
    8
}
