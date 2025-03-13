use makepad_widgets::*;
use gen_components::*;

use crate::home::spaces_dock::PageSwitchAction;

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;
    use link::gen_components::*; 

    use crate::setting::side_bar::Sidebar;
    use crate::setting::router::RouterPage;


    pub SettingPage = {{SettingPage}} {
        height: Fill,
        width: Fill,
        flow: Right,
        visible: false,
        show_bg: true,
        draw_bg: {
            color: #d8d4cf
        }
        spacing: 2
        <GView> {
            border_radius: 10.0,
            width: 300,
            background_color: #fff,
            <Sidebar> {}
        }
        <GView> {
            border_radius: 10.0,
            background_color: #fff,
            <RouterPage> {}
        }
    }
}

#[derive(Widget, Live, LiveHook)]
pub struct SettingPage {
    #[deref]
    view: View,
}

impl Widget for SettingPage {
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)  
    }

    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        self.widget_match_event(cx, event, scope);
    }
    
}

impl WidgetMatchEvent for SettingPage {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions, _scope: &mut Scope) {
        for action in actions.iter() {
            match action.cast() {
                PageSwitchAction::SwitchToSetting => {
                    self.view.visible = true;
                    cx.redraw_all();
                },
                PageSwitchAction::SwitchToHome => {
                    self.view.visible = false;
                    cx.redraw_all();
                },
                _ => {}
            }
        }
    }
}


#[derive(Clone, Debug, DefaultNone)]
pub enum SwitchPageAction {
    AccountPage,
    NotificationPage,
    KeyboardPage,
    None
}
