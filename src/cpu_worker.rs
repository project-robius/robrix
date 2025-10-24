//! Lightweight single-thread CPU worker.
//!
//! This module keeps a single background thread alive for the duration of
//! the application. The worker owns an mpsc channel; the sender side lives
//! in a `OnceLock`, so the thread remains active until the process exits.
//! Jobs are executed immediately and their data drops afterwards, so there
//! is no memory leak risk even if the thread stays resident.
//!
//! TODO:
//! * add an explicit `shutdown()` helper that enqueues `CpuJob::Shutdown`
//!   and joins the worker thread when the application is torn down.
//! * evaluate migrating to a small thread pool if we add more CPU-bound
//!   tasks in the future or need increased throughput.
//!
use makepad_widgets::{log, Cx, CxOsApi};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::{
    atomic::AtomicBool,
    mpsc::{self, Sender, TryRecvError},
    Arc, OnceLock,
};
use std::thread;
use std::time::Duration;

use crate::{
    room::member_search::{search_room_members_streaming_with_sort, PrecomputedMemberSort},
    shared::mentionable_text_input::SearchResult,
};
use matrix_sdk::room::RoomMember;

pub enum CpuJob {
    SearchRoomMembers(SearchRoomMembersJob),
    Shutdown,
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

static CPU_WORKER_SENDER: OnceLock<Sender<CpuJob>> = OnceLock::new();

/// Initializes the global CPU worker thread. Safe to call multiple times; only the first call will
/// spawn the worker.
pub fn init(cx: &mut Cx) {
    if CPU_WORKER_SENDER.get().is_some() {
        return;
    }

    let (sender, receiver) = mpsc::channel::<CpuJob>();

    if CPU_WORKER_SENDER.set(sender).is_err() {
        // Another thread managed to install the sender first; nothing to do.
        return;
    }

    cx.spawn_thread(move || loop {
        match receiver.try_recv() {
            Ok(job) => {
                let continue_running = match catch_unwind(AssertUnwindSafe(|| dispatch_job(job))) {
                    Ok(should_continue) => should_continue,
                    Err(err) => {
                        log!("CPU worker job panicked: {:?}", err);
                        true
                    }
                };

                if !continue_running {
                    log!("CPU worker thread exiting");
                    break;
                }
            }
            Err(TryRecvError::Empty) => {
                // No work at the moment; yield briefly to avoid busy-spinning
                thread::sleep(Duration::from_millis(1));
            }
            Err(TryRecvError::Disconnected) => {
                log!("CPU worker channel disconnected, exiting thread");
                break;
            }
        }
    });
}

fn dispatch_job(job: CpuJob) -> bool {
    match job {
        CpuJob::SearchRoomMembers(params) => {
            run_member_search(params);
            true
        }
        CpuJob::Shutdown => false,
    }
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

/// Spawns a job on the dedicated CPU worker thread.
pub fn spawn_cpu_job(job: CpuJob) {
    match CPU_WORKER_SENDER.get() {
        Some(sender) => {
            if sender.send(job).is_err() {
                log!("Failed to submit job to CPU worker: worker thread has exited");
            }
        }
        None => {
            log!("CPU worker not initialized; dropping job");
        }
    }
}
