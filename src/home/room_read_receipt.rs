use crate::shared::avatar::{AvatarRef, AvatarWidgetRefExt};
use makepad_widgets::*;
use matrix_sdk::ruma::{events::receipt::Receipt, EventId, OwnedUserId, RoomId};
use std::cmp;
use crate::shared::avatar::set_avatar_and_get_username;
live_design! {
    import makepad_draw::shader::std::*;
    import makepad_widgets::base::*;
    import makepad_widgets::theme_desktop_dark::*;
    import crate::shared::avatar::*;
    import crate::shared::styles::*;

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
        height: Fit,
        plus: <Label> {
            draw_text: {
                color: #x0,
                text_style: <TITLE_TEXT>{ font_size: 11}
            }
            text: ""
        }
    }
}

#[derive(Live, Widget, LiveHook)]
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
    #[live]
    plus: Option<LivePtr>,
    #[rust]
    buttons: Vec<AvatarRef>,
    #[rust]
    label: Option<LabelRef>,
    #[rust]
    total_num_seen: usize,

}

#[derive(Clone, Debug, DefaultNone)]
pub enum AvatarRowAction {
    HoverIn(Rect),
    HoverOut,
    None,
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

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, _walk: Walk) -> DrawStep {
        for v in self.buttons.iter_mut() {
            let _ = v.draw(cx, scope);
        }
        if self.total_num_seen > 5 {
            if let Some(label) = &mut self.label {
                label.set_text(&format!(" + {:?}", self.total_num_seen - 5));
                let _ = label.draw(cx, scope);
            }
        }
        DrawStep::done()
    }
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
        self.label = Some(WidgetRef::new_from_ptr(cx, self.plus).as_label());
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
