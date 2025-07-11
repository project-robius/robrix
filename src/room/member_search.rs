//! Room member search functionality for @mentions
//!
//! This module provides efficient searching of room members with streaming results
//! to support responsive UI when users type @mentions.

use std::sync::Arc;
use makepad_widgets::Cx;
use matrix_sdk::room::RoomMember;
use matrix_sdk::ruma::OwnedRoomId;
use crate::shared::mentionable_text_input::MentionableTextInputAction;
use crate::sliding_sync::current_user_id;

/// Search room members in background thread with streaming support
pub fn search_room_members_streaming(
    members: Arc<Vec<RoomMember>>,
    search_text: String,
    max_results: usize,
    _can_notify_room: bool,
    room_id: OwnedRoomId,
    search_id: u64,
) {
    let search_text_lower = search_text.to_lowercase();
    
    // Get current user ID to filter out self-mentions
    let current_user_id = current_user_id();
    
    // Constants for batching
    const BATCH_SIZE: usize = 3;  // Smaller batches for faster initial response
    const HIGH_PRIORITY_THRESHOLD: u8 = 3; // Priority 0-2 are high priority
    const FIRST_BATCH_SIZE: usize = 1; // Send first match immediately
    
    // For empty search, return all members (up to max_results)
    if search_text.is_empty() {
        let mut all_results = Vec::new();
        for member in members.iter().take(max_results) {
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
            
            // Send first batch immediately for responsiveness
            if all_results.len() == FIRST_BATCH_SIZE {
                Cx::post_action(MentionableTextInputAction::MemberSearchPartialResults {
                    room_id: room_id.clone(),
                    search_id,
                    results: all_results.clone(),
                    is_complete: false,
                    search_text: search_text.clone(),
                });
            }
        }
        
        // Send final results
        Cx::post_action(MentionableTextInputAction::MemberSearchPartialResults {
            room_id,
            search_id,
            results: all_results,
            is_complete: true,
            search_text,
        });
        return;
    }
    
    let mut high_priority_batch = Vec::new();
    let mut normal_priority_batch = Vec::new();
    let mut found_count = 0;
    let mut first_sent = false;
    
    for member in members.iter() {
        if found_count >= max_results {
            break;
        }
        
        // Skip the current user - users should not be able to mention themselves
        if let Some(ref current_id) = current_user_id {
            if member.user_id() == current_id {
                continue;
            }
        }
        
        // Check if this member matches the search text
        if user_matches_search(member, &search_text_lower) {
            let display_name = member
                .display_name()
                .map(|d| d.to_owned())
                .unwrap_or_else(|| member.user_id().to_string());
            
            let priority = get_match_priority(member, &search_text);
            found_count += 1;
            
            // Send first match immediately for instant feedback
            if !first_sent {
                first_sent = true;
                Cx::post_action(MentionableTextInputAction::MemberSearchPartialResults {
                    room_id: room_id.clone(),
                    search_id,
                    results: vec![(display_name.clone(), member.clone())],
                    is_complete: false,
                    search_text: search_text.clone(),
                });
            }
            
            // Batch by priority
            if priority < HIGH_PRIORITY_THRESHOLD {
                high_priority_batch.push((priority, display_name, member.clone()));
                if high_priority_batch.len() >= BATCH_SIZE {
                    send_partial_results(&mut high_priority_batch, &room_id, search_id, &search_text, false);
                }
            } else {
                normal_priority_batch.push((priority, display_name, member.clone()));
                if normal_priority_batch.len() >= BATCH_SIZE {
                    send_partial_results(&mut normal_priority_batch, &room_id, search_id, &search_text, false);
                }
            }
        }
    }
    
    // Send any remaining high priority results
    if !high_priority_batch.is_empty() {
        send_partial_results(&mut high_priority_batch, &room_id, search_id, &search_text, false);
    }
    
    // Send any remaining normal priority results
    if !normal_priority_batch.is_empty() {
        send_partial_results(&mut normal_priority_batch, &room_id, search_id, &search_text, false);
    }
    
    // Send completion signal with empty results
    Cx::post_action(MentionableTextInputAction::MemberSearchPartialResults {
        room_id,
        search_id,
        results: Vec::new(),
        is_complete: true,
        search_text,
    });
}

/// Helper function to send partial results
fn send_partial_results(
    batch: &mut Vec<(u8, String, RoomMember)>,
    room_id: &OwnedRoomId,
    search_id: u64,
    search_text: &str,
    is_complete: bool,
) {
    if batch.is_empty() {
        return;
    }
    
    // Sort batch by priority (lower = better)
    batch.sort_by_key(|(priority, _, _)| *priority);
    
    // Convert to results format
    let results: Vec<(String, RoomMember)> = batch
        .drain(..)
        .map(|(_, display_name, member)| (display_name, member))
        .collect();
    
    Cx::post_action(MentionableTextInputAction::MemberSearchPartialResults {
        room_id: room_id.clone(),
        search_id,
        results,
        is_complete,
        search_text: search_text.to_string(),
    });
}

/// Helper function to check if a user matches the search text
fn user_matches_search(member: &RoomMember, search_text: &str) -> bool {
    // Early return for empty search
    if search_text.is_empty() {
        return true;
    }
    
    // Check display name
    if let Some(display_name) = member.display_name() {
        if display_name.to_lowercase().contains(search_text) {
            return true;
        }
    }
    
    // Check user ID (without the @ prefix and domain for convenience)
    let user_id = member.user_id();
    let user_id_str = user_id.as_str();
    
    // Try full user ID
    if user_id_str.to_lowercase().contains(search_text) {
        return true;
    }
    
    // Try just the localpart (username without @domain)
    let localpart = user_id.localpart();
    if localpart.to_lowercase().contains(search_text) {
        return true;
    }
    
    false
}

/// Helper function to determine match priority for sorting
/// Lower values = higher priority (better matches shown first)
fn get_match_priority(member: &RoomMember, search_text: &str) -> u8 {
    let search_text_lower = search_text.to_lowercase();

    let display_name = member
        .display_name()
        .map(|n| n.to_string())
        .unwrap_or_else(|| member.user_id().to_string());

    let display_name_lower = display_name.to_lowercase();
    let localpart = member.user_id().localpart();
    let localpart_lower = localpart.to_lowercase();

    // Priority 0: Exact case-sensitive match (highest priority)
    if display_name == search_text || localpart == search_text {
        return 0;
    }

    // Priority 1: Exact match (case-insensitive)
    if display_name_lower == search_text_lower || localpart_lower == search_text_lower {
        return 1;
    }

    // Priority 2: Case-sensitive prefix match
    if display_name.starts_with(search_text) || localpart.starts_with(search_text) {
        return 2;
    }

    // Priority 3: Display name starts with search text (case-insensitive)
    if display_name_lower.starts_with(&search_text_lower) {
        return 3;
    }

    // Priority 4: Localpart starts with search text (case-insensitive)
    if localpart_lower.starts_with(&search_text_lower) {
        return 4;
    }

    // Priority 5: Display name contains search text at word boundary
    if let Some(pos) = display_name_lower.find(&search_text_lower) {
        // Check if it's at the start of a word (preceded by space or at start)
        if pos == 0 || display_name_lower.chars().nth(pos - 1) == Some(' ') {
            return 5;
        }
    }

    // Priority 6: Localpart contains search text at word boundary
    if let Some(pos) = localpart_lower.find(&search_text_lower) {
        // Check if it's at the start of a word (preceded by non-alphanumeric or at start)
        if pos == 0 || !localpart_lower.chars().nth(pos - 1).unwrap_or('a').is_alphanumeric() {
            return 6;
        }
    }

    // Priority 7: Display name contains search text (anywhere)
    if display_name_lower.contains(&search_text_lower) {
        return 7;
    }

    // Priority 8: Localpart contains search text (anywhere)
    if localpart_lower.contains(&search_text_lower) {
        return 8;
    }

    // Should not reach here if user_matches_search returned true
    u8::MAX
}