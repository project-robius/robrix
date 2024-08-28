pub use makepad_widgets;
pub mod app;
mod contacts;
mod discover;

/// Core UI content: the main home screen (rooms list), room screen.
pub mod home;
/// User profile info and a user profile sliding pane.
mod profile;
/// Shared UI components.
pub mod shared;
/// Generating text previews of timeline events/messages.
mod event_preview;


// Matrix stuff
pub mod sliding_sync;
pub mod avatar_cache;
pub mod media_cache;

pub mod utils;
pub mod temp_storage;
