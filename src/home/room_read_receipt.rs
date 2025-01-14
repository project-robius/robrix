use crate::shared::avatar::{AvatarRef, AvatarWidgetRefExt};
use crate::home::room_screen::RoomScreenTooltipActions;
use crate::sliding_sync::{submit_async_request, MatrixRequest};
use crate::utils::human_readable_list;
use indexmap::IndexMap;
use makepad_widgets::*;
use matrix_sdk::ruma::{events::receipt::Receipt, EventId, OwnedUserId, RoomId};
use matrix_sdk_ui::timeline::EventTimelineItem;
use std::cmp;
const MAX_VISIBLE_AVATARS_IN_READ_RECEIPT_ROW : usize = 5;
const TOOLTIP_LENGTH: f64 = 100.0;

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::avatar::*;
    use crate::shared::styles::*;

    pub AvatarRow = {{AvatarRow}} {
        button: <Avatar> {
            width: 15.0,
            height: 15.0,
            text_view = { 
                text = { 
                    draw_text: {
                        text_style: { font_size: 6.0 }
                    }
                }
            }
        }
        margin: {top: 12, right: 120, bottom: 3, left: 10},
        width: Fit,
        height: 30,
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
    // A vector containing its avatarRef, its drawn status and username
    // Storing the drawn status helps prevent unnecessary user profile request in the draw_walk function
    #[rust]
    buttons: Vec<(AvatarRef, bool, String)>,
    #[rust]
    label: Option<LabelRef>,
    #[rust]
    total_num_seen: usize,
    #[redraw] 
    #[rust] 
    area: Area,
    #[rust]
    human_readable_usernames: String, 
}

impl Widget for AvatarRow {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        let uid = self.widget_uid();
        if self.total_num_seen == 0 { return; }
        match event.hits(cx, self.area) {
            Hit::FingerHoverIn(finger_event) => {
                // Temporary hack to improve the issue that the tooltip is cut off by the right side of the screen
                // As the width of the tooltip not currently calculated, it is difficult to prevent the tooltip from being cut off
                // If the mouse position is too close to right side of the screen, the tooltip will be left-aligned to the reaction button 
                let tooltip_pos = if finger_event.abs.x > cx.default_window_size().x - TOOLTIP_LENGTH {
                    DVec2 {
                        x: self.area.rect(cx).pos.x,
                        y: finger_event.abs.y
                    }
                } else {
                    finger_event.abs
                };
                cx.widget_action(uid, &scope.path, RoomScreenTooltipActions::HoverIn{
                    tooltip_pos,
                    tooltip_text: format!("Seen by {:?}\n{}", self.total_num_seen, self.human_readable_usernames), 
                    tooltip_width: TOOLTIP_LENGTH
                });
            }
            Hit::FingerHoverOut(_) => {
                cx.widget_action(uid, &scope.path, RoomScreenTooltipActions::HoverOut);
            }
            _ => {}
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        cx.begin_turtle(walk, Layout::default());
        for (avatar_ref, _, _) in self.buttons.iter_mut() {
            let _ = avatar_ref.draw(cx, scope);
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
    
    /// Set a row of Avatars based on receipt index map
    ///
    /// Given a sequence of receipts, this will set each of the first MAX_VISIBLE_AVATARS_IN_READ_RECEIPT_ROW
    /// Avatars in this row to the corresponding user's avatar, and
    /// display the given number of total receipts as a label. The
    /// index map is expected to yield tuples of (user_id, receipt),
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
                self.buttons.push((WidgetRef::new_from_ptr(cx, self.button).as_avatar(), false, String::new()));
            }
        }
        self.total_num_seen = receipts_map.len();
        self.label = Some(WidgetRef::new_from_ptr(cx, self.plus).as_label());
        for ((avatar_ref, drawn, username_ref), (user_id, _)) in self.buttons.iter_mut().zip(receipts_map.iter().rev()) {
            // Set avatar_profile_opt to be None so that the function may fetch the user profile from profile cache
            if !*drawn {
                let (username, drawn_status) = avatar_ref.set_avatar_and_get_username(cx, room_id, user_id, None, event_id); 
                *drawn = drawn_status;
                *username_ref = username;
            }
        }
        let username_arr: Vec<&String> = self.buttons.iter().map(|(_, _, username)| username).collect();
        self.human_readable_usernames = human_readable_list(&username_arr);
    }
}
impl AvatarRowRef {
    /// Handles hover in action
    pub fn hover_in(&self, actions: &Actions) -> RoomScreenTooltipActions {
        if let Some(item) = actions.find_widget_action(self.widget_uid()) {
            item.cast()
        } else {
            RoomScreenTooltipActions::None
        }
    }
    /// Handles hover out action
    pub fn hover_out(&self, actions: &Actions) -> bool {
        if let Some(item) = actions.find_widget_action(self.widget_uid()) {
            matches!(item.cast(), RoomScreenTooltipActions::HoverOut)
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
    /// Set a row of Avatars based on receipt's index map
    pub fn set_avatar_row(&mut self, cx: &mut Cx, room_id: &RoomId, event_id: Option<&EventId>, receipts_map: &IndexMap<OwnedUserId, Receipt>) {
        if let Some(ref mut inner) = self.borrow_mut() {
            inner.set_avatar_row(cx, room_id, event_id, receipts_map);
        }
    }
}

/// Populate the read receipts avatar row in a message item
/// 
/// Given a reference to item widget (typically a MessageEventMarker), a Cx2d, a
/// room ID, and an EventTimelineItem, this will populate the avatar
/// row of the item with the read receipts of the event.
///
pub fn populate_read_receipts(item: &WidgetRef, cx: &mut Cx, room_id: &RoomId, event_tl_item: &EventTimelineItem) {
    item.avatar_row(id!(avatar_row)).set_avatar_row(cx, room_id, event_tl_item.event_id(), event_tl_item.read_receipts());
}
