//! Room member search functionality for @mentions
//!
//! This module provides efficient searching of room members with streaming results
//! to support responsive UI when users type @mentions.

use std::sync::Arc;
use std::sync::mpsc::Sender;
use std::collections::BinaryHeap;
use matrix_sdk::room::{RoomMember, RoomMemberRole};
use unicode_segmentation::UnicodeSegmentation;
use crate::shared::mentionable_text_input::SearchResult;
use crate::sliding_sync::current_user_id;
use makepad_widgets::log;

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
        let power_rank = match member.suggested_role_for_power_level() {
            RoomMemberRole::Administrator => 0,
            RoomMemberRole::Moderator => 1,
            RoomMemberRole::User => 2,
        };
        
        // Get normalized display name
        let raw_name = member.display_name()
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
                Some(c) if c.is_alphabetic() => 0,  // Shouldn't happen if stripped is empty
                Some(c) if c.is_numeric() => 1,     // Shouldn't happen if stripped is empty
                _ => 2,  // Symbols
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
    sortable_members.sort_by(|a, b| {
        match a.0.cmp(&b.0) {
            std::cmp::Ordering::Equal => match a.1.cmp(&b.1) {
                std::cmp::Ordering::Equal => a.2.cmp(&b.2),
                other => other,
            },
            other => other,
        }
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

/// Search room members in background thread with streaming support (backward compatible)
pub fn search_room_members_streaming(
    members: Arc<Vec<RoomMember>>,
    search_text: String,
    max_results: usize,
    sender: Sender<SearchResult>,
) {
    search_room_members_streaming_with_sort(members, search_text, max_results, sender, None)
}

/// Search room members with optional pre-computed sort data
pub fn search_room_members_streaming_with_sort(
    members: Arc<Vec<RoomMember>>,
    search_text: String,
    max_results: usize,
    sender: Sender<SearchResult>,
    precomputed_sort: Option<Arc<PrecomputedMemberSort>>,
) {
    // Get current user ID to filter out self-mentions
    // Note: We capture this once at the start to avoid repeated global state access
    let current_user_id = current_user_id();
    

    // Constants for batching
    const BATCH_SIZE: usize = 10;  // Send results in batches

    // For empty search, use pre-computed sort if available
    if search_text.is_empty() {
        let all_results: Vec<usize> = if let Some(ref sort_data) = precomputed_sort {
            // Ultra-fast path: O(K) - just take first K from pre-sorted indices
            sort_data.sorted_indices
                .iter()
                .take(max_results)
                .copied()
                .collect()
        } else {
            // Fallback: compute on the fly (should rarely happen)
            let mut valid_members: Vec<(u8, u8, usize)> = Vec::with_capacity(members.len());
            
            for (index, member) in members.iter().enumerate() {
                // Skip the current user
                if let Some(ref current_id) = current_user_id {
                    if member.user_id() == current_id {
                        continue;
                    }
                }
                
                // Get power level rank (0=highest priority)
                let power_rank = match member.suggested_role_for_power_level() {
                    RoomMemberRole::Administrator => 0,
                    RoomMemberRole::Moderator => 1,
                    RoomMemberRole::User => 2,
                };
                
                // Get normalized display name
                let raw_name = member.display_name()
                    .map(|n| n.trim())
                    .filter(|n| !n.is_empty())
                    .unwrap_or_else(|| member.user_id().localpart());
                
                // Determine name category based on stripped name for consistency
                let stripped = raw_name.trim_start_matches(|c: char| !c.is_alphanumeric());
                let name_category = if !stripped.is_empty() {
                    match stripped.chars().next() {
                        Some(c) if c.is_alphabetic() => 0,  // Letters
                        Some(c) if c.is_numeric() => 1,     // Numbers
                        _ => 2,                              // Symbols
                    }
                } else {
                    2  // All symbols
                };
                
                valid_members.push((power_rank, name_category, index));
            }
            
            // Sort all members by (power_rank, name_category, then by actual name)
            valid_members.sort_by(|a, b| {
                match a.0.cmp(&b.0) {
                    std::cmp::Ordering::Equal => match a.1.cmp(&b.1) {
                        std::cmp::Ordering::Equal => {
                            // Only compute display names when needed for comparison
                            let name_a = members[a.2].display_name()
                                .map(|n| n.trim())
                                .filter(|n| !n.is_empty())
                                .unwrap_or_else(|| members[a.2].user_id().localpart());
                            let name_b = members[b.2].display_name()
                                .map(|n| n.trim())
                                .filter(|n| !n.is_empty())
                                .unwrap_or_else(|| members[b.2].user_id().localpart());
                            
                            // Simple case-insensitive comparison without creating new strings
                            name_a.chars()
                                .map(|c| c.to_ascii_lowercase())
                                .cmp(name_b.chars().map(|c| c.to_ascii_lowercase()))
                        },
                        other => other,
                    },
                    other => other,
                }
            });
            
            // Take only the first max_results
            valid_members.truncate(max_results);
            
            // Extract just the indices
            valid_members.into_iter()
                .map(|(_, _, idx)| idx)
                .collect()
        };
        
        let mut sent_count = 0;
        
        // Send in batches
        while sent_count < all_results.len() {
            let batch_end = (sent_count + BATCH_SIZE).min(all_results.len());
            let batch: Vec<_> = all_results[sent_count..batch_end].to_vec();
            sent_count = batch_end;

            let is_last = sent_count >= all_results.len();
            let search_result = SearchResult {
                results: batch,
                is_complete: is_last,
                search_text: search_text.clone(),
            };
            
            if sender.send(search_result).is_err() {
                return;
            }
        }
        
        // If no results were sent, send completion signal
        if all_results.is_empty() {
            let completion_result = SearchResult {
                results: Vec::new(),
                is_complete: true,
                search_text,
            };
            let _ = sender.send(completion_result);
        }
        return;
    }

    // Use a max-heap to keep only the top max_results (with best/smallest priorities)
    // Max-heap keeps the worst element (highest priority value) at the top for easy replacement
    let mut top_matches: BinaryHeap<(u8, usize)> = BinaryHeap::with_capacity(max_results);

    // Track if we have enough high-priority matches to stop early
    let mut high_priority_count = 0;

    for (index, member) in members.iter().enumerate() {
        
        // Skip the current user - users should not be able to mention themselves
        if let Some(ref current_id) = current_user_id {
            if member.user_id() == current_id {
                continue;
            }
        }

        // Check if this member matches the search text and get priority
        if let Some(priority) = match_member_with_priority(member, &search_text) {
            // Count high-priority matches (0-3 are exact or starts-with matches)
            if priority <= 3 {
                high_priority_count += 1;
            }

            // Add to heap - maintain top K elements with smallest priorities
            if top_matches.len() < max_results {
                top_matches.push((priority, index));
            } else if let Some(&(worst_priority, _)) = top_matches.peek() {
                // Only add if this match is better (smaller priority) than the worst in heap
                if priority < worst_priority {
                    top_matches.pop();  // Remove worst element
                    top_matches.push((priority, index));
                }
            }

            // Soft early exit: continue searching a bit more even after finding enough
            // high-priority matches to ensure we don't miss better matches
            // Only exit if we have significantly more high-priority matches than needed
            if high_priority_count >= max_results * 2 {
                break;
            }
        }
    }


    // Extract results from heap and sort them with stable secondary sorting
    let mut all_matches: Vec<(u8, usize)> = top_matches.into_iter().collect();
    
    // Sort by priority first, then by power level, then by name category, then by sort_key
    // This ensures consistency with empty search sorting
    all_matches.sort_by(|(priority_a, idx_a), (priority_b, idx_b)| {
        match priority_a.cmp(priority_b) {
            std::cmp::Ordering::Equal => {
                // Same priority - use precomputed sort keys if available
                if let Some(ref sort_data) = precomputed_sort {
                    // Get precomputed keys for efficient comparison
                    let key_a = &sort_data.member_keys[*idx_a];
                    let key_b = &sort_data.member_keys[*idx_b];
                    
                    // Sort by: power_rank → name_category → sort_key
                    match key_a.power_rank.cmp(&key_b.power_rank) {
                        std::cmp::Ordering::Equal => {
                            match key_a.name_category.cmp(&key_b.name_category) {
                                std::cmp::Ordering::Equal => key_a.sort_key.cmp(&key_b.sort_key),
                                other => other,
                            }
                        }
                        other => other,
                    }
                } else {
                    // Fallback: compute on the fly (should rarely happen)
                    let member_a = &members[*idx_a];
                    let member_b = &members[*idx_b];
                    
                    // Get power level ranks
                    let power_a = match member_a.suggested_role_for_power_level() {
                        RoomMemberRole::Administrator => 0,
                        RoomMemberRole::Moderator => 1,
                        RoomMemberRole::User => 2,
                    };
                    let power_b = match member_b.suggested_role_for_power_level() {
                        RoomMemberRole::Administrator => 0,
                        RoomMemberRole::Moderator => 1,
                        RoomMemberRole::User => 2,
                    };
                    
                    match power_a.cmp(&power_b) {
                        std::cmp::Ordering::Equal => {
                            // Same power level - sort by display name
                            let name_a = member_a.display_name()
                                .map(|n| n.trim())
                                .filter(|n| !n.is_empty())
                                .unwrap_or_else(|| member_a.user_id().localpart());
                            let name_b = member_b.display_name()
                                .map(|n| n.trim())
                                .filter(|n| !n.is_empty())
                                .unwrap_or_else(|| member_b.user_id().localpart());
                            
                            // Use efficient ASCII lowercase for ASCII strings
                            if name_a.is_ascii() && name_b.is_ascii() {
                                name_a.chars()
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

    // Send results in sorted batches
    let mut sent_count = 0;
    let total_results = all_matches.len();

    while sent_count < total_results {
        let batch_end = (sent_count + BATCH_SIZE).min(total_results);

        let batch: Vec<usize> = all_matches
            .get(sent_count..batch_end)
            .map(|slice| slice.iter()
                .map(|(_, idx)| *idx)
                .collect())
            .unwrap_or_else(Vec::new);

        if batch.is_empty() {
            break; // Safety: prevent infinite loop
        }

        sent_count = batch_end;
        let is_last_batch = sent_count >= total_results;


        let search_result = SearchResult {
            results: batch,
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

/// Check if search_text appears after a word boundary in text
/// Word boundaries include: punctuation, symbols, and other non-alphanumeric characters
/// For ASCII text, also supports case-insensitive matching
fn check_word_boundary_match(text: &str, search_text: &str, case_insensitive: bool) -> bool {
    // Prepare text for matching based on case sensitivity
    let (text_to_search, search_pattern) = if case_insensitive && search_text.is_ascii() {
        (text.to_lowercase(), search_text.to_lowercase())
    } else {
        (text.to_string(), search_text.to_string())
    };
    
    // Find all occurrences of search_pattern
    for (index, _) in text_to_search.match_indices(&search_pattern) {
        if index == 0 {
            // At the beginning of text is a valid boundary
            continue; // But we already check starts_with elsewhere
        }
        
        // Check the character before the match
        if let Some(prev_char) = text_to_search[..index].chars().last() {
            // Consider it a word boundary if previous char is not alphanumeric
            if !prev_char.is_alphanumeric() {
                return true;
            }
        }
    }
    false
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
    // Early return for empty search - all members match with lowest priority
    if search_text.is_empty() {
        return Some(10);
    }

    let display_name = member.display_name();
    let user_id = member.user_id().as_str();
    let localpart = member.user_id().localpart();
    
    // Determine if we should do case-insensitive search (only for pure ASCII text)
    let case_insensitive = search_text.is_ascii();
    
    // For ASCII searches, prepare lowercase versions once
    let (search_lower, display_lower, user_id_lower) = if case_insensitive {
        (
            Some(search_text.to_lowercase()),
            display_name.map(|d| d.to_lowercase()),
            Some(user_id.to_lowercase())
        )
    } else {
        (None, None, None)
    };

    // Priority 0: Exact display name match (case-sensitive)
    if display_name == Some(search_text) {
        return Some(0);
    }

    // Priority 1: Exact display name match (case-insensitive for ASCII)
    if let (Some(ref search_l), Some(ref display_l)) = (&search_lower, &display_lower) {
        if display_l == search_l {
            return Some(1);
        }
    }

    // Priority 2: Exact user ID match (with or without @)
    let search_with_at = if !search_text.starts_with('@') {
        format!("@{}", search_text)
    } else {
        search_text.to_string()
    };
    
    if user_id == search_with_at || user_id == search_text {
        return Some(2);
    }

    // Priority 3: Exact user ID match (case-insensitive for ASCII)
    if let (Some(ref user_id_l), Some(ref search_l)) = (&user_id_lower, &search_lower) {
        let search_with_at_lower = if !search_l.starts_with('@') {
            format!("@{}", search_l)
        } else {
            search_l.clone()
        };
        if user_id_l == &search_with_at_lower || user_id_l == search_l {
            return Some(3);
        }
    }

    // Priority 4: Display name starts with search text (case-sensitive)
    if display_name.map_or(false, |d| d.starts_with(search_text)) {
        return Some(4);
    }

    // Priority 5: Display name starts with search text (case-insensitive for ASCII)
    if let (Some(ref search_l), Some(ref display_l)) = (&search_lower, &display_lower) {
        if display_l.starts_with(search_l) {
            return Some(5);
        }
    }

    // Priority 6: User ID/localpart starts with search text (case-sensitive)
    if user_id.starts_with(&search_with_at) || user_id.starts_with(search_text) || 
       localpart.starts_with(search_text) {
        return Some(6);
    }

    // Priority 7: User ID/localpart starts with search text (case-insensitive)
    if let Some(ref search_l) = search_lower {
        let search_with_at_lower = if !search_l.starts_with('@') {
            format!("@{}", search_l)
        } else {
            search_l.clone()
        };
        if let Some(ref user_id_l) = user_id_lower {
            if user_id_l.starts_with(&search_with_at_lower) || user_id_l.starts_with(search_l) {
                return Some(7);
            }
        }
    }

    // Priority 8: Display name contains search text (at word boundary or anywhere)
    if let Some(display) = display_name {
        // Check for space boundary (most common)
        if display.contains(&format!(" {}", search_text)) {
            return Some(8);
        }
        
        // Check for other word boundaries
        if check_word_boundary_match(display, search_text, case_insensitive) {
            return Some(8);
        }
        
        // Check for general substring match
        if case_insensitive {
            if let (Some(ref display_l), Some(ref search_l)) = (&display_lower, &search_lower) {
                if display_l.contains(search_l) {
                    return Some(8);
                }
            }
        } else {
            if display.contains(search_text) {
                return Some(8);
            }
        }
    }

    // Priority 9: User ID contains search text anywhere
    if case_insensitive {
        if let (Some(ref search_l), Some(ref user_id_l)) = (&search_lower, &user_id_lower) {
            if user_id_l.contains(search_l) {
                return Some(9);
            }
        }
    } else {
        if user_id.contains(search_text) || localpart.contains(search_text) {
            return Some(9);
        }
    }

    // For non-ASCII text with complex graphemes, check grapheme-based matching
    if !case_insensitive && search_text.graphemes(true).count() != search_text.chars().count() {
        if let Some(display) = display_name {
            if grapheme_starts_with(display, search_text, false) {
                return Some(8); // Treat as display name contains
            }
        }
    }

    // No match found
    None
}



#[cfg(test)]
mod tests {
    use super::*;

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
        names.sort_by(|a, b| {
            match a.1.0.cmp(&b.1.0) {
                std::cmp::Ordering::Equal => a.1.1.cmp(&b.1.1),
                other => other,
            }
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
        assert_ne!(decomposed.graphemes(true).count(), decomposed.chars().count());
        assert_eq!(decomposed.graphemes(true).count(), 1); // Shows as 1 grapheme
        assert_eq!(decomposed.chars().count(), 2); // But is 2 chars
        
        // Simple Chinese - grapheme count == char count
        assert_eq!("你好".graphemes(true).count(), "你好".chars().count());
    }
}
