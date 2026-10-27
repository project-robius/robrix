//! The RoomsListHeader contains the title label and loading spinner for rooms list.
//!
//! This widget is designed to be reused across both Desktop and Mobile variants 
//! of the RoomsSideBar to avoid code duplication.

use std::mem::discriminant;

use makepad_widgets::*;
use matrix_sdk_ui::sync_service::State;
use ruma::{MilliSecondsSinceUnixEpoch, OwnedRoomId};
use tokio::sync::watch;

use crate::{
    app::{AppState, AppStateAction},
    avatar_cache,
    home::navigation_tab_bar::{NavigationBarAction, SelectedTab},
    profile::user_profile_cache,
    room_preview_cache,
    shared::{
        image_viewer::{ImageViewerAction, ImageViewerError, LoadState},
        popup_list::{PopupKind, enqueue_popup_notification},
    },
    utils::time_ago,
};

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*


    mod.widgets.RoomsListHeader = #(RoomsListHeader::register_widget(vm)) {
        width: Fill,
        height: Fit,
        padding: Inset{bottom: 4}
        flow: Right,
        spacing: 3,

        tooltip_bg_color_syncing: (COLOR_ACTIVE_PRIMARY)
        tooltip_bg_color_offline: (COLOR_BG_DANGER_RED_SOLID)
        tooltip_bg_color_synced: (COLOR_BG_ACCEPT_GREEN_SOLID)

        header_title := Label {
            width: Fill,
            height: Fit,
            padding: 0
            margin: Inset{left: 5, top: -1}
            flow: Flow.Right { wrap: true },
            max_lines: 3,
            text_overflow: Ellipsis,
            text: "All Rooms"
            draw_text +: {
                color: (COLOR_TEXT_TITLE)
                text_style: TITLE_TEXT {}
            }
        },

        // As wide as the cloud icons, so the title doesn't move when the narrower spinner shows.
        sync_status := View {
            width: 26, height: Fit,
            margin: Inset{right: 3}
            flow: Overlay,
            align: Align{x: 0.5}

            loading_spinner := LoadingSpinner {
                visible: true,
                width: 20,
                height: 20,
                draw_bg +: {
                    color: (COLOR_ACTIVE_PRIMARY)
                    stroke_width: 3.0
                }
            }

            offline_icon := View {
                visible: false,
                width: Fit, height: Fit,
                Icon {
                    draw_icon +: {
                        svg: (ICON_CLOUD_OFFLINE),
                        color: (COLOR_FG_DANGER_RED),
                    }
                    icon_walk: Walk{width: 25, height: Fit, margin: Inset{left: 1, bottom: 1}}
                }
            }

            synced_icon := View {
                visible: false,
                width: Fit, height: Fit,
                Icon {
                    draw_icon +: {
                        svg: (ICON_CLOUD_CHECKMARK),
                        color: (COLOR_FG_ACCEPT_GREEN),
                    }
                    icon_walk: Walk{width: 25, height: Fit, margin: Inset{left: 1, bottom: 2}}
                }
            }
        }
    }
}

#[derive(Script, Widget)]
pub struct RoomsListHeader {
    #[deref] view: View,

    #[live] tooltip_bg_color_syncing: Vec4f,
    #[live] tooltip_bg_color_offline: Vec4f,
    #[live] tooltip_bg_color_synced: Vec4f,

    #[rust(State::Idle)] sync_state: State,
    #[rust] room_list_sync_status: Option<RoomListSyncStatus>,
    #[rust] sync_status_area: Area,
    #[rust] shown_tooltip: Option<SyncStatus>,

    /// Used for updating the name of the currently-selected space.
    #[rust] displayed_space: Option<OwnedRoomId>,
}

impl ScriptHook for RoomsListHeader {
    fn on_after_new(&mut self, vm: &mut ScriptVm) {
        vm.with_cx_mut(|cx| {
            self.room_list_sync_status = cx.global::<RoomListSyncStatusGlobal>().0.clone();
            self.show_sync_status(cx, &mut Scope::empty());
        });
    }
}

impl Widget for RoomsListHeader {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if let Event::Actions(actions) = event {
            for action in actions {
                match action.downcast_ref() {
                    Some(RoomsListHeaderAction::SetSyncStatus(status)) => {
                        self.room_list_sync_status = Some(status.clone());
                        self.show_sync_status(cx, scope);
                        continue;
                    }
                    Some(RoomsListHeaderAction::StateUpdate(new_state)) => {
                        if discriminant(&self.sync_state) == discriminant(new_state) {
                            continue;
                        }
                        if matches!(new_state, State::Offline) {
                            enqueue_popup_notification(
                                "Cannot reach the Matrix homeserver. Please check your connection.",
                                PopupKind::Error,
                                Some(4.0),
                            );
                            // Since there is no timeout for fetching media, send an action to ImageViewer when syncing is offline.
                            cx.action(ImageViewerAction::Show(LoadState::Error(ImageViewerError::Offline)));
                        } else if matches!(self.sync_state, State::Offline) {
                            // Clear stale `Requested`/`Failed` entries from global caches,
                            // as any requests submitted while offline have likely failed,
                            // leaving entries that permanently block re-fetching.
                            // Note: per-room caches (media, link preview) are cleared
                            // by RoomScreen in response to the StateUpdate action.
                            user_profile_cache::clear_all_pending_requests();
                            avatar_cache::clear_all_pending_and_failed_requests();
                            room_preview_cache::clear_all_pending_requests();
                            // Redraw everything so that widgets re-fetch any info they were still waiting on.
                            cx.redraw_all();
                            // Now that we're no longer offline, we also need to tell the
                            // ProfileIcon to refresh itself and fetch our own user's profile again.
                            SignalToUI::set_ui_signal();
                        }
                        self.sync_state = new_state.clone();
                        self.show_sync_status(cx, scope);
                        continue;
                    }
                    _ => {}
                }

                if let Some(NavigationBarAction::TabSelected(tab)) = action.downcast_ref() {
                    let header_title = self.view.label(cx, ids!(header_title));
                    match tab {
                        SelectedTab::Space { space_name_id } => {
                            header_title.set_text(cx, &space_name_id.display());
                            self.displayed_space = Some(space_name_id.room_id().clone());
                        }
                        _ => {
                            header_title.set_text(cx, "All Rooms");
                            self.displayed_space = None;
                        }
                    }
                    continue;
                }

                // If the name of the currently-selected space was changed, update the header title.
                if let Some(AppStateAction::RoomNameUpdated(new_room_name)) = action.downcast_ref()
                    && self.displayed_space.as_ref().is_some_and(|id| id == new_room_name.room_id())
                {
                    self.view.label(cx, ids!(header_title)).set_text(cx, &new_room_name.display());
                    continue;
                }
            }
        }

        // Show tooltips for the sync status icons.
        match event.hits(cx, self.sync_status_area) {
            Hit::FingerLongPress(_) | Hit::FingerHoverIn(_) => self.show_sync_status_tooltip(cx, scope, false),
            Hit::FingerUp(fe) if fe.is_over && fe.is_primary_hit() => self.show_sync_status_tooltip(cx, scope, false),
            Hit::FingerHoverOut(_) => {
                self.shown_tooltip = None;
                cx.widget_action(self.widget_uid(), TooltipAction::HoverOut);
            }
            _ => {}
        }

        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        let step = self.view.draw_walk(cx, scope, walk);
        self.sync_status_area = self.view.child(live_id!(sync_status)).area();
        step
    }
}

impl RoomsListHeader {
    fn get_sync_status(&self) -> SyncStatus {
        // If we are offline, keep showing the offline_icon,
        // as showing the loading_spinner would be misleading if we're offline.
        if matches!(self.sync_state, State::Offline) {
            SyncStatus::Offline
        } else if self.room_list_sync_status.as_ref().is_some_and(|status| status.is_fully_synced) {
            SyncStatus::Synced
        } else {
            SyncStatus::Syncing
        }
    }

    fn show_sync_status(&mut self, cx: &mut Cx, scope: &mut Scope) {
        let sync_status = self.get_sync_status();
        self.view.view(cx, ids!(loading_spinner)).set_visible(cx, sync_status == SyncStatus::Syncing);
        self.view.view(cx, ids!(offline_icon)).set_visible(cx, sync_status == SyncStatus::Offline);
        self.view.view(cx, ids!(synced_icon)).set_visible(cx, sync_status == SyncStatus::Synced);
        // If the sync status changed while its tooltip was shown, update the tooltip to match.
        if self.shown_tooltip.is_some_and(|shown| shown != sync_status) {
            self.show_sync_status_tooltip(cx, scope, true);
        }
    }

    /// Shows the sync status tooltip, or with `only_if_shown`, just updates it if it's still showing.
    fn show_sync_status_tooltip(&mut self, cx: &mut Cx, scope: &mut Scope, only_if_shown: bool) {
        let sync_status = self.get_sync_status();
        let (status_text, bg_color) = match sync_status {
            SyncStatus::Syncing => ("Syncing...", self.tooltip_bg_color_syncing),
            SyncStatus::Offline => ("Offline", self.tooltip_bg_color_offline),
            SyncStatus::Synced => ("Fully synced", self.tooltip_bg_color_synced),
        };
        // Until we get a sync response, show when we last synced before Robrix was closed.
        let last_synced_at = self.room_list_sync_status.as_ref()
            .and_then(|status| *status.last_sync_time.borrow())
            .or_else(|| scope.data.get::<AppState>()?.last_synced_at);
        let text = match last_synced_at.and_then(time_ago) {
            Some(age) => format!("{status_text}\nLast sync: {}", age.to_ascii_lowercase()),
            None => status_text.to_string(),
        };
        let widget_rect = self.sync_status_area.rect(cx);
        let options = CalloutTooltipOptions {
            bg_color: Some(bg_color),
            position: TooltipPosition::Left,
            ..Default::default()
        };
        self.shown_tooltip = Some(sync_status);
        let action = if only_if_shown {
            TooltipAction::Update { text, widget_rect, options }
        } else {
            TooltipAction::HoverIn { text, widget_rect, options }
        };
        cx.widget_action(self.widget_uid(), action);
    }
}

/// Actions that can be handled by the `RoomsListHeader`.
#[derive(Debug)]
pub enum RoomsListHeaderAction {
    /// An action received by the RoomsListHeader that will show or hide
    /// its sync status indicator (and loading spinner) based on the given status.
    SetSyncStatus(RoomListSyncStatus),
    /// An action received by the RoomsListHeader indicating the sync service state has changed.
    StateUpdate(State),
}

#[derive(Clone, Debug)]
pub struct RoomListSyncStatus {
    pub is_fully_synced: bool,
    pub last_sync_time: watch::Receiver<Option<MilliSecondsSinceUnixEpoch>>,
}

/// A `Cx` global holding the latest [`RoomsListHeaderAction::SetSyncStatus`],
/// which a `RoomsListHeader` shows when it's created.
#[derive(Clone, Debug, Default)]
pub struct RoomListSyncStatusGlobal(pub Option<RoomListSyncStatus>);

/// Returns when we last got a sync response, so we can save it in the app state.
pub fn get_last_synced_at(cx: &mut Cx) -> Option<MilliSecondsSinceUnixEpoch> {
    cx.global::<RoomListSyncStatusGlobal>().0.as_ref().and_then(|status| *status.last_sync_time.borrow())
}

#[derive(Clone, Copy, PartialEq)]
enum SyncStatus {
    Syncing,
    Offline,
    Synced,
}
