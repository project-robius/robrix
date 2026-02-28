#![recursion_limit = "256"]

use std::{path::Path, sync::OnceLock};

use makepad_widgets::ScriptNew;
use robius_directories::ProjectDirs;

pub use makepad_widgets;

#[macro_export]
macro_rules! live {
    ($($tt:tt)*) => {
        makepad_widgets::script! { $($tt)* }
    };
}

pub type LivePtr = makepad_widgets::ScriptValue;

pub trait ApplyOverCompat {
    fn apply_over(self, cx: &mut makepad_widgets::Cx, script: makepad_widgets::ScriptMod);
}

impl<T> ApplyOverCompat for &mut T
where
    T: makepad_widgets::ScriptApply,
{
    fn apply_over(self, cx: &mut makepad_widgets::Cx, script: makepad_widgets::ScriptMod) {
        cx.with_vm(|vm| self.script_apply_eval(vm, script));
    }
}

impl<T> ApplyOverCompat for &T
where
    T: makepad_widgets::ScriptApply + Clone,
{
    fn apply_over(self, cx: &mut makepad_widgets::Cx, script: makepad_widgets::ScriptMod) {
        let mut target = self.clone();
        cx.with_vm(|vm| target.script_apply_eval(vm, script));
    }
}

pub trait AnimatorCompat {
    fn animator_in_state(
        &self,
        cx: &makepad_widgets::Cx,
        check_state_pair: &[makepad_widgets::LiveId; 2],
    ) -> bool;
}

impl AnimatorCompat for makepad_widgets::Animator {
    fn animator_in_state(
        &self,
        cx: &makepad_widgets::Cx,
        check_state_pair: &[makepad_widgets::LiveId; 2],
    ) -> bool {
        self.in_state(cx, check_state_pair)
    }
}

pub trait AnimatorActionCompat {
    fn is_animating(&self) -> bool;
}

impl AnimatorActionCompat for makepad_widgets::AnimatorAction {
    fn is_animating(&self) -> bool {
        matches!(self, makepad_widgets::AnimatorAction::Animating { .. })
    }
}

pub fn widget_ref_from_live_ptr(
    cx: &mut makepad_widgets::Cx,
    ptr: Option<LivePtr>,
) -> makepad_widgets::WidgetRef {
    ptr.map_or_else(makepad_widgets::WidgetRef::empty, |value| {
        cx.with_vm(|vm| makepad_widgets::WidgetRef::script_from_value(vm, value))
    })
}

pub fn view_from_live_ptr(
    cx: &mut makepad_widgets::Cx,
    ptr: Option<LivePtr>,
) -> makepad_widgets::View {
    cx.with_vm(|vm| match ptr {
        Some(value) => makepad_widgets::View::script_from_value(vm, value),
        None => makepad_widgets::View::script_new(vm),
    })
}

/// The top-level main application module.
pub mod app;
/// Function for loading and saving persistent application/session state.
pub mod persistence;
/// The settings screen and settings-related content/widgets.
pub mod settings;

/// Login screen
pub mod login;
/// Logout confirmation and state management
pub mod logout;
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


/// All content related to TSP (Trust Spanning Protocol) wallets/identities.
#[cfg(feature = "tsp")]
pub mod tsp;
/// Dummy TSP module with placeholder widgets, for builds without TSP.
#[cfg(not(feature = "tsp"))]
pub mod tsp_dummy;


// Matrix stuff
pub mod sliding_sync;
pub mod space_service_sync;
pub mod avatar_cache;
pub mod media_cache;
pub mod verification;

pub mod utils;
pub mod temp_storage;
pub mod location;

#[cfg(test)]
mod script_parse_smoke;

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
