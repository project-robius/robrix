use makepad_widgets::ScriptVm;

pub mod login_screen;
pub mod login_status_modal;
pub mod oidc_login;

pub fn script_mod(vm: &mut ScriptVm) {
    login_status_modal::script_mod(vm);
    login_screen::script_mod(vm);
}
