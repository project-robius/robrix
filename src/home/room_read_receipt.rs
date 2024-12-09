use crate::shared::avatar::{AvatarRef, AvatarWidgetRefExt};
use indexmap::IndexMap;
use makepad_widgets::*;
use matrix_sdk::ruma::{events::receipt::Receipt, EventId, OwnedUserId, RoomId};
use std::cmp;
const MAX_VISIBLE_AVATARS_IN_READ_RECEIPT_ROW : usize = 5;
live_design! {
    import makepad_draw::shader::std::*;
    import makepad_widgets::base::*;
    import makepad_widgets::theme_desktop_dark::*;
    import crate::shared::avatar::*;
    import crate::shared::styles::*;

    AvatarRow = {{AvatarRow}} {
        debug: true,
        button: <Avatar> {
            width: 15.0,
            height: 15.0,
            text_view = { text = { draw_text: {
                text_style: { font_size: 6.0 }
            }}}
        }
        margin: {top: 3, right: 10, bottom: 3, left: 10}
        // width: Fit,
        // height: Fit,
        width: 100,
        height: 50,
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
    #[live]
    draw_text: DrawText,
    #[deref]
    pub deref: View,
    #[walk]
    walk: Walk,
    #[live]
    button: Option<LivePtr>,
    #[layout]
    layout: Layout,
    #[live]
    plus: Option<LivePtr>,
    #[rust]
    buttons: Vec<AvatarRef>,
    #[rust]
    label: Option<LabelRef>,
    #[rust]
    total_num_seen: usize,
    #[redraw] #[rust] area: Area,

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
        if self.total_num_seen == 0 { return; }
        match event.hits(cx, self.area) {
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

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        cx.begin_turtle(walk, Layout::default());
        for v in self.buttons.iter_mut() {
            let _ = v.draw(cx, scope);
        }
        if self.total_num_seen > MAX_VISIBLE_AVATARS_IN_READ_RECEIPT_ROW {
            if let Some(label) = &mut self.label {
                label.set_text(&format!(" + {:?}", self.total_num_seen - MAX_VISIBLE_AVATARS_IN_READ_RECEIPT_ROW));
                let _ = label.draw(cx, scope);
            }
        }
        cx.end_turtle_with_area(&mut self.area);
        DrawStep::done()
    }
}
impl AvatarRow {
    
    /// Set a row of Avatars based on receipt iterator
    ///
    /// Given a sequence of receipts, this will set each of the first MAX_VISIBLE_AVATARS_IN_READ_RECEIPT_ROW
    /// Avatars in this row to the corresponding user's avatar, and
    /// display the given number of total receipts as a label. The
    /// iterator is expected to yield tuples of (user_id, receipt),
    /// where the receipt is ignored.
    fn set_avatar_row(
        &mut self,
        cx: &mut Cx,
        room_id: &RoomId,
        event_id: Option<&EventId>,
        receipts_map: &IndexMap<OwnedUserId, Receipt>) {
        if receipts_map.len() != self.buttons.len() {
            self.buttons.clear();
            for _ in 0..cmp::min(MAX_VISIBLE_AVATARS_IN_READ_RECEIPT_ROW, receipts_map.len()) {
                self.buttons.push(WidgetRef::new_from_ptr(cx, self.button).as_avatar());
            }
        }
        self.total_num_seen = receipts_map.len();
        self.label = Some(WidgetRef::new_from_ptr(cx, self.plus).as_label());
    
        for (avatar_ref, (user_id, _)) in self.buttons.iter().zip(receipts_map) {
            avatar_ref.set_avatar_and_get_username(cx, room_id, user_id, None, event_id);
        }
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
            matches!(item.cast(), AvatarRowAction::HoverOut)
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
    pub fn set_avatar_row(&mut self, cx: &mut Cx, room_id: &RoomId, event_id: Option<&EventId>, receipts_map: &IndexMap<OwnedUserId, Receipt>) {
        if let Some(ref mut inner) = self.borrow_mut() {
            inner.set_avatar_row(cx, room_id, event_id, receipts_map);
        }
    }
}
