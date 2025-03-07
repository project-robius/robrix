use makepad_widgets::*;

use crate::home::spaces_dock::PageSwitchAction;

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    pub SettingPage = {{SettingPage}} {
        height: Fill,
        width: Fill,
        visible: false,
        show_bg: true,
        draw_bg: {
            color: #000
        }
        <Label> {
            text: "sasasa",
            draw_text: {
                color: #000
            }
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


