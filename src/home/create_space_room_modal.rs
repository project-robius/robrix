//! A modal dialog for creating a new room under a specific space.

use makepad_widgets::*;
use ruma::OwnedUserId;

use crate::{
    app::{AppStateAction, SelectedRoom},
    home::add_room::{
        CreateRoomRequest, CreateRoomRequestOrigin, CreateRoomResultAction, CreateRoomVisibility,
    },
    home::rooms_list::{RoomsListAction, RoomsListRef},
    shared::popup_list::{PopupKind, enqueue_popup_notification},
    shared::styles::{
        COLOR_ACTIVE_PRIMARY_DARKER, COLOR_FG_ACCEPT_GREEN, COLOR_FG_DANGER_RED,
        COLOR_TEXT_WARNING_NOT_FOUND,
    },
    sliding_sync::{MatrixRequest, submit_async_request},
    utils::RoomNameId,
};

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.CreateSpaceRoomModal = #(CreateSpaceRoomModal::register_widget(vm)) {
        width: Fit
        height: Fit

        RoundedView {
            width: 460
            height: Fit
            align: Align{x: 0.5}
            flow: Down
            padding: Inset{top: 28, right: 24, bottom: 22, left: 24}
            spacing: 12

            show_bg: true
            draw_bg +: {
                color: (COLOR_PRIMARY)
                border_radius: 8.0
            }

            title := Label {
                width: Fill
                height: Fit
                flow: Flow.Right{wrap: true}
                draw_text +: {
                    text_style: TITLE_TEXT {font_size: 14}
                    color: #000
                }
                text: "Create Room in Space"
            }

            subtitle := AddRoomFieldHint {
                text: ""
            }

            room_name_input := RobrixTextInput {
                draw_text +: {
                    text_style: REGULAR_TEXT {font_size: 11}
                    color: #000
                }
                empty_text: "Room name"
            }

            topic_input := RobrixTextInput {
                draw_text +: {
                    text_style: REGULAR_TEXT {font_size: 11}
                    color: #000
                }
                empty_text: "Topic (optional)"
            }

            visibility_view := View {
                width: Fill
                height: Fit
                flow: Down
                spacing: 6

                visibility_label := AddRoomFieldLabel {
                    text: "Visibility"
                }

                visibility_hint := AddRoomFieldHint {
                    text: "Private rooms are invite-only. Public rooms can be found and joined by others."
                }

                visibility_dropdown := AddRoomFormDropdown {
                    width: Fill { max: 240 }
                }
            }

            invitees_view := View {
                width: Fill
                height: Fit
                flow: Down
                spacing: 6

                invitees_label := AddRoomFieldLabel {
                    text: "Invite user IDs (optional)"
                }

                invitees_hint := AddRoomFieldHint {
                    text: "Separate multiple Matrix user IDs with commas or spaces."
                }

                invitees_input := RobrixTextInput {
                    draw_text +: {
                        text_style: REGULAR_TEXT {font_size: 11}
                        color: #000
                    }
                    empty_text: "@alice:matrix.org, @bob:example.org"
                }
            }

            status_label_view := View {
                visible: false
                width: Fill
                height: Fit
                flow: Right
                align: Align{y: 0.5}
                spacing: 8

                status_spinner := LoadingSpinner {
                    visible: false
                    width: 16
                    height: 16
                    draw_bg +: {
                        color: (COLOR_ACTIVE_PRIMARY)
                        border_size: 2.5
                    }
                }

                status_label := Label {
                    width: Fill
                    height: Fit
                    flow: Flow.Right{wrap: true}
                    draw_text +: {
                        text_style: REGULAR_TEXT {font_size: 11}
                        color: #000
                    }
                    text: ""
                }
            }

            buttons_view := View {
                width: Fill
                height: Fit
                flow: Right
                padding: Inset{top: 6, bottom: 2}
                align: Align{x: 1.0, y: 0.5}
                spacing: 16

                cancel_button := RobrixNeutralIconButton {
                    width: 132
                    align: Align{x: 0.5, y: 0.5}
                    padding: 12
                    draw_icon.svg: (ICON_FORBIDDEN)
                    icon_walk: Walk{width: 16, height: 16, margin: Inset{left: -2, right: -1}}
                    text: "Cancel"
                }

                create_button := RobrixPositiveIconButton {
                    width: 168
                    align: Align{x: 0.5, y: 0.5}
                    padding: 12
                    draw_icon.svg: (ICON_ADD)
                    icon_walk: Walk{width: 16, height: 16, margin: Inset{left: -2, right: -1}}
                    text: "Create Room"
                }
            }
        }
    }
}

#[derive(Clone, Debug)]
pub enum CreateSpaceRoomModalAction {
    Open(RoomNameId),
    Close,
}

#[derive(Clone, Debug, Default)]
enum CreateSpaceRoomModalState {
    #[default]
    WaitingForUserInput,
    WaitingForCreate(CreateRoomRequest),
    WaitingForRoomLoad(RoomNameId),
    CreateError,
}

#[derive(Script, ScriptHook, Widget)]
pub struct CreateSpaceRoomModal {
    #[deref]
    view: View,
    #[rust]
    state: CreateSpaceRoomModalState,
    #[rust]
    space_name_id: Option<RoomNameId>,
}

impl Widget for CreateSpaceRoomModal {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        self.widget_match_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl WidgetMatchEvent for CreateSpaceRoomModal {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions, _scope: &mut Scope) {
        let cancel_button = self.view.button(cx, ids!(cancel_button));
        if cancel_button.clicked(actions)
            || actions
                .iter()
                .any(|a| matches!(a.downcast_ref(), Some(ModalAction::Dismissed)))
        {
            if cancel_button.clicked(actions) {
                cx.action(CreateSpaceRoomModalAction::Close);
            }
            return;
        }

        let create_button = self.view.button(cx, ids!(create_button));
        let room_name_input = self.view.text_input(cx, ids!(room_name_input));
        let topic_input = self.view.text_input(cx, ids!(topic_input));
        let invitees_input = self.view.text_input(cx, ids!(invitees_input));
        let visibility_dropdown = self.view.drop_down(cx, ids!(visibility_dropdown));

        if create_button.clicked(actions) {
            let room_name = room_name_input.text().trim().to_string();
            if room_name.is_empty() {
                self.show_status(cx, "Please enter a room name.", PopupKind::Error, false);
                room_name_input.set_key_focus(cx);
                return;
            }

            let invite_user_ids = match parse_invited_user_ids(&invitees_input.text()) {
                Ok(user_ids) => user_ids,
                Err(error) => {
                    self.show_status(cx, &error, PopupKind::Error, false);
                    invitees_input.set_key_focus(cx);
                    return;
                }
            };

            let Some(space_name_id) = self.space_name_id.as_ref() else {
                self.show_status(
                    cx,
                    "No parent space is selected for this room.",
                    PopupKind::Error,
                    false,
                );
                return;
            };

            let topic = topic_input.text().trim().to_string();
            let request = CreateRoomRequest {
                name: room_name,
                topic: (!topic.is_empty()).then_some(topic),
                visibility: match visibility_dropdown.selected_item() {
                    1 => CreateRoomVisibility::Public,
                    _ => CreateRoomVisibility::Private,
                },
                parent_space_id: Some(space_name_id.room_id().clone()),
                is_space: false,
                invite_user_ids,
                origin: CreateRoomRequestOrigin::SpaceLobbyModal,
            };
            submit_async_request(MatrixRequest::CreateRoom(request.clone()));
            self.state = CreateSpaceRoomModalState::WaitingForCreate(request);
            self.sync_state(cx);
            return;
        }

        if let CreateSpaceRoomModalState::WaitingForCreate(request) = &self.state {
            for action in actions {
                if let Some(CreateRoomResultAction::Created {
                    room_name_id,
                    parent_space_id,
                    warning,
                    origin,
                    ..
                }) = action.downcast_ref()
                    && *origin == CreateRoomRequestOrigin::SpaceLobbyModal
                    && *parent_space_id == request.parent_space_id
                {
                    let parent_space_label = self
                        .space_name_id
                        .as_ref()
                        .map(ToString::to_string)
                        .unwrap_or_else(|| String::from("the selected space"));
                    enqueue_popup_notification(
                        format!("Created room {} in {}.", room_name_id, parent_space_label),
                        PopupKind::Success,
                        Some(4.0),
                    );
                    if let Some(warning) = warning {
                        enqueue_popup_notification(warning.clone(), PopupKind::Warning, Some(6.0));
                    }
                    self.state =
                        CreateSpaceRoomModalState::WaitingForRoomLoad(room_name_id.clone());
                    self.sync_state(cx);
                    break;
                }

                if let Some(CreateRoomResultAction::Failed {
                    parent_space_id,
                    error,
                    origin,
                    ..
                }) = action.downcast_ref()
                    && *origin == CreateRoomRequestOrigin::SpaceLobbyModal
                    && *parent_space_id == request.parent_space_id
                {
                    self.state = CreateSpaceRoomModalState::CreateError;
                    self.show_status(cx, error, PopupKind::Error, false);
                    self.sync_state(cx);
                    invitees_input.set_key_focus(cx);
                    return;
                }
            }
        }

        if let CreateSpaceRoomModalState::WaitingForRoomLoad(room_name_id) = self.state.clone() {
            if self.try_open_loaded_room(cx, &room_name_id) {
                return;
            }
            for action in actions {
                if let Some(AppStateAction::RoomLoadedSuccessfully {
                    room_name_id: loaded_room_name_id,
                    ..
                }) = action.downcast_ref()
                    && loaded_room_name_id.room_id() == room_name_id.room_id()
                {
                    cx.widget_action(
                        self.widget_uid(),
                        RoomsListAction::Selected(SelectedRoom::JoinedRoom {
                            room_name_id: room_name_id.clone(),
                        }),
                    );
                    self.state = CreateSpaceRoomModalState::WaitingForUserInput;
                    cx.action(CreateSpaceRoomModalAction::Close);
                    return;
                }
            }
        }
    }
}

impl CreateSpaceRoomModal {
    fn try_open_loaded_room(&mut self, cx: &mut Cx, room_name_id: &RoomNameId) -> bool {
        if !cx
            .get_global::<RoomsListRef>()
            .is_room_loaded(room_name_id.room_id())
        {
            return false;
        }

        cx.widget_action(
            self.widget_uid(),
            RoomsListAction::Selected(SelectedRoom::JoinedRoom {
                room_name_id: room_name_id.clone(),
            }),
        );
        self.state = CreateSpaceRoomModalState::WaitingForUserInput;
        cx.action(CreateSpaceRoomModalAction::Close);
        true
    }

    fn show_status(&mut self, cx: &mut Cx, message: &str, kind: PopupKind, show_spinner: bool) {
        let status_view = self.view.view(cx, ids!(status_label_view));
        let status_spinner = self.view.widget(cx, ids!(status_spinner));
        let mut status_label = self.view.label(cx, ids!(status_label));
        let color = match kind {
            PopupKind::Success => COLOR_FG_ACCEPT_GREEN,
            PopupKind::Warning => COLOR_TEXT_WARNING_NOT_FOUND,
            PopupKind::Error => COLOR_FG_DANGER_RED,
            PopupKind::Info | PopupKind::Blank => COLOR_ACTIVE_PRIMARY_DARKER,
        };

        status_label.set_text(cx, message);
        script_apply_eval!(cx, status_label, {
            draw_text +: {
                color: #(color),
            }
        });
        status_view.set_visible(cx, true);
        status_spinner.set_visible(cx, show_spinner);
        self.view.redraw(cx);
    }

    fn sync_state(&mut self, cx: &mut Cx) {
        let room_name_input = self.view.text_input(cx, ids!(room_name_input));
        let topic_input = self.view.text_input(cx, ids!(topic_input));
        let invitees_input = self.view.text_input(cx, ids!(invitees_input));
        let create_button = self.view.button(cx, ids!(create_button));
        let cancel_button = self.view.button(cx, ids!(cancel_button));

        let is_busy = matches!(
            self.state,
            CreateSpaceRoomModalState::WaitingForCreate(_)
                | CreateSpaceRoomModalState::WaitingForRoomLoad(_)
        );
        room_name_input.set_is_read_only(cx, is_busy);
        topic_input.set_is_read_only(cx, is_busy);
        invitees_input.set_is_read_only(cx, is_busy);
        create_button.set_enabled(cx, !is_busy);
        cancel_button.set_enabled(cx, !is_busy);
        create_button.set_text(
            cx,
            if matches!(self.state, CreateSpaceRoomModalState::WaitingForCreate(_)) {
                "Creating Room..."
            } else if matches!(self.state, CreateSpaceRoomModalState::WaitingForRoomLoad(_)) {
                "Opening Room..."
            } else {
                "Create Room"
            },
        );

        if matches!(self.state, CreateSpaceRoomModalState::WaitingForCreate(_)) {
            self.show_status(cx, "Creating room in this space...", PopupKind::Info, true);
        } else if matches!(self.state, CreateSpaceRoomModalState::WaitingForRoomLoad(_)) {
            self.show_status(
                cx,
                "Room created. Waiting for it to finish syncing before opening...",
                PopupKind::Info,
                true,
            );
        }
    }

    pub fn show(&mut self, cx: &mut Cx, space_name_id: RoomNameId) {
        self.space_name_id = Some(space_name_id.clone());
        self.state = CreateSpaceRoomModalState::WaitingForUserInput;

        self.view
            .label(cx, ids!(title))
            .set_text(cx, "Create Room in Space");
        self.view
            .label(cx, ids!(subtitle))
            .set_text(cx, &format!("Parent space: {}", space_name_id));

        self.view
            .drop_down(cx, ids!(visibility_dropdown))
            .set_labels(cx, vec![String::from("Private"), String::from("Public")]);
        self.view
            .drop_down(cx, ids!(visibility_dropdown))
            .set_selected_item(cx, 0);
        self.view
            .text_input(cx, ids!(room_name_input))
            .set_text(cx, "");
        self.view.text_input(cx, ids!(topic_input)).set_text(cx, "");
        self.view
            .text_input(cx, ids!(invitees_input))
            .set_text(cx, "");
        self.view
            .view(cx, ids!(status_label_view))
            .set_visible(cx, false);
        self.view.label(cx, ids!(status_label)).set_text(cx, "");
        self.view
            .widget(cx, ids!(status_spinner))
            .set_visible(cx, false);
        self.view
            .button(cx, ids!(create_button))
            .set_text(cx, "Create Room");
        self.sync_state(cx);
        self.view
            .text_input(cx, ids!(room_name_input))
            .set_key_focus(cx);
        self.view.redraw(cx);
    }
}

impl CreateSpaceRoomModalRef {
    pub fn show(&self, cx: &mut Cx, space_name_id: RoomNameId) {
        let Some(mut inner) = self.borrow_mut() else {
            return;
        };
        inner.show(cx, space_name_id);
    }
}

fn parse_invited_user_ids(input: &str) -> Result<Vec<OwnedUserId>, String> {
    let mut user_ids = Vec::new();
    for raw in input
        .split(|character: char| character == ',' || character.is_whitespace())
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let user_id = ruma::UserId::parse(raw)
            .map_err(|_| format!("Invalid user ID: {raw}. Expected format: @user:server"))?;
        if !user_ids.contains(&user_id) {
            user_ids.push(user_id.to_owned());
        }
    }
    Ok(user_ids)
}
