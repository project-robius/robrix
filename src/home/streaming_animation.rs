use std::time::{Duration, Instant};

const FINISHED_STREAM_TIMEOUT: Duration = Duration::from_secs(30);
const LIVE_STREAM_STALL_TIMEOUT: Duration = Duration::from_secs(5 * 60);

/// Animation state for a single streaming message.
/// Tracks an MSC4357 live message and drives character-by-character reveal.
pub struct StreamingAnimState {
    pub target_text: String,
    pub target_char_count: usize,
    pub displayed_char_count: usize,
    pub displayed_byte_offset: usize,
    pub chars_per_second: f64,
    pub fractional_chars: f64,
    pub last_update_time: Instant,
    pub last_tick_time: Instant,
    pub animation_start_time: Instant,
    pub chars_at_last_update: usize,
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
            chars_per_second: 1.0,
            fractional_chars: 0.0,
            last_update_time: now,
            last_tick_time: now,
            animation_start_time: now,
            chars_at_last_update: 0,
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
        restored.chars_at_last_update = common_chars;
        restored.animation_start_time = previous.animation_start_time;
        restored.timeline_index = previous.timeline_index;
        restored.update_speed();
        restored
    }

    pub fn update_target(&mut self, new_text: &str, is_live: bool) {
        self.target_text.clear();
        self.target_text.push_str(new_text);
        self.target_char_count = new_text.chars().count();
        self.is_live = is_live;

        // Clamp display pointers if the new text is shorter than what was already displayed.
        if self.displayed_char_count > self.target_char_count {
            self.displayed_char_count = self.target_char_count;
            self.displayed_byte_offset = self.target_text
                .char_indices()
                .nth(self.target_char_count)
                .map_or(self.target_text.len(), |(i, _)| i);
        }

        let now = Instant::now();
        self.chars_at_last_update = self.displayed_char_count;
        self.last_update_time = now;
        self.last_tick_time = now;
        self.update_speed();
        // Reserve only the deficit (reserve(n) guarantees capacity >= len + n).
        let needed = new_text.len() + 4;
        if self.display_buffer.capacity() < needed {
            self.display_buffer.reserve(needed - self.display_buffer.len());
        }
    }

    fn update_speed(&mut self) {
        let remaining = self.target_char_count.saturating_sub(self.displayed_char_count);
        if remaining > 0 {
            self.chars_per_second = remaining as f64;
            if self.chars_per_second < 30.0 {
                self.chars_per_second = 30.0;
            }
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
        let gap = self.target_char_count - self.displayed_char_count;
        let mut changed = false;

        let speed = if gap > 500 {
            let jump = gap - 50;
            self.advance_displayed(jump);
            changed = true;
            self.chars_per_second
        } else if gap > 200 {
            self.chars_per_second * 3.0
        } else {
            self.chars_per_second
        };

        self.fractional_chars += speed * elapsed.as_secs_f64();
        let advance = self.fractional_chars.floor() as usize;
        self.fractional_chars -= advance as f64;
        if advance > 0 {
            self.advance_displayed(advance);
            changed = true;
        }
        changed
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
        assert_eq!(s.displayed_char_count, 5);
        assert!(s.chars_per_second > 0.0);
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
    fn test_tick_advances() {
        let mut s = make_state("Hello, world!");
        s.chars_per_second = 4.0;
        let changed = s.tick_with_elapsed(Duration::from_millis(500));
        assert!(changed);
        assert_eq!(s.displayed_char_count, 2);
    }

    #[test]
    fn test_tick_large_gap() {
        let mut s = make_state(&"a".repeat(1000));
        s.chars_per_second = 0.1;
        assert!(s.tick_with_elapsed(Duration::from_secs(1)));
        assert!(s.displayed_char_count > 900);
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
        s.chars_per_second = 20.0;
        assert!(!s.tick_with_elapsed(Duration::ZERO));
        assert_eq!(s.displayed_char_count, 0);
    }

}
