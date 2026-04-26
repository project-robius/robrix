use makepad_widgets::ScriptVm;

pub mod settings_screen;
pub mod account_settings;
pub mod app_settings;
pub mod app_preferences;

pub fn script_mod(vm: &mut ScriptVm) {
    account_settings::script_mod(vm);
    app_settings::script_mod(vm);
    settings_screen::script_mod(vm);
}

/// How a settings sub-widget should be (re)populated.
///
/// Both modes re-apply animator-driven controls (dropdown index, toggle
/// / radio active state), `script_apply_eval`-driven things (avatar
/// image, button colors), and code-derived text (Labels, toggle labels).
/// They only differ on *user-mutable* text inputs — currently
/// `display_name_input` and `thumb_custom_input`.
#[derive(Clone, Copy)]
pub(crate) enum PopulateMode {
    /// Full populate — authoritatively writes user-mutable inputs from
    /// the cached source-of-truth and resets derived button states
    /// (e.g. display-name accept/cancel) to their "not editing" defaults.
    /// Used when the settings screen is first shown or when a fresh
    /// profile / preference set arrives.
    Initial,
    /// Refresh after an `Apply::ScriptReapply` walk — same shape as
    /// `Initial`, but leaves user-mutable inputs alone so any
    /// in-progress edit survives. Buttons whose enabled state depends
    /// on "input matches cached value" get re-derived from the current
    /// input instead of forced to a known default.
    AfterReapply,
}
