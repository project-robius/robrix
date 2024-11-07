use crate::shared::avatar::{AvatarRef, AvatarWidgetRefExt};
use makepad_widgets::*;
use matrix_sdk::ruma::{events::receipt::Receipt, EventId, OwnedUserId, RoomId};
use std::cmp;
use crate::shared::avatar::{set_avatar_and_get_username};
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
    buttons: Vec<AvatarRef>,
    #[rust]
    total_num_seen: usize,
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
                Some(Hit::FingerHoverIn(finger_event)) => {
                    let rect = Rect {
                        pos: finger_event.abs,
                        size: DVec2::new(),
                    };
                    cx.widget_action(uid, &scope.path, AvatarRowAction::HoverIn(rect));
                }
                Some(Hit::FingerHoverOut(_)) => {
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
    fn set_range(&mut self, cx: &mut Cx, total_num_seen: usize) {
        if total_num_seen != self.buttons.len() {
            self.buttons.clear();
            for _ in 0..cmp::min(5, total_num_seen) {
                self.buttons.push(WidgetRef::new_from_ptr(cx, self.button).as_avatar());
            }
        }
        self.total_num_seen = total_num_seen;
    }

    fn iter(&self) -> std::slice::Iter<'_, AvatarRef> {
        self.buttons.iter()
    }
}
impl AvatarRowRef {
    /// Handles hover in action
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
    /// Handles hover out action
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
    /// Get the total number of people seen 
    pub fn total_num_seen(&self) -> usize {
        if let Some(ref mut inner) = self.borrow() {
            inner.total_num_seen
        } else {
            0
        }
    }
    /// Set a row of Avatars based on receipt iterator
    pub fn set_avatar_row<'a, T>(&mut self, cx: &mut Cx, room_id: &RoomId, event_id: Option<&EventId>, receipts_len: usize, receipts_iter: T) 
        where T:Iterator<Item = (&'a OwnedUserId, &'a Receipt)> {
        if let Some(ref mut inner) = self.borrow_mut() {
            inner.set_range(cx, receipts_len);
            for (avatar_ref, (user_id, _)) in inner.iter().zip(receipts_iter) {
                set_avatar_and_get_username(cx, avatar_ref.clone(), room_id, &user_id, None, event_id);
            }
        }
    }
}
