use crate::home::room_screen::RoomScreenTooltipActions;
use crate::profile::user_profile_cache::get_user_display_name_for_room;
use crate::settings::app_preferences::AppPreferencesGlobal;
use crate::shared::avatar::{AvatarRef, AvatarWidgetRefExt};
use crate::sliding_sync::TimelineKind;
use crate::utils::{distinct_user_labels, human_readable_list};
use indexmap::IndexMap;
use makepad_widgets::*;
use crate::{LivePtr, widget_ref_from_live_ptr};
use matrix_sdk::ruma::{events::receipt::Receipt, OwnedUserId, OwnedRoomId};
use matrix_sdk_ui::timeline::EventTimelineItem;

use std::cmp;


/// The maximum number of items to display in the read receipts AvatarRow
/// and its accompanying tooltip.
pub const MAX_VISIBLE_AVATARS_IN_READ_RECEIPT: usize = 3;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*


    mod.widgets.AvatarRow = #(AvatarRow::register_widget(vm)) {
        align: Align{y: 0.5},
        avatar_template: Avatar {
            width: 15.0,
            height: 15.0,
            text_view +: {
                text +: {
                    draw_text +: {
                        text_style: theme.font_regular { font_size: 6.0 }
                    }
                }
            }
        }
        margin: Inset{top: 5, right: 0},
        width: Fit,
        height: 15.0,
        plus_template: Label {
            padding: 0,
            flow: Flow.Right { wrap: false },
            draw_text +: {
                color: #x0,
                text_style: TITLE_TEXT { font_size: 10}
            }
            text: ""
        }
    }
}
/// The widget that displays a list of read receipts.
#[derive(Script, ScriptHook, WidgetRef, WidgetSet, WidgetRegister)]
pub struct AvatarRow {
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
    buttons: Vec<(AvatarRef, bool)>,
    #[rust]
    label: Option<LabelRef>,
    /// The area of the widget
    #[rust]
    area: Area,
    /// The read receipts for this row, keyed by user id.
    #[rust]
    read_receipts: Option<indexmap::IndexMap<matrix_sdk::ruma::OwnedUserId, Receipt>>,
    /// The timeline these read receipts belong to.
    #[rust]
    timeline_kind: Option<TimelineKind>,
}

impl WidgetNode for AvatarRow {
    fn widget_uid(&self) -> WidgetUid { self.deref.widget_uid() }
    fn walk(&mut self, _cx: &mut Cx) -> Walk { self.walk }
    fn area(&self) -> Area { self.area }
    fn redraw(&mut self, cx: &mut Cx) {
        self.draw_text.redraw(cx);
        self.area.redraw(cx);
    }
    fn layer_areas(&self) -> Vec<(&'static str, Area)> { vec![("draw_text", self.draw_text.area())] }
    fn visible(&self) -> bool { self.deref.visible() }
    fn set_visible(&mut self, cx: &mut Cx, visible: bool) { self.deref.set_visible(cx, visible) }
    fn set_scroll_pos(&mut self, cx: &mut Cx, v: Vec2d) { self.deref.set_scroll_pos(cx, v) }

    /// Visits the view's children plus the avatars and the "+N" label.
    ///
    /// The avatars and label come from templates, so the view doesn't know about them.
    /// Listing them here lets Makepad's widget tree track them and drop them when needed.
    fn children(&self, visit: &mut dyn FnMut(LiveId, WidgetRef)) {
        self.deref.children(visit);
        for (i, (avatar, _)) in self.buttons.iter().enumerate() {
            visit(live_id_num!(avatar, i as u64), WidgetRef::clone(avatar));
        }
        if let Some(label) = &self.label {
            visit(id!(plus_label), WidgetRef::clone(label));
        }
    }
    fn skip_widget_tree_search(&self) -> bool { true }
    fn cancel_children_impl(&self, visit: &mut dyn FnMut(LiveId, WidgetRef)) -> bool {
        self.visible() && self.deref.visit_cancel(visit)
    }
    fn find_widgets_from_point(&self, cx: &Cx, point: DVec2, found: &mut dyn FnMut(&WidgetRef)) {
        self.deref.find_widgets_from_point(cx, point, found)
    }
    fn selection_text_len(&self) -> usize { self.deref.selection_text_len() }
    fn selection_point_to_char_index(&self, cx: &Cx, abs: DVec2) -> Option<usize> { self.deref.selection_point_to_char_index(cx, abs) }
    fn selection_set(&mut self, anchor: usize, cursor: usize) { self.deref.selection_set(anchor, cursor) }
    fn selection_clear(&mut self) { self.deref.selection_clear() }
    fn selection_select_all(&mut self) { self.deref.selection_select_all() }
    fn selection_get_text_for_range(&self, start: usize, end: usize) -> String { self.deref.selection_get_text_for_range(start, end) }
    fn selection_get_full_text(&self) -> String { self.deref.selection_get_full_text() }
}

impl Widget for AvatarRow {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        // The avatars come from a template, so nothing else passes events to them;
        // we forward actions to them directly, since they need those for async image loads.
        if let Event::Actions(_) = event {
            for (avatar_ref, _) in self.buttons.iter() {
                avatar_ref.handle_event(cx, event, scope);
            }
        }

        let Some(read_receipts) = &self.read_receipts else {
            return;
        };
        if read_receipts.is_empty() {
            return;
        }
        let uid: WidgetUid = self.widget_uid();
        let widget_rect = self.area.rect(cx);

        let should_hover_in = match event.hits(cx, self.area) {
            Hit::FingerLongPress(_)
            | Hit::FingerHoverIn(..) => true,
            Hit::FingerUp(fue) if fue.is_over && fue.is_primary_hit() => true,
            Hit::FingerHoverOut(_) => {
                cx.widget_action(uid,  RoomScreenTooltipActions::HoverOut);
                false
            }
            _ => false,
        };
        if should_hover_in {
            if let Some(read_receipts) = &self.read_receipts {
                cx.widget_action(
                    uid, 
                    RoomScreenTooltipActions::HoverInReadReceipt {
                        widget_rect,
                        read_receipts: read_receipts.clone(),
                    },
                );
            }
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        if !cx.global::<AppPreferencesGlobal>().0.show_read_receipts {
            self.area = Area::Empty;
            return DrawStep::done();
        }
        if self.read_receipts.as_ref().is_none_or(|r| r.is_empty()) {
            // If we drew nothing, clear the widget's area
            self.area = Area::Empty;
            return DrawStep::done();
        }
        // Avatars show a text placeholder while being fetched,
        // keep retrying them until their image arrives.
        self.update_undrawn_avatars(cx);
        cx.begin_turtle(walk, Layout::default());
        for (avatar_ref, _) in self.buttons.iter_mut() {
            let _ = avatar_ref.draw(cx, scope);
        }
        if self.read_receipts.as_ref().is_some_and(|r| r.len() > MAX_VISIBLE_AVATARS_IN_READ_RECEIPT) {
            if let Some(label) = &mut self.label {
                let _ = label.draw(cx, scope);
            }
        }
        cx.end_turtle_with_area(&mut self.area);
        DrawStep::done()
    }
}
impl AvatarRow {
    /// Sets the avatar row with the given receipts map.
    ///
    /// If the length of the receipts map changes, the number of avatar buttons is updated.
    /// Each avatar button is then updated with the correct username and drawn status by calling
    /// `set_avatar_and_get_username` on it.
    /// Finally, the `read_receipts` field is updated to contain a clone of the given receipts map.
    ///
    /// This function is called by the `RoomScreen` widget when it needs to update the read receipts list.
    pub fn set_avatar_row(
        &mut self,
        cx: &mut Cx,
        timeline_kind: &TimelineKind,
        receipts_map: &IndexMap<OwnedUserId, Receipt>,
    ) {
        // Rebuild the list of avatars if anything visible changes.
        let receipts_changed = self.read_receipts.as_ref().is_none_or(|existing| {
            existing.len() != receipts_map.len() ||
                !existing.keys().rev().take(MAX_VISIBLE_AVATARS_IN_READ_RECEIPT).eq(
                    receipts_map.keys().rev().take(MAX_VISIBLE_AVATARS_IN_READ_RECEIPT)
                )
        });
        if receipts_changed {
            self.buttons.clear();
            for _ in 0..cmp::min(MAX_VISIBLE_AVATARS_IN_READ_RECEIPT, receipts_map.len()) {
                self.buttons.push((
                    widget_ref_from_live_ptr(cx, self.avatar_template).as_avatar(),
                    false,
                ));
            }
            let label = widget_ref_from_live_ptr(cx, self.plus_template).as_label();
            if receipts_map.len() > MAX_VISIBLE_AVATARS_IN_READ_RECEIPT {
                label.set_text(cx, &format!(
                    " + {}",
                    receipts_map.len() - MAX_VISIBLE_AVATARS_IN_READ_RECEIPT,
                ));
            }
            self.label = Some(label);
            self.read_receipts = Some(receipts_map.clone());
            // Tell the widget tree to pick up the list of new avatars
            cx.widget_tree_mark_dirty(self.widget_uid());
        }
        self.timeline_kind = Some(timeline_kind.clone());
        self.update_undrawn_avatars(cx);
    }

    /// Populates avatars that haven't been drawn.
    ///
    /// An avatar stays marked un-drawn while its image is being fetched
    /// (showing a text placeholder), so this gets retried on each draw.
    fn update_undrawn_avatars(&mut self, cx: &mut Cx) {
        let Some(read_receipts) = self.read_receipts.as_ref() else { return };
        let Some(timeline_kind) = self.timeline_kind.as_ref() else { return };
        for ((avatar_ref, drawn), (user_id, _)) in
            self.buttons.iter_mut().zip(read_receipts.iter().rev())
        {
            if !*drawn {
                let (_, drawn_status) = avatar_ref.set_avatar_and_get_username(
                    cx,
                    timeline_kind,
                    user_id,
                    None,
                    None,
                    true,
                );
                *drawn = drawn_status;
            }
        }
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
    pub fn set_avatar_row(
        &mut self,
        cx: &mut Cx,
        timeline_kind: &TimelineKind,
        receipts_map: &IndexMap<OwnedUserId, Receipt>,
    ) {
        if let Some(ref mut inner) = self.borrow_mut() {
            inner.set_avatar_row(cx, timeline_kind, receipts_map);
        }
    }
}

/// Populate the read receipts avatar row in a message item
///
/// Given a reference to item widget (typically a MessageEventMarker), a Cx2d, a
/// room ID, and an EventTimelineItem, this will populate the avatar
/// row of the item with the read receipts of the event.
///
pub fn populate_read_receipts(
    item: &WidgetRef,
    cx: &mut Cx,
    timeline_kind: &TimelineKind,
    event_tl_item: &EventTimelineItem,
) {
    item.avatar_row(cx, ids!(avatar_row)).set_avatar_row(
        cx,
        timeline_kind,
        event_tl_item.read_receipts(),
    );
}

/// Populate the tooltip text for a read receipts avatar row.
///
/// Given a Cx2d, an IndexMap of read receipts, and a room ID, this
/// will populate the tooltip text for the read receipts avatar row.
///
/// The tooltip will contain up to the first `MAX_VISIBLE_AVATARS_IN_READ_RECEIPT` displayable names of the users
/// who have seen this event. If there are more than `MAX_VISIBLE_AVATARS_IN_READ_RECEIPT` users, the tooltip
/// will contain the string "and N others".
pub fn populate_tooltip(
    cx: &mut Cx,
    read_receipts: IndexMap<OwnedUserId, Receipt>,
    room_id: &OwnedRoomId,
) -> String {
    format!(
        "Seen by {}:\n{}",
        read_receipts.len(),
        tooltip_list_of_users(cx, read_receipts.keys().rev(), room_id),
    )
}

/// Returns a string list of the given users for display in a tooltip.
pub fn tooltip_list_of_users<'a>(
    cx: &mut Cx,
    user_ids: impl ExactSizeIterator<Item = &'a OwnedUserId>,
    room_id: &OwnedRoomId,
) -> String {
    let count = user_ids.len();
    let ids_and_names: Vec<(&OwnedUserId, Option<String>)> = user_ids
        .take(MAX_VISIBLE_AVATARS_IN_READ_RECEIPT)
        .map(|user_id| (
            user_id,
            get_user_display_name_for_room(cx, user_id.clone(), Some(room_id), true).into_option(),
        ))
        .collect();
    let people: Vec<(&str, Option<&str>)> = ids_and_names.iter()
        .map(|(user_id, name)| (user_id.as_str(), name.as_deref()))
        .collect();
    let mut labels = distinct_user_labels(&people);
    // Everyone else just gets counted.
    labels.resize(count, String::new());
    human_readable_list(&labels, MAX_VISIBLE_AVATARS_IN_READ_RECEIPT)
}
