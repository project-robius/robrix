use makepad_widgets::*;
live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;
    pub ViewList = {{ViewList}} {
        width: 275
        height: Fit
        flow: Down
    }

}

#[derive(Live, LiveHook, Widget)]
pub struct ViewList {
    #[redraw]
    #[live]
    draw_bg: DrawQuad,
    #[layout]
    layout: Layout,
    #[walk]
    walk: Walk,
    #[rust]
    widgetref_list: Vec<WidgetRef>,
}
impl ViewList {
    pub fn set_widgetref_list(&mut self, widgetref_list: Vec<WidgetRef>) {
        self.widgetref_list = widgetref_list;
    }
}
impl ViewListRef {
    pub fn set_widgetref_list(&mut self, widgetref_list: Vec<WidgetRef>) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_widgetref_list(widgetref_list);
        } else {
            log!("ViewList is not initialized.");
        }
    }
}

impl Widget for ViewList {
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        cx.begin_turtle(walk, self.layout);
        for widget_ref in self.widgetref_list.iter_mut() {
            widget_ref.draw_all(cx, scope);
        }
        cx.end_turtle();
        DrawStep::done()
    }
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        for widget_ref in self.widgetref_list.iter_mut() {
            widget_ref.handle_event(cx, event, scope);
        }
    }
}
