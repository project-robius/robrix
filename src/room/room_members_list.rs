//! A filterable list of a room's joined and invited members,
//! showing each member's avatar, display name, and role.
//!
//! The list doesn't fetch members itself: its host provides them, e.g., a RoomScreen
//! gives its timeline's members to a docked list, while a popped-out list's screen fetches them.

use std::{borrow::Cow, collections::HashSet, sync::Arc};

use makepad_widgets::*;
use matrix_sdk::{room::{RoomMember, RoomMemberRole}, ruma::{OwnedRoomId, events::room::{member::MembershipState, power_levels::UserPowerLevel}}};

use crate::{
    avatar_cache,
    profile::{
        user_profile::{UserProfile, UserProfileAndRoomId, UserProfilePaneInfo, UserProfileSlidingPaneRef, member_display_name, role_name},
        user_profile_cache,
    },
    shared::{avatar::AvatarWidgetRefExt, list_rows::{handle_row_actions, status_row}, room_filter_input_bar::RoomFilterInputBarWidgetRefExt},
    utils::RoomNameId,
};

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.RoomMembersList = #(RoomMembersList::register_widget(vm)) {
        width: Fill, height: Fill
        flow: Down, spacing: 6
        color_hover: (COLOR_LIST_ROW_HOVER)

        member_filter_bar := mod.widgets.RoomFilterInputBar {
            input +: { text_input +: { empty_text: "Filter members..." } }
        }

        member_count_label := Label {
            width: Fill, height: Fit
            padding: Inset{left: 4, right: 4}
            max_lines: 1, text_overflow: Ellipsis
            draw_text +: { color: #737373, text_style: REGULAR_TEXT {font_size: 8.5} }
            text: ""
        }

        members_list := PortalList {
            width: Fill, height: Fill
            flow: Down
            auto_tail: false
            keep_invisible: false

            member_row := mod.widgets.AvatarListRow { padding: Inset{left: 6, right: 6} }
            loading_row := mod.widgets.ListLoadingRow {}
            empty_row := mod.widgets.ListEmptyRow {}
        }
    }
}

/// The result of a [`crate::sliding_sync::MatrixRequest::GetRoomMembersList`] request.
///
/// This is NOT a widget action.
#[derive(Debug)]
pub enum RoomMembersFetchAction {
    Fetched {
        room_id: OwnedRoomId,
        members: Arc<Vec<RoomMember>>,
    },
    Failed {
        room_id: OwnedRoomId,
        error: String,
    },
}

/// Emitted when a room's membership may have changed, e.g., someone joined or was promoted,
/// or some members went missing from the local store (e.g., due to a gap in the sync),
/// such that every shown list of that room's members should re-fetch them.
///
/// This is NOT a widget action.
#[derive(Debug)]
pub struct RoomMembersChanged {
    pub room_id: OwnedRoomId,
}

/// Widget actions emitted by a [`RoomMembersList`].
#[derive(Clone, Debug, Default)]
pub enum RoomMembersListAction {
    /// The user clicked on the given member of the given room.
    MemberClicked {
        room_name_id: RoomNameId,
        member: RoomMember,
    },
    #[default]
    None,
}

/// A member of the room, with the info needed to display, sort, and filter it.
struct MemberEntry {
    member: RoomMember,
    name: String,
    role: Cow<'static, str>,
    /// The display name and user ID in lowercase, for filtering.
    search_text: String,
}

impl MemberEntry {
    fn new(member: RoomMember) -> Self {
        let display_name = member_display_name(&member);
        let mut name = if member.name_ambiguous() {
            format!("{display_name} ({})", member.user_id())
        } else {
            display_name.to_owned()
        };
        if member.is_account_user() {
            name.push_str(" (you)");
        }
        let search_text = format!("{} {}", display_name, member.user_id()).to_lowercase();
        Self { role: role_text(&member), name, search_text, member }
    }

    fn is_invited(&self) -> bool {
        matches!(self.member.membership(), MembershipState::Invite)
    }
}

/// Returns the role of the given member to show beneath their name.
fn role_text(member: &RoomMember) -> Cow<'static, str> {
    if matches!(member.membership(), MembershipState::Invite) {
        return "Invited".into();
    }
    let role = member.suggested_role_for_power_level();
    let default_level = match role {
        RoomMemberRole::Administrator => 100,
        RoomMemberRole::Moderator => 50,
        RoomMemberRole::Creator | RoomMemberRole::User => return role_name(role).into(),
    };
    let role = role_name(role);
    // Show custom power levels, e.g., "Moderator (75)".
    match member.power_level() {
        UserPowerLevel::Int(level) if i64::from(level) != default_level => format!("{role} ({level})").into(),
        _ => role.into(),
    }
}

/// The state of a [`RoomMembersList`] that is saved and restored along with its room's timeline.
#[derive(Clone, Default)]
pub struct SavedRoomMembersList {
    filter_text: String,
    first_id_and_scroll: (usize, f64),
}

#[derive(Script, ScriptHook, Widget)]
pub struct RoomMembersList {
    #[deref] view: View,
    #[live] color_hover: Vec4f,

    #[rust] room_name_id: Option<RoomNameId>,
    /// The members given by our host, unsorted, or `None` until they're fetched.
    #[rust] members: Option<Arc<Vec<RoomMember>>>,
    /// All members, sorted: joined before invited, then by power level, then by name.
    #[rust] entries: Vec<MemberEntry>,
    /// Indices into `entries` of the members that match the current filter.
    #[rust] filtered: Vec<usize>,
    /// The scroll position to restore once members have been fetched.
    #[rust] pending_scroll: Option<(usize, f64)>,
    /// The filter text as entered, and in lowercase for matching.
    #[rust] filter_text: String,
    #[rust] filter: String,
    #[rust] error: Option<String>,
    #[rust] hovered_index: Option<usize>,
    /// The indices of the rows currently drawn with the hover color.
    #[rust] hover_colored: HashSet<usize>,
    /// The indices of the rows whose content is up to date, which needn't be set again when drawn.
    #[rust] populated_rows: HashSet<usize>,
    /// Whether all avatars were fully drawn, i.e., none are still being fetched.
    #[rust(true)] is_fully_drawn: bool,
}

impl Widget for RoomMembersList {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        // A list that hasn't been given a room yet has nothing to do.
        if self.room_name_id.is_none() { return }
        self.view.handle_event(cx, event, scope);

        if !self.is_fully_drawn && matches!(event, Event::Signal) {
            avatar_cache::process_avatar_updates(cx);
            self.redraw(cx);
        }

        let Event::Actions(actions) = event else { return };

        let filter_bar = self.view.child_by_path(ids!(member_filter_bar)).as_room_filter_input_bar();
        if let Some(keywords) = filter_bar.changed(actions) {
            self.filter = keywords.to_lowercase();
            self.filter_text = keywords;
            self.apply_filter();
            self.populated_rows.clear();
            self.hovered_index = None;
            self.pending_scroll = None;
            self.members_list().set_first_id_and_scroll(0, 0.0);
            self.redraw(cx);
        }

        let (clicked, hover_changed) = handle_row_actions(&self.members_list(), actions, &mut self.hovered_index);
        if hover_changed {
            self.redraw(cx);
        }
        if let Some(entry) = clicked
            .and_then(|index| self.filtered.get(index))
            .and_then(|&i| self.entries.get(i))
            && let Some(room_name_id) = self.room_name_id.clone()
        {
            cx.widget_action(
                self.widget_uid(),
                RoomMembersListAction::MemberClicked { room_name_id, member: entry.member.clone() },
            );
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        let status = self.status();
        let mut fully_drawn = true;
        while let Some(item) = self.view.draw_walk(cx, scope, walk).step() {
            let list_ref = item.as_portal_list();
            let Some(mut list) = list_ref.borrow_mut() else { continue };
            let count = if status.is_some() { 1 } else { self.filtered.len() };
            list.set_item_range(cx, 0, count);
            while let Some(index) = list.next_visible_item(cx) {
                if index >= count { continue; }
                let row = if let Some((text, is_loading)) = &status {
                    status_row(cx, &mut list, index, *is_loading, text)
                } else {
                    let Some(entry) = self.filtered.get(index).and_then(|&i| self.entries.get(i)) else { continue };
                    let (mut row, existed) = list.item_with_existed(cx, index, id!(member_row));
                    if !existed {
                        self.hover_colored.remove(&index);
                        self.populated_rows.remove(&index);
                    }
                    // Like the timeline, only set a row's content when it's new or has changed.
                    if !self.populated_rows.contains(&index) {
                        row.child_by_path(ids!(info.title)).set_text(cx, &entry.name);
                        row.child_by_path(ids!(info.subtitle)).set_text(cx, &entry.role);
                        let avatar_url = entry.member.avatar_url().map(|u| u.to_owned());
                        if row.avatar(cx, ids!(avatar)).show_user(cx, avatar_url.as_ref(), &entry.name) {
                            self.populated_rows.insert(index);
                        } else {
                            fully_drawn = false;
                        }
                    }
                    // Applying a color redraws the row, so only do so when its hover state changes.
                    let is_hovered = self.hovered_index == Some(index);
                    if is_hovered != self.hover_colored.contains(&index) {
                        let color = if is_hovered { self.color_hover } else { Vec4f::default() };
                        script_apply_eval!(cx, row, { draw_bg.color: #(color) });
                        if is_hovered { self.hover_colored.insert(index); } else { self.hover_colored.remove(&index); }
                    }
                    row
                };
                row.draw_all(cx, scope);
            }
        }
        self.is_fully_drawn = fully_drawn;
        DrawStep::done()
    }
}

impl RoomMembersList {
    fn members_list(&self) -> PortalListRef {
        self.view.child_by_path(ids!(members_list)).as_portal_list()
    }

    /// Shows the given members of the given room, or a loading notice if they're `None`.
    ///
    /// Showing a different room clears the filter and scroll position.
    fn set_members(&mut self, cx: &mut Cx, room_name_id: &RoomNameId, members: Option<Arc<Vec<RoomMember>>>) {
        if self.room_name_id.as_ref().is_none_or(|r| r.room_id() != room_name_id.room_id()) {
            self.reset(cx);
        }
        self.room_name_id = Some(room_name_id.clone());
        let Some(members) = members else { return };
        if self.members.as_ref().is_some_and(|m| Arc::ptr_eq(m, &members)) {
            return;
        }
        self.error = None;
        let mut entries: Vec<MemberEntry> = members.iter().cloned().map(MemberEntry::new).collect();
        self.members = Some(members);
        entries.sort_by(|a, b| a.is_invited().cmp(&b.is_invited())
            .then_with(|| b.member.power_level().cmp(&a.member.power_level()))
            .then_with(|| a.search_text.cmp(&b.search_text))
        );
        self.entries = entries;
        self.apply_filter();
        self.populated_rows.clear();
        self.update_count_label(cx);
        if let Some((first_id, scroll)) = self.pending_scroll.take() {
            self.members_list().set_first_id_and_scroll(first_id, scroll);
        }
        self.redraw(cx);
    }

    /// Clears this list, such that it shows nothing until `set_members()` is called.
    fn reset(&mut self, cx: &mut Cx) {
        self.room_name_id = None;
        self.members = None;
        self.entries.clear();
        self.filtered.clear();
        self.filter_text.clear();
        self.filter.clear();
        self.pending_scroll = None;
        self.error = None;
        self.hovered_index = None;
        self.hover_colored.clear();
        self.populated_rows.clear();
        self.update_count_label(cx);
        self.view.child_by_path(ids!(member_filter_bar)).as_room_filter_input_bar().clear(cx);
        self.members_list().set_first_id_and_scroll(0, 0.0);
        self.redraw(cx);
    }

    fn apply_filter(&mut self) {
        self.filtered = self.entries.iter().enumerate()
            .filter(|(_, entry)| self.filter.is_empty() || entry.search_text.contains(&self.filter))
            .map(|(i, _)| i)
            .collect();
    }

    /// Returns the status text to show instead of member rows, and whether it's a loading status.
    fn status(&self) -> Option<(Cow<'static, str>, bool)> {
        if !self.filtered.is_empty() {
            None
        } else if let Some(error) = self.error.as_ref().filter(|_| self.entries.is_empty()) {
            Some((format!("Failed to load members: {error}").into(), false))
        } else if self.members.is_none() {
            Some(("Loading members...".into(), true))
        } else if self.entries.is_empty() {
            Some(("This room has no members.".into(), false))
        } else {
            Some(("No members match this filter.".into(), false))
        }
    }

    fn update_count_label(&mut self, cx: &mut Cx) {
        let invited = self.entries.iter().filter(|e| e.is_invited()).count();
        let joined = self.entries.len() - invited;
        let text = match (joined, invited) {
            (0, 0) => String::new(),
            (1, 0) => String::from("1 member"),
            (j, 0) => format!("{j} members"),
            (j, i) => format!("{j} {}, {i} invited", if j == 1 { "member" } else { "members" }),
        };
        self.view.child_by_path(ids!(member_count_label)).set_text(cx, &text);
    }
}

impl RoomMembersListRef {
    /// Returns this list's state, e.g., to be restored when its room's timeline is shown again.
    pub fn save_state(&self) -> SavedRoomMembersList {
        let Some(inner) = self.borrow() else { return SavedRoomMembersList::default() };
        let list = inner.members_list();
        SavedRoomMembersList {
            filter_text: inner.filter_text.clone(),
            // Members may not have arrived to apply the last restored position to.
            first_id_and_scroll: inner.pending_scroll.unwrap_or((list.first_id(), list.scroll_position())),
        }
    }

    /// Restores the given saved state, with the scroll position applied once members are shown.
    pub fn restore_state(&self, cx: &mut Cx, room_name_id: &RoomNameId, saved: SavedRoomMembersList) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.reset(cx);
        inner.room_name_id = Some(room_name_id.clone());
        inner.filter = saved.filter_text.to_lowercase();
        inner.view.child_by_path(ids!(member_filter_bar)).as_room_filter_input_bar().set_text(cx, &saved.filter_text);
        inner.filter_text = saved.filter_text;
        inner.pending_scroll = Some(saved.first_id_and_scroll);
    }

    /// See [`RoomMembersList::set_members()`].
    pub fn set_members(&self, cx: &mut Cx, room_name_id: &RoomNameId, members: Option<Arc<Vec<RoomMember>>>) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_members(cx, room_name_id, members);
        }
    }

    /// Shows the given error if no members have been shown yet.
    pub fn set_error(&self, cx: &mut Cx, error: String) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.error = Some(error);
            inner.redraw(cx);
        }
    }

    /// See [`RoomMembersList::reset()`].
    pub fn reset(&self, cx: &mut Cx) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.reset(cx);
        }
    }
}

/// Shows the profile of the given room member in the given user profile sliding pane.
pub fn show_member_profile(
    cx: &mut Cx,
    pane: &UserProfileSlidingPaneRef,
    room_name_id: &RoomNameId,
    member: RoomMember,
) {
    // Cache the member info first, otherwise the pane would drop it upon its next refresh.
    user_profile_cache::insert_room_member(cx, room_name_id.room_id().clone(), member.clone());
    pane.set_info(cx, UserProfilePaneInfo {
        profile_and_room_id: UserProfileAndRoomId {
            user_profile: UserProfile::from(&member),
            room_id: room_name_id.room_id().clone(),
        },
        room_name: room_name_id.to_string(),
        room_member: Some(member),
    });
    pane.show(cx);
}
