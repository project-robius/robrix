use gen_components::*;
use makepad_widgets::*;
use super::setting_page::SwitchPageAction;

live_design! {
    use link::widgets::*;
    use link::theme::*;
    use link::shaders::*;
    
    use link::gen_components::*;

    pub Sidebar = {{Sidebar}} {
        height: Fill,
        width: Fill,
        flow: Down,
        spacing: 10,
        align: {
            x: 0.5,
            y: 0.5
        }
        // margin: {top: 12, bottom: 12, left: 12}
        padding: {top: 12, bottom: 12, left: 12, right: 12}


        <GLabel> {
            text: "Slider"
            font_size: 30.0
            color:#fff
            padding:{bottom: 10.0}
        }

        to_account = <GButton>{
            theme: Dark,
            width: Fill,
            height: 40.0,
            slot: <View> {
                spacing: 10,
                padding: {left: 10.0},
                <GSvg>{
                    height: 18.0,
                    width: 18.0,
                    color: #fff,
                    cursor: Help,
                    src: dep("crate://self/resources/icons/people.svg"),
                }
                <GLabel>{
                    font_size: 13.0,
                    text: "Account",
                    color: #fff
                }
            }
        }

        to_notification = <GButton>{
            theme: Dark,
            width: Fill,
            height: 40.0,
            slot: <View> {
                spacing: 10,
                padding: {left: 10.0},
                <GSvg>{
                    height: 18.0,
                    width: 18.0,
                    color: #fff,
                    cursor: Help,
                    src: dep("crate://self/resources/icons/not.svg"),
                }
                <GLabel>{
                    font_size: 13.0,
                    text: "Notifications"
                }
            }
        }

        to_keyboard= <GButton>{
            theme: Dark,
            width: Fill,
            height: 40.0,
            slot: <View> {
                spacing: 10,
                padding: {left: 10.0},
                <GSvg>{
                    height: 18.0,
                    width: 18.0,
                    color: #fff,
                    cursor: Help,
                    src: dep("crate://self/resources/icons/keyboard.svg"),
                }
                <GLabel>{
                    font_size: 13.0,
                    text: "Keyboard"
                }
            }
        }
    }
        
}

#[derive(Widget, Live, LiveHook)]
pub struct Sidebar {
    #[deref]
    view: View,
}

impl Widget for Sidebar {
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)  
    }

    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        self.widget_match_event(cx, event, scope);
    }
    
}

impl WidgetMatchEvent for Sidebar {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions, _scope: &mut Scope) {
        if self.gbutton(id!(to_account)).clicked(&actions).is_some() {
            cx.action(SwitchPageAction::AccountPage);
        }

        if self.gbutton(id!(to_notification)).clicked(&actions).is_some() {
            cx.action(SwitchPageAction::NotificationPage);
        }

        if self.gbutton(id!(to_keyboard)).clicked(&actions).is_some() {
            cx.action(SwitchPageAction::KeyboardPage);
        }
    }
    
}


