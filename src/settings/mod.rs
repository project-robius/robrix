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

/// How a settings sub-widget should be (re)populated. Both modes re-apply
/// animator-driven controls, `script_apply_eval` outputs, and code-derived text.
/// They only differ on *user-mutable* text inputs (currently `display_name_input`
/// and `thumb_custom_input`).
#[derive(Clone, Copy)]
pub(crate) enum PopulateMode {
    /// Full populate. Writes user-mutable inputs from the cached source
    /// of truth and resets derived button states (e.g. display-name
    /// accept/cancel) to their "not editing" defaults. Used on initial
    /// open or when fresh prefs / a fresh profile arrive.
    Initial,
    /// Refresh after `Apply::ScriptReapply`. Same as `Initial` but leaves
    /// user-mutable inputs alone (preserves in-progress edits) and
    /// re-derives "edited" button states from the current input.
    AfterReapply,
}
