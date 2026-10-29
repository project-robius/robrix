//! Navigation history and tracking of which screens are shown in what order.
//!
//! Handles going backwards and forwards through the nav stack, in a way that
//! is consistent across both the desktop and mobile view modes, even after transitions.

use makepad_widgets::*;

use crate::{
    app::{AppState, SelectedRoom},
    home::{
        navigation_tab_bar::SelectedTab,
        home_screen::effective_is_desktop,
        rooms_list::RoomsListRef,
    },
    room::room_pane,
    shared::context_menu::is_context_menu_shown,
};

/// The places that going back and forward return to.
///
/// `None` means "home": the rooms list in mobile view mode, which is only ever the very first place.
/// In desktop view mode, home isn't a place at all.
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
        self.push_previous_place(current);
        self.forward.clear();
    }

    /// Returns the places that going back from the `current` place can return to, from most recent to oldest.
    ///
    /// This calls `can_show()` on every place that might be shown, and if `can_show`
    /// returns false, it skips that place. This is useful for things like a room that the user
    /// has recently left, but is still deep within the nav history.
    pub fn previous_places<'a>(
        &'a self,
        current: Option<&'a SelectedRoom>,
        can_show: impl Fn(Option<&SelectedRoom>) -> bool + 'a,
    ) -> impl Iterator<Item = Option<SelectedRoom>> + 'a {
        let oldest_place = self.back.first().map_or(current, Option::as_ref);
        let fallback_places = std::iter::once(oldest_place)
            .flat_map(|place| std::iter::successors(fallback_place(place), |place| fallback_place(place.as_ref())));
        self.back.iter().rev().cloned()
            .chain(fallback_places)
            .filter(move |place| can_return_to(place.as_ref(), current, &can_show))
    }

    /// Returns where going back N `steps` from the `current` place would go to, if anywhere.
    ///
    /// See [`NavHistory::previous_places()`] for which places this skips.
    pub fn previous_place(&self, current: Option<&SelectedRoom>, steps: usize, can_show: impl Fn(Option<&SelectedRoom>) -> bool) -> Option<Option<SelectedRoom>> {
        self.previous_places(current, can_show).nth(steps.checked_sub(1)?)
    }

    /// Records that we went back N `steps` from `current` to its `previous_place()`,
    /// dropping any places that were skipped (where `can_show` returned false).
    pub fn record_going_back_from(&mut self, current: Option<SelectedRoom>, steps: usize, can_show: impl Fn(Option<&SelectedRoom>) -> bool) {
        // Home is only ever the very first place, so it's never a place that going forward returns to.
        if current.is_some() {
            self.forward.push(current.clone());
        }
        Self::move_places(&mut self.back, &mut self.forward, current.as_ref(), steps, can_show);
    }

    /// Returns the places that going forward from the `current` place can go to, from the nearest to the farthest.
    ///
    /// Like `previous_places()`, this calls `can_show()` on every place that might be shown;
    /// if `can_show` returns false, it skips that place.
    pub fn next_places<'a>(
        &'a self,
        current: Option<&'a SelectedRoom>,
        can_show: impl Fn(Option<&SelectedRoom>) -> bool + 'a,
    ) -> impl Iterator<Item = Option<SelectedRoom>> + 'a {
        self.forward.iter().rev().filter(move |place| can_return_to(place.as_ref(), current, &can_show)).cloned()
    }

    /// Returns where going forward N `steps` from the `current` place will go to, if anywhere.
    pub fn next_place(&self, current: Option<&SelectedRoom>, steps: usize, can_show: impl Fn(Option<&SelectedRoom>) -> bool) -> Option<Option<SelectedRoom>> {
        self.next_places(current, can_show).nth(steps.checked_sub(1)?)
    }

    /// Records that we went forward N `steps` from `current` to its `next_place()`,
    /// dropping any places that were skipped (where `can_show` returned false).
    pub fn record_going_forward_from(&mut self, current: Option<SelectedRoom>, steps: usize, can_show: impl Fn(Option<&SelectedRoom>) -> bool) {
        self.push_previous_place(current.clone());
        Self::move_places(&mut self.forward, &mut self.back, current.as_ref(), steps, can_show);
    }

    /// Pushes a place that going back will return to.
    ///
    /// Home is only ever the very first place, so we skip it if there are places before it.
    fn push_previous_place(&mut self, place: Option<SelectedRoom>) {
        if place.is_some() || self.back.is_empty() {
            self.back.push(place);
        }
    }

    /// Moves the N `steps` popped off `from` that we can return to onto `to`, except the last,
    /// which is the new current place. Like a browser, we can then retrace the places that we jumped past.
    fn move_places(
        from: &mut Vec<Option<SelectedRoom>>,
        to: &mut Vec<Option<SelectedRoom>>,
        current: Option<&SelectedRoom>,
        steps: usize,
        can_show: impl Fn(Option<&SelectedRoom>) -> bool,
    ) {
        let mut steps_left = steps;
        while steps_left > 0 && let Some(place) = from.pop() {
            if can_return_to(place.as_ref(), current, &can_show) {
                steps_left -= 1;
                if steps_left > 0 {
                    to.push(place);
                }
            }
        }
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

/// An action emitted to go back the given number of places in the nav history,
/// e.g., via the title bar's go-back button or its menu of previous places.
#[derive(Debug)]
pub struct GoBackAction {
    pub steps: usize,
}

/// An action emitted to go forward the given number of places in the nav history,
/// e.g., via the mouse's forward button or the title bar's menu of next places.
#[derive(Debug)]
pub struct GoForwardAction {
    pub steps: usize,
}

/// Returns whether two places are the same.
///
/// This considers their variant, not just their RoomId.
/// So an invite and the joined room with the same room ID are not the same place.
pub fn is_same_place(place: Option<&SelectedRoom>, other: Option<&SelectedRoom>) -> bool {
    place == other && place.map(std::mem::discriminant) == other.map(std::mem::discriminant)
}

/// Returns whether going back or forward from the `current` place can go to the given place,
/// which must be another place that `can_show` accepts.
fn can_return_to(place: Option<&SelectedRoom>, current: Option<&SelectedRoom>, can_show: impl Fn(Option<&SelectedRoom>) -> bool) -> bool {
    place != current && can_show(place)
}

/// Returns the place that going back from the given place would go to if there was no history (its fallback).
///
/// For a thread timeline or pane, this returns to its timeline. All others return home.
fn fallback_place(place: Option<&SelectedRoom>) -> Option<Option<SelectedRoom>> {
    match place? {
        SelectedRoom::Thread { room_name_id, .. } => Some(Some(SelectedRoom::JoinedRoom { room_name_id: room_name_id.clone() })),
        SelectedRoom::RoomPane { room_name_id, kind } => Some(Some(room_pane::timeline_screen(
            room_name_id,
            &room_pane::popped_out_from(room_name_id.room_id(), kind),
        ))),
        SelectedRoom::JoinedRoom { .. } | SelectedRoom::InvitedRoom { .. } | SelectedRoom::Space { .. } => Some(None),
    }
}

/// Returns a check for whether a place can still be shown, i.e., its room is still in the rooms list.
///
/// Home is only a place in mobile view mode, so desktop view mode never goes back or forward to it.
pub fn can_show(cx: &mut Cx) -> impl Fn(Option<&SelectedRoom>) -> bool + use<> {
    let is_desktop = effective_is_desktop(cx);
    let rooms_list = cx.has_global::<RoomsListRef>().then(|| cx.get_global::<RoomsListRef>().clone());
    move |place| match place {
        None => !is_desktop,
        // Joined spaces aren't in the rooms list.
        Some(SelectedRoom::Space { .. }) => true,
        Some(screen) => rooms_list.as_ref().is_none_or(|rooms_list| rooms_list.get_room_state(screen.room_id()).is_some()),
    }
}

/// Returns whether we can go back or forward in the nav history right now.
///
/// For ex, if a context menu or modal is shown, we don't want to handle
/// navigational gestures or actions, so this returns false in those cases.
pub fn can_navigate_history(cx: &mut Cx, ui: &WidgetRef, app_state: &AppState) -> bool {
    if !app_state.logged_in
        || !matches!(app_state.selected_tab, SelectedTab::Home | SelectedTab::Space { .. } | SelectedTab::AddRoom)
        || is_context_menu_shown(cx, ui)
    {
        return false;
    }
    let mut is_modal_open = false;
    ui.view(cx, ids!(overlay_container)).children(&mut |_id, child| {
        is_modal_open |= child.as_modal().is_open();
    });
    !is_modal_open
}

/// Returns the current place: the shown screen, or `None` for home.
///
/// While the AddRoom screen is shown, this is the place that it covers.
pub fn current_place<'a>(cx: &mut Cx, ui: &WidgetRef, app_state: &'a AppState) -> Option<&'a SelectedRoom> {
    // In mobile view mode, the selected screen isn't shown while the rooms list is.
    // But a screen waiting behind the AddRoom screen is still the current place.
    app_state.selected_room.as_ref().filter(|_| effective_is_desktop(cx)
        || matches!(app_state.selected_tab, SelectedTab::AddRoom)
        || ui.stack_navigation(cx, ids!(view_stack)).destination_view().is_some()
    )
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

    fn any(_: Option<&SelectedRoom>) -> bool { true }

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

        assert_eq!(history.previous_place(Some(&c), 1, any), Some(Some(b.clone())));
        history.record_going_back_from(Some(c.clone()), 1, any);
        assert_eq!(history.previous_place(Some(&b), 1, any), Some(Some(a.clone())));
        history.record_going_back_from(Some(b.clone()), 1, any);
        assert_eq!(history.previous_place(Some(&a), 1, any), Some(None));
        assert_eq!(history.forward, vec![Some(c.clone()), Some(b.clone())]);

        history.record_going_forward_from(Some(a.clone()), 1, any);
        assert_eq!(history.back, vec![Some(a)]);
        assert_eq!(history.forward, vec![Some(c)]);
    }

    #[test]
    fn navigating_discards_the_forward_places() {
        let (a, b, c) = (room("!a:example.org"), room("!b:example.org"), room("!c:example.org"));
        let mut history = NavHistory::default();
        history.record_navigation_from(Some(a.clone()));
        history.record_going_back_from(Some(b), 1, any);
        history.record_navigation_from(Some(a.clone()));
        assert_eq!(history.back, vec![Some(a)]);
        assert!(history.forward.is_empty());
        assert_eq!(history.previous_place(Some(&c), 1, any).flatten(), Some(room("!a:example.org")));
    }

    #[test]
    fn without_history_a_thread_goes_back_to_its_room_and_a_room_goes_home() {
        let mut history = NavHistory::default();
        assert_eq!(
            history.previous_place(Some(&thread("!a:example.org")), 1, any),
            Some(Some(room("!a:example.org"))),
        );
        assert_eq!(history.previous_place(Some(&room("!a:example.org")), 1, any), Some(None));
        assert_eq!(history.previous_place(None, 1, any), None);

        history.record_navigation_from(Some(room("!b:example.org")));
        assert_eq!(
            history.previous_place(Some(&thread("!a:example.org")), 1, any),
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
    fn going_back_several_places_lets_us_go_forward_through_them() {
        let (a, b, c, d) = (room("!a:example.org"), room("!b:example.org"), room("!c:example.org"), room("!d:example.org"));
        let mut history = NavHistory::default();
        for place in [None, Some(a.clone()), Some(b.clone()), Some(c.clone())] {
            history.record_navigation_from(place);
        }
        assert_eq!(
            history.previous_places(Some(&d), any).collect::<Vec<_>>(),
            vec![Some(c.clone()), Some(b.clone()), Some(a.clone()), None],
        );
        assert_eq!(history.previous_place(Some(&d), 3, any), Some(Some(a.clone())));
        history.record_going_back_from(Some(d.clone()), 3, any);
        assert_eq!(history.back, vec![None]);
        assert_eq!(history.next_places(Some(&a), any).collect::<Vec<_>>(), vec![Some(b.clone()), Some(c.clone()), Some(d.clone())]);

        assert_eq!(history.next_place(Some(&a), 2, any), Some(Some(c.clone())));
        history.record_going_forward_from(Some(a.clone()), 2, any);
        assert_eq!(history.back, vec![None, Some(a), Some(b)]);
        assert_eq!(history.forward, vec![Some(d)]);
    }

    #[test]
    fn going_back_past_the_oldest_place_goes_where_it_returns_to() {
        let (a, b) = (room("!a:example.org"), room("!b:example.org"));
        let mut history = NavHistory::default();
        history.record_navigation_from(Some(thread("!a:example.org")));
        assert_eq!(
            history.previous_places(Some(&b), any).collect::<Vec<_>>(),
            vec![Some(thread("!a:example.org")), Some(a.clone()), None],
        );
        history.record_going_back_from(Some(b.clone()), 3, any);
        assert!(history.back.is_empty());
        assert_eq!(history.next_places(None, any).collect::<Vec<_>>(), vec![Some(thread("!a:example.org")), Some(b)]);
        assert_eq!(history.previous_places(None, any).next(), None);
    }

    #[test]
    fn going_back_and_forward_skips_the_current_place() {
        let (a, b) = (room("!a:example.org"), room("!b:example.org"));
        let mut history = NavHistory::default();
        history.record_navigation_from(Some(a.clone()));
        history.record_navigation_from(Some(b.clone()));
        assert_eq!(history.previous_places(Some(&a), any).collect::<Vec<_>>(), vec![Some(b.clone()), None]);

        history.record_going_back_from(Some(a.clone()), 2, any);
        assert!(history.back.is_empty());
        assert_eq!(history.next_places(None, any).collect::<Vec<_>>(), vec![Some(b.clone()), Some(a.clone())]);
        assert_eq!(history.previous_places(None, any).next(), None);
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
