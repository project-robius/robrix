use makepad_widgets::ScriptVm;

pub mod register_screen;
pub mod register_status_modal;
mod validation;

pub fn script_mod(vm: &mut ScriptVm) {
    register_status_modal::script_mod(vm);
    register_screen::script_mod(vm);
}
