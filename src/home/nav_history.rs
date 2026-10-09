//! Navigation history and tracking of which screens are shown in what order.
//!
//! Handles going backwards and forwards through the nav stack, in a way that
//! is consistent across both the desktop and mobile view modes, even after transitions.

use makepad_widgets::*;

use crate::{
    app::{AppState, SelectedRoom},
    home::{
        navigation_tab_bar::SelectedTab,
        new_message_context_menu::NewMessageContextMenuWidgetRefExt,
        room_context_menu::RoomContextMenuWidgetRefExt,
        rooms_list::RoomsListRef,
    },
    room::room_pane,
};

/// The places that going back and forward return to.
///
/// `None` means "home": the desktop home/welcome tab or the rooms list in mobile view mode.
///
/// The current place/screen isn't store here, it's in [`AppState::selected_room`].
#[derive(Clone, Debug, Default)]
pub struct NavHistory {
    /// The places that going back returns to, from the oldest to the most recent.
    pub back: Vec<Option<SelectedRoom>>,
    /// The places that going forward returns to, from the farthest to the nearest.
    pub forward: Vec<Option<SelectedRoom>>,
}

impl NavHistory {
    /// Records that we're navigating away from `current`,
    /// which also discards any places that we could have gone forward to.
    pub fn record_navigation_from(&mut self, current: Option<SelectedRoom>) {
        self.back.push(current);
        self.forward.clear();
    }

    /// Returns where the next go-back gesture will return to from the `current` place, if any.
    ///
    /// This calls `can_show()` on every place that might be shown, and if `can_show`
    /// returns false, it skips that place. This is useful for things like a room that the user
    /// has recently left, but is still deep within the nav history.
    ///
    /// Without any history, a thread or pane returns to its timeline,
    /// and every other screen returns to the home/welcome tab.
    pub fn previous_place(&self, current: Option<&SelectedRoom>, can_show: impl Fn(&SelectedRoom) -> bool) -> Option<Option<SelectedRoom>> {
        if let Some(previous_place) = self.back.iter().rev().find(|place| place.as_ref().is_none_or(&can_show)) {
            return Some(previous_place.clone());
        }
        match current? {
            SelectedRoom::Thread { room_name_id, .. } => Some(Some(SelectedRoom::JoinedRoom { room_name_id: room_name_id.clone() })),
            SelectedRoom::RoomPane { room_name_id, kind } => Some(Some(room_pane::timeline_screen(
                room_name_id,
                &room_pane::popped_out_from(room_name_id.room_id(), kind),
            ))),
            SelectedRoom::JoinedRoom { .. } | SelectedRoom::InvitedRoom { .. } | SelectedRoom::Space { .. } => Some(None),
        }
    }

    /// Records that we went back from `current` to its `previous_place()`,
    /// dropping any places that were skipped (where `can_show` returned false).
    pub fn record_going_back_from(&mut self, current: Option<SelectedRoom>, can_show: impl Fn(&SelectedRoom) -> bool) {
        while let Some(place) = self.back.pop() {
            if place.as_ref().is_none_or(&can_show) { break; }
        }
        self.forward.push(current);
    }

    /// Returns where the next go-forward gesture will go to, if anywhere.
    ///
    /// Like `previous_place()`, this calls `can_show()` on every place that might be shown;
    /// if `can_show` returns false, it skips that place.
    pub fn next_place(&self, can_show: impl Fn(&SelectedRoom) -> bool) -> Option<Option<SelectedRoom>> {
        self.forward.iter().rev().find(|place| place.as_ref().is_none_or(&can_show)).cloned()
    }

    /// Records that we went forward from `current` to its `next_place()`,
    /// dropping any places that were skipped (where `can_show` returned false).
    pub fn record_going_forward_from(&mut self, current: Option<SelectedRoom>, can_show: impl Fn(&SelectedRoom) -> bool) {
        while let Some(place) = self.forward.pop() {
            if place.as_ref().is_none_or(&can_show) { break; }
        }
        self.back.push(current);
    }

    /// Replaces the given screen with another one throughout this history,
    /// e.g., a pane that went back into its room.
    pub fn replace(&mut self, screen: &SelectedRoom, replacement: &SelectedRoom, current: Option<&SelectedRoom>) {
        for place in self.back.iter_mut().chain(self.forward.iter_mut()) {
            if place.as_ref() == Some(screen) {
                *place = Some(replacement.clone());
            }
        }
        self.drop_duplicate_places(current);
    }

    /// Drops any place that's the same as the one next to it, including the `current` place.
    fn drop_duplicate_places(&mut self, current: Option<&SelectedRoom>) {
        self.back.dedup();
        self.forward.dedup();
        if self.back.last().map(Option::as_ref) == Some(current) {
            self.back.pop();
        }
        if self.forward.last().map(Option::as_ref) == Some(current) {
            self.forward.pop();
        }
    }

    /// Discards all history and starts fresh.
    pub fn clear(&mut self) {
        self.back.clear();
        self.forward.clear();
    }

    pub fn screens_mut(&mut self) -> impl Iterator<Item = &mut SelectedRoom> {
        self.back.iter_mut().chain(self.forward.iter_mut()).flatten()
    }
}

/// An action emitted to go back in the nav history via the title bar's go-back button.
#[derive(Debug)]
pub struct GoBackAction;

/// An action emitted to go forward in the nav history, e.g., via the mouse's forward button.
#[derive(Debug)]
pub struct GoForwardAction;

/// Returns a check for whether a screen can still be shown, i.e., its room is still in the rooms list.
pub fn can_show(cx: &mut Cx) -> impl Fn(&SelectedRoom) -> bool + use<> {
    let rooms_list = cx.has_global::<RoomsListRef>().then(|| cx.get_global::<RoomsListRef>().clone());
    move |screen| match screen {
        // Joined spaces aren't in the rooms list.
        SelectedRoom::Space { .. } => true,
        _ => rooms_list.as_ref().is_none_or(|rooms_list| rooms_list.get_room_state(screen.room_id()).is_some()),
    }
}

/// Returns whether we can go back or forward in the nav history right now.
///
/// For ex, if a context menu or modal is shown, we don't want to handle
/// navigational gestures or actions, so this returns false in those cases.
pub fn can_navigate_history(cx: &mut Cx, ui: &WidgetRef, app_state: &AppState) -> bool {
    if !app_state.logged_in
        || !matches!(app_state.selected_tab, SelectedTab::Home | SelectedTab::Space { .. })
        || ui.new_message_context_menu(cx, ids!(new_message_context_menu)).is_currently_shown(cx)
        || ui.room_context_menu(cx, ids!(room_context_menu)).is_currently_shown(cx)
    {
        return false;
    }
    let mut is_modal_open = false;
    ui.view(cx, ids!(overlay_container)).children(&mut |_id, child| {
        is_modal_open |= child.as_modal().is_open();
    });
    !is_modal_open
}

impl SelectedRoom {
    /// This is for screens leaving the mobile nav stack, since the desktop dock can show them again.
    pub fn drop_resources_unless_in_saved_dock(&self, cx: &mut Cx, app_state: &AppState) {
        let tab_id = self.tab_id();
        let is_in_saved_dock = std::iter::once(&app_state.saved_dock_state_home)
            .chain(app_state.saved_dock_state_per_space.values())
            .any(|saved| saved.open_rooms.contains_key(&tab_id));
        if !is_in_saved_dock {
            self.drop_resources(cx);
        }
    }
}

#[cfg(test)]
mod tests {
    use matrix_sdk::ruma::{OwnedEventId, OwnedRoomId};
    use crate::{room::room_pane::RoomPaneKind, utils::RoomNameId};
    use super::*;

    fn room(raw_room_id: &str) -> SelectedRoom {
        SelectedRoom::JoinedRoom {
            room_name_id: RoomNameId::empty(OwnedRoomId::try_from(raw_room_id).unwrap()),
        }
    }

    fn any(_: &SelectedRoom) -> bool { true }

    fn thread(raw_room_id: &str) -> SelectedRoom {
        SelectedRoom::Thread {
            room_name_id: RoomNameId::empty(OwnedRoomId::try_from(raw_room_id).unwrap()),
            thread_root_event_id: OwnedEventId::try_from("$root:example.org").unwrap(),
        }
    }

    #[test]
    fn going_back_and_forward_retraces_navigation() {
        let (a, b, c) = (room("!a:example.org"), room("!b:example.org"), room("!c:example.org"));
        let mut history = NavHistory::default();
        history.record_navigation_from(Some(a.clone()));
        history.record_navigation_from(Some(b.clone()));

        assert_eq!(history.previous_place(Some(&c), any), Some(Some(b.clone())));
        history.record_going_back_from(Some(c.clone()), any);
        assert_eq!(history.previous_place(Some(&b), any), Some(Some(a.clone())));
        history.record_going_back_from(Some(b.clone()), any);
        assert_eq!(history.previous_place(Some(&a), any), Some(None));
        assert_eq!(history.forward, vec![Some(c.clone()), Some(b.clone())]);

        history.record_going_forward_from(Some(a.clone()), any);
        assert_eq!(history.back, vec![Some(a)]);
        assert_eq!(history.forward, vec![Some(c)]);
    }

    #[test]
    fn navigating_discards_the_forward_places() {
        let (a, b, c) = (room("!a:example.org"), room("!b:example.org"), room("!c:example.org"));
        let mut history = NavHistory::default();
        history.record_navigation_from(Some(a.clone()));
        history.record_going_back_from(Some(b), any);
        history.record_navigation_from(Some(a.clone()));
        assert_eq!(history.back, vec![Some(a)]);
        assert!(history.forward.is_empty());
        assert_eq!(history.previous_place(Some(&c), any).flatten(), Some(room("!a:example.org")));
    }

    #[test]
    fn home_is_a_place_that_going_back_and_forward_returns_to() {
        let (a, b) = (room("!a:example.org"), room("!b:example.org"));
        let mut history = NavHistory::default();
        history.record_navigation_from(Some(a.clone()));
        history.record_navigation_from(None);

        assert_eq!(history.previous_place(Some(&b), any), Some(None));
        history.record_going_back_from(Some(b.clone()), any);
        assert_eq!(history.previous_place(None, any), Some(Some(a.clone())));
        history.record_going_back_from(None, any);
        assert_eq!(history.forward, vec![Some(b), None]);

        history.record_going_forward_from(Some(a.clone()), any);
        assert_eq!(history.back, vec![Some(a)]);
    }

    #[test]
    fn without_history_a_thread_goes_back_to_its_room_and_a_room_goes_home() {
        let mut history = NavHistory::default();
        assert_eq!(
            history.previous_place(Some(&thread("!a:example.org")), any),
            Some(Some(room("!a:example.org"))),
        );
        assert_eq!(history.previous_place(Some(&room("!a:example.org")), any), Some(None));
        assert_eq!(history.previous_place(None, any), None);

        history.record_navigation_from(Some(room("!b:example.org")));
        assert_eq!(
            history.previous_place(Some(&thread("!a:example.org")), any),
            Some(Some(room("!b:example.org"))),
        );
    }

    #[test]
    fn replacing_a_screen_drops_the_places_it_duplicates() {
        let (room_a, x, y) = (room("!a:example.org"), room("!x:example.org"), room("!y:example.org"));
        let pane = SelectedRoom::RoomPane {
            room_name_id: RoomNameId::empty(OwnedRoomId::try_from("!a:example.org").unwrap()),
            kind: RoomPaneKind::Members,
        };

        let mut history = NavHistory {
            back: vec![Some(room_a.clone()), Some(pane.clone()), Some(x.clone())],
            forward: vec![Some(pane.clone())],
        };
        history.replace(&pane, &room_a, Some(&y));
        assert_eq!(history.back, vec![Some(room_a.clone()), Some(x.clone())]);
        assert_eq!(history.forward, vec![Some(room_a.clone())]);

        history.back = vec![Some(x.clone()), Some(room_a.clone()), Some(pane.clone())];
        history.forward = vec![Some(y.clone()), Some(pane.clone())];
        history.replace(&pane, &room_a, Some(&room_a));
        assert_eq!(history.back, vec![Some(x)]);
        assert_eq!(history.forward, vec![Some(y)]);
    }

    #[test]
    fn going_back_and_forward_skips_a_room_that_cant_be_shown() {
        let (a, b, c) = (room("!a:example.org"), room("!b:example.org"), room("!c:example.org"));
        let can_show = |screen: &SelectedRoom| screen.room_id() != b.room_id();
        let mut history = NavHistory::default();
        history.record_navigation_from(Some(a.clone()));
        history.record_navigation_from(Some(b.clone()));
        history.record_navigation_from(Some(thread("!b:example.org")));

        assert_eq!(history.previous_place(Some(&c), can_show), Some(Some(a.clone())));
        assert_eq!(history.back.len(), 3);
        history.record_going_back_from(Some(c.clone()), can_show);
        assert!(history.back.is_empty());

        history.forward.push(Some(b.clone()));
        assert_eq!(history.next_place(can_show), Some(Some(c.clone())));
        history.record_going_forward_from(Some(a.clone()), can_show);
        assert!(history.forward.is_empty());
        assert_eq!(history.back, vec![Some(a)]);
    }

    #[test]
    fn screens_mut_skips_home() {
        let (a, b) = (room("!a:example.org"), room("!b:example.org"));
        let mut history = NavHistory {
            back: vec![Some(a.clone()), None],
            forward: vec![Some(b.clone())],
        };
        let screens: Vec<SelectedRoom> = history.screens_mut().map(|screen| screen.clone()).collect();
        assert_eq!(screens, vec![a, b]);
    }
}
