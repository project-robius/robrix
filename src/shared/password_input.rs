//! A password text input with an eye button that shows or hides the entered text.

use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.ICON_EYE_OPEN   = crate_resource("self://resources/icons/eye_open.svg")
    mod.widgets.ICON_EYE_CLOSED = crate_resource("self://resources/icons/eye_closed.svg")

    mod.widgets.PasswordEyeButton = RobrixNeutralIconButton {
        width: Fit, height: Fit
        align: Align{x: 0.5, y: 0.5}
        padding: 5
        spacing: 0
        margin: 0
        // Leave the key focus in the text input, so toggling doesn't tear down the IME.
        grab_key_focus: false
        draw_bg +: {
            color: (COLOR_SECONDARY * 1.05)
        }
        draw_icon +: {
            color: #8C8C8C,
        }
        icon_walk: Walk{width: 18, height: 18, margin: 0}
        text: ""
    }

    mod.widgets.PasswordTextInput = #(PasswordTextInput::register_widget(vm)) {
        width: Fill, height: Fit
        flow: Overlay
        align: Align{x: 1.0, y: 0.5}

        text_input := RobrixTextInput {
            width: Fill, height: Fit
            flow: Flow.Right { wrap: false },
            // The right padding leaves enough room for the eye icon button,
            // which gets overlaid on top of it.
            padding: Inset{top: 10, bottom: 10, left: 10, right: 38}
            empty_text: "Password"
            is_password: true,
            is_multiline: false,
            autocapitalize: None,
            autocorrect: Disabled,
            content_type: Password,
        }

        View {
            width: 38, height: Fill
            align: Align{x: 0.5, y: 0.5}

            // One button for both states: Tab focus stays put when its icon swaps.
            eye_button := mod.widgets.PasswordEyeButton {
                draw_icon.svg: (mod.widgets.ICON_EYE_CLOSED)
            }
        }
    }
}

/// A password text input whose eye button toggles between showing and hiding the text.
#[derive(Script, Widget)]
pub struct PasswordTextInput {
    #[source] source: ScriptObjectRef,
    #[deref] view: View,

    /// Whether the input is currently showing regular text (`true`) or hidden text (`false`).
    #[rust] is_text_shown: bool,
}

impl ScriptHook for PasswordTextInput {
    fn on_after_apply(
        &mut self,
        _vm: &mut ScriptVm,
        _apply: &Apply,
        _scope: &mut Scope,
        _value: ScriptValue,
    ) {
        // A re-apply resets the children to the DSL's hidden text, so our state matches again.
        self.is_text_shown = false;
    }
}

impl Widget for PasswordTextInput {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        self.match_event(cx, event);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }

    fn text(&self) -> String {
        self.text_input().text()
    }

    fn set_text(&mut self, cx: &mut Cx, v: &str) {
        self.show_text(cx, false);
        self.text_input().set_text(cx, v);
    }

    fn set_key_focus(&self, cx: &mut Cx) {
        self.text_input().set_key_focus(cx);
    }

    fn key_focus(&self, cx: &Cx) -> bool {
        self.text_input().key_focus(cx)
    }

    fn set_disabled(&mut self, cx: &mut Cx, disabled: bool) {
        self.text_input().set_disabled(cx, disabled);
        self.view.button(cx, ids!(eye_button)).set_enabled(cx, !disabled);
    }

    fn disabled(&self, cx: &Cx) -> bool {
        self.text_input().disabled(cx)
    }
}

impl MatchEvent for PasswordTextInput {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        if self.view.button(cx, ids!(eye_button)).clicked(actions) {
            let is_shown = self.is_text_shown;
            self.show_text(cx, !is_shown);
        }
    }
}

impl PasswordTextInput {
    fn text_input(&self) -> WidgetRef {
        self.view.child_by_path(ids!(text_input))
    }

    fn show_text(&mut self, cx: &mut Cx, is_shown: bool) {
        if self.is_text_shown == is_shown {
            return;
        }
        self.is_text_shown = is_shown;
        self.view.text_input(cx, ids!(text_input)).toggle_is_password(cx);
        let mut eye_button = self.view.button(cx, ids!(eye_button));
        if is_shown {
            script_apply_eval!(cx, eye_button, {
                draw_icon.svg: mod.widgets.ICON_EYE_OPEN,
            });
        } else {
            script_apply_eval!(cx, eye_button, {
                draw_icon.svg: mod.widgets.ICON_EYE_CLOSED,
            });
        }
        self.redraw(cx);
    }
}

impl PasswordTextInputRef {
    /// See [`TextInputRef::changed()`]: the text the user just typed.
    pub fn changed(&self, actions: &Actions) -> Option<String> {
        self.text_input_ref().changed(actions)
    }

    /// See [`TextInputRef::returned()`]: the user pressed Enter.
    pub fn returned(&self, actions: &Actions) -> Option<(String, KeyModifiers)> {
        self.text_input_ref().returned(actions)
    }

    /// See [`TextInputRef::set_is_read_only()`].
    pub fn set_is_read_only(&self, cx: &mut Cx, is_read_only: bool) {
        self.text_input_ref().set_is_read_only(cx, is_read_only);
    }

    fn text_input_ref(&self) -> TextInputRef {
        self.child_by_path(ids!(text_input)).as_text_input()
    }
}
