//! Room member search functionality for @mentions
//!
//! This module provides efficient searching of room members with streaming results
//! to support responsive UI when users type @mentions.

use std::sync::Arc;
use std::sync::mpsc::Sender;
use matrix_sdk::room::RoomMember;
use unicode_segmentation::UnicodeSegmentation;
use crate::shared::mentionable_text_input::SearchResult;
use crate::sliding_sync::current_user_id;

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
        
        for member in members.iter() {
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
            
            all_results.push((display_name, member.clone()));
            
            // Send in batches
            if all_results.len() >= sent_count + BATCH_SIZE {
                let batch_end = std::cmp::min(all_results.len(), sent_count + BATCH_SIZE);
                let batch: Vec<_> = all_results[sent_count..batch_end].to_vec();
                sent_count = batch_end;
                
                let search_result = SearchResult {
                    results: batch,
                    is_complete: false,
                    search_text: search_text.clone(),
                };
                if sender.send(search_result).is_err() {
                    return;
                }
            }
        }
        
        // Send any remaining results
        if sent_count < all_results.len() {
            let remaining: Vec<_> = all_results[sent_count..].to_vec();
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
    
    // Collect all matching results with their priorities
    let mut all_matches: Vec<(u8, String, RoomMember)> = Vec::new();
    
    for member in members.iter() {
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
            all_matches.push((priority, display_name, member.clone()));
            
            // Stop collecting after we have enough matches
            if all_matches.len() >= max_results * 3 {
                break;
            }
        }
    }
    
    
    // Sort all results by priority
    all_matches.sort_by_key(|(priority, _, _)| *priority);
    
    // Take only max_results
    all_matches.truncate(max_results);
    
    // Send results in sorted batches
    let mut sent_count = 0;
    let total_results = all_matches.len();
    
    while sent_count < total_results {
        let batch_end = std::cmp::min(total_results, sent_count + BATCH_SIZE);
        let batch: Vec<(String, RoomMember)> = all_matches[sent_count..batch_end]
            .iter()
            .map(|(_, name, member)| (name.clone(), member.clone()))
            .collect();
        
        sent_count = batch_end;
        let is_last_batch = sent_count >= total_results;
        
        
        let search_result = SearchResult {
            results: batch,
            is_complete: is_last_batch,
            search_text: search_text.clone(),
        };
        
        if sender.send(search_result).is_err() {
            return;
        }
    }
    
    // If we didn't send any results, send completion signal
    if total_results == 0 {
        let completion_result = SearchResult {
            results: Vec::new(),
            is_complete: true,
            search_text,
        };
        if sender.send(completion_result).is_err() {
            return;
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
        
        let grapheme_matches = if case_insensitive && h_grapheme.chars().all(|c| c.is_ascii()) && n_grapheme.chars().all(|c| c.is_ascii()) {
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
    let case_insensitive = search_text.chars().all(|c| c.is_ascii());
    
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
        }
        
        // Check localpart
        let localpart = member.user_id().localpart();
        if localpart.to_lowercase().starts_with(&search_lower) {
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
            // Only fall back to grapheme search for complex cases
            // (e.g., when search text has combining characters or emojis)
            if search_text.graphemes(true).count() != search_text.chars().count() {
                if grapheme_starts_with(display_name, search_text, false) {
                    return true;
                }
            }
        }
        
        // Check localpart - simple starts_with is sufficient
        let localpart = member.user_id().localpart();
        if localpart.starts_with(search_text) {
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
    let case_insensitive = search_text.chars().all(|c| c.is_ascii());

    // Priority 0: Exact case-sensitive match (highest priority)
    if display_name == search_text || localpart == search_text {
        return 0;
    }

    // Priority 1: Exact match (case-insensitive for ASCII)
    if case_insensitive {
        if display_name.to_lowercase() == search_text.to_lowercase() || 
           localpart.to_lowercase() == search_text.to_lowercase() {
            return 1;
        }
    }

    // Priority 2: Starts with search text (case-sensitive)
    if display_name.starts_with(search_text) || localpart.starts_with(search_text) {
        return 2;
    }

    // Priority 3: Starts with search text (case-insensitive for ASCII)
    if case_insensitive {
        let search_lower = search_text.to_lowercase();
        if display_name.to_lowercase().starts_with(&search_lower) {
            return 3;
        }
    }

    // Priority 4: Localpart starts with search text (case-insensitive)
    if case_insensitive && localpart.to_lowercase().starts_with(&search_text.to_lowercase()) {
        return 4;
    }

    // Priority 5: Display name contains search text at word boundary
    if display_name.contains(&format!(" {}", search_text)) {
        return 5;
    }

    // Priority 6: Other matches (shouldn't happen with optimized search)
    6
}