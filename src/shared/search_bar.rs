use makepad_widgets::*;

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::styles::*;
    use crate::shared::icon_button::RobrixIconButton;

    ICON_SEARCH = dep("crate://self/resources/icons/search.svg")

    pub SearchBar = {{SearchBar}}<RoundedView> {
        width: Fill,
        height: Fit,

        show_bg: true,
        draw_bg: {
            color: (COLOR_PRIMARY)
        }

        padding: {top: 3, bottom: 3, left: 10, right: 10}
        spacing: 4,
        align: {x: 0.0, y: 0.5},

        draw_bg: {
            radius: 0.0,
            border_color: #d8d8d8,
            border_width: 0.6,
        }

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

            empty_message: "Search..."

            draw_text: {
                text_style: { font_size: 10 },
            }
        }

        clear_button = <RobrixIconButton> {
            visible: false,
            padding: {left: 10, right: 10}
            draw_icon: {
                svg_file: (ICON_CLOSE),
                color: (COLOR_TEXT_INPUT_IDLE)
            }
            icon_walk: {width: 10, height: Fit}
        }
    }
}

#[derive(Live, LiveHook, Widget)]
pub struct SearchBar {
    #[deref]
    view: View,
}

/// Actions emitted by the search bar based on user interaction with it.
#[derive(Clone, Debug, DefaultNone)]
pub enum SearchBarAction {
    /// The user has entered a search query.
    Search(String),
    /// The user has cleared the search query.
    ResetSearch,
    None
}

impl Widget for SearchBar {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        self.widget_match_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl WidgetMatchEvent for SearchBar {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions, scope: &mut Scope) {
        let input = self.text_input(id!(input));
        let clear_button = self.button(id!(clear_button));

        // Handle user changing the input text
        if let Some(keywords) = input.changed(actions) {
            clear_button.set_visible(cx, !keywords.is_empty());
            let widget_uid = self.widget_uid(); 
            if keywords.is_empty() {
                cx.widget_action(
                    widget_uid,
                    &scope.path,
                    SearchBarAction::ResetSearch
                );
            } else {
                cx.widget_action(
                    widget_uid,
                    &scope.path,
                    SearchBarAction::Search(keywords)
                );
            }
        }

        // Handle user clicked the clear button
        if clear_button.clicked(actions) {
            input.set_text(cx, "");
            clear_button.set_visible(cx, false);
            input.set_key_focus(cx);

            cx.widget_action(
                self.widget_uid(),
                &scope.path,
                SearchBarAction::ResetSearch,
            );
        }
    }
}
