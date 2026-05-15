//! Status modal shared by both OIDC and UIAA branches.
//!
//! Task 1 scaffolds the widget; full wiring comes in later phases.

use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.RegisterStatusModal = #(RegisterStatusModal::register_widget(vm)) {
        width: Fit,
        height: Fit

        // TODO: Phase 2 wires title + status text + cancel button.
    }
}

#[derive(Script, ScriptHook, Widget)]
pub struct RegisterStatusModal {
    #[deref] view: View,
}

impl Widget for RegisterStatusModal {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
    }
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}
