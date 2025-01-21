use crate::app::AppState;
use crate::shared::avatar::{AvatarRef, AvatarWidgetRefExt};
use crate::home::room_screen::RoomScreenTooltipActions;
use crate::utils::{self, human_readable_list};
use indexmap::IndexMap;
use makepad_widgets::*;
use matrix_sdk::ruma::{events::receipt::Receipt, EventId, OwnedUserId, RoomId};
use matrix_sdk_ui::timeline::EventTimelineItem;
use std::cmp;
const TOOLTIP_WIDTH: f64 = 150.0;
live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::avatar::*;
    use crate::shared::styles::*;

    pub AvatarRow = {{AvatarRow}} {
        avatar_template: <Avatar> {
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
        margin: {top: 12, right: 50},
        width: Fit,
        height: 15.0,
        plus_template: <Label> {
            draw_text: {
                color: #x0,
                text_style: <TITLE_TEXT>{ font_size: 11}
            }
            text: ""
        }
    }
}
/// The widget that displays a list of read receipts.
#[derive(Live, Widget, LiveHook)]
pub struct AvatarRow {
    #[redraw]
    #[live]
    draw_text: DrawText,
    #[deref]
    deref: View,
    #[walk]
    walk: Walk,
    /// The template for the avatars
    #[live]
    avatar_template: Option<LivePtr>,
    #[layout]
    layout: Layout,
    /// Label template for truncated number of people seen
    #[live]
    plus_template: Option<LivePtr>,
    /// A vector containing its avatarRef, its drawn status and username
    ///
    /// Storing the drawn status helps prevent unnecessary set avatar in the draw_walk function
    #[rust]
    buttons: Vec<(AvatarRef, bool, String)>,
    #[rust]
    label: Option<LabelRef>,
    /// The area of the widget
    #[redraw]
    #[rust]
    area: Area,
    #[rust]
    read_receipts: Option<indexmap::IndexMap<matrix_sdk::ruma::OwnedUserId, Receipt>>
}

impl Widget for AvatarRow {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        let Some(read_receipts) = &self.read_receipts else { return };
        if read_receipts.len() == 0 { return; }
        let uid: WidgetUid = self.widget_uid();
        let app_state = scope.data.get_mut::<AppState>().unwrap();
        let widget_rect = self.area.rect(cx);
        match event.hits(cx, self.area) {
            Hit::FingerHoverIn(_) => {
                let mut too_close_to_right = false;
                if let Some(window_geom) = &app_state.window_geom {
                    if (widget_rect.pos.x + widget_rect.size.x) + TOOLTIP_WIDTH > window_geom.inner_size.x {
                        too_close_to_right = true;
                    }
                }
                let tooltip_pos =  if too_close_to_right {
                    DVec2 {
                        x: widget_rect.pos.x + (widget_rect.size.x - TOOLTIP_WIDTH),
                        y: widget_rect.pos.y + widget_rect.size.y
                    }
                } else {
                    DVec2 {
                        x: widget_rect.pos.x + widget_rect.size.x,
                        y: widget_rect.pos.y - widget_rect.size.y / 2.0
                    }
                };
                let callout_offset = if too_close_to_right {
                    TOOLTIP_WIDTH - (widget_rect.size.x - 10.0) / 2.0
                } else {
                    (widget_rect.size.y - 10.0) / 2.0
                };
                if let Some(read_receipts) = &self.read_receipts {
                    cx.widget_action(uid, &scope.path, RoomScreenTooltipActions::HoverInReadReceipt{
                        tooltip_pos,
                        callout_offset,
                        read_receipts: read_receipts.clone(),
                        tooltip_width: TOOLTIP_WIDTH,
                        pointing_up: too_close_to_right,
                    });
                }
            }
            Hit::FingerHoverOut(_) => {
                cx.widget_action(uid, &scope.path, RoomScreenTooltipActions::HoverOut);
            }
            _ => {}
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        let Some(read_receipts) = &self.read_receipts else { return DrawStep::done() };
        if read_receipts.len() == 0 { return DrawStep::done() }
        cx.begin_turtle(walk, Layout::default());
        for (avatar_ref, _, _) in self.buttons.iter_mut() {
            let _ = avatar_ref.draw(cx, scope);
        }
        if read_receipts.len() > utils::MAX_VISIBLE_NUMBER_OF_ITEMS {
            if let Some(label) = &mut self.label {
                label.set_text(&format!(" + {:?}", read_receipts.len() - utils::MAX_VISIBLE_NUMBER_OF_ITEMS));
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
    /// avatars in this row to the corresponding user's avatar, and
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
            for _ in 0..cmp::min(utils::MAX_VISIBLE_NUMBER_OF_ITEMS, receipts_map.len()) {
                self.buttons.push((WidgetRef::new_from_ptr(cx, self.avatar_template).as_avatar(), false, String::new()));
            }
        }
        self.label = Some(WidgetRef::new_from_ptr(cx, self.plus_template).as_label());
        for ((avatar_ref, drawn, username_ref), (user_id, _)) in self.buttons.iter_mut().zip(receipts_map.iter().rev()) {
            if !*drawn {
                let (username, drawn_status) = avatar_ref.set_avatar_and_get_username(cx, room_id, user_id, None, event_id); 
                *drawn = drawn_status;
                *username_ref = username;
            }
        }
        self.read_receipts = Some(receipts_map.clone());
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
    /// Returns true if the action is a hover out
    pub fn hover_out(&self, actions: &Actions) -> bool {
        if let Some(item) = actions.find_widget_action(self.widget_uid()) {
            matches!(item.cast(), RoomScreenTooltipActions::HoverOut)
        } else {
            false
        }
    }
    /// See [`AvatarRow::set_avatar_row()`].
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

/// Populate the tooltip text for a read receipts avatar row.
/// 
/// Given a Cx2d, an IndexMap of read receipts, and a room ID, this
/// will populate the tooltip text for the read receipts avatar row.
/// 
/// The tooltip will contain up to the first 5 displayable names of the users
/// who have seen this event. If there are more than 5 users, the tooltip
/// will contain the string "and N others".
pub fn populate_tooltip(cx: &mut Cx, read_receipts: IndexMap<OwnedUserId, Receipt>, room_id: &RoomId) -> String {
    let mut display_names: Vec<String> = read_receipts.iter().rev().take(5).map(|(user_id, _)| {
        if let (Some(profile), _ ) = crate::profile::user_profile_cache::get_user_profile_and_room_member(cx, user_id.clone(), room_id, true) {
            profile.displayable_name().to_owned()
        } else {
            user_id.to_string()
        }
    }).collect();
    for _ in display_names.len()..read_receipts.len() {
        display_names.push(String::from(""));
    }
    format!("Seen by {:?} people\n{}", read_receipts.len(), human_readable_list(&display_names))     
}