//! A text input used to filter a list of rooms/spaces
//! with a search icon and a button to clear the input.

use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*


    mod.widgets.RoomFilterInputBar = set_type_default() do #(RoomFilterInputBar::register_widget(vm)) {
        ..mod.widgets.RoundedView

        width: Fill,
        height: 35,

        show_bg: true,
        draw_bg +: {
            color: (COLOR_PRIMARY)
            border_radius: 4.0
            border_color: (COLOR_SECONDARY)
            border_size: 1.0
        }

        padding: Inset{top: 3, bottom: 3, left: 10, right: 4.5}
        margin: 0
        spacing: 4,
        align: Align{x: 0.0, y: 0.5},

        Icon {
            draw_icon +: {
                svg: (ICON_SEARCH),
                color: (COLOR_TEXT_INPUT_IDLE),
            }
            icon_walk: Walk{width: 14, height: 14}
        }

        input := RobrixTextInput {
            width: Fill,
            height: Fit,
            flow: Right, // do not wrap
            padding: 5
            
            empty_text: "Filter rooms & spaces..."
            
            draw_bg.border_size: 0.0
            draw_text +: {
                text_style: theme.font_regular { font_size: 10 },
            }
        }

        clear_button := RobrixNeutralIconButton {
            visible: false,
            margin: 0
            padding: Inset{top: 5, bottom: 5, left: 9, right: 9},
            spacing: 0,
            align: Align{x: 0.5, y: 0.5}
            draw_icon.svg: (ICON_CLOSE)
            icon_walk: Walk{width: Fit, height: 10, margin: 0}
        }
    }
}

/// A text input (with a search icon and cancel button) used to filter a list of rooms/spaces.
///
/// See the module-level docs for more detail.
#[derive(Script, Widget)]
pub struct RoomFilterInputBar {
    #[deref] view: View,
}

impl ScriptHook for RoomFilterInputBar {
    fn on_after_apply(
        &mut self,
        _vm: &mut ScriptVm,
        apply: &Apply,
        _scope: &mut Scope,
        _value: ScriptValue,
    ) {
        // The clear button's visibility mirrors "the input has text" — that's
        // runtime state, not something the DSL knows about. So when the
        // widget tree is re-applied (e.g. after a preference change), the
        // apply walk resets the button to the DSL's `visible: false` and we
        // lose it. Re-derive visibility from the input's current text here
        // so it survives.
        if !apply.is_script_reapply() {
            return;
        }
        let cx = _vm.cx_mut();
        let has_text = !self.text_input(cx, ids!(input)).text().is_empty();
        self.button(cx, ids!(clear_button)).set_visible(cx, has_text);
    }
}

/// Actions emitted by the `RoomFilterInputBar` based on user interaction with it.
#[derive(Clone, Debug, Default)]
pub enum FilterAction {
    /// The user has changed the text entered into the filter bar.
    Changed(String),
    #[default]
    None,
}

impl ActionDefaultRef for FilterAction {
    fn default_ref() -> &'static Self {
        static DEFAULT: FilterAction = FilterAction::None;
        &DEFAULT
    }
}

/// An action emitted by the HomeScreen or RoomsSideBar when the keywords in the
/// main filter input bar (for rooms and spaces) changes.
///
/// This is a separate action type from [`FilterAction`] so that consumers
/// like `RoomsList` and `SpacesBar` only react to filter changes from
/// the home screen's filter bar, ignoring other filter bar instances.
#[derive(Debug)]
pub enum MainFilterAction {
    /// The user changed the home screen's filter text to the given keywords.
    Changed(String),
}

impl Widget for RoomFilterInputBar {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        self.widget_match_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl RoomFilterInputBar {
    /// Returns `Some(keywords)` if the filter text in this filter input bar
    /// was changed in the given `actions`.
    /// The returned keywords are already trimmed of whitespace.
    pub fn changed(&self, actions: &Actions) -> Option<String> {
        if let Some(item) = actions.find_widget_action(self.widget_uid()) {
            if let FilterAction::Changed(keywords) = item.cast() {
                return Some(keywords);
            }
        }
        None
    }
}

impl RoomFilterInputBarRef {
    /// See [`RoomFilterInputBar::changed()`].
    pub fn changed(&self, actions: &Actions) -> Option<String> {
        self.borrow().and_then(|inner| inner.changed(actions))
    }
}

impl WidgetMatchEvent for RoomFilterInputBar {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions, _scope: &mut Scope) {
        let input = self.text_input(cx, ids!(input));
        let clear_button = self.button(cx, ids!(clear_button));

        // Handle user changing the input text
        if let Some(keywords) = input.changed(actions) {
            // Trim whitespace, and only alloc a new string if it was trimmed.
            let keywords_trimmed = keywords.trim();
            let keywords = if keywords_trimmed.len() < keywords.len() {
                keywords_trimmed.to_string()
            } else {
                keywords
            };
            clear_button.set_visible(cx, !keywords.is_empty());
            clear_button.reset_hover(cx);
            cx.widget_action(
                self.widget_uid(), 
                FilterAction::Changed(keywords)
            );
        }

        if clear_button.clicked(actions) {
            input.set_text(cx, "");
            clear_button.set_visible(cx, false);
            input.set_key_focus(cx);
            cx.widget_action(
                self.widget_uid(), 
                FilterAction::Changed(String::new())
            );
        }
    }
}
