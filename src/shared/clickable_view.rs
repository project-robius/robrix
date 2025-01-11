use makepad_widgets::*;

live_design! {
    use link::theme::*;
    use link::widgets::*;

    pub ClickableView = {{ClickableView}} {
        width: Fit, height: Fit
    }
}

#[derive(Live, LiveHook, Widget)]
pub struct ClickableView {
    #[deref]
    view: View,
}
#[derive(Clone, DefaultNone, Debug)]
pub enum ClickableViewAction {
    None,
    Click,
}

impl Widget for ClickableView {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        let uid = self.widget_uid();

        match event.hits(cx, self.view.area()){
            Hit::FingerDown(_fe) => {
                cx.set_key_focus(self.view.area());
            }
            Hit::FingerUp(fe) => if fe.was_tap() {
                cx.widget_action(uid, &scope.path, ClickableViewAction::Click);
            }
            _ =>()
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl ClickableViewRef {
    pub fn clicked(&self, actions: &Actions) -> bool {
        if let ClickableViewAction::Click = actions.find_widget_action(self.widget_uid()).cast() {
            return true;
        }
        false
    }
    pub fn set_visible(&self, visible: bool) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.visible = visible
        }
    }
}
