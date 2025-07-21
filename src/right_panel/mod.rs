use makepad_widgets::*;
/// Handles search functionality in the right panel
pub mod search_message;
use crate::shared::message_search_input_bar::MessageSearchAction;

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::styles::*;
    use crate::shared::message_search_input_bar::*;
    use crate::right_panel::search_message::*;
    use crate::shared::icon_button::RobrixIconButton;
    pub RightPanel = {{RightPanel}} {
        width: 400, height: Fill,
        flow: Down,
        visible: false
        nav1 = <StackNavigation> {
            width: Fill, height: Fill
            padding: 0.0
            root_view = <View> {
                padding: 0.0,
            }

            search_result_view = <StackNavigationView> {
                full_screen: false
                width: Fill, height: Fill
                padding: 0,
                draw_bg: {
                    color: (COLOR_SECONDARY)
                }
                flow: Down
                body = {
                    margin: {top: 0.0 },
                    search_screen = <SearchScreen> {}
                }
                header = {
                    padding: {bottom: 10., top: 10.}
                    content = {
                        title_container = {
                            title = {
                                draw_text: {
                                    wrap: Ellipsis,
                                    text_style: { font_size: 10. }
                                    color: #B,
                                }
                                text: "Search Results"
                            }
                        }
                    }
                }
            }
        }
    }
}

#[derive(Live, LiveHook, Widget)]
pub struct RightPanel {
    #[deref]
    view: View,
}

impl Widget for RightPanel {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        self.widget_match_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl WidgetMatchEvent for RightPanel {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions, _scope: &mut Scope) {
        if self.view.button(id!(back_button)).clicked(actions) {
            self.view.set_visible(cx, true);
        }

        for action in actions.iter() {
            match action.as_widget_action().cast() {
                MessageSearchAction::Click(_) => {
                    self.view.set_visible(cx, true);
                    self.view.stack_navigation(id!(nav1)).pop_to_root(cx);
                    self.view
                        .stack_navigation(id!(nav1))
                        .push(cx, live_id!(search_result_view));
                }
                MessageSearchAction::Changed(search_term) => {
                    if !search_term.is_empty() {
                        self.view.set_visible(cx, true);
                        self.view.stack_navigation(id!(nav1)).pop_to_root(cx);
                        self.view
                            .stack_navigation(id!(nav1))
                            .push(cx, live_id!(search_result_view));
                    } else {
                        self.view.set_visible(cx, false);
                    }
                }
                _ => {}
            }

            if let StackNavigationTransitionAction::HideEnd(_) = action.as_widget_action().cast() {
                if !self.view.stack_navigation(id!(nav1)).can_pop() {
                    self.view.set_visible(cx, false);
                }
            }
        }
    }
}
