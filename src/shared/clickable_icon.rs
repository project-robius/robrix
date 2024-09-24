use makepad_widgets::*;

live_design! {
    import makepad_widgets::base::*;
    import makepad_widgets::theme_desktop_dark::*;

    ClickableIcon = {{ClickableIcon}} {}
}

#[derive(Live, LiveHook, Widget)]
pub struct ClickableIcon {

    #[redraw]
    #[live]
    draw_icon: DrawIcon,
    #[live]
    icon_walk: Walk,
    #[walk]
    walk: Walk,

    #[layout]
    layout: Layout,

    #[live(true)]
    visible: bool,
}


#[derive(Clone, DefaultNone, Debug)]
pub enum ClickableIconAction {
    None,
    Clicked,
}

impl Widget for ClickableIcon {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        let uid = self.widget_uid().clone();

        match event.hits(cx, self.draw_icon.area()) {
            Hit::FingerUp(fe) => if fe.was_tap() {
                cx.widget_action(uid, &scope.path, ClickableIconAction::Clicked);
            }
            _ => ()
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, _scope: &mut Scope, _walk: Walk) -> DrawStep {

        if !self.visible {
            return DrawStep::done();
        }

        self.draw_icon.draw_walk(cx, self.icon_walk);
        DrawStep::done()
    }

}

impl ClickableIcon {
    fn set_visible(&mut self, cx: &mut Cx, visible: bool) {
        self.visible = visible;
        cx.redraw_area(self.draw_icon.area());
    }
}

impl ClickableIconRef {
    pub fn clicked(&self, actions: &Actions) -> bool {
        if let ClickableIconAction::Clicked = actions.find_widget_action(self.widget_uid()).cast() {
            return true;
        }
        false
    }

    pub fn set_visible(&self, cx: &mut Cx, visible: bool) {
       if let Some(mut inner) = self.borrow_mut() {
            inner.set_visible(cx, visible);
       }
    }
}