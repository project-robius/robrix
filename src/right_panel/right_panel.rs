use makepad_widgets::*;

use crate::right_panel::search_message::handle_search_input;

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::styles::*;
    use crate::shared::message_search_input_bar::*;
    pub RightPanel = {{RightPanel}} {
        width: 200, height: Fill,
        flow: Down,
        right_panel_header = <View> {
            height: 100,
            flow: Down,
            padding: 10,
            spacing: 10,
            <Label> {
                text: "Veri"
                draw_text: {
                    text_style: <TITLE_TEXT>{font_size: 10},
                    color: #000
                }
            }
            <MessageSearchInputBar> {
                width: Fill,
            }
        }
        
        nav1 = <StackNavigation> {
            debug: true
            width: Fill, height: Fill
            root_view = <View>{
                // debug: true
                //padding: {left: 20.0, right: 20.0}
                <Label> { text: "nav1 root"}
                
            }

            search_result_view = <StackNavigationView> {
                full_screen: false
                width: Fill, height: Fill
                //padding: {left: 100.0, right: 100.0}
                padding: {left: 20.0, right: 20.0}
                draw_bg: {color: #x0}
                flow: Down
                body = { <Label> {
                    text: "nav1 view1"}
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
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions, scope: &mut Scope) {
        for action in actions.iter() {
            println!("handle_search_input");
            handle_search_input(&mut self.view, cx, action, scope);
        }
    }
}

impl RightPanelRef {
    pub fn open(&self, cx: &mut Cx, view_id: LiveId) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.view.stack_navigation(id!(nav1)).push(cx, view_id);
        }
    }
}

pub fn right_panel_handler(cx: &mut Cx, widget_ref: &WidgetRef, action: &Action) {
    match action.downcast_ref() {
        Some(RightPanelAction::OpenMessageSearchResult) => {
            widget_ref.stack_navigation(id!(nav1)).push(cx,live_id!(search_result_view));
        },
        _ => {}
    }
}

#[derive(DefaultNone, Clone, Debug)]
pub enum RightPanelAction {
    OpenMessageSearchResult,
    None
}