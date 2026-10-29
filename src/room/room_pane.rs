//! Panes that show extra info about a room (e.g., its member list).
//!
//! A pane is docked to one edge of a RoomScreen's timeline,
//! or popped out into its own dock tab (desktop) or stack view (mobile).
//! Docked panes are saved and restored along with their timeline's UI state,
//! as are their loaded contents and scroll position.

use std::{borrow::Cow, cell::RefCell, collections::HashMap};

use makepad_widgets::*;
use serde::{Deserialize, Serialize};

use ruma::{OwnedRoomId, RoomId};

use crate::{app::SelectedRoom, home::rooms_list::RoomsListAction, room::pane_dock::SavedPaneContent, sliding_sync::TimelineKind, utils::RoomNameId};

/// The kinds of panes that can be shown for a room.
///
/// This isn't `Copy`, so that a kind of pane can carry data, e.g., which content it shows.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RoomPaneKind {
    /// The list of the room's members.
    Members,
    /// The list of the room's pinned messages.
    PinnedMessages,
    /// The list of the room's threads.
    Threads,
    /// The room's details and common actions.
    RoomInfo,
}

impl RoomPaneKind {
    /// The title shown in the pane's header.
    pub fn title(&self) -> Cow<'static, str> {
        match self {
            RoomPaneKind::Members => Cow::Borrowed("Members"),
            RoomPaneKind::PinnedMessages => Cow::Borrowed("Pinned messages"),
            RoomPaneKind::Threads => Cow::Borrowed("Threads"),
            RoomPaneKind::RoomInfo => Cow::Borrowed("Room info"),
        }
    }

    /// A unique string for this kind, used to build its popped-out tab's ID.
    pub fn as_str(&self) -> Cow<'static, str> {
        match self {
            RoomPaneKind::Members => Cow::Borrowed("members"),
            RoomPaneKind::PinnedMessages => Cow::Borrowed("pinned_messages"),
            RoomPaneKind::Threads => Cow::Borrowed("threads"),
            RoomPaneKind::RoomInfo => Cow::Borrowed("room_info"),
        }
    }
}

/// Which edge of the room screen a pane is docked to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum PaneSide {
    Top,
    Bottom,
    Left,
    #[default]
    Right,
}

impl PaneSide {
    /// The side after this one when cycling through all sides.
    pub fn next(self) -> Self {
        match self {
            PaneSide::Right => PaneSide::Bottom,
            PaneSide::Bottom => PaneSide::Left,
            PaneSide::Left => PaneSide::Top,
            PaneSide::Top => PaneSide::Right,
        }
    }

    /// Whether a pane on this side spans the dock's full height.
    pub fn is_vertical(self) -> bool {
        matches!(self, PaneSide::Left | PaneSide::Right)
    }

    /// The tooltip for a pane's edge button, which moves the pane to this side.
    pub fn move_tooltip(self) -> &'static str {
        match self {
            PaneSide::Top => "Move to the top",
            PaneSide::Bottom => "Move to the bottom",
            PaneSide::Left => "Move to the left",
            PaneSide::Right => "Move to the right",
        }
    }
}

/// Where and how large a docked pane is.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct PaneLayout {
    pub side: PaneSide,
    /// The pane's extent along its resizable axis:
    /// its width if docked to the left/right, its height if docked to the top/bottom.
    pub edge_size: f64,
}

impl Default for PaneLayout {
    fn default() -> Self {
        Self { side: PaneSide::Right, edge_size: 275.0 }
    }
}

#[derive(Default)]
struct RoomPanes {
    /// The layout the user last chose, which newly-opened panes start with.
    last_layout: Option<PaneLayout>,
    /// Panes to dock in a timeline the next time it's shown.
    pending: HashMap<TimelineKind, Vec<(RoomPaneKind, Option<SavedPaneContent>)>>,
    /// The panes that are currently popped out into a separate view.
    /// * Key: the popped-out pane's room ID and kind.
    /// * Value: the pane's saved state while it's not being shown.
    popped_out: HashMap<(OwnedRoomId, RoomPaneKind), Option<SavedPaneContent>>,
    /// The timeline that each pane was last popped out of, which it can return to.
    /// Unlike `popped_out`, this outlives a dropped pane, since the nav history can reopen it.
    pane_origins: HashMap<(OwnedRoomId, RoomPaneKind), TimelineKind>,
    /// The popped-out panes opened in place of another one. Like `pane_origins`, this outlives dropped panes.
    /// * Key: the opened pane's room ID and kind.
    /// * Value: the kind of pane that it was opened in place of.
    opened_in_place: HashMap<(OwnedRoomId, RoomPaneKind), RoomPaneKind>,
}

impl RoomPanes {
    fn waiting_replaced_pane(&self, key: &(OwnedRoomId, RoomPaneKind)) -> Option<(OwnedRoomId, RoomPaneKind)> {
        let replaced_key = (key.0.clone(), self.opened_in_place.get(key)?.clone());
        matches!(self.popped_out.get(&replaced_key), Some(Some(_))).then_some(replaced_key)
    }
}

thread_local! {
    static ROOM_PANES: RefCell<RoomPanes> = RefCell::new(RoomPanes::default());
}

fn with_room_panes<R>(f: impl FnOnce(&mut RoomPanes) -> R) -> R {
    ROOM_PANES.with_borrow_mut(f)
}

/// Emitted when panes are waiting to be docked in the given timeline,
/// such that a dock currently showing that timeline can dock them right away.
///
/// This is NOT a widget action.
#[derive(Clone, Debug)]
pub struct RoomPanesPending {
    pub timeline_kind: TimelineKind,
}

/// Returns the layout that the user last chose, which newly-opened panes start with.
pub fn last_layout() -> PaneLayout {
    with_room_panes(|rp| rp.last_layout.unwrap_or_default())
}

/// Remembers the layout that the user chose for a pane.
pub fn set_last_layout(layout: PaneLayout) {
    with_room_panes(|rp| rp.last_layout = Some(layout));
}

/// Docks a pane of the given kind in the given timeline once it's shown,
/// or right away if it's currently shown, restoring the given saved state into it.
pub fn dock_when_shown(
    cx: &mut Cx,
    timeline_kind: TimelineKind,
    kind: RoomPaneKind,
    saved: Option<SavedPaneContent>,
) {
    with_room_panes(|rp| {
        let pending = rp.pending.entry(timeline_kind.clone()).or_default();
        match pending.iter_mut().find(|(pending_kind, _)| *pending_kind == kind) {
            Some((_, pending_saved)) => *pending_saved = saved.or(pending_saved.take()),
            None => pending.push((kind, saved)),
        }
    });
    cx.action(RoomPanesPending { timeline_kind });
}

/// Takes the pane(s) waiting to be docked in the given timeline, along with the state(s) to restore into them.
pub fn take_pending(timeline_kind: &TimelineKind) -> Vec<(RoomPaneKind, Option<SavedPaneContent>)> {
    with_room_panes(|rp| rp.pending.remove(timeline_kind).unwrap_or_default())
}

/// Drops the panes waiting to be docked in the given timeline, as its screen was closed for good.
pub fn drop_pending(timeline_kind: &TimelineKind) {
    with_room_panes(|rp| rp.pending.remove(timeline_kind));
}

/// Requests that a pane be shown in its own dock tab (desktop) or stack view (mobile).
///
/// The `widget_uid` is that of the widget requesting the pop-out,
/// which must be within the RoomScreen, just like when opening a thread.
pub fn pop_out(
    cx: &mut Cx,
    widget_uid: WidgetUid,
    room_name_id: &RoomNameId,
    kind: RoomPaneKind,
    timeline_kind: TimelineKind,
    saved: SavedPaneContent,
) {
    with_room_panes(|rp| {
        let key = (room_name_id.room_id().clone(), kind.clone());
        rp.pane_origins.insert(key.clone(), timeline_kind);
        rp.popped_out.insert(key, Some(saved));
        rp.opened_in_place.retain(|(room_id, opened_kind), replaced_kind|
            room_id != room_name_id.room_id() || (*opened_kind != kind && *replaced_kind != kind)
        );
    });
    cx.widget_action(
        widget_uid,
        RoomsListAction::Selected(SelectedRoom::RoomPane {
            room_name_id: room_name_id.clone(),
            kind,
        }),
    );
}

/// Returns the timeline that the given popped-out pane came from, or else its room's main timeline.
pub fn popped_out_from(room_id: &RoomId, kind: &RoomPaneKind) -> TimelineKind {
    with_room_panes(|rp| rp.pane_origins.get(&(room_id.to_owned(), kind.clone())).cloned())
        .unwrap_or_else(|| TimelineKind::MainRoom { room_id: room_id.to_owned() })
}

/// Takes the saved state of the given popped-out pane, so its original roomscreen can show the same pane.
pub fn take_popped_out_state(room_id: &RoomId, kind: &RoomPaneKind) -> Option<SavedPaneContent> {
    // A pane that the nav history reopens (after we already dropped it) needs to be
    // popped out again, such that it can save its state again.
    with_room_panes(|rp| rp.popped_out.entry((room_id.to_owned(), kind.clone())).or_default().take())
}

/// Records that a popped-out pane of the given `kind` is opened in place of one of the `replaced_kind`,
/// so it shares that pane's spot and returns to the same timeline.
pub fn open_in_place_of(room_id: &RoomId, kind: RoomPaneKind, replaced_kind: RoomPaneKind) {
    with_room_panes(|rp| {
        let key = (room_id.to_owned(), kind);
        match rp.pane_origins.get(&(room_id.to_owned(), replaced_kind.clone())).cloned() {
            Some(timeline_kind) => rp.pane_origins.insert(key.clone(), timeline_kind),
            None => rp.pane_origins.remove(&key),
        };
        rp.opened_in_place.insert(key, replaced_kind);
    });
}

/// Returns whether the given screens are popped-out panes, one of which was opened in place of the other.
pub fn share_spot(screen: &SelectedRoom, other_screen: &SelectedRoom) -> bool {
    let (
        SelectedRoom::RoomPane { room_name_id, kind },
        SelectedRoom::RoomPane { room_name_id: other_room_name_id, kind: other_kind },
    ) = (screen, other_screen) else {
        return false;
    };
    let room_id = room_name_id.room_id();
    room_id == other_room_name_id.room_id() && with_room_panes(|rp|
        rp.opened_in_place.get(&(room_id.clone(), kind.clone())) == Some(other_kind)
            || rp.opened_in_place.get(&(room_id.clone(), other_kind.clone())) == Some(kind)
    )
}

/// Keeps the given saved state of the `replaced` pane, if the given screen was opened in place of it.
pub fn keep_replaced_state(replaced: &SelectedRoom, screen: &SelectedRoom, saved: Option<SavedPaneContent>) {
    let (SelectedRoom::RoomPane { room_name_id, kind: replaced_kind }, SelectedRoom::RoomPane { kind, .. }) = (replaced, screen) else {
        return;
    };
    with_room_panes(|rp| {
        let room_id = room_name_id.room_id();
        if rp.opened_in_place.get(&(room_id.clone(), kind.clone())) == Some(replaced_kind) {
            rp.popped_out.insert((room_id.clone(), replaced_kind.clone()), saved);
        }
    });
}

/// Returns the popped-out pane that the given popped-out pane was opened in place of,
/// if that pane's state is waiting for us to go back to it.
pub fn replaced_pane_to_restore(screen: &SelectedRoom) -> Option<SelectedRoom> {
    let SelectedRoom::RoomPane { room_name_id, kind } = screen else { return None };
    let (_, replaced_kind) = with_room_panes(|rp| rp.waiting_replaced_pane(&(room_name_id.room_id().clone(), kind.clone())))?;
    Some(SelectedRoom::RoomPane { room_name_id: room_name_id.clone(), kind: replaced_kind })
}

/// Saves the state of the given popped-out pane while its screen is hidden,
/// unless that pane has since been closed or returned to its timeline.
pub fn save_popped_out_state(room_id: &RoomId, kind: &RoomPaneKind, saved: SavedPaneContent) {
    with_room_panes(|rp| {
        if let Some(pane_saved) = rp.popped_out.get_mut(&(room_id.to_owned(), kind.clone())) {
            *pane_saved = Some(saved);
        }
    });
}

/// Drops the given popped-out pane and its saved state, as its screen was closed for good.
pub fn drop_popped_out(room_id: &RoomId, kind: &RoomPaneKind) {
    with_room_panes(|rp| {
        let key = (room_id.to_owned(), kind.clone());
        rp.popped_out.remove(&key);
        if let Some(replaced_key) = rp.waiting_replaced_pane(&key) {
            rp.popped_out.remove(&replaced_key);
        }
    });
}

/// Returns the screen that shows the given timeline of the given room.
pub fn timeline_screen(room_name_id: &RoomNameId, timeline_kind: &TimelineKind) -> SelectedRoom {
    match timeline_kind.thread_root_event_id() {
        Some(thread_root_event_id) => SelectedRoom::Thread {
            room_name_id: room_name_id.clone(),
            thread_root_event_id: thread_root_event_id.clone(),
        },
        None => SelectedRoom::JoinedRoom { room_name_id: room_name_id.clone() },
    }
}

/// Returns the layout that should be saved to persistent storage, if the user ever chose one.
pub fn saved_layout() -> Option<PaneLayout> {
    with_room_panes(|rp| rp.last_layout)
}

/// Restores the layout that was previously saved to persistent storage.
pub fn restore_saved_layout(layout: Option<PaneLayout>) {
    with_room_panes(|rp| rp.last_layout = layout);
}

/// Forgets all pending and popped-out panes, and the last layout, e.g., upon logout.
pub fn clear_all() {
    with_room_panes(|rp| *rp = RoomPanes::default());
}
