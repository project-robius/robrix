//! Manages backwards pagination for a timeline, mostly between the UI and the async background task.
//!
/// One "new" behavior that we ensure is that the "loading older messages" notice
/// should always be visible in the timeline UI until the pagination completes
/// AND the new page of messages has been processed by the timelines.
/// This avoids the issue of prematurely hiding the loading message, which was probably
/// confusing for the user to see the loading message disappear without the new page
/// of messages being added to the timeline.
///
/// A backwards pagination request can be queued, running, or waiting for the UI to consume its completion.
/// Reaching the start of the timeline is remembered until the history is reset.
#[derive(Default)]
pub(super) struct BackwardsPaginationState {
    /// The UI asked for a page, but the backend hasn't reported that it started yet.
    is_queued: bool,
    /// The backend is loading a page or waiting for its timeline updates to be ready.
    is_running: bool,
    /// A successful result, which is waiting for [`Self::take_completed_result`]
    ///
    /// * If `true`, it means that means the start was reached.
    /// * If several results arrive before the UI consumes them, any `true` result is kept.
    did_completed_page_reach_start: Option<bool>,
    /// Whether the current history is known to include the start of the timeline.
    is_fully_paginated: bool,
}

impl BackwardsPaginationState {
    /// Returns whether the loading indicator should be visible.
    pub(super) fn is_loading(&self) -> bool {
        self.is_queued || self.is_running || self.did_completed_page_reach_start.is_some()
    }

    pub(super) fn is_fully_paginated(&self) -> bool {
        self.is_fully_paginated
    }

    /// Updates what we know about the pagination state after the UI has received new timeline items.
    ///
    /// ## Arguments
    /// * `is_empty`: whether the updated item list is empty. If so, this forgets the
    ///   old history's  start and changes any pending completion to `false`.
    /// * `has_timeline_start`: whether the first item is a timeline-start marker.
    ///   This confirms the new history's start (only when the list isn't empty).
    /// * `was_timeline_reset`: whether to discard the old history's start, even if
    ///   the updated list isn't empty. Also changes any pending completion to `false`.
    ///
    /// Thread-focused and event-focused timelines have no timeline-start item,
    /// so their pagination results tell us when to stop.
    pub(super) fn mark_items_updated(&mut self, is_empty: bool, has_timeline_start: bool, was_timeline_reset: bool) {
        if is_empty || was_timeline_reset {
            self.is_fully_paginated = false;
            self.did_completed_page_reach_start = self.did_completed_page_reach_start.map(|_| false);
        }
        if !is_empty {
            self.is_fully_paginated |= has_timeline_start;
        }
    }

    /// Returns whether we need more history in the timeline (whether we should keep back paginating).
    pub(super) fn needs_more_history(
        &self,
        is_near_start: bool,
        has_older_content: bool,
        fills_viewport: bool,
        is_searching: bool,
    ) -> bool {
        !self.is_fully_paginated
            && (is_near_start || !has_older_content || !fills_viewport || is_searching)
    }

    /// Records that a request is queued so that the loading indicator appears
    /// before the backend starts that request.
    pub(super) fn mark_requested(&mut self) {
        self.is_queued = true;
        self.did_completed_page_reach_start = None;
    }

    /// Records that the backend started pagination, including a request beyond this timeline UI.
    pub(super) fn mark_running(&mut self) {
        self.is_queued = false;
        self.is_running = true;
    }

    /// Records a successful pagination result, for use after the backend has sent its items to the UI.
    ///
    /// `is_fully_paginated` means that this pagination reached the timeline start.
    ///
    /// The loading indicator stays active until [`Self::take_completed_result`] consumes the result.
    pub(super) fn mark_completed(&mut self, is_fully_paginated: bool) {
        self.is_running = false;
        self.did_completed_page_reach_start = Some(is_fully_paginated || self.did_completed_page_reach_start == Some(true));
    }

    /// Consumes a successful result, only once no request is queued or running.
    ///
    /// * Returns `None` while we're still loading or have no pagination result yet.
    /// * Otherwise, returns whether the current history reaches the start.
    ///   That will be remembered for future calls to this function too.
    pub(super) fn take_completed_result(&mut self) -> Option<bool> {
        if self.is_queued || self.is_running {
            return None;
        }
        let is_fully_paginated = self.did_completed_page_reach_start.take()?;
        self.is_fully_paginated |= is_fully_paginated;
        Some(self.is_fully_paginated)
    }

    /// Marks a failed request as final, but keeps any earlier successful result for the UI to consume.
    pub(super) fn mark_error(&mut self) {
        self.is_running = false;
        self.is_queued = false;
        // Note that an earlier page in this batch of UI updates may have succeeded.
    }

    /// Forgets all requests, results, and knowledge of where history starts for a replaced timeline.
    pub(super) fn reset(&mut self) {
        *self = Self::default();
    }
}
