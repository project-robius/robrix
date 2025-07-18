use makepad_widgets::*;

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
                back_button = <RobrixIconButton> {
                    width: 250,
                    height: 40
                    padding: 10
                    margin: {top: 5, bottom: 10}
                    align: {x: 0.5, y: 0.5}
                    draw_bg: {
                        color: (COLOR_ACTIVE_PRIMARY)
                    }
                    draw_text: {
                        color: (COLOR_PRIMARY)
                        text_style: <REGULAR_TEXT> {}
                    }
                    text: "Back"
                }
            }

            search_result_view = <StackNavigationView> {
                full_screen: false
                width: Fill, height: Fill
                padding: 0,
                draw_bg: {color: #x0}
                flow: Down
                body = {
                    margin: {top: 0.0 },
                    search_screen = <SearchScreen> {}
                }
                header = {
                    content = {
                        title_container = {
                            title = {
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
    #[deref] view: View,
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
            if let MessageSearchAction::Click(_) = action.as_widget_action().cast() {
                self.view.set_visible(cx, true);
                self.view.stack_navigation(id!(nav1)).push(cx,live_id!(search_result_view));
            }
            if let StackNavigationTransitionAction::HideEnd(_) = action.as_widget_action().cast() {
                self.view.set_visible(cx, false);
            }
        }
        
    }
}

impl RightPanelRef {
    pub fn open(&self, cx: &mut Cx, view_id: LiveId) {
        if let Some(inner) = self.borrow() {
            inner.view.stack_navigation(id!(nav1)).push(cx, view_id);
        }
    }
}

#[derive(DefaultNone, Clone, Debug)]
pub enum RightPanelAction {
    OpenMessageSearchResult,
    None
}