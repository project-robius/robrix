use std::time::{Duration, Instant};

const FINISHED_STREAM_TIMEOUT: Duration = Duration::from_secs(30);
const LIVE_STREAM_STALL_TIMEOUT: Duration = Duration::from_secs(5 * 60);

/// Characters to reveal per amortized chunk, closer to Moly's small-block growth.
const REVEAL_CHUNK_SIZE: usize = 2;
/// Fixed cadence for releasing each chunk.
const REVEAL_INTERVAL: Duration = Duration::from_millis(55);
/// Characters to reveal immediately when new content arrives after the UI had caught up.
const ARRIVAL_BURST: usize = 1;
/// When the stream is finished and this few chars remain, snap to the end.
const FINISH_SNAP_THRESHOLD: usize = 20;

/// Animation state for a single streaming message.
/// Tracks an MSC4357 live message and drives character-by-character reveal.
pub struct StreamingAnimState {
    pub target_text: String,
    pub target_char_count: usize,
    pub displayed_char_count: usize,
    pub displayed_byte_offset: usize,
    pub fractional_chunks: f64,
    pub last_update_time: Instant,
    pub last_tick_time: Instant,
    pub animation_start_time: Instant,
    pub display_buffer: String,
    /// Whether the message currently carries the MSC4357 `live` field.
    pub is_live: bool,
    pub timeline_index: Option<usize>,
}

impl StreamingAnimState {
    pub fn new(initial_text: &str, is_live: bool) -> Self {
        let char_count = initial_text.chars().count();
        let now = Instant::now();
        Self {
            target_text: initial_text.to_string(),
            target_char_count: char_count,
            displayed_char_count: 0,
            displayed_byte_offset: 0,
            fractional_chunks: 0.0,
            last_update_time: now,
            last_tick_time: now,
            animation_start_time: now,
            display_buffer: String::with_capacity(initial_text.len() + 4),
            is_live,
            timeline_index: None,
        }
    }

    pub fn restore(previous: &Self, new_text: &str, is_live: bool) -> Self {
        let mut restored = Self::new(new_text, is_live);
        let visible_prefix = &previous.target_text[..previous.displayed_byte_offset];
        let (common_chars, common_bytes) = common_prefix_len(visible_prefix, new_text);

        restored.displayed_char_count = common_chars;
        restored.displayed_byte_offset = common_bytes;
        restored.animation_start_time = previous.animation_start_time;
        restored.timeline_index = previous.timeline_index;
        restored
    }

    pub fn update_target(&mut self, new_text: &str, is_live: bool) {
        let prev_char_count = self.target_char_count;
        let had_backlog = self.displayed_char_count < prev_char_count;

        self.target_text.clear();
        self.target_text.push_str(new_text);
        self.target_char_count = new_text.chars().count();
        self.is_live = is_live;

        // Clamp char count if the new text is shorter than what was already displayed.
        if self.displayed_char_count > self.target_char_count {
            self.displayed_char_count = self.target_char_count;
        }

        // Always recalculate byte offset: the new text may have different
        // byte widths at already-displayed positions (e.g. markdown formatting
        // changes between streaming updates).
        self.displayed_byte_offset = self.target_text
            .char_indices()
            .nth(self.displayed_char_count)
            .map_or(self.target_text.len(), |(i, _)| i);

        // Arrival burst: only when we had fully caught up and were waiting
        // for more text. If backlog already exists, stay on the amortized cadence.
        let added_chars = self.target_char_count.saturating_sub(prev_char_count);
        if added_chars > 0 && !had_backlog {
            self.advance_displayed(added_chars.min(ARRIVAL_BURST));
        }

        let now = Instant::now();
        self.last_update_time = now;
        // If the animation had already caught up and was waiting for more text,
        // restart the frame clock so idle time doesn't count as reveal time.
        // If backlog already existed, keep the clock to preserve smooth cadence.
        if !had_backlog {
            self.last_tick_time = now;
        }
        // Reserve only the deficit (reserve(n) guarantees capacity >= len + n).
        let needed = new_text.len() + 4;
        if self.display_buffer.capacity() < needed {
            self.display_buffer.reserve(needed - self.display_buffer.len());
        }
    }

    pub fn advance_displayed(&mut self, chars_to_add: usize) {
        if chars_to_add == 0 || self.displayed_char_count >= self.target_char_count { return; }
        let remaining = &self.target_text[self.displayed_byte_offset..];
        let mut byte_advance = 0;
        let mut actual_chars = 0;
        for (byte_idx, _char) in remaining.char_indices() {
            if actual_chars >= chars_to_add { byte_advance = byte_idx; break; }
            actual_chars += 1;
        }
        if actual_chars <= chars_to_add && byte_advance == 0 && !remaining.is_empty() {
            byte_advance = remaining.len();
        }
        self.displayed_char_count = (self.displayed_char_count + actual_chars).min(self.target_char_count);
        self.displayed_byte_offset = (self.displayed_byte_offset + byte_advance).min(self.target_text.len());
    }

    pub fn tick(&mut self) -> bool {
        let now = Instant::now();
        let elapsed = now.saturating_duration_since(self.last_tick_time);
        self.last_tick_time = now;
        self.tick_with_elapsed(elapsed)
    }

    pub fn tick_with_elapsed(&mut self, elapsed: Duration) -> bool {
        if self.displayed_char_count >= self.target_char_count { return false; }
        let remaining = self.target_char_count - self.displayed_char_count;

        // Finish snap: when the stream is done and only a few chars remain, show them all.
        if !self.is_live && remaining <= FINISH_SNAP_THRESHOLD {
            self.advance_displayed(remaining);
            return true;
        }

        // Moly-style amortization: reveal fixed-size chunks at a fixed cadence
        // instead of accelerating as backlog grows.
        self.fractional_chunks += elapsed.as_secs_f64() / REVEAL_INTERVAL.as_secs_f64();
        let advance_chunks = self.fractional_chunks.floor() as usize;
        self.fractional_chunks -= advance_chunks as f64;
        if advance_chunks > 0 {
            self.advance_displayed(advance_chunks * REVEAL_CHUNK_SIZE);
            return true;
        }
        false
    }

    pub fn fill_display_buffer(&mut self) {
        self.display_buffer.clear();
        self.display_buffer.push_str(&self.target_text[..self.displayed_byte_offset]);
        self.display_buffer.push_str(" \u{25CF}");
    }

    pub fn needs_frame(&self) -> bool {
        self.displayed_char_count < self.target_char_count
    }

    /// Streaming is complete when the live field is absent and all text has been revealed.
    pub fn is_complete(&self) -> bool {
        !self.needs_frame() && !self.is_live
    }

    pub fn timeout_after(&self) -> Duration {
        if self.is_live {
            LIVE_STREAM_STALL_TIMEOUT
        } else {
            FINISHED_STREAM_TIMEOUT
        }
    }

    pub fn is_timed_out(&self) -> bool {
        self.last_update_time.elapsed() > self.timeout_after()
    }
}

fn common_prefix_len(lhs: &str, rhs: &str) -> (usize, usize) {
    let mut chars = 0;
    let mut bytes = 0;
    let mut lhs_chars = lhs.chars();

    for (byte_idx, rhs_char) in rhs.char_indices() {
        let Some(lhs_char) = lhs_chars.next() else {
            break;
        };
        if lhs_char != rhs_char {
            break;
        }
        chars += 1;
        bytes = byte_idx + rhs_char.len_utf8();
    }

    (chars, bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_state(text: &str) -> StreamingAnimState {
        StreamingAnimState::new(text, true)
    }

    #[test]
    fn test_advance_ascii() {
        let mut s = make_state("Hello, world!");
        s.advance_displayed(5);
        assert_eq!(s.displayed_char_count, 5);
        assert_eq!(&s.target_text[..s.displayed_byte_offset], "Hello");
    }

    #[test]
    fn test_advance_utf8_multibyte() {
        let mut s = make_state("你好世界abcd");
        s.advance_displayed(2);
        assert_eq!(s.displayed_char_count, 2);
        assert_eq!(&s.target_text[..s.displayed_byte_offset], "你好");
    }

    #[test]
    fn test_advance_clamps_at_end() {
        let mut s = make_state("abc");
        s.advance_displayed(100);
        assert_eq!(s.displayed_char_count, 3);
        assert_eq!(s.displayed_byte_offset, 3);
    }

    #[test]
    fn test_update_target_extends() {
        let mut s = make_state("Hello");
        s.advance_displayed(5);
        s.update_target("Hello, world!", true);
        assert_eq!(s.target_char_count, 13);
        // Arrival burst reveals only the newly added chars, capped by ARRIVAL_BURST.
        assert_eq!(s.displayed_char_count, 5 + ARRIVAL_BURST.min(8));
    }

    #[test]
    fn test_update_target_uses_single_char_burst_when_waiting_for_new_text() {
        let mut s = make_state("Hello");
        s.advance_displayed(5);
        s.update_target("Hello, world!", true);
        assert_eq!(s.displayed_char_count, 6);
    }

    #[test]
    fn test_update_target_does_not_burst_while_backlog_exists() {
        let mut s = make_state("Hello");
        s.advance_displayed(2);
        s.update_target("Hello!", true);
        // When backlog already exists, keep the amortized cadence instead of
        // applying a fresh burst on every incoming update.
        assert_eq!(s.displayed_char_count, 2);
    }

    #[test]
    fn test_update_target_shrinks_safely() {
        let mut s = make_state("Hello, world!");
        s.advance_displayed(10);
        s.update_target("Hi", true);
        assert_eq!(s.displayed_char_count, 2);
        assert_eq!(s.displayed_byte_offset, 2);
        s.fill_display_buffer();
        assert!(s.display_buffer.starts_with("Hi"));
    }

    #[test]
    fn test_update_target_recalculates_byte_offset_for_different_prefix() {
        // Simulate: displayed 5 ASCII chars, then text replaced with CJK characters.
        // Old byte offset (5) would be inside a multi-byte char in the new text.
        let mut s = make_state("hello world");
        s.advance_displayed(5);
        assert_eq!(s.displayed_byte_offset, 5);

        // New text has 5+ chars but first 5 chars are 3-byte CJK.
        // Without the fix, displayed_byte_offset stays 5, crashing on slice.
        s.update_target("你好世界测试数据", true);
        assert_eq!(s.displayed_char_count, 5);
        // 5 CJK chars × 3 bytes = 15
        assert_eq!(s.displayed_byte_offset, 15);
        // Must not panic:
        s.fill_display_buffer();
        assert!(s.display_buffer.starts_with("你好世界测"));
    }

    #[test]
    fn test_tick_advances() {
        let mut s = make_state("Hello, world!");
        let changed = s.tick_with_elapsed(REVEAL_INTERVAL);
        assert!(changed);
        assert_eq!(s.displayed_char_count, REVEAL_CHUNK_SIZE);
    }

    #[test]
    fn test_tick_waits_for_full_chunk_interval() {
        let mut s = make_state("Hello, world!");
        assert!(!s.tick_with_elapsed(REVEAL_INTERVAL / 2));
        assert_eq!(s.displayed_char_count, 0);
    }

    #[test]
    fn test_tick_large_gap_smooth() {
        let mut s = make_state(&"a".repeat(1000));
        // Even after a large elapsed gap, keep a steady amortized pace.
        assert!(s.tick_with_elapsed(Duration::from_secs(1)));
        assert!(s.displayed_char_count >= 30);
        assert!(s.displayed_char_count <= 40);
    }

    #[test]
    fn test_fill_display_buffer() {
        let mut s = make_state("Hello");
        s.advance_displayed(3);
        s.fill_display_buffer();
        assert!(s.display_buffer.starts_with("Hel"));
        assert!(s.display_buffer.contains('\u{25CF}') || s.display_buffer.contains('●'));
    }

    #[test]
    fn test_is_complete_msc4357() {
        let mut s = make_state("Hi");
        s.advance_displayed(2);
        // is_live=true → not complete even though all text revealed
        assert!(!s.is_complete());
        // Simulate final edit without live field
        s.is_live = false;
        assert!(s.is_complete());
    }

    #[test]
    fn test_update_target_sets_live() {
        let mut s = make_state("Hello");
        assert!(s.is_live);
        s.update_target("Hello, world!", false);
        assert!(!s.is_live);
    }

    #[test]
    fn test_restore_preserves_common_prefix() {
        // Extension: keep what was already displayed
        let mut prev = make_state("Hello, world!");
        prev.advance_displayed(5);
        let restored = StreamingAnimState::restore(&prev, "Hello, world!!!", true);
        assert_eq!(restored.displayed_char_count, 5);
        assert_eq!(&restored.target_text[..restored.displayed_byte_offset], "Hello");

        // Divergence: clamp to the common prefix
        let mut prev2 = make_state("Hello, world!");
        prev2.advance_displayed(12);
        let restored2 = StreamingAnimState::restore(&prev2, "Hello there", true);
        assert_eq!(&restored2.target_text[..restored2.displayed_byte_offset], "Hello");
    }

    #[test]
    fn test_timeout_split_by_live_state() {
        // Live stream survives 31s idle (5min stall timeout)
        let mut live = make_state("Hello");
        live.last_update_time = Instant::now() - Duration::from_secs(31);
        assert!(!live.is_timed_out());

        // Finished stream times out after 31s (30s cleanup timeout)
        let mut finished = make_state("Hello");
        finished.is_live = false;
        finished.last_update_time = Instant::now() - Duration::from_secs(31);
        assert!(finished.is_timed_out());
    }

    #[test]
    fn test_tick_zero_elapsed() {
        let mut s = make_state("Hello");
        assert!(!s.tick_with_elapsed(Duration::ZERO));
        assert_eq!(s.displayed_char_count, 0);
    }

    #[test]
    fn test_update_target_preserves_tick_clock_when_backlog_already_exists() {
        let mut s = make_state("Hello, world!");
        s.advance_displayed(3);
        let before = Instant::now() - Duration::from_millis(120);
        s.last_tick_time = before;

        s.update_target("Hello, world!!!", true);

        assert_eq!(s.last_tick_time, before);
    }

    #[test]
    fn test_update_target_resets_tick_clock_when_waiting_for_new_text() {
        let mut s = make_state("Hello");
        s.advance_displayed(5);
        let before = Instant::now() - Duration::from_secs(5);
        s.last_tick_time = before;

        s.update_target("Hello!", true);

        assert!(s.last_tick_time > before);
    }

    #[test]
    fn test_finish_snap() {
        let mut s = make_state(&"a".repeat(30));
        s.advance_displayed(20);
        // 10 remaining but is_live=true → normal tick, no snap.
        s.tick_with_elapsed(Duration::from_millis(16));
        assert!(s.displayed_char_count < 30);

        // Mark as finished → remaining <= FINISH_SNAP_THRESHOLD → snaps to end.
        s.is_live = false;
        assert!(s.tick_with_elapsed(Duration::from_millis(1)));
        assert_eq!(s.displayed_char_count, 30);
    }

}
