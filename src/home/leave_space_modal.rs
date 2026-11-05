//! A modal dialog for leaving a space.
//!
//! It lists every room and subspace that the user has joined within that space,
//! and the user chooses which of those they also want to leave.

use makepad_widgets::*;

use crate::{
    home::rooms_list::RoomsListRef,
    shared::{
        avatar::{AvatarWidgetRefExt, TextAvatarBgColor},
        list_rows::handle_row_actions,
        styles::set_view_bg_color,
    },
    space_service_sync::{JoinedRoomInSpace, SpaceRequest, SpaceRoomListAction},
    utils::RoomNameId,
};


script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*


    let SelectRoomsButton = RobrixIconButton {
        padding: Inset{top: 7, bottom: 7, left: 10, right: 10}
        spacing: 0
        icon_walk: Walk{width: 0, height: 0}
        draw_bg +: {
            color: (COLOR_BG_ACCENT_TINT)
            color_hover: (COLOR_BG_ACCENT_TINT_HOVER)
            color_down: (COLOR_BG_ACCENT_TINT_DOWN)
        }
        draw_text +: {
            color: (COLOR_ACTIVE_PRIMARY_DARKER)
            color_hover: (COLOR_ACTIVE_PRIMARY_DARKER)
            color_down: (COLOR_ACTIVE_PRIMARY_DARKER)
            text_style: REGULAR_TEXT {font_size: 9.5}
        }
    }

    mod.widgets.LeaveSpaceModal = set_type_default() do #(LeaveSpaceModal::register_widget(vm)) {
        ..mod.widgets.SmallModal

        color: (COLOR_TRANSPARENT)
        color_hover: (COLOR_LIST_ITEM_BG_HOVER)
        leave_icon_color: (COLOR_FG_DANGER_RED)
        leave_icon_color_disabled: (COLOR_FG_DISABLED)

        title := ModalTitle { margin: Inset{bottom: 4} }

        body := ModalBody {}

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

                select_all_button := SelectRoomsButton { text: "Select All" }
                select_none_button := SelectRoomsButton { text: "Select None" }

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
            scroll_bars: ScrollBars {
                show_scroll_x: false, show_scroll_y: true,
                scroll_bar_y: ScrollBar {drag_scrolling: true}
            }

            rooms_list := FlatList {
                width: Fill, height: Fit
                scroll_bars: ScrollBars { show_scroll_x: false, show_scroll_y: false }

                room_row := mod.widgets.AvatarListRow {
                    height: Fit
                    padding: Inset{left: 6, right: 12, top: 6, bottom: 6}
                    // Replacing this child keeps its spot in the row, so the checkbox comes first.
                    avatar_box := View {
                        width: Fit, height: Fit
                        flow: Right
                        align: Align{y: 0.5}
                        // The box is drawn at its left edge, so the rest of its width is a gap before the avatar.
                        checkbox := CheckBoxFlat {
                            width: 30, height: 30
                            padding: 0
                            label_walk: Walk{width: 0, height: 0}
                            draw_bg +: { size: 20.0 }
                        }
                        avatar := Avatar { width: 30, height: 30 }
                    }
                    info +: {
                        notice := Label {
                            visible: false
                            width: Fill, height: Fit, max_lines: 1, text_overflow: Ellipsis, padding: 0
                            draw_text +: { color: (COLOR_FG_INFO_BLUE), text_style: theme.font_regular {font_size: 9, line_spacing: 1.0} }
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

            leave_button := RobrixNegativeIconButton {
                width: Fit{min: FitBound.Abs(120.0)}
                align: Align{x: 0.5, y: 0.5}
                padding: 15,
                draw_icon.svg: (ICON_LOGOUT)
                icon_walk: Walk{width: 16, height: 16, margin: Inset{left: -2, right: -1} }
                text: "Leave"
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

/// A row in the list of rooms that can be left along with the space.
struct RoomRow {
    joined_room: JoinedRoomInSpace,
    key: LiveId,
    is_selected: bool,
    /// Whether this row's content has been fully set, including its avatar.
    is_populated: bool,
}

impl RoomRow {
    fn new(joined_room: JoinedRoomInSpace) -> Self {
        Self {
            key: LiveId::from_str(joined_room.room_name_id.room_id().as_str()),
            joined_room,
            is_selected: false,
            is_populated: false,
        }
    }
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
    #[rust] rows: Vec<RoomRow>,
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
            for row in &mut self.rows {
                let Some(item) = list.item(cx, row.key, id!(room_row)) else { continue };
                if !row.is_populated {
                    let room = &row.joined_room;
                    item.child_by_path(ids!(info.title)).set_text(cx, &room.room_name_id.display());
                    let subtitle = match (room.is_space, room.canonical_alias.as_ref()) {
                        (true, Some(alias)) => format!("Subspace  ·  {alias}"),
                        (true, None) => "Subspace".into(),
                        (false, Some(alias)) => alias.to_string(),
                        (false, None) => String::new(),
                    };
                    let subtitle_label = item.child_by_path(ids!(info.subtitle));
                    subtitle_label.set_visible(cx, !subtitle.is_empty());
                    subtitle_label.set_text(cx, &subtitle);
                    let notice_label = item.child_by_path(ids!(info.notice));
                    notice_label.set_visible(cx, !room.other_spaces.is_empty());
                    notice_label.set_text(cx, &format!("Also in {}", room.other_spaces.join(", ")));
                    row.is_populated = item.avatar(cx, ids!(avatar)).show_room(
                        cx,
                        room.room_name_id.room_id(),
                        room.avatar_url.as_ref(),
                        room.room_name_id.name_for_avatar(),
                        TextAvatarBgColor::Default,
                    );
                }
                let checkbox = item.check_box(cx, ids!(checkbox));
                if checkbox.active(cx) != row.is_selected {
                    checkbox.set_active(cx, row.is_selected, Animate::No);
                }
                let color = if self.hovered_row == Some(row.key) { self.color_hover } else { self.color };
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
        for action in actions {
            match action.downcast_ref() {
                Some(SpaceRoomListAction::JoinedRoomsInSpace { space_id, rooms })
                    if *space_id == shown_space_id && self.state == LeaveSpaceModalState::WaitingForRooms =>
                {
                    self.state = LeaveSpaceModalState::WaitingForUserInput;
                    self.rows = rooms.iter().cloned().map(RoomRow::new).collect();
                    self.set_leave_button_enabled(cx, true);
                    did_children_change = true;
                }
                Some(
                    SpaceRoomListAction::UpdatedChildren { .. }
                    | SpaceRoomListAction::PaginationState { .. }
                    | SpaceRoomListAction::PaginationError { .. }
                ) => did_children_change = true,
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

        if did_children_change && self.is_loading_rooms {
            self.add_rooms_from_rooms_list(cx);
        }

        let select_all_clicked = self.view.button(cx, ids!(select_all_button)).clicked(actions);
        if select_all_clicked || self.view.button(cx, ids!(select_none_button)).clicked(actions) {
            for row in &mut self.rows {
                row.is_selected = select_all_clicked;
            }
            self.redraw(cx);
        }

        let items = self.view.flat_list(cx, ids!(rooms_list)).items_with_actions(actions);
        for (key, item) in &items {
            if let Some(is_selected) = item.check_box(cx, ids!(checkbox)).changed(actions)
                && let Some(row) = self.rows.iter_mut().find(|row| row.key == *key)
            {
                row.is_selected = is_selected;
            }
        }
        let (tapped_row, hover_changed) = handle_row_actions(items, false, actions, &mut self.hovered_row);
        if let Some(row) = tapped_row.and_then(|key| self.rows.iter_mut().find(|row| row.key == key)) {
            row.is_selected = !row.is_selected;
        }
        if tapped_row.is_some() || hover_changed {
            self.redraw(cx);
        }

        let leave_button = self.view.button(cx, ids!(leave_button));
        if leave_button.clicked(actions)
            && let Some(space_name_id) = self.space_name_id.clone()
        {
            self.view.label(cx, ids!(title)).set_text(cx, "Leaving this space...");
            self.view.label(cx, ids!(body)).set_text(cx, &format!(
                "Leaving \"{space_name_id}\".\n\n\
                Waiting for confirmation from the homeserver...",
            ));
            self.view.view(cx, ids!(select_view)).set_visible(cx, false);
            self.view.view(cx, ids!(rooms_view)).set_visible(cx, false);
            leave_button.set_text(cx, "Leaving...");
            self.set_leave_button_enabled(cx, false);
            // The request is already on its way, so "Cancel" would be a lie.
            cancel_button.set_text(cx, "Close");
            self.state = LeaveSpaceModalState::WaitingForLeaveResult;
            self.redraw(cx);

            let rooms_to_leave = self.rows.iter()
                .filter(|row| row.is_selected)
                .map(|row| row.joined_room.room_name_id.room_id().clone())
                .collect();
            if !Self::send_space_request(cx, SpaceRequest::LeaveSpace { space_name_id, rooms_to_leave }) {
                self.show_final_state(cx, "Error leaving space!", "Failed to send leave space request.\n\nPlease restart Robrix.");
            }
        }
    }
}

impl LeaveSpaceModal {
    /// Adds the rooms that only the rooms list knows about, e.g., those within a subspace that the user hasn't joined.
    ///
    /// New rows go at the end, so that the rows don't move right as the user is clicking them.
    fn add_rooms_from_rooms_list(&mut self, cx: &mut Cx) {
        let Some(space_name_id) = self.space_name_id.as_ref() else { return };
        let (joined_rooms, is_loading) = cx.get_global::<RoomsListRef>().get_joined_rooms_in_space(space_name_id.room_id());
        for row in joined_rooms.into_iter().map(RoomRow::new) {
            if !self.rows.iter().any(|existing| existing.key == row.key) {
                self.rows.push(row);
            }
        }
        self.is_loading_rooms = is_loading;
        let has_rooms = !self.rows.is_empty();
        self.view.view(cx, ids!(loading_view)).set_visible(cx, is_loading);
        self.view.view(cx, ids!(select_view)).set_visible(cx, has_rooms || is_loading);
        self.view.view(cx, ids!(rooms_view)).set_visible(cx, has_rooms);
        self.redraw(cx);
    }

    /// Sets whether the leave button can be clicked, and makes it look that way too.
    fn set_leave_button_enabled(&mut self, cx: &mut Cx, is_enabled: bool) {
        let leave_button = self.view.button(cx, ids!(leave_button));
        leave_button.set_enabled(cx, is_enabled);
        leave_button.set_disabled(cx, !is_enabled);
        if let Some(mut button) = leave_button.borrow_mut() {
            button.draw_icon.color = if is_enabled { self.leave_icon_color } else { self.leave_icon_color_disabled };
        }
    }

    /// Shows the given message with just an okay button.
    fn show_final_state(&mut self, cx: &mut Cx, title: &str, description: &str) {
        self.state = LeaveSpaceModalState::Finished;
        self.view.label(cx, ids!(title)).set_text(cx, title);
        self.view.label(cx, ids!(body)).set_text(cx, description);
        self.view.view(cx, ids!(rooms_view)).set_visible(cx, false);
        self.view.button(cx, ids!(leave_button)).set_visible(cx, false);
        self.view.button(cx, ids!(cancel_button)).set_visible(cx, false);
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

    pub fn show(&mut self, cx: &mut Cx, space_name_id: RoomNameId) {
        self.view.label(cx, ids!(title)).set_text(cx, "Leave this space?");
        self.view.label(cx, ids!(body)).set_text(cx, &format!(
            "Are you sure you want to leave \"{space_name_id}\"?\n\n\
            If this is a private space, you won't be able to join this space \
            without being re-invited to it.",
        ));

        // Reset the UI state
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
        self.set_leave_button_enabled(cx, false);
        cancel_button.set_text(cx, "Cancel");
        cancel_button.set_visible(cx, true);
        cancel_button.reset_hover(cx);
        self.view.button(cx, ids!(okay_button)).set_visible(cx, false);
        self.rows.clear();
        self.is_loading_rooms = true;
        self.hovered_row = None;
        self.state = LeaveSpaceModalState::WaitingForRooms;
        self.view.redraw(cx);

        cx.get_global::<RoomsListRef>().request_missing_space_children(&space_name_id);
        let space_id = space_name_id.room_id().clone();
        self.space_name_id = Some(space_name_id);
        if !Self::send_space_request(cx, SpaceRequest::GetJoinedRoomsInSpace { space_id }) {
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
