//! A text input used to filter the rooms list
//! with a search icon and a button to clear the input.
//!
//! This is a dedicated widget instead of a general "SearchBar"
//! in order for us to be able to place it inside of a `CachedWidget`
//! and have a single instance be shared across the Mobile and Desktop app views.

use makepad_widgets::*;

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::styles::*;
    use crate::shared::icon_button::RobrixIconButton;

    pub RoomFilterInputBar = {{RoomFilterInputBar}}<RoundedView> {
        width: Fill,
        height: 35,

        show_bg: true,
        draw_bg: {
            color: (COLOR_PRIMARY),
            border_radius: 4.0,
            border_color: (COLOR_SECONDARY),
            border_size: 1.0,
        }

        padding: {top: 3, bottom: 3, left: 10, right: 4.5}
        margin: 0
        spacing: 4,
        align: {x: 0.0, y: 0.5},

        <Icon> {
            draw_icon: {
                svg_file: (ICON_SEARCH),
                fn get_color(self) -> vec4 {
                    return (COLOR_TEXT_INPUT_IDLE);
                }
            }
            icon_walk: {width: 14, height: Fit}
        }

        input = <RobrixTextInput> {
            width: Fill,
            height: Fit,
            flow: Right, // do not wrap

            empty_text: "Filter rooms & spaces..."

            draw_text: {
                text_style: { font_size: 10 },
            }
        }

        clear_button = <RobrixIconButton> {
            visible: false,
            padding: {top: 5, bottom: 5, left: 9, right: 9},
            spacing: 0,
            align: {x: 0.5, y: 0.5}
            draw_bg: {
                color: (COLOR_SECONDARY)
            }
            draw_icon: {
                svg_file: (ICON_CLOSE),
                color: (COLOR_TEXT)
            }
            icon_walk: {width: Fit, height: 10, margin: 0}
        }
    }
}

/// A text input (with a search icon and cancel button) used to filter the rooms list.
///
/// See the module-level docs for more detail.
#[derive(Live, LiveHook, Widget)]
pub struct RoomFilterInputBar {
    #[deref]
    view: View,
}

/// Actions emitted by the `RoomFilterInputBar` based on user interaction with it.
#[derive(Clone, Debug, DefaultNone)]
pub enum RoomFilterAction {
    /// The user has changed the text entered into the filter bar.
    Changed(String),
    None,
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

impl WidgetMatchEvent for RoomFilterInputBar {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions, scope: &mut Scope) {
        let input = self.text_input(ids!(input));
        let clear_button = self.button(ids!(clear_button));

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
                &scope.path,
                RoomFilterAction::Changed(keywords),
            );
        }

        if clear_button.clicked(actions) {
            input.set_text(cx, "");
            clear_button.set_visible(cx, false);
            input.set_key_focus(cx);
            cx.widget_action(
                self.widget_uid(),
                &scope.path,
                RoomFilterAction::Changed(String::new()),
            );
        }
    }
}
