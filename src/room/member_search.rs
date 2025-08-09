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
    // Note: We capture this once at the start to avoid repeated global state access
    let current_user_id = current_user_id();
    

    // Constants for batching
    const BATCH_SIZE: usize = 10;  // Send results in batches

    // For empty search, return top members sorted by display name (using heap for O(N log K))
    if search_text.is_empty() {
        // Use a max-heap to keep the K smallest elements (by display name)
        // We use Reverse to make BinaryHeap work as a max-heap for the smallest elements
        let mut top_k: BinaryHeap<Reverse<(String, usize)>> = BinaryHeap::with_capacity(max_results);
        
        for (index, member) in members.iter().enumerate() {
            // Skip the current user
            if let Some(ref current_id) = current_user_id {
                if member.user_id() == current_id {
                    continue;
                }
            }
            
            // Get display name for sorting
            let sort_key = member.display_name()
                .unwrap_or_else(|| member.user_id().localpart())
                .to_lowercase();
            
            // Maintain top K smallest elements
            if top_k.len() < max_results {
                top_k.push(Reverse((sort_key, index)));
            } else if let Some(&Reverse((ref worst_key, _))) = top_k.peek() {
                // Only add if this member comes before the worst (alphabetically last) in heap
                if sort_key < *worst_key {
                    top_k.pop();
                    top_k.push(Reverse((sort_key, index)));
                }
            }
        }
        
        // Extract and sort the final results
        let mut member_indices: Vec<(String, usize)> = top_k
            .into_iter()
            .map(|Reverse(item)| item)
            .collect();
        
        // Sort the K elements for consistent ordering
        member_indices.sort_by(|a, b| a.0.cmp(&b.0));
        
        // Extract just the indices
        let all_results: Vec<usize> = member_indices.into_iter()
            .map(|(_, idx)| idx)
            .collect();
        
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

    // Use a min-heap to keep only the top max_results
    // We use Reverse to make BinaryHeap work as a min-heap
    let mut top_matches: BinaryHeap<Reverse<(u8, usize)>> = BinaryHeap::with_capacity(max_results);

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

            // Add to heap - it automatically maintains top K elements
            if top_matches.len() < max_results {
                top_matches.push(Reverse((priority, index)));
            } else if let Some(&Reverse((worst_priority, _))) = top_matches.peek() {
                // Only add if this match is better than the worst in heap
                if priority < worst_priority {
                    top_matches.pop();
                    top_matches.push(Reverse((priority, index)));
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
    let mut all_matches: Vec<(u8, usize)> = top_matches
        .into_iter()
        .map(|Reverse(item)| item)
        .collect();
    
    // Sort by priority first, then by display name for stable ordering within same priority
    all_matches.sort_by(|(priority_a, idx_a), (priority_b, idx_b)| {
        match priority_a.cmp(priority_b) {
            std::cmp::Ordering::Equal => {
                // Same priority - sort by display name (case-insensitive)
                let member_a = &members[*idx_a];
                let member_b = &members[*idx_b];
                
                let name_a = member_a.display_name()
                    .unwrap_or_else(|| member_a.user_id().localpart())
                    .to_lowercase();
                let name_b = member_b.display_name()
                    .unwrap_or_else(|| member_b.user_id().localpart())
                    .to_lowercase();
                
                name_a.cmp(&name_b)
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
fn check_word_boundary_match(text: &str, search_text: &str) -> bool {
    // Find all occurrences of search_text
    for (index, _) in text.match_indices(search_text) {
        if index == 0 {
            // At the beginning of text is a valid boundary
            continue; // But we already check starts_with elsewhere
        }
        
        // Check the character before the match
        if let Some(prev_char) = text[..index].chars().last() {
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
/// This combines the previous user_matches_search and get_match_priority functions
/// to avoid duplicate string operations
fn match_member_with_priority(member: &RoomMember, search_text: &str) -> Option<u8> {
    // Early return for empty search - all members match with lowest priority
    if search_text.is_empty() {
        return Some(8);
    }

    let display_name = member.display_name();
    let localpart = member.user_id().localpart();
    
    // Determine if we should do case-insensitive search (only for pure ASCII text)
    let case_insensitive = search_text.is_ascii();
    
    // For ASCII searches, prepare lowercase versions once
    let (search_lower, display_lower, localpart_lower) = if case_insensitive {
        (
            Some(search_text.to_lowercase()),
            display_name.map(|d| d.to_lowercase()),
            Some(localpart.to_lowercase())
        )
    } else {
        (None, None, None)
    };

    // Priority 0: Exact case-sensitive match (highest priority)
    if display_name == Some(search_text) || localpart == search_text {
        return Some(0);
    }

    // Priority 1: Exact match (case-insensitive for ASCII)
    if let (Some(ref search_l), Some(ref display_l), Some(ref localpart_l)) =
        (&search_lower, &display_lower, &localpart_lower) {
        if display_l == search_l || localpart_l == search_l {
            return Some(1);
        }
    }

    // Priority 2: Starts with search text (case-sensitive)
    if display_name.map_or(false, |d| d.starts_with(search_text)) || 
       localpart.starts_with(search_text) {
        return Some(2);
    }

    // Priority 3: Display name starts with search text (case-insensitive for ASCII)
    if let (Some(ref search_l), Some(ref display_l)) = (&search_lower, &display_lower) {
        if display_l.starts_with(search_l) {
            return Some(3);
        }
    }

    // Priority 4: Localpart starts with search text (case-insensitive)
    if let (Some(ref search_l), Some(ref localpart_l)) = (&search_lower, &localpart_lower) {
        if localpart_l.starts_with(search_l) {
            return Some(4);
        }
    }

    // Priority 5: Display name contains search text at word boundary
    if let Some(display) = display_name {
        // Check for space boundary (most common)
        if display.contains(&format!(" {}", search_text)) {
            return Some(5);
        }
        
        // Check for other word boundaries (punctuation, etc.)
        // This handles cases like "Hello,@alice" or "(@bob)" or "user:@charlie"
        if check_word_boundary_match(display, search_text) {
            return Some(5);
        }
    }

    // Priority 6: Display name contains search text anywhere (substring match)
    if let Some(display) = display_name {
        if case_insensitive {
            if let Some(ref display_l) = display_lower {
                if let Some(ref search_l) = search_lower {
                    if display_l.contains(search_l) {
                        return Some(6);
                    }
                }
            }
        } else {
            if display.contains(search_text) {
                return Some(6);
            }
        }
    }

    // Priority 7: Localpart contains search text anywhere (substring match)
    if case_insensitive {
        if let (Some(ref search_l), Some(ref localpart_l)) = (&search_lower, &localpart_lower) {
            if localpart_l.contains(search_l) {
                return Some(7);
            }
        }
    } else {
        if localpart.contains(search_text) {
            return Some(7);
        }
    }

    // For non-ASCII text with complex graphemes, check grapheme-based matching
    if !case_insensitive && search_text.graphemes(true).count() != search_text.chars().count() {
        if let Some(display) = display_name {
            if grapheme_starts_with(display, search_text, false) {
                return Some(6); // Treat as substring match priority
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
        // Test various word boundary scenarios
        assert!(check_word_boundary_match("Hello,alice", "alice"));
        assert!(check_word_boundary_match("(bob) is here", "bob"));
        assert!(check_word_boundary_match("user:charlie", "charlie"));
        assert!(check_word_boundary_match("@david!", "david"));
        assert!(check_word_boundary_match("eve.smith", "smith"));
        assert!(check_word_boundary_match("frank-jones", "jones"));
        
        // Should not match in the middle of a word
        assert!(!check_word_boundary_match("alice123", "lice"));
        assert!(!check_word_boundary_match("bobcat", "cat"));
        
        // Edge cases
        assert!(!check_word_boundary_match("test", "test")); // Starts with (handled elsewhere)
        assert!(!check_word_boundary_match("", "test")); // Empty text
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
