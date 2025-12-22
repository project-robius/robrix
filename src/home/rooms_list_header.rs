//! The RoomsListHeader contains the title label and loading spinner for rooms list.
//!
//! This widget is designed to be reused across both Desktop and Mobile variants 
//! of the RoomsSideBar to avoid code duplication.

use std::mem::discriminant;

use makepad_widgets::*;
use matrix_sdk_ui::sync_service::State;

use crate::{home::navigation_tab_bar::{NavigationBarAction, SelectedTab}, shared::{image_viewer::{ImageViewerAction, ImageViewerError, LoadState}, popup_list::{PopupItem, PopupKind, enqueue_popup_notification}}};

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::styles::*;
    use crate::shared::helpers::*;

    pub RoomsListHeader = {{RoomsListHeader}} {
        width: Fill,
        height: 30,
        padding: {bottom: 4}
        flow: Right,
        visible: true,
        align: {x: 0, y: 0.5}
        spacing: 3,

        header_title = <Label> {
            width: Fill,
            height: Fit,
            flow: Right, // do not wrap
            text: "All Rooms"
            draw_text: {
                color: #x0
                text_style: <TITLE_TEXT>{}
                wrap: Ellipsis
            }
        },

        <View> {
            width: Fit, height: Fit,
            align: {x: 0, y: 0.5},
            margin: {right: 3}
            flow: Overlay,

            loading_spinner = <LoadingSpinner> {
                visible: false,
                width: 20,
                height: 20,
                draw_bg: {
                    color: (COLOR_ACTIVE_PRIMARY)
                    border_size: 3.0,
                }
            }

            offline_icon = <View> {
                visible: false,
                width: Fit, height: Fit,
                <Icon> {
                    draw_icon: {
                        svg_file: (ICON_CLOUD_OFFLINE),
                        color: (COLOR_FG_DANGER_RED),
                    }
                    icon_walk: {width: 35, height: Fit, margin: {left: -5, bottom: 4}}
                }
            }

            synced_icon = <View> {
                visible: true,
                width: Fit, height: Fit,
                <Icon> {
                    draw_icon: {
                        svg_file: (ICON_CLOUD_CHECKMARK),
                        color: (COLOR_FG_ACCEPT_GREEN),
                    }
                    icon_walk: {width: 25, height: Fit, margin: {left: 1, bottom: 2}}
                }
            }
        }
    }
}

#[derive(Live, LiveHook, Widget)]
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
                        self.view.view(ids!(loading_spinner)).set_visible(cx, *is_syncing);
                        self.view.view(ids!(synced_icon)).set_visible(cx, !*is_syncing);
                        self.view.view(ids!(offline_icon)).set_visible(cx, false);
                        self.redraw(cx);
                        continue;
                    }
                    Some(RoomsListHeaderAction::StateUpdate(new_state)) => {
                        if discriminant(&self.sync_state) == discriminant(new_state) {
                            continue;
                        }
                        if matches!(new_state, State::Offline) {
                            self.view.view(ids!(loading_spinner)).set_visible(cx, false);
                            self.view.view(ids!(synced_icon)).set_visible(cx, false);
                            self.view.view(ids!(offline_icon)).set_visible(cx, true);
                            enqueue_popup_notification(PopupItem {
                                message: "Cannot reach the Matrix homeserver. Please check your connection.".into(),
                                auto_dismissal_duration: None,
                                kind: PopupKind::Error,
                            });
                            // Since there is no timeout for fetching media, send an action to ImageViewer when syncing is offline.
                            cx.action(ImageViewerAction::Show(LoadState::Error(ImageViewerError::Offline)));
                        }
                        self.sync_state = new_state.clone();
                        self.redraw(cx);
                        continue;
                    }
                    _ => {}
                }

                if let Some(NavigationBarAction::TabSelected(tab)) = action.downcast_ref() {
                    let header_title = self.view.label(ids!(header_title));
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
