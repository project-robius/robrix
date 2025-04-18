use makepad_widgets::Cx;

pub mod audio_message_ui;
pub mod audio_controller;
pub mod audio_playback_window;

pub fn live_design(cx: &mut Cx) {
    audio_message_ui::live_design(cx);
    audio_controller::live_design(cx);
    audio_playback_window::live_design(cx);
}
