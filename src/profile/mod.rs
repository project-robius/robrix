use makepad_widgets::ScriptVm;

pub mod user_profile;
pub mod user_profile_cache;

pub fn script_mod(vm: &mut ScriptVm) {
    user_profile::script_mod(vm);
}
