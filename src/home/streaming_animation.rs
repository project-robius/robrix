use std::time::Instant;
use matrix_sdk::ruma::OwnedUserId;

/// How a streaming session was detected.
#[derive(Debug, Clone, PartialEq)]
pub enum StreamDetection {
    /// Detected by heuristic: prefix match + recency + not self.
    Heuristic,
}

/// Animation state for a single streaming message.
pub struct StreamingAnimState {
    pub target_text: String,
    pub target_char_count: usize,
    pub displayed_char_count: usize,
    pub displayed_byte_offset: usize,
    pub chars_per_frame: f64,
    pub fractional_chars: f64,
    pub last_update_time: Instant,
    pub animation_start_time: Instant,
    pub chars_at_last_update: usize,
    pub display_buffer: String,
    pub sender_stopped_typing: bool,
    pub sender_user_id: OwnedUserId,
    pub detection: StreamDetection,
}

impl StreamingAnimState {
    pub fn new(initial_text: &str, sender_user_id: OwnedUserId, detection: StreamDetection) -> Self {
        let char_count = initial_text.chars().count();
        let now = Instant::now();
        Self {
            target_text: initial_text.to_string(),
            target_char_count: char_count,
            displayed_char_count: 0,
            displayed_byte_offset: 0,
            chars_per_frame: 1.0,
            fractional_chars: 0.0,
            last_update_time: now,
            animation_start_time: now,
            chars_at_last_update: 0,
            display_buffer: String::with_capacity(initial_text.len() + 4),
            sender_stopped_typing: false,
            sender_user_id,
            detection,
        }
    }

    pub fn update_target(&mut self, new_text: &str) {
        self.target_text.clear();
        self.target_text.push_str(new_text);
        self.target_char_count = new_text.chars().count();

        // Clamp display pointers if the new text is shorter than what was already displayed.
        if self.displayed_char_count > self.target_char_count {
            self.displayed_char_count = self.target_char_count;
            // Re-derive byte offset to stay on char boundary.
            self.displayed_byte_offset = self.target_text
                .char_indices()
                .nth(self.target_char_count)
                .map_or(self.target_text.len(), |(i, _)| i);
        }

        self.chars_at_last_update = self.displayed_char_count;
        self.last_update_time = Instant::now();
        let remaining = self.target_char_count.saturating_sub(self.displayed_char_count);
        if remaining > 0 {
            self.chars_per_frame = remaining as f64 / 60.0;
            if self.chars_per_frame < 0.5 { self.chars_per_frame = 0.5; }
        }
        // Fix: reserve uses wrong base — reserve(n) guarantees capacity >= len + n,
        // not capacity >= n.  Compare against capacity and reserve only the deficit.
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
        if self.displayed_char_count >= self.target_char_count { return false; }
        let gap = self.target_char_count - self.displayed_char_count;
        let mut changed = false;

        let speed = if gap > 500 {
            let jump = gap - 50;
            self.advance_displayed(jump);
            changed = true;
            self.chars_per_frame
        } else if gap > 200 {
            self.chars_per_frame * 3.0
        } else {
            self.chars_per_frame
        };

        self.fractional_chars += speed;
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

    /// Check if streaming is complete.
    /// Completes when the sender stops typing and all text has been revealed.
    pub fn is_complete(&self) -> bool {
        if self.displayed_char_count < self.target_char_count { return false; }
        self.sender_stopped_typing
    }

    pub fn is_timed_out(&self) -> bool {
        self.last_update_time.elapsed().as_secs() > 30
    }

}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_state(text: &str) -> StreamingAnimState {
        let user_id: OwnedUserId = "@bot:example.com".try_into().unwrap();
        StreamingAnimState::new(text, user_id, StreamDetection::Heuristic)
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
        assert_eq!(s.displayed_char_count, 5);
        s.update_target("Hello, world!");
        assert_eq!(s.target_char_count, 13);
        assert_eq!(s.displayed_char_count, 5);
        assert!(s.chars_per_frame > 0.0);
    }

    #[test]
    fn test_update_target_shrinks_safely() {
        let mut s = make_state("Hello, world!");
        s.advance_displayed(10);
        s.update_target("Hi");
        assert_eq!(s.displayed_char_count, 2);
        assert_eq!(s.displayed_byte_offset, 2);
        // Should not panic
        s.fill_display_buffer();
        assert!(s.display_buffer.starts_with("Hi"));
    }

    #[test]
    fn test_tick_advances() {
        let mut s = make_state("Hello, world!");
        s.chars_per_frame = 2.0;
        let changed = s.tick();
        assert!(changed);
        assert_eq!(s.displayed_char_count, 2);
    }

    #[test]
    fn test_tick_no_change_when_complete() {
        let mut s = make_state("Hi");
        s.advance_displayed(2);
        let changed = s.tick();
        assert!(!changed);
    }

    #[test]
    fn test_tick_large_gap_returns_true() {
        let mut s = make_state(&"a".repeat(1000));
        s.chars_per_frame = 0.1; // very slow, fractional won't trigger
        let changed = s.tick();
        assert!(changed); // should still return true due to the jump
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
    fn test_is_complete_heuristic() {
        let mut s = make_state("Hi");
        s.advance_displayed(2);
        assert!(!s.is_complete());
        s.sender_stopped_typing = true;
        assert!(s.is_complete());
    }


    #[test]
    fn test_advance_zero_is_noop() {
        let mut s = make_state("Hello");
        s.advance_displayed(0);
        assert_eq!(s.displayed_char_count, 0);
        assert_eq!(s.displayed_byte_offset, 0);
    }
}
