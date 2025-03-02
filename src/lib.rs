use std::{path::Path, sync::OnceLock};

use robius_directories::ProjectDirs;

pub use makepad_widgets;
pub mod app;
pub mod persistent_state;

/// Generating text previews of timeline events/messages.
mod event_preview;
/// Core UI content: the main home screen (rooms list), room screen.
pub mod home;
/// Login screen
pub mod login;
/// User profile info and a user profile sliding pane.
mod profile;
/// Shared UI components.
pub mod shared;
/// A modal/dialog popup for interactive verification of users/devices.
mod verification_modal;

// Matrix stuff
pub mod avatar_cache;
pub mod media_cache;
pub mod sliding_sync;
pub mod verification;

pub mod location;
pub mod temp_storage;
pub mod utils;

pub const APP_QUALIFIER: &str = "org";
pub const APP_ORGANIZATION: &str = "robius";
pub const APP_NAME: &str = "robrix";

pub fn project_dir() -> &'static ProjectDirs {
    static ROBRIX_PROJECT_DIRS: OnceLock<ProjectDirs> = OnceLock::new();

    ROBRIX_PROJECT_DIRS.get_or_init(|| {
        ProjectDirs::from(APP_QUALIFIER, APP_ORGANIZATION, APP_NAME)
            .expect("Failed to obtain Robrix project directory")
    })
}

pub fn app_data_dir() -> &'static Path {
    project_dir().data_dir()
}
