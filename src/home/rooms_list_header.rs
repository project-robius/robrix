//! The RoomsListHeader contains the title label and loading spinner for rooms list.
//!
//! This widget is designed to be reused across both Desktop and Mobile variants 
//! of the RoomsSideBar to avoid code duplication.

use std::mem::discriminant;

use makepad_widgets::*;
use matrix_sdk_ui::sync_service::State;
use ruma::OwnedRoomId;

use crate::{
    app::AppStateAction,
    avatar_cache,
    home::navigation_tab_bar::{NavigationBarAction, SelectedTab},
    profile::user_profile_cache,
    room_preview_cache,
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
        align: Align{y: 0.5}
        spacing: 3,

        header_title := Label {
            width: Fill,
            height: Fit,
            padding: 0
            margin: Inset{left: 5}
            flow: Flow.Right { wrap: false },
            max_lines: 1,
            text_overflow: Ellipsis,
            text: "All Rooms"
            draw_text +: {
                color: (RBX_FG_PRIMARY)
                // Regular weight (thinner) — reads as a title via size, not boldness.
                text_style: REGULAR_TEXT { font_size: 14 }
            }
        },

        // Room directory shortcut. Hidden until a directory screen exists in
        // this fork; the slot keeps the header's layout identical to robrix2.
        // Sized rather than Fit so the transparent click area below fills a
        // real hit target instead of shrink-wrapping the 18px icon.
        open_directory_button := View {
            visible: false,
            width: (RBX_CONTROL_H_SM),
            height: (RBX_CONTROL_H_SM)
            margin: Inset{right: 1}
            flow: Overlay,
            align: Align{x: 0.5, y: 0.5}

            Icon {
                draw_icon +: {
                    svg: (ICON_HIERARCHY)
                    color: (RBX_FG_SECONDARY)
                }
                icon_walk: Walk{width: 18, height: Fit, margin: Inset{bottom: 2}}
            }

            directory_click_area := Button {
                width: (RBX_CONTROL_H_SM),
                height: (RBX_CONTROL_H_SM)
                padding: Inset{top: 6, bottom: 6, left: 6, right: 6}
                spacing: 0,
                text: ""
                draw_bg +: {
                    color: #0000
                    color_hover: #0000
                    color_down: #0000
                    border_color: #0000
                    border_color_hover: #0000
                    border_color_down: #0000
                    border_color_focus: #0000
                    border_size: 0.0
                    border_radius: 0.0
                }
                draw_text +: {
                    color: #0000
                    color_hover: #0000
                    color_down: #0000
                    color_focus: #0000
                }
                icon_walk: Walk{width: 0, height: 0}
            }
        }

        // Search: emits `RoomsListHeaderAction::OpenRoomFilterModal` so whoever
        // hosts the rooms/spaces filter bar can bring it into focus.
        open_room_filter_modal_button := View {
            width: (RBX_CONTROL_H_SM),
            height: (RBX_CONTROL_H_SM)
            margin: Inset{right: 1}
            flow: Overlay,
            align: Align{x: 0.5, y: 0.5}

            Icon {
                draw_icon +: {
                    svg: (ICON_SEARCH)
                    color: (RBX_FG_SECONDARY)
                }
                icon_walk: Walk{width: 18, height: Fit, margin: Inset{bottom: 2}}
            }

            click_area := Button {
                width: (RBX_CONTROL_H_SM),
                height: (RBX_CONTROL_H_SM)
                padding: Inset{top: 6, bottom: 6, left: 6, right: 6}
                spacing: 0,
                text: ""
                draw_bg +: {
                    color: #0000
                    color_hover: #0000
                    color_down: #0000
                    border_color: #0000
                    border_color_hover: #0000
                    border_color_down: #0000
                    border_color_focus: #0000
                    border_size: 0.0
                    border_radius: 0.0
                }
                draw_text +: {
                    color: #0000
                    color_hover: #0000
                    color_down: #0000
                    color_focus: #0000
                }
                icon_walk: Walk{width: 0, height: 0}
            }
        }

        View {
            width: Fit, height: Fit,
            margin: Inset{right: 3}
            flow: Overlay,

            loading_spinner := LoadingSpinner {
                visible: false,
                width: 20,
                height: 20,
                draw_bg +: {
                    color: (RBX_ACCENT)
                    border_size: 3.0
                }
            }

            offline_icon := View {
                visible: false,
                width: Fit, height: Fit,
                Icon {
                    draw_icon +: {
                        svg: (ICON_CLOUD_OFFLINE),
                        color: (RBX_DANGER_FG),
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
                        color: (RBX_SUCCESS_FG),
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

    /// Used for updating the name of the currently-selected space.
    #[rust] displayed_space: Option<OwnedRoomId>,
}

impl Widget for RoomsListHeader {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if let Event::Actions(actions) = event {
            if self.view.button(cx, ids!(open_room_filter_modal_button.click_area)).clicked(actions) {
                cx.action(RoomsListHeaderAction::OpenRoomFilterModal);
            }

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
                            room_preview_cache::clear_all_pending_requests();
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
        for (view, text, bg_color) in [
            (self.view.view(cx, ids!(loading_spinner)), "Syncing...",   crate::shared::design_tokens::RBX_ACCENT),
            (self.view.view(cx, ids!(offline_icon)),    "Offline",      crate::shared::design_tokens::RBX_DANGER_FG),
            (self.view.view(cx, ids!(synced_icon)),     "Fully synced", crate::shared::design_tokens::RBX_SUCCESS_FG),
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
    /// The header's search icon was clicked: bring the rooms/spaces filter bar into focus.
    OpenRoomFilterModal,
    /// An action received by the RoomsListHeader that will show or hide
    /// its sync status indicator (and loading spinner) based on the given boolean.
    SetSyncStatus(bool),
    /// An action received by the RoomsListHeader indicating the sync service state has changed.
    StateUpdate(State),
}
