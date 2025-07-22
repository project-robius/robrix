use std::{path::Path, sync::OnceLock};

use robius_directories::ProjectDirs;

pub use makepad_widgets;

/// The top-level main application module.
pub mod app;
/// Function for loading and saving persistent application/session state.
pub mod persistent_state;
/// The settings screen and settings-related content/widgets.
pub mod settings;

/// Login screen
pub mod login;
/// Core UI content: the main home screen (rooms list), room screen.
pub mod home;
/// User profile info and a user profile sliding pane.
pub mod profile;
/// A modal/dialog popup for interactive verification of users/devices.
mod verification_modal;
/// A modal/dialog popup for joining/leaving rooms, including confirming invite accept/reject.
mod join_leave_room_modal;
/// Shared UI components.
pub mod shared;
/// Generating text previews of timeline events/messages.
mod event_preview;
pub mod room;
pub mod right_panel;

// Matrix stuff
pub mod sliding_sync;
pub mod avatar_cache;
pub mod media_cache;
pub mod verification;

pub mod utils;
pub mod temp_storage;
pub mod location;


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
