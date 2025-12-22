//! Lightweight wrapper for CPU-bound tasks.
//!
//! Currently each job is handled by spawning a detached native thread via
//! Makepad's `cx.spawn_thread`. This keeps the implementation simple while
//! still moving CPU-heavy work off the UI thread.
//!
//! ## Future TODOs
//! - TODO: Add task queue with priority and deduplication
//! - TODO: Limit max concurrent tasks (e.g., 2-4 workers)
//! - TODO: Add platform-specific thread pool (desktop only, via #[cfg])
//! - TODO: Support task cancellation and timeout
//! - TODO: Add progress callbacks for long-running tasks

use makepad_widgets::{Cx, CxOsApi};
use std::sync::{atomic::AtomicBool, mpsc::Sender, Arc};
use crate::{
    room::member_search::{search_room_members_streaming_with_sort, PrecomputedMemberSort},
    shared::mentionable_text_input::SearchResult,
};
use matrix_sdk::room::RoomMember;

pub enum CpuJob {
    SearchRoomMembers(SearchRoomMembersJob),
}

pub struct SearchRoomMembersJob {
    pub members: Arc<Vec<RoomMember>>,
    pub search_text: String,
    pub max_results: usize,
    pub sender: Sender<SearchResult>,
    pub search_id: u64,
    pub precomputed_sort: Option<Arc<PrecomputedMemberSort>>,
    pub cancel_token: Option<Arc<AtomicBool>>,
}

fn run_member_search(params: SearchRoomMembersJob) {
    let SearchRoomMembersJob {
        members,
        search_text,
        max_results,
        sender,
        search_id,
        precomputed_sort,
        cancel_token,
    } = params;

    search_room_members_streaming_with_sort(
        members,
        search_text,
        max_results,
        sender,
        search_id,
        precomputed_sort,
        cancel_token,
    );
}

/// Spawns a CPU-bound job on a detached native thread.
pub fn spawn_cpu_job(cx: &mut Cx, job: CpuJob) {
    cx.spawn_thread(move || match job {
        CpuJob::SearchRoomMembers(params) => run_member_search(params),
    });
}
