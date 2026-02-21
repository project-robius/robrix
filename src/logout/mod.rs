use makepad_widgets::ScriptVm;

pub mod logout_confirm_modal;
pub mod logout_state_machine;
pub mod logout_errors;

pub fn script_mod(vm: &mut ScriptVm) {
    logout_confirm_modal::script_mod(vm);
}
