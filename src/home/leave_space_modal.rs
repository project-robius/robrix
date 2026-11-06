//! A modal dialog for leaving a space.
//!
//! It shows a tree of every room and subspace that the user has joined within that space,
//! and the user chooses which of those they also want to leave.

use imbl::Vector;
use makepad_widgets::*;
use matrix_sdk::{RoomDisplayName, RoomState, ruma::OwnedRoomId};
use matrix_sdk_ui::spaces::SpaceRoom;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
    home::{
        rooms_list::RoomsListRef,
        space_tree::{SpaceRoomInfo, TreeEntry, TreeFilter, build_filtered_tree, set_tree_row_position},
    },
    shared::{
        avatar::{AvatarWidgetRefExt, TextAvatarBgColor},
        list_rows::handle_row_actions,
        styles::set_view_bg_color,
    },
    space_service_sync::{SpaceRequest, SpaceRoomExt, SpaceRoomListAction},
    utils::RoomNameId,
};

/// The horizontal indent width (in pixels) per tree level.
const TREE_INDENT_WIDTH: f64 = 28.0;


script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*


    mod.widgets.LeaveSpaceModal = set_type_default() do #(LeaveSpaceModal::register_widget(vm)) {
        ..mod.widgets.LargeModal

        color: (COLOR_TRANSPARENT)
        color_hover: (COLOR_LIST_ITEM_BG_HOVER)
        leave_icon_color: (COLOR_FG_DANGER_RED)
        leave_icon_color_disabled: (COLOR_FG_DISABLED)

        title := ModalTitle { margin: Inset{bottom: 4} }

        body := ModalBody {}

        warning := ModalWarning {
            text: "This is a private space, so you won't be able to join it again without being re-invited."
        }

        select_view := View {
            visible: false
            width: Fill, height: Fit
            flow: Down, spacing: 10
            margin: Inset{top: 15}

            ModalBody { text: "Select any rooms that you also want to leave:" }

            View {
                width: Fill, height: Fit
                flow: Flow.Right{wrap: true, row_align: RowAlign.Center}, spacing: 8
                wrap_spacing: 8
                margin: Inset{left: 6}

                select_all_button := mod.widgets.SelectRowsButton { text: "Select All" }
                select_none_button := mod.widgets.SelectRowsButton { text: "Select None" }

                // This sits outside of the list so it's always in view, and the list doesn't move when it goes away.
                loading_view := View {
                    width: Fit, height: Fit
                    flow: Right, spacing: 8
                    align: Align{y: 0.5}
                    margin: Inset{left: 6}

                    LoadingSpinner {
                        width: 18, height: 18
                        draw_bg +: { color: (COLOR_ACTIVE_PRIMARY), stroke_width: 2.5 }
                    }
                    Label {
                        width: Fit, height: Fit
                        padding: 0
                        draw_text +: { color: (COLOR_LIST_ROW_TEXT_MUTED), text_style: theme.font_regular {font_size: 10} }
                        text: "Loading rooms…"
                    }
                }
            }
        }

        rooms_view := View {
            width: Fill
            // We can't take all of the unused height, since the buttons below need some of it too.
            height: Fit { max: FitBound.Rel{base: Base.Unused, factor: 0.7} }
            flow: Down
            margin: Inset{top: 8}
            // The modal has empty space around this list, so the bars go there instead of on top of the rooms.
            scroll_bars: ScrollBars {
                show_scroll_x: true, show_scroll_y: true,
                bars_outside_view: true
                scroll_bar_x: ScrollBar {drag_scrolling: true}
                scroll_bar_y: ScrollBar {drag_scrolling: true}
            }

            rooms_list := FlatList {
                width: Fit, height: Fit
                scroll_bars: ScrollBars { show_scroll_x: false, show_scroll_y: false }

                room_row := RoundedView {
                    // A deeply-nested row can be wider than the modal, which is what the list scrolls sideways for.
                    width: Fit { min: FitBound.Rel{base: Base.Full, factor: 1.0} }
                    height: Fit
                    flow: Overlay
                    show_bg: true, cursor: MouseCursor.Hand
                    grab_key_focus: false
                    draw_bg +: { color: (COLOR_TRANSPARENT), border_radius: 5.0 }

                    row_content := View {
                        width: Fit, height: Fit
                        flow: Right
                        align: Align{y: 0.5}
                        padding: Inset{left: 6, right: 12, top: 6, bottom: 6}

                        indent_spacer := View { width: 0, height: Fit }

                        // The box is drawn at its left edge, so the rest of its width is a gap before the avatar.
                        checkbox := CheckBoxFlat {
                            width: 30, height: 30
                            padding: 0
                            label_walk: Walk{width: 0, height: 0}
                            draw_bg +: { size: 20.0 }
                        }
                        avatar := Avatar { width: 30, height: 30 }

                        info := View {
                            width: Fit, height: Fit, flow: Down, spacing: 1
                            margin: Inset{left: 9}
                            title := Label {
                                width: Fit, height: Fit, padding: 0
                                draw_text +: { color: (COLOR_TEXT), text_style: theme.font_bold {font_size: 11, line_spacing: 1.0} }
                            }
                            subtitle := Label {
                                width: Fit, height: Fit, padding: 0
                                draw_text +: { color: (COLOR_LIST_ROW_TEXT_MUTED), text_style: theme.font_regular {font_size: 9.5, line_spacing: 1.0} }
                            }
                            notice := Label {
                                visible: false
                                width: Fit, height: Fit, padding: 0
                                draw_text +: { color: (COLOR_FG_INFO_BLUE), text_style: theme.font_regular {font_size: 9, line_spacing: 1.0} }
                            }
                        }
                    }

                    // This comes last so that the row's height is known by the time it's laid out.
                    tree_lines := mod.widgets.TreeLines {
                        draw_bg +: {
                            // These run the lines from the middle of a parent's checkbox to the left of its child's.
                            stem_offset: 16.0
                            elbow_end_offset: 4.0
                        }
                    }
                }
            }
        }

        buttons_view := ModalButtonsRow {
            cancel_button := RobrixNeutralIconButton {
                width: Fit{min: FitBound.Abs(120.0)}
                align: Align{x: 0.5, y: 0.5}
                padding: 15,
                draw_icon.svg: (ICON_FORBIDDEN)
                icon_walk: Walk{width: 16, height: 16, margin: Inset{left: -2, right: -1} }
                text: "Cancel"
            }

            back_button := RobrixNeutralIconButton {
                visible: false
                width: Fit{min: FitBound.Abs(120.0)}
                align: Align{x: 0.5, y: 0.5}
                padding: 15,
                draw_icon.svg: (ICON_ARROW_LEFT)
                icon_walk: Walk{width: 16, height: 16, margin: Inset{left: -2, right: -1} }
                text: "Go Back"
            }

            leave_button := RobrixNegativeIconButton {
                width: Fit{min: FitBound.Abs(120.0)}
                align: Align{x: 0.5, y: 0.5}
                padding: 15,
                draw_icon.svg: (ICON_LOGOUT)
                icon_walk: Walk{width: 16, height: 16, margin: Inset{left: -2, right: -1} }
                text: "Leave"
            }

            confirm_button := RobrixNegativeIconButton {
                visible: false
                width: Fit{min: FitBound.Abs(120.0)}
                align: Align{x: 0.5, y: 0.5}
                padding: 15,
                draw_icon.svg: (ICON_LOGOUT)
                icon_walk: Walk{width: 16, height: 16, margin: Inset{left: -2, right: -1} }
                text: "Yes, Leave"
            }

            okay_button := RobrixIconButton {
                visible: false
                width: Fit{min: FitBound.Abs(120.0)}
                align: Align{x: 0.5, y: 0.5}
                padding: 15,
                draw_icon.svg: (ICON_CHECKMARK)
                icon_walk: Walk{width: 16, height: 16, margin: Inset{left: -2, right: -1} }
                text: "Okay"
            }
        }
    }
}

/// Actions emitted by other widgets to show or hide the `LeaveSpaceModal`.
#[derive(Clone, Debug)]
pub enum LeaveSpaceModalAction {
    /// Open the modal to leave the given space.
    Open(RoomNameId),
    /// Close the modal.
    Close,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum LeaveSpaceModalState {
    /// Waiting for the list of rooms that the user has joined within the space.
    #[default]
    WaitingForRooms,
    /// Waiting for the user to choose which rooms to leave.
    WaitingForUserInput,
    /// Waiting for the result of leaving the space.
    WaitingForLeaveResult,
    /// We're showing the final success or failure message.
    Finished,
}

#[derive(Script, ScriptHook, Widget)]
pub struct LeaveSpaceModal {
    #[deref] view: View,

    #[live] color: Vec4f,
    #[live] color_hover: Vec4f,
    #[live] leave_icon_color: Vec4f,
    #[live] leave_icon_color_disabled: Vec4f,

    #[rust] state: LeaveSpaceModalState,
    #[rust] space_name_id: Option<RoomNameId>,
    /// The rooms and subspaces directly within each space, as far as we know.
    #[rust] children_cache: FxHashMap<OwnedRoomId, Vector<SpaceRoom>>,
    /// The names of the other joined top-level spaces that a given room is also in.
    #[rust] other_spaces: FxHashMap<OwnedRoomId, Vec<String>>,
    /// The spaces whose children we've requested and are keeping up to date.
    #[rust] requested_spaces: FxHashSet<OwnedRoomId>,
    /// The rows of the tree. A room appears more than once if it's within multiple subspaces.
    #[rust] tree_entries: Vec<TreeEntry>,
    /// The indices of the rows whose content has been fully set, including their avatars.
    #[rust] populated_rows: FxHashSet<usize>,
    #[rust] selected_rooms: FxHashSet<OwnedRoomId>,
    /// The rooms to leave along with the space, which are set while we ask the user to confirm leaving it.
    #[rust] rooms_to_confirm: Option<Vec<OwnedRoomId>>,
    /// Whether we might not have found all of the rooms in this space yet.
    #[rust] is_loading_rooms: bool,
    #[rust] hovered_row: Option<LiveId>,
}

impl Widget for LeaveSpaceModal {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        self.widget_match_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        while let Some(step) = self.view.draw_walk(cx, scope, walk).step() {
            // Here, we only need to handle drawing the list of rooms.
            let list_ref = step.as_flat_list();
            let Some(mut list) = list_ref.borrow_mut() else { continue };
            for (index, entry) in self.tree_entries.iter().enumerate() {
                let TreeEntry::Item { info, level, is_last, parent_mask } = entry else { continue };
                let key = LiveId(index as u64);
                let Some(item) = list.item(cx, key, id!(room_row)) else { continue };
                let is_joined = is_joined(info);
                if !self.populated_rows.contains(&index) {
                    item.child_by_path(ids!(info.title)).set_text(cx, &info.name);
                    let subtitle = match (info.is_space(), is_joined, info.canonical_alias.as_ref()) {
                        (true, false, _) => "Subspace (not joined)".into(),
                        (true, true, Some(alias)) => format!("Subspace  ·  {alias}"),
                        (true, true, None) => "Subspace".into(),
                        (false, _, Some(alias)) => alias.to_string(),
                        (false, _, None) => String::new(),
                    };
                    let subtitle_label = item.child_by_path(ids!(info.subtitle));
                    subtitle_label.set_visible(cx, !subtitle.is_empty());
                    subtitle_label.set_text(cx, &subtitle);
                    let notice_label = item.child_by_path(ids!(info.notice));
                    let other_spaces = self.other_spaces.get(&info.id);
                    notice_label.set_visible(cx, other_spaces.is_some());
                    if let Some(other_spaces) = other_spaces {
                        notice_label.set_text(cx, &format!("Also in {}", other_spaces.join(", ")));
                    }
                    // A subspace that the user hasn't joined only shows where its rooms are, so it can't be chosen.
                    if let Some(mut checkbox) = item.check_box(cx, ids!(checkbox)).borrow_mut() {
                        checkbox.animator_toggle(cx, !is_joined, Animate::No, ids!(disabled.on), ids!(disabled.off));
                    }
                    if let Some(mut row) = item.borrow_mut::<View>() {
                        row.cursor = is_joined.then_some(MouseCursor::Hand);
                    }
                    // The top-level rooms have no parent in this tree to connect to, so we start at the level below them.
                    set_tree_row_position(
                        &item,
                        ids!(indent_spacer),
                        *level as isize - 1,
                        *is_last,
                        *parent_mask >> 1,
                        TREE_INDENT_WIDTH,
                        *level as f64 * TREE_INDENT_WIDTH,
                    );
                    if item.avatar(cx, ids!(avatar)).show_room(
                        cx,
                        &info.id,
                        info.avatar.uri(),
                        Some(&info.name),
                        TextAvatarBgColor::Default,
                    ) {
                        self.populated_rows.insert(index);
                    }
                }
                let checkbox = item.check_box(cx, ids!(checkbox));
                let is_selected = self.selected_rooms.contains(&info.id);
                if checkbox.active(cx) != is_selected {
                    checkbox.set_active(cx, is_selected, Animate::No);
                }
                let color = if is_joined && self.hovered_row == Some(key) { self.color_hover } else { self.color };
                set_view_bg_color(cx, &item.as_view(), color);
                item.draw_all(cx, scope);
            }
            // Drop the rows of any previously-shown space.
            list.items.retain_visible();
        }
        DrawStep::done()
    }
}

impl WidgetMatchEvent for LeaveSpaceModal {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions, _scope: &mut Scope) {
        let cancel_button = self.view.button(cx, ids!(cancel_button));
        let okay_button = self.view.button(cx, ids!(okay_button));
        if cancel_button.clicked(actions) || okay_button.clicked(actions) {
            cx.action(LeaveSpaceModalAction::Close);
            return;
        }

        let Some(shown_space_id) = self.space_name_id.as_ref().map(|sni| sni.room_id().clone()) else { return };

        let mut did_children_change = false;
        let mut did_loading_change = false;
        for action in actions {
            match action.downcast_ref() {
                Some(SpaceRoomListAction::JoinedRoomsInSpace { space_id, is_private, children, other_spaces })
                    if *space_id == shown_space_id && self.state == LeaveSpaceModalState::WaitingForRooms =>
                {
                    self.state = LeaveSpaceModalState::WaitingForUserInput;
                    self.view.label(cx, ids!(warning)).set_visible(cx, *is_private);
                    self.other_spaces = other_spaces.clone();
                    for (parent_id, children) in children {
                        self.add_children(cx, parent_id, children);
                    }
                    self.set_negative_button_enabled(cx, ids!(leave_button), true);
                    did_children_change = true;
                }
                Some(SpaceRoomListAction::DetailedChildren { space_id, children, .. })
                    if self.requested_spaces.contains(space_id) =>
                {
                    self.add_children(cx, space_id, children);
                    did_children_change = true;
                }
                Some(SpaceRoomListAction::UpdatedChildren { space_id, .. })
                    if self.requested_spaces.contains(space_id) =>
                {
                    Self::request_children(cx, RoomNameId::empty(space_id.clone()));
                    did_loading_change = true;
                }
                Some(
                    SpaceRoomListAction::UpdatedChildren { .. }
                    | SpaceRoomListAction::PaginationState { .. }
                    | SpaceRoomListAction::PaginationError { .. }
                ) => did_loading_change = true,
                Some(SpaceRoomListAction::LeaveSpaceResult { space_name_id, result })
                    if *space_name_id.room_id() == shown_space_id && self.state == LeaveSpaceModalState::WaitingForLeaveResult =>
                {
                    match result {
                        Ok(()) => self.show_final_state(cx, "Left space!", &format!("Successfully left \"{space_name_id}\".")),
                        Err(e) => self.show_final_state(
                            cx,
                            "Error leaving space!",
                            &format!("Failed to leave space \"{space_name_id}\".\n\nError: {e}"),
                        ),
                    }
                }
                _ => {}
            }
        }

        if self.state != LeaveSpaceModalState::WaitingForUserInput { return }

        if did_children_change {
            self.tree_entries.clear();
            build_filtered_tree(
                &self.children_cache,
                &mut self.tree_entries,
                &mut Vec::new(),
                &shown_space_id,
                &TreeFilter { matches: &is_joined, shows_all_within_matches: false },
                0,
                0,
            );
            self.populated_rows.clear();
        }
        if did_children_change || (did_loading_change && self.is_loading_rooms) {
            self.is_loading_rooms = cx.get_global::<RoomsListRef>().is_space_loading(&shown_space_id);
            self.set_selection_visible(cx, self.rooms_to_confirm.is_none());
            self.redraw(cx);
        }

        let select_all_clicked = self.view.button(cx, ids!(select_all_button)).clicked(actions);
        if select_all_clicked || self.view.button(cx, ids!(select_none_button)).clicked(actions) {
            self.selected_rooms.clear();
            if select_all_clicked {
                self.selected_rooms.extend(get_joined_room_ids(&self.tree_entries).cloned());
            }
            self.redraw(cx);
        }

        let items = self.view.flat_list(cx, ids!(rooms_list)).items_with_actions(actions);
        for (key, item) in &items {
            if let Some(is_selected) = item.check_box(cx, ids!(checkbox)).changed(actions) {
                self.set_row_selected(key.0 as usize, is_selected);
                self.redraw(cx);
            }
        }
        let (tapped_row, hover_changed) = handle_row_actions(items, false, actions, &mut self.hovered_row);
        if let Some(index) = tapped_row.map(|key| key.0 as usize)
            && let Some(TreeEntry::Item { info, .. }) = self.tree_entries.get(index)
        {
            self.set_row_selected(index, !self.selected_rooms.contains(&info.id));
        }
        if tapped_row.is_some() || hover_changed {
            self.redraw(cx);
        }

        if self.view.button(cx, ids!(leave_button)).clicked(actions) {
            self.set_confirmation_visible(cx, true);
        }
        if self.view.button(cx, ids!(back_button)).clicked(actions) {
            self.set_confirmation_visible(cx, false);
        }

        let confirm_button = self.view.button(cx, ids!(confirm_button));
        if confirm_button.clicked(actions)
            && let Some(rooms_to_leave) = self.rooms_to_confirm.take()
            && let Some(space_name_id) = self.space_name_id.clone()
        {
            self.view.label(cx, ids!(title)).set_text(cx, "Leaving this space...");
            self.view.label(cx, ids!(body)).set_text(cx, &format!(
                "Leaving \"{space_name_id}\".\n\n\
                Waiting for confirmation from the homeserver...",
            ));
            self.view.label(cx, ids!(warning)).set_visible(cx, false);
            self.view.button(cx, ids!(back_button)).set_visible(cx, false);
            confirm_button.set_text(cx, "Leaving...");
            self.set_negative_button_enabled(cx, ids!(confirm_button), false);
            // The request is already on its way, so "Cancel" would be a lie.
            cancel_button.set_text(cx, "Close");
            cancel_button.set_visible(cx, true);
            cancel_button.reset_hover(cx);
            self.state = LeaveSpaceModalState::WaitingForLeaveResult;
            // These spaces are on their way out, so we mustn't ask about them anymore.
            self.requested_spaces.clear();
            self.redraw(cx);

            if !Self::send_space_request(cx, SpaceRequest::LeaveSpace { space_name_id, rooms_to_leave }) {
                self.show_final_state(cx, "Error leaving space!", "Failed to send leave space request.\n\nPlease restart Robrix.");
            }
        }
    }
}

impl LeaveSpaceModal {
    /// Merges the given children into those we know of, and requests the children of any new subspaces.
    /// We can't just replace them, since the rooms we started with may not be in the space's room list yet.
    fn add_children(&mut self, cx: &mut Cx, space_id: &OwnedRoomId, children: &Vector<SpaceRoom>) {
        for subspace in children.iter().filter(|child| child.is_space()) {
            if self.requested_spaces.insert(subspace.room_id.clone()) {
                Self::request_children(cx, RoomNameId::new(
                    RoomDisplayName::Named(subspace.display_name.clone()),
                    subspace.room_id.clone(),
                ));
            }
        }
        let mut all_children = children.clone();
        if let Some(known_children) = self.children_cache.get(space_id) {
            let child_ids: FxHashSet<_> = children.iter().map(|child| &child.room_id).collect();
            all_children.extend(known_children.iter().filter(|child| !child_ids.contains(&child.room_id)).cloned());
        }
        self.children_cache.insert(space_id.clone(), all_children);
    }

    /// Selects or deselects the given row, along with everything beneath it if it's a subspace.
    fn set_row_selected(&mut self, index: usize, is_selected: bool) {
        let Some(TreeEntry::Item { level: row_level, .. }) = self.tree_entries.get(index) else { return };
        // The tree is flattened depth-first, so whatever's within a subspace is the deeper rows right after it.
        let num_rows_within = self.tree_entries[index + 1..].iter()
            .take_while(|entry| matches!(entry, TreeEntry::Item { level, .. } if level > row_level))
            .count();
        for room_id in get_joined_room_ids(&self.tree_entries[index..=index + num_rows_within]) {
            if is_selected {
                self.selected_rooms.insert(room_id.clone());
            } else {
                self.selected_rooms.remove(room_id);
            }
        }
    }

    /// Shows or hides the list of rooms, along with its buttons and loading spinner.
    fn set_selection_visible(&mut self, cx: &mut Cx, is_visible: bool) {
        let has_rooms = !self.tree_entries.is_empty();
        self.view.view(cx, ids!(loading_view)).set_visible(cx, self.is_loading_rooms);
        self.view.view(cx, ids!(select_view)).set_visible(cx, is_visible && (has_rooms || self.is_loading_rooms));
        self.view.view(cx, ids!(rooms_view)).set_visible(cx, is_visible && has_rooms);
    }

    /// Swaps between choosing which rooms to leave and confirming exactly what will be left.
    /// Going back keeps the same rooms selected.
    fn set_confirmation_visible(&mut self, cx: &mut Cx, is_visible: bool) {
        let Some(space_name_id) = self.space_name_id.clone() else { return };
        if is_visible {
            let (mut num_rooms, mut num_subspaces) = (0, 0);
            let mut counted = FxHashSet::default();
            for entry in &self.tree_entries {
                if let TreeEntry::Item { info, .. } = entry
                    && self.selected_rooms.contains(&info.id)
                    && counted.insert(&info.id)
                {
                    if info.is_space() { num_subspaces += 1 } else { num_rooms += 1 }
                }
            }
            let count = |num: usize, noun: &str| format!("{num} {noun}{}", if num == 1 { "" } else { "s" });
            let body = match (num_rooms, num_subspaces) {
                (0, 0) => format!("You're about to leave \"{space_name_id}\", but you'll stay in all of your rooms within it."),
                (rooms, 0) => format!("You're about to leave \"{space_name_id}\", along with {} within it.", count(rooms, "room")),
                (0, subspaces) => format!("You're about to leave \"{space_name_id}\", along with {} within it.", count(subspaces, "subspace")),
                (rooms, subspaces) => format!(
                    "You're about to leave \"{space_name_id}\", along with {} and {} within it.",
                    count(rooms, "room"),
                    count(subspaces, "subspace"),
                ),
            };
            self.rooms_to_confirm = Some(self.selected_rooms.iter().cloned().collect());
            self.view.label(cx, ids!(title)).set_text(cx, "Are you sure?");
            self.view.label(cx, ids!(body)).set_text(cx, &body);
        } else {
            self.rooms_to_confirm = None;
            self.show_selection_text(cx, &space_name_id);
        }
        self.set_selection_visible(cx, !is_visible);
        for (button, is_shown) in [
            (ids!(cancel_button), !is_visible),
            (ids!(leave_button), !is_visible),
            (ids!(back_button), is_visible),
            (ids!(confirm_button), is_visible),
        ] {
            let button = self.view.button(cx, button);
            button.set_visible(cx, is_shown);
            button.reset_hover(cx);
        }
        self.redraw(cx);
    }

    /// Shows the question that we ask while the user chooses which rooms to leave.
    fn show_selection_text(&mut self, cx: &mut Cx, space_name_id: &RoomNameId) {
        self.view.label(cx, ids!(title)).set_text(cx, "Leave this space?");
        self.view.label(cx, ids!(body)).set_text(cx, &format!("Are you sure you want to leave \"{space_name_id}\"?"));
    }

    /// Sets whether the given negative button can be clicked, and makes it look that way too.
    fn set_negative_button_enabled(&mut self, cx: &mut Cx, button: &[LiveId], is_enabled: bool) {
        let button = self.view.button(cx, button);
        button.set_enabled(cx, is_enabled);
        button.set_disabled(cx, !is_enabled);
        if let Some(mut button) = button.borrow_mut() {
            button.draw_icon.color = if is_enabled { self.leave_icon_color } else { self.leave_icon_color_disabled };
        }
    }

    /// Shows the given message with just an okay button.
    fn show_final_state(&mut self, cx: &mut Cx, title: &str, description: &str) {
        self.state = LeaveSpaceModalState::Finished;
        self.view.label(cx, ids!(title)).set_text(cx, title);
        self.view.label(cx, ids!(body)).set_text(cx, description);
        self.view.view(cx, ids!(rooms_view)).set_visible(cx, false);
        for button in [ids!(cancel_button), ids!(back_button), ids!(leave_button), ids!(confirm_button)] {
            self.view.button(cx, button).set_visible(cx, false);
        }
        let okay_button = self.view.button(cx, ids!(okay_button));
        okay_button.set_visible(cx, true);
        okay_button.reset_hover(cx);
        self.redraw(cx);
    }

    /// Returns whether the given request was sent to the background space service.
    fn send_space_request(cx: &mut Cx, request: SpaceRequest) -> bool {
        cx.get_global::<RoomsListRef>().get_space_request_sender()
            .is_some_and(|sender| sender.send(request).is_ok())
    }

    /// Requests the rooms directly within the given space, returning whether that request was sent.
    fn request_children(cx: &mut Cx, space_name_id: RoomNameId) -> bool {
        let parent_chain = cx.get_global::<RoomsListRef>()
            .get_space_parent_chain(space_name_id.room_id())
            .unwrap_or_default();
        Self::send_space_request(cx, SpaceRequest::GetDetailedChildren { space_name_id, parent_chain })
    }

    pub fn show(&mut self, cx: &mut Cx, space_name_id: RoomNameId) {
        self.show_selection_text(cx, &space_name_id);

        // Reset the UI state
        self.view.label(cx, ids!(warning)).set_visible(cx, false);
        self.view.view(cx, ids!(select_view)).set_visible(cx, true);
        self.view.view(cx, ids!(loading_view)).set_visible(cx, true);
        let rooms_view = self.view.view(cx, ids!(rooms_view));
        rooms_view.set_visible(cx, false);
        rooms_view.set_scroll_pos(cx, Vec2d::default());
        let leave_button = self.view.button(cx, ids!(leave_button));
        let cancel_button = self.view.button(cx, ids!(cancel_button));
        leave_button.set_text(cx, "Leave");
        leave_button.set_visible(cx, true);
        leave_button.reset_hover(cx);
        self.set_negative_button_enabled(cx, ids!(leave_button), false);
        cancel_button.set_text(cx, "Cancel");
        cancel_button.set_visible(cx, true);
        cancel_button.reset_hover(cx);
        self.view.button(cx, ids!(back_button)).set_visible(cx, false);
        let confirm_button = self.view.button(cx, ids!(confirm_button));
        confirm_button.set_text(cx, "Yes, Leave");
        confirm_button.set_visible(cx, false);
        self.set_negative_button_enabled(cx, ids!(confirm_button), true);
        self.view.button(cx, ids!(okay_button)).set_visible(cx, false);
        self.rooms_to_confirm = None;
        self.children_cache.clear();
        self.other_spaces.clear();
        self.requested_spaces.clear();
        self.tree_entries.clear();
        self.populated_rows.clear();
        self.selected_rooms.clear();
        self.is_loading_rooms = true;
        self.hovered_row = None;
        self.state = LeaveSpaceModalState::WaitingForRooms;
        self.view.redraw(cx);

        cx.get_global::<RoomsListRef>().request_missing_space_children(&space_name_id);
        let space_id = space_name_id.room_id().clone();
        self.requested_spaces.insert(space_id.clone());
        self.space_name_id = Some(space_name_id.clone());
        if !Self::request_children(cx, space_name_id)
            || !Self::send_space_request(cx, SpaceRequest::GetJoinedRoomsInSpace { space_id })
        {
            self.show_final_state(cx, "Error leaving space!", "Failed to find your rooms in this space.\n\nPlease restart Robrix.");
        }
    }
}

impl LeaveSpaceModalRef {
    pub fn show(&self, cx: &mut Cx, space_name_id: RoomNameId) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.show(cx, space_name_id);
    }
}

/// Returns whether the user has joined the given room or subspace, which is what lets them leave it.
fn is_joined(info: &SpaceRoomInfo) -> bool {
    info.state == Some(RoomState::Joined)
}

/// Returns the IDs of the rooms and subspaces that the user has joined among the given entries.
fn get_joined_room_ids(tree_entries: &[TreeEntry]) -> impl Iterator<Item = &OwnedRoomId> {
    tree_entries.iter().filter_map(|entry| match entry {
        TreeEntry::Item { info, .. } if is_joined(info) => Some(&info.id),
        _ => None,
    })
}
