//! A room's pane (e.g., its member list) that was popped out of its room
//! into its own dock tab (desktop) or stack view (mobile).

use makepad_widgets::*;
use matrix_sdk_ui::sync_service::State as SyncServiceState;

use crate::{
    app::AppStateAction,
    home::rooms_list_header::RoomsListHeaderAction,
    profile::user_profile::UserProfileSlidingPaneWidgetExt,
    room::{
        room_members_list::{RoomMembersChanged, RoomMembersFetchAction, RoomMembersListAction, RoomMembersListWidgetRefExt, show_member_profile},
        pane_dock::set_pane_title,
        room_pane::RoomPaneKind,
    },
    sliding_sync::{MatrixRequest, submit_async_request},
    utils::RoomNameId,
};

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.RoomPaneScreen = set_type_default() do #(RoomPaneScreen::register_widget(vm)) {
        ..mod.widgets.SolidView
        width: Fill, height: Fill
        flow: Overlay
        show_bg: true
        draw_bg +: { color: (COLOR_PRIMARY) }

        pane_screen_content := View {
            width: Fill, height: Fill
            flow: Down
            padding: Inset{top: 8, right: 10, bottom: 8, left: 10}

            header := View {
                width: Fill, height: Fit
                flow: Right
                spacing: 6
                margin: Inset{bottom: 8}

                title_row := mod.widgets.RoomPaneTitle {
                    pane_icon +: { draw_icon +: { svg: (ICON_MEMBERS) } }
                }

                return_button := RobrixNeutralIconButton {
                    padding: Inset{top: 6, bottom: 6, left: 10, right: 12}
                    spacing: 6
                    draw_icon.svg: (ICON_JUMP)
                    icon_walk: Walk{width: 12, height: 12}
                    text: "Return to room"
                }
            }

            content := View {
                width: Fill, height: Fill
                flow: Down
                room_members := mod.widgets.RoomMembersList { visible: false }
            }
        }

        // Shown when clicking on a member, on top of all other content.
        user_profile_sliding_pane := mod.widgets.UserProfileSlidingPane { }
    }
}

/// Widget actions emitted by a [`RoomPaneScreen`].
#[derive(Clone, Debug, Default)]
pub enum RoomPaneScreenAction {
    /// The user asked to dock this pane back into its room.
    /// The popped-out pane's tab or view should be closed, and the room shown.
    ReturnToRoom {
        room_name_id: RoomNameId,
        kind: RoomPaneKind,
    },
    #[default]
    None,
}

#[derive(Script, ScriptHook, Widget)]
pub struct RoomPaneScreen {
    #[deref] view: View,
    /// The room and kind of pane being displayed.
    #[rust] displayed: Option<(RoomNameId, RoomPaneKind)>,
}

impl Widget for RoomPaneScreen {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if let Event::Actions(actions) = event {
            let mut members_changed = false;
            for action in actions {
                if let Some(
                    AppStateAction::RoomNameUpdated(new_name)
                    | AppStateAction::RoomLoadedSuccessfully { room_name_id: new_name, .. }
                ) = action.downcast_ref()
                    && let Some((room_name_id, kind)) = self.displayed.clone()
                    && room_name_id.room_id() == new_name.room_id()
                {
                    self.set_displayed(cx, new_name, kind);
                }

                // Without a timeline, we fetch this room's members ourselves.
                let Some((room_name_id, _)) = self.displayed.as_ref() else { continue };
                let members_list = self.view.child_by_path(ids!(content.room_members)).as_room_members_list();
                match action.downcast_ref() {
                    Some(RoomMembersFetchAction::Fetched { room_id, members }) if room_id == room_name_id.room_id() => {
                        members_list.set_members(cx, room_name_id, Some(members.clone()));
                    }
                    Some(RoomMembersFetchAction::Failed { room_id, error }) if room_id == room_name_id.room_id() => {
                        error!("Failed to fetch members of room {room_id}: {error}");
                        members_list.set_error(cx, error.clone());
                    }
                    _ => {}
                }
                members_changed |= action.downcast_ref::<RoomMembersChanged>()
                    .is_some_and(|changed| changed.room_id == *room_name_id.room_id());
                // Retry syncing members that failed to sync while we were offline.
                members_changed |= matches!(
                    action.downcast_ref(),
                    Some(RoomsListHeaderAction::StateUpdate(state)) if !matches!(state, SyncServiceState::Offline)
                );
            }
            if members_changed {
                self.fetch_members(false);
            }
        }

        let profile_pane = self.view.user_profile_sliding_pane(cx, ids!(user_profile_sliding_pane));
        let actions = cx.capture_actions(|cx| {
            // While the profile pane is shown, it gets all of the user's input.
            if profile_pane.is_currently_shown(cx) && crate::utils::is_interactive_hit_event(event) {
                profile_pane.handle_event(cx, event, scope);
            } else {
                self.view.handle_event(cx, event, scope);
            }
        });

        let mut unhandled = ActionsBuf::new();
        for action in actions {
            if let RoomMembersListAction::MemberClicked { room_name_id, member } = action.as_widget_action().cast() {
                show_member_profile(cx, &profile_pane, &room_name_id, member);
                // There's no timeline here in which to jump to a read receipt.
                profile_pane.button(cx, ids!(jump_to_read_receipt_button)).set_visible(cx, false);
                self.redraw(cx);
                continue;
            }
            unhandled.push(action);
        }
        let return_clicked = self.view.button(cx, ids!(return_button)).clicked(&unhandled);
        cx.extend_actions(unhandled);

        if return_clicked && let Some((room_name_id, kind)) = self.displayed.clone() {
            cx.widget_action(self.widget_uid(), RoomPaneScreenAction::ReturnToRoom { room_name_id, kind });
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl RoomPaneScreen {
    /// Displays the given kind of pane for the given room.
    ///
    /// Call this again with the same room whenever its name changes.
    pub fn set_displayed(&mut self, cx: &mut Cx, room_name_id: &RoomNameId, kind: RoomPaneKind) {
        let is_same = self.displayed.as_ref()
            .is_some_and(|(r, k)| r.room_id() == room_name_id.room_id() && *k == kind);
        let members = self.view.child_by_path(ids!(content.room_members)).as_room_members_list();
        if !is_same {
            self.view.user_profile_sliding_pane(cx, ids!(user_profile_sliding_pane)).reset(cx);
            members.reset(cx);
        }
        set_pane_title(cx, &self.view.widget(cx, ids!(title_row)), kind.title());
        self.view.label(cx, ids!(pane_room)).set_text(cx, &room_name_id.to_string());
        let members_widget = self.view.child_by_path(ids!(content.room_members));
        members_widget.set_visible(cx, kind == RoomPaneKind::Members);
        self.displayed = Some((room_name_id.clone(), kind));
        match kind {
            // Also re-fetch upon re-showing, in case we missed changes while hidden.
            RoomPaneKind::Members => {
                members.set_members(cx, room_name_id, None);
                self.fetch_members(false);
            }
        }
        self.redraw(cx);
    }

    /// Fetches the displayed room's members.
    fn fetch_members(&self, local_only: bool) {
        if let Some((room_name_id, RoomPaneKind::Members)) = self.displayed.as_ref() {
            submit_async_request(MatrixRequest::GetRoomMembersList {
                room_id: room_name_id.room_id().clone(),
                local_only,
            });
        }
    }

    /// Stops displaying this screen's pane.
    pub fn hide_displayed(&mut self, cx: &mut Cx) {
        self.view.user_profile_sliding_pane(cx, ids!(user_profile_sliding_pane)).reset(cx);
        self.view.child_by_path(ids!(content.room_members)).as_room_members_list().reset(cx);
        self.displayed = None;
    }
}

impl RoomPaneScreenRef {
    /// See [`RoomPaneScreen::set_displayed()`].
    pub fn set_displayed(&self, cx: &mut Cx, room_name_id: &RoomNameId, kind: RoomPaneKind) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_displayed(cx, room_name_id, kind);
        }
    }

    /// See [`RoomPaneScreen::hide_displayed()`].
    pub fn hide_displayed(&self, cx: &mut Cx) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.hide_displayed(cx);
        }
    }
}
