use makepad_widgets::ScriptVm;

pub mod settings_screen;
pub mod account_settings;

pub fn script_mod(vm: &mut ScriptVm) {
    account_settings::script_mod(vm);
    settings_screen::script_mod(vm);
}
