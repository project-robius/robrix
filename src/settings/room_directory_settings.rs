//! The room directories section within the SettingsScreen.
//!
//! When the user searches for rooms and spaces to join, we search their homeserver's room directory
//! and each room directory that's checked here, like activematrixrooms.com.

use std::cell::RefCell;

use makepad_widgets::*;
use ruma::{OwnedServerName, ServerName};

use crate::{
    app::{AppState, ConfirmDeleteAction},
    directory_search::RoomDirectory,
    settings::is_settings_screen_shown,
    shared::{confirmation_modal::ConfirmationModalContent, popup_list::{PopupKind, enqueue_popup_notification}},
    sliding_sync::current_user_id,
    utils,
};

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.RoomDirectorySettings = #(RoomDirectorySettings::register_widget(vm)) {
        width: Fill, height: Fit
        flow: Down

        LineH { width: 425, padding: 10, margin: Inset{top: 20, bottom: 5} }

        TitleLabel {
            text: "Room Directories"
        }

        mod.widgets.SettingsSectionDescription {
            body: "<ul><li>We search your homeserver's room directory and each checked one below.</li><li>Searches are anonymous and go through your homeserver.</li></ul>"
        }

        no_room_directories_label := View {
            width: Fill, height: Fit
            Label {
                width: Fill, height: Fit
                padding: 0
                margin: Inset{top: 10, bottom: 0, left: 13, right: 10},
                flow: Flow.Right{wrap: true},
                draw_text +: {
                    color: (COLOR_TEXT_WARNING_NOT_FOUND),
                    text_style: MESSAGE_TEXT_STYLE { font_size: 11 },
                }
                text: "You're only searching your homeserver's room directory."
            }
        }

        RoundedView {
            width: Fill, height: Fit
            margin: 5,

            show_bg: true,
            draw_bg +: {
                color: (COLOR_SETTINGS_LIST_BG)
                border_radius: 4.0
            }

            room_directories_list := FlatList {
                width: Fill, height: Fit
                flow: Down,
                scroll_bars: ScrollBars { show_scroll_x: false, show_scroll_y: false },

                room_directory_entry := View {
                    width: Fill, height: Fit
                    flow: Down

                    View {
                        width: Fill, height: Fit
                        flow: Right,
                        padding: Inset{left: 12, right: 10, top: 5, bottom: 5}
                        align: Align{y: 0.5}
                        spacing: 8

                        // The box is drawn at its left edge, so the rest of its width is a gap before the server name.
                        enabled_checkbox := CheckBoxFlat {
                            width: 28, height: 36
                            padding: 0
                            label_walk: Walk{width: 0, height: 0}
                            draw_bg +: { size: 20.0 }
                        }

                        // This takes up the space between the checkbox and remove button, so a long server name gets cut off.
                        View {
                            width: Fill, height: Fit
                            server_link := LinkLabel {
                                width: Fit { max: FitBound.Rel{base: Base.Full, factor: 1.0} }
                                margin: 0
                                draw_text +: {
                                    text_style: mod.widgets.SETTINGS_REGULAR_TEXT_STYLE {},
                                    color: (COLOR_LINK),
                                    color_hover: (COLOR_LINK_HOVER),
                                    color_down: (COLOR_LINK_HOVER),
                                    color_focus: (COLOR_LINK),
                                    max_lines: 1,
                                    text_overflow: TextOverflow.Ellipsis,
                                }
                                // This draws the link's underline.
                                draw_bg +: {
                                    color: (COLOR_LINK),
                                    color_hover: (COLOR_LINK_HOVER),
                                    color_down: (COLOR_LINK_HOVER),
                                    color_focus: (COLOR_LINK),
                                }
                            }
                        }

                        remove_button := RobrixNegativeIconButton {
                            spacing: 0
                            padding: 10
                            draw_icon.svg: (ICON_TRASH)
                            icon_walk: Walk{width: 16, height: 16}
                        }
                    }

                    LineH { padding: 10, margin: Inset{left: 5, right: 5} }
                }
            }
        }

        View {
            width: Fill, height: Fit
            margin: Inset{left: 5, top: 5, bottom: 5}
            flow: Right
            align: Align{y: 0.5}
            spacing: 8

            add_room_directory_input := RobrixTextInput {
                width: Fill { max: 300 }, height: Fit
                flow: Flow.Right { wrap: false }
                padding: Inset{left: 10, right: 10, top: 10, bottom: 10}
                empty_text: "Add a server, like example.org"
                autocapitalize: None,
                autocorrect: Disabled,
                keep_focus_on_submit: true,
            }

            add_room_directory_button := RobrixPositiveIconButton {
                height: mod.widgets.SETTINGS_BUTTON_HEIGHT,
                padding: Inset{left: 12, right: 15}
                draw_icon.svg: (ICON_ADD)
                icon_walk: Walk{width: 16, height: 16}
                text: "Add"
            }
        }
    }
}


/// The room directories section, which lists the other room directories that we can search.
///
/// The list itself lives in [`AppState::app_prefs`], which this widget changes as the user edits it.
#[derive(Script, ScriptHook, Widget)]
pub struct RoomDirectorySettings {
    #[deref] view: View,
    /// We always search our own homeserver's room directory, so we don't list it.
    #[rust] own_server: Option<OwnedServerName>,
}

/// Actions emitted by the [`RoomDirectorySettings`] widget.
#[derive(Debug)]
pub enum RoomDirectorySettingsAction {
    /// The user confirmed that we should stop searching this server's room directory.
    Remove(OwnedServerName),
}

impl Widget for RoomDirectorySettings {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);

        let Event::Actions(actions) = event else { return };
        // A hidden settings screen can't be clicked, but we still handle its non-interactive actions.
        let is_shown = is_settings_screen_shown(scope);
        let Some(app_state) = scope.data.get_mut::<AppState>() else { return };
        let directories = &mut app_state.app_prefs.room_directories;
        let mut is_changed = false;

        if is_shown {
            for (key, item) in self.view.flat_list(cx, ids!(room_directories_list)).items_with_actions(actions) {
                let Some(directory) = directories.iter_mut()
                    .find(|directory| LiveId::from_str(directory.server.as_str()) == key)
                else { continue };
                if item.link_label(cx, ids!(server_link)).clicked(actions) {
                    utils::open_url(&format!("https://{}", directory.server));
                }
                if let Some(is_enabled) = item.check_box(cx, ids!(enabled_checkbox)).changed(actions) {
                    directory.is_enabled = is_enabled;
                    is_changed = true;
                    let message = if is_enabled { "Now searching" } else { "Stopped searching" };
                    enqueue_popup_notification(
                        format!("{message} the room directory of {}.", directory.server),
                        PopupKind::Success,
                        Some(3.0),
                    );
                }
                let remove_button = item.button(cx, ids!(remove_button));
                if remove_button.clicked(actions) {
                    // The confirmation modal covers this button, so it never gets the hover-out event.
                    remove_button.reset_hover(cx);
                    let server = directory.server.clone();
                    let content = ConfirmationModalContent {
                        title_text: "Remove Room Directory".into(),
                        body_text: format!("Stop searching the room directory of {server}? You can add it back at any time.").into(),
                        accept_button_text: Some("Remove".into()),
                        on_accept_clicked: Some(Box::new(move |cx| cx.action(RoomDirectorySettingsAction::Remove(server)))),
                        ..Default::default()
                    };
                    cx.action(ConfirmDeleteAction::Show(RefCell::new(Some(content))));
                }
            }
        }

        for action in actions {
            if let Some(RoomDirectorySettingsAction::Remove(server)) = action.downcast_ref() {
                directories.retain(|directory| directory.server != *server);
                is_changed = true;
                enqueue_popup_notification(
                    format!("Removed the room directory of {server}."),
                    PopupKind::Success,
                    Some(3.0),
                );
            }
        }

        let input = self.view.text_input(cx, ids!(add_room_directory_input));
        if is_shown
            && (self.view.button(cx, ids!(add_room_directory_button)).clicked(actions) || input.returned(actions).is_some())
        {
            let text = input.text().trim().to_ascii_lowercase();
            // We also accept a link to the server, like `https://example.org/`.
            let server_name = ["https://", "http://"].iter()
                .find_map(|scheme| text.strip_prefix(scheme))
                .unwrap_or(&text)
                .trim_end_matches('/');
            match ServerName::parse(server_name) {
                Ok(server) if Some(&server) == self.own_server.as_ref() => {
                    enqueue_popup_notification("We always search your homeserver's room directory.", PopupKind::Info, Some(4.0));
                }
                Ok(server) if directories.iter().any(|directory| directory.server == server) => {
                    enqueue_popup_notification(
                        format!("The room directory of {server} is already listed."),
                        PopupKind::Info,
                        Some(4.0),
                    );
                }
                Ok(server) => {
                    enqueue_popup_notification(
                        format!("Now searching the room directory of {server}."),
                        PopupKind::Success,
                        Some(3.0),
                    );
                    directories.push(RoomDirectory { server, is_enabled: true });
                    is_changed = true;
                    input.set_text(cx, "");
                }
                Err(_) if text.is_empty() => {}
                Err(_) => {
                    enqueue_popup_notification("Enter a server name, like example.org.", PopupKind::Error, Some(4.0));
                }
            }
        }

        if is_changed {
            app_state.app_prefs.on_room_directories_changed(cx);
            self.view.redraw(cx);
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        let has_room_directories = scope.data.get::<AppState>().is_some_and(|app_state|
            app_state.app_prefs.room_directories.iter_except(self.own_server.as_deref()).next().is_some()
        );
        self.view.view(cx, ids!(no_room_directories_label)).set_visible(cx, !has_room_directories);

        while let Some(step) = self.view.draw_walk(cx, scope, walk).step() {
            // Here, we only need to handle drawing the list of room directories.
            let list_ref = step.as_flat_list();
            let Some(mut list) = list_ref.borrow_mut() else { continue };
            let Some(app_state) = scope.data.get::<AppState>() else { continue };
            for directory in app_state.app_prefs.room_directories.iter_except(self.own_server.as_deref()) {
                let Some(item) = list.item(cx, LiveId::from_str(directory.server.as_str()), id!(room_directory_entry)) else { continue };
                item.link_label(cx, ids!(server_link)).set_text(cx, directory.server.as_str());
                let checkbox = item.check_box(cx, ids!(enabled_checkbox));
                if checkbox.active(cx) != directory.is_enabled {
                    checkbox.set_active(cx, directory.is_enabled, Animate::No);
                }
                item.draw_all(cx, &mut Scope::empty());
            }
            // Drop the entries of any room directories that were removed.
            list.items.retain_visible();
        }
        DrawStep::done()
    }
}

impl RoomDirectorySettingsRef {
    /// Finds out which homeserver is ours, whose room directory we don't list.
    pub fn populate(&self, cx: &mut Cx) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.own_server = current_user_id().map(|user_id| user_id.server_name().to_owned());
        inner.redraw(cx);
    }
}
