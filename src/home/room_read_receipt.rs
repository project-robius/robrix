use crate::shared::avatar::Avatar;
use makepad_widgets::*;
use std::cmp;
live_design! {
    import makepad_draw::shader::std::*;
    import crate::shared::avatar::*;

    AvatarRow = {{AvatarRow}} {
        button: <Avatar> {
            width: 15.0,
            height: 15.0,
            text_view = { text = { draw_text: {
                text_style: { font_size: 6.0 }
            }}}
        }
        margin: {top: 3, right: 10, bottom: 3, left: 10}
        width: Fit,
        height: Fit
    }
}

#[derive(Live, Widget)]
pub struct AvatarRow {
    #[redraw]
    #[rust]
    area: Area,
    #[redraw]
    #[live]
    draw_text: DrawText,
    #[deref]
    pub deref: View,
    #[walk]
    walk: Walk,
    #[live]
    button: Option<LivePtr>,
    #[live(false)]
    hover_actions_enabled: bool,
    #[rust]
    buttons: Vec<Avatar>,
    #[rust]
    count: usize,
}

#[derive(Clone, Debug, DefaultNone)]
pub enum AvatarRowAction {
    HoverIn(Rect),
    HoverOut,
    None,
}
impl LiveHook for AvatarRow {
    fn after_apply(&mut self, cx: &mut Cx, apply: &mut Apply, index: usize, nodes: &[LiveNode]) {
        for button in self.buttons.iter_mut() {
            if let Some(index) = nodes.child_by_name(index, live_id!(button).as_field()) {
                button.apply(cx, apply, index, nodes);
            }
        }
        self.area.redraw(cx);
    }
}
impl Widget for AvatarRow {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        let uid = self.widget_uid();
        for button in self.buttons.iter_mut() {
            match button.hit(cx, event, self.area) {
                Hit::FingerHoverIn(finger_event) => {
                    let rect = Rect {
                        pos: finger_event.abs,
                        size: DVec2::new(),
                    };
                    cx.widget_action(uid, &scope.path, AvatarRowAction::HoverIn(rect));
                }
                Hit::FingerHoverOut(_) => {
                    cx.widget_action(uid, &scope.path, AvatarRowAction::HoverOut);
                }
                _ => {}
            }
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        for v in self.buttons.iter_mut() {
            let _ = v.draw(cx, scope);
        }
        DrawStep::done()
    }

    fn widget_to_data(
        &self,
        cx: &mut Cx,
        actions: &Actions,
        nodes: &mut LiveNodeVec,
        path: &[LiveId],
    ) -> bool {
        false
    }

    fn data_to_widget(&mut self, cx: &mut Cx, nodes: &[LiveNode], path: &[LiveId]) {}
}
impl AvatarRow {
    pub fn set_range(&mut self, cx: &mut Cx, count: usize) {
        if count != self.buttons.len() {
            self.buttons.clear();
            for _ in 0..cmp::min(5, count) {
                self.buttons.push(Avatar::new_from_ptr(cx, self.button));
            }
        }
        self.count = count;
    }

    pub fn iter_mut(&mut self) -> std::slice::IterMut<'_, Avatar> {
        self.buttons.iter_mut()
    }
}
impl AvatarRowRef {
    pub fn set_range(&self, cx: &mut Cx, count: usize) {
        if let Some(ref mut inner) = self.borrow_mut() {
            inner.set_range(cx, count);
        }
    }
    pub fn len(&self) -> usize {
        if let Some(ref mut inner) = self.borrow_mut() {
            inner.count
        } else {
            0
        }
    }
    pub fn hover_in(&self, actions: &Actions) -> Option<Rect> {
        if let Some(item) = actions.find_widget_action(self.widget_uid()) {
            match item.cast() {
                AvatarRowAction::HoverIn(rect) => Some(rect),
                _ => None,
            }
        } else {
            None
        }
    }
    pub fn hover_out(&self, actions: &Actions) -> bool {
        if let Some(item) = actions.find_widget_action(self.widget_uid()) {
            match item.cast() {
                AvatarRowAction::HoverOut => true,
                _ => false,
            }
        } else {
            false
        }
    }
}
