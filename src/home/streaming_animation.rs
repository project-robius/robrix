use std::time::Instant;
use matrix_sdk::ruma::OwnedUserId;

/// How a streaming session was detected.
#[derive(Debug, Clone, PartialEq)]
pub enum StreamDetection {
    /// Confirmed by MSC4357 live flag in event content.
    Msc4357Live,
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
    pub was_at_end: bool,
    pub detection: StreamDetection,
}

impl StreamingAnimState {
    pub fn new(initial_text: &str, sender_user_id: OwnedUserId, detection: StreamDetection, was_at_end: bool) -> Self {
        let char_count = initial_text.chars().count();
        Self {
            target_text: initial_text.to_string(),
            target_char_count: char_count,
            displayed_char_count: 0,
            displayed_byte_offset: 0,
            chars_per_frame: 1.0,
            fractional_chars: 0.0,
            last_update_time: Instant::now(),
            animation_start_time: Instant::now(),
            chars_at_last_update: 0,
            display_buffer: String::with_capacity(initial_text.len() + 4),
            sender_stopped_typing: false,
            sender_user_id,
            was_at_end,
            detection,
        }
    }

    pub fn update_target(&mut self, new_text: &str) {
        self.target_text.clear();
        self.target_text.push_str(new_text);
        self.target_char_count = new_text.chars().count();
        self.chars_at_last_update = self.displayed_char_count;
        self.last_update_time = Instant::now();
        let remaining = self.target_char_count.saturating_sub(self.displayed_char_count);
        if remaining > 0 {
            self.chars_per_frame = remaining as f64 / 60.0;
            if self.chars_per_frame < 0.5 { self.chars_per_frame = 0.5; }
        }
        if self.display_buffer.capacity() < new_text.len() + 4 {
            self.display_buffer.reserve(new_text.len() + 4 - self.display_buffer.capacity());
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
        let speed = if gap > 500 {
            let jump = gap - 50;
            self.advance_displayed(jump);
            self.chars_per_frame
        } else if gap > 200 {
            self.chars_per_frame * 3.0
        } else {
            self.chars_per_frame
        };
        self.fractional_chars += speed;
        let advance = self.fractional_chars.floor() as usize;
        self.fractional_chars -= advance as f64;
        if advance > 0 { self.advance_displayed(advance); true } else { false }
    }

    pub fn fill_display_buffer(&mut self) {
        self.display_buffer.clear();
        self.display_buffer.push_str(&self.target_text[..self.displayed_byte_offset]);
        self.display_buffer.push_str(" \u{25CF}");
    }

    pub fn is_complete(&self) -> bool {
        if self.displayed_char_count < self.target_char_count { return false; }
        match self.detection {
            StreamDetection::Msc4357Live => false,
            StreamDetection::Heuristic => self.sender_stopped_typing,
        }
    }

    pub fn is_timed_out(&self) -> bool {
        self.last_update_time.elapsed().as_secs() > 30
    }

    pub fn catch_up_to_wall_clock(&mut self) {
        let elapsed = self.last_update_time.elapsed();
        let elapsed_frames = elapsed.as_secs_f64() * 60.0;
        let expected = self.chars_at_last_update + (elapsed_frames * self.chars_per_frame) as usize;
        let target = expected.min(self.target_char_count);
        if target > self.displayed_char_count { self.advance_displayed(target - self.displayed_char_count); }
    }
}
