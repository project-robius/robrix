//! The RoomsListHeader contains the title label and loading spinner for rooms list.
//!
//! This widget is designed to be reused across both Desktop and Mobile variants 
//! of the RoomsSideBar to avoid code duplication.

use std::mem::discriminant;

use makepad_widgets::*;
use matrix_sdk_ui::sync_service::State;

use crate::{
    avatar_cache,
    home::navigation_tab_bar::{NavigationBarAction, SelectedTab},
    profile::user_profile_cache,
    shared::{
        image_viewer::{ImageViewerAction, ImageViewerError, LoadState},
        popup_list::{PopupKind, enqueue_popup_notification},
    },
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

        header_title := Label {
            width: Fill,
            height: Fit,
            padding: 0
            margin: Inset{left: 5, top: -1}
            flow: Right, // do not wrap
            text: "All Rooms"
            draw_text +: {
                color: #x0
                text_style: TITLE_TEXT {}
            }
        },

        View {
            width: Fit, height: Fit,
            margin: Inset{right: 3}
            flow: Overlay,

            loading_spinner := LoadingSpinner {
                visible: false,
                width: 20,
                height: 20,
                draw_bg +: {
                    color: (COLOR_ACTIVE_PRIMARY)
                    border_size: 3.0
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
                visible: true,
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

#[derive(Script, ScriptHook, Widget)]
pub struct RoomsListHeader {
    #[deref] view: View,

    #[rust(State::Idle)] sync_state: State,
}

impl Widget for RoomsListHeader {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if let Event::Actions(actions) = event {
            for action in actions {
                match action.downcast_ref() {
                    Some(RoomsListHeaderAction::SetSyncStatus(is_syncing)) => {
                        // If we are offline, keep showing the offline_icon,
                        // as showing the loading_spinner would be misleading if we're offline.
                        if matches!(self.sync_state, State::Offline) {
                            continue;
                        }
                        self.view.view(cx, ids!(loading_spinner)).set_visible(cx, *is_syncing);
                        self.view.view(cx, ids!(synced_icon)).set_visible(cx, !*is_syncing);
                        self.view.view(cx, ids!(offline_icon)).set_visible(cx, false);
                        self.redraw(cx);
                        continue;
                    }
                    Some(RoomsListHeaderAction::StateUpdate(new_state)) => {
                        if discriminant(&self.sync_state) == discriminant(new_state) {
                            continue;
                        }
                        if matches!(new_state, State::Offline) {
                            self.view.view(cx, ids!(loading_spinner)).set_visible(cx, false);
                            self.view.view(cx, ids!(synced_icon)).set_visible(cx, false);
                            self.view.view(cx, ids!(offline_icon)).set_visible(cx, true);
                            enqueue_popup_notification(
                                "Cannot reach the Matrix homeserver. Please check your connection.",
                                PopupKind::Error,
                                Some(4.0),
                            );
                            // Since there is no timeout for fetching media, send an action to ImageViewer when syncing is offline.
                            cx.action(ImageViewerAction::Show(LoadState::Error(ImageViewerError::Offline)));
                        } else if matches!(self.sync_state, State::Offline) {
                            // Transitioning away from Offline: reset to the default
                            // loading state so the sync indicator can take over again.
                            self.view.view(cx, ids!(loading_spinner)).set_visible(cx, true);
                            self.view.view(cx, ids!(synced_icon)).set_visible(cx, false);
                            self.view.view(cx, ids!(offline_icon)).set_visible(cx, false);

                            // Clear stale `Requested`/`Failed` entries from global caches,
                            // as any requests submitted while offline have likely failed,
                            // leaving entries that permanently block re-fetching.
                            // Note: per-room caches (media, link preview) are cleared
                            // by RoomScreen in response to the StateUpdate action.
                            user_profile_cache::clear_all_pending_requests();
                            avatar_cache::clear_all_pending_and_failed_requests();
                            // Now that we're no longer offline, we also need to tell the
                            // ProfileIcon to refresh itself and fetch our own user's profile again.
                            SignalToUI::set_ui_signal();
                        }
                        self.sync_state = new_state.clone();
                        self.redraw(cx);
                        continue;
                    }
                    _ => {}
                }

                if let Some(NavigationBarAction::TabSelected(tab)) = action.downcast_ref() {
                    let header_title = self.view.label(cx, ids!(header_title));
                    match tab {
                        SelectedTab::Space { space_name_id } => {
                            header_title.set_text(cx, &space_name_id.to_string());
                        }
                        _ => header_title.set_text(cx, "All Rooms"),
                    }
                    continue;
                }
            }
        }

        // Show tooltips for the sync status icons.
        for (view, text, bg_color) in [
            (self.view.view(cx, ids!(loading_spinner)), "Syncing...",   vec4(0.059, 0.533, 0.996, 1.0)), // COLOR_ACTIVE_PRIMARY #0f88fe
            (self.view.view(cx, ids!(offline_icon)),    "Offline",      vec4(0.863, 0.0, 0.020, 1.0)),   // COLOR_FG_DANGER_RED #DC0005
            (self.view.view(cx, ids!(synced_icon)),     "Fully synced", vec4(0.075, 0.533, 0.031, 1.0)), // COLOR_FG_ACCEPT_GREEN #138808
        ] {
            if !view.visible() {
                continue;
            }
            match event.hits(cx, view.area()) {
                Hit::FingerLongPress(_) | Hit::FingerHoverIn(_) => {
                    cx.widget_action(
                        self.widget_uid(),
                        TooltipAction::HoverIn {
                            text: text.to_string(),
                            widget_rect: view.area().rect(cx),
                            options: CalloutTooltipOptions {
                                text_color: vec4(1.0, 1.0, 1.0, 1.0), // COLOR_PRIMARY
                                bg_color,
                                position: TooltipPosition::Left,
                                ..Default::default()
                            },
                        },
                    );
                }
                Hit::FingerHoverOut(_) => {
                    cx.widget_action(self.widget_uid(), TooltipAction::HoverOut);
                }
                _ => {}
            }
        }

        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

/// Actions that can be handled by the `RoomsListHeader`.
#[derive(Debug)]
pub enum RoomsListHeaderAction {
    /// An action received by the RoomsListHeader that will show or hide
    /// its sync status indicator (and loading spinner) based on the given boolean.
    SetSyncStatus(bool),
    /// An action received by the RoomsListHeader indicating the sync service state has changed.
    StateUpdate(State),
}
