//! Panes that show extra info about a room (e.g., its member list).
//!
//! A pane is docked to one edge of a RoomScreen's timeline,
//! or popped out into its own dock tab (desktop) or stack view (mobile).
//! Docked panes are saved and restored along with their timeline's UI state.

use std::{cell::RefCell, collections::HashMap};

use makepad_widgets::*;
use serde::{Deserialize, Serialize};

use ruma::{OwnedRoomId, RoomId};

use crate::{app::SelectedRoom, home::rooms_list::RoomsListAction, sliding_sync::TimelineKind, utils::RoomNameId};

/// The kinds of panes that can be shown for a room.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RoomPaneKind {
    /// The list of the room's members.
    Members,
}

impl RoomPaneKind {
    /// The title shown in the pane's header.
    pub fn title(self) -> &'static str {
        match self {
            RoomPaneKind::Members => "Members",
        }
    }

    /// A unique string for this kind, used to build its popped-out tab's ID.
    pub fn as_str(self) -> &'static str {
        match self {
            RoomPaneKind::Members => "members",
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
        Self { side: PaneSide::Right, edge_size: 300.0 }
    }
}

#[derive(Default)]
struct RoomPanes {
    /// The layout the user last chose, which newly-opened panes start with.
    last_layout: Option<PaneLayout>,
    /// Panes to dock in a timeline the next time it's shown, e.g., upon returning from a pop-out.
    pending: HashMap<TimelineKind, Vec<RoomPaneKind>>,
    /// The timeline that each popped-out pane came from, which it returns to.
    popped_out_from: HashMap<(OwnedRoomId, RoomPaneKind), TimelineKind>,
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
/// or right away if it's currently shown.
pub fn dock_when_shown(cx: &mut Cx, timeline_kind: TimelineKind, kind: RoomPaneKind) {
    with_room_panes(|rp| {
        let pending = rp.pending.entry(timeline_kind.clone()).or_default();
        if !pending.contains(&kind) {
            pending.push(kind);
        }
    });
    cx.action(RoomPanesPending { timeline_kind });
}

/// Takes the panes waiting to be docked in the given timeline.
pub fn take_pending(timeline_kind: &TimelineKind) -> Vec<RoomPaneKind> {
    with_room_panes(|rp| rp.pending.remove(timeline_kind).unwrap_or_default())
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
) {
    with_room_panes(|rp| rp.popped_out_from.insert((room_name_id.room_id().clone(), kind), timeline_kind));
    cx.widget_action(
        widget_uid,
        RoomsListAction::Selected(SelectedRoom::RoomPane {
            room_name_id: room_name_id.clone(),
            kind,
        }),
    );
}

/// Returns the timeline that the given popped-out pane came from, or else its room's main timeline.
pub fn popped_out_from(room_id: &RoomId, kind: RoomPaneKind) -> TimelineKind {
    with_room_panes(|rp| rp.popped_out_from.get(&(room_id.to_owned(), kind)).cloned())
        .unwrap_or_else(|| TimelineKind::MainRoom { room_id: room_id.to_owned() })
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

/// Forgets all pending panes and the last layout, e.g., upon logout.
pub fn clear_all() {
    with_room_panes(|rp| *rp = RoomPanes::default());
}
