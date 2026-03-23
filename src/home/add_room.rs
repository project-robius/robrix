//! A top-level view for adding (joining) or exploring new rooms and spaces.

use makepad_widgets::*;
use matrix_sdk::RoomState;
use ruma::{
    IdParseError, MatrixToUri, MatrixUri, OwnedRoomId, OwnedRoomOrAliasId, OwnedServerName,
    matrix_uri::MatrixId,
    room::{JoinRuleSummary, RoomType},
};

use crate::{
    app::AppStateAction,
    home::{
        invite_screen::JoinRoomResultAction, navigation_tab_bar::NavigationBarAction,
        rooms_list::RoomsListRef,
    },
    room::{FetchedRoomAvatar, FetchedRoomPreview, RoomPreviewAction},
    shared::{
        avatar::AvatarWidgetRefExt,
        popup_list::{PopupKind, enqueue_popup_notification},
    },
    sliding_sync::{MatrixRequest, submit_async_request},
    utils::{self, RoomNameId},
};

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.AddRoomCreateModeButton = mod.widgets.RadioButtonTabFlat {
        height: 38
        padding: Inset{left: 0, right: 0, top: 9, bottom: 9}
        label_walk +: {
            margin: Inset{left: 14., right: 14.}
        }
        draw_bg +: {
            border_size: 1.0
            border_radius: 8.0
            color: #xF5F7FB
            color_hover: #xEEF4FF
            color_down: #xE2ECFF
            color_active: (COLOR_ACTIVE_PRIMARY)
            color_disabled: #xF7F7F7

            border_color: (COLOR_SECONDARY_DARKER)
            border_color_hover: (COLOR_ACTIVE_PRIMARY)
            border_color_down: (COLOR_ACTIVE_PRIMARY_DARKER)
            border_color_active: (COLOR_ACTIVE_PRIMARY)
            border_color_focus: (COLOR_ACTIVE_PRIMARY)
            border_color_disabled: (COLOR_BG_DISABLED)
        }
        draw_text +: {
            color: (COLOR_TEXT)
            color_hover: (COLOR_TEXT)
            color_down: (COLOR_TEXT)
            color_focus: (COLOR_TEXT)
            color_active: (COLOR_PRIMARY)
            color_disabled: (COLOR_FG_DISABLED)
            text_style: MESSAGE_TEXT_STYLE { font_size: 10.5 }
        }
    }

    mod.widgets.AddRoomFormDropdown = mod.widgets.DropDownFlat {
        width: Fill
        height: 40
        margin: 0
        padding: Inset{left: 12, right: 30, top: 11, bottom: 9}
        draw_text +: {
            color: (MESSAGE_TEXT_COLOR)
            color_hover: (MESSAGE_TEXT_COLOR)
            color_focus: (MESSAGE_TEXT_COLOR)
            color_down: (MESSAGE_TEXT_COLOR)
            color_disabled: (COLOR_FG_DISABLED)
            text_style: MESSAGE_TEXT_STYLE { font_size: 11 }
        }
        draw_bg +: {
            border_radius: 4.0
            border_size: 1.0
            color: (COLOR_PRIMARY)
            color_hover: (COLOR_PRIMARY)
            color_focus: (COLOR_PRIMARY)
            color_down: (COLOR_PRIMARY)
            color_disabled: #xF5F5F5
            border_color: (COLOR_SECONDARY_DARKER)
            border_color_hover: (COLOR_ACTIVE_PRIMARY)
            border_color_focus: (COLOR_ACTIVE_PRIMARY_DARKER)
            border_color_down: (COLOR_ACTIVE_PRIMARY_DARKER)
            border_color_disabled: (COLOR_BG_DISABLED)
            arrow_color: (MESSAGE_TEXT_COLOR)
            arrow_color_hover: (MESSAGE_TEXT_COLOR)
            arrow_color_focus: (MESSAGE_TEXT_COLOR)
            arrow_color_down: (MESSAGE_TEXT_COLOR)
            arrow_color_disabled: (COLOR_FG_DISABLED)
        }
    }

    mod.widgets.AddRoomFieldLabel = Label {
        width: Fit, height: Fit
        draw_text +: {
            color: (MESSAGE_TEXT_COLOR),
            text_style: MESSAGE_TEXT_STYLE { font_size: 10.5 },
        }
    }

    mod.widgets.AddRoomFieldHint = Label {
        width: Fill, height: Fit
        flow: Flow.Right{wrap: true},
        draw_text +: {
            color: #x666,
            text_style: MESSAGE_TEXT_STYLE { font_size: 10 },
        }
    }



    // The main view that allows the user to add (join) or explore new rooms/spaces.
    mod.widgets.AddRoomScreen = #(AddRoomScreen::register_widget(vm)) {
        ..mod.widgets.ScrollYView

        width: Fill, height: Fill,
        flow: Down,
        padding: Inset{top: 5, left: 15, right: 15, bottom: 0},

        title := TitleLabel {
            flow: Flow.Right{wrap: true},
            draw_text +: {
                text_style: TITLE_TEXT {font_size: 13},
                color: #000
            }
            text: "Add/Explore Rooms and Spaces"
            draw_text +: {
                text_style: theme.font_regular {font_size: 18},
            }
        }

        LineH { padding: 10, margin: Inset{top: 10, right: 2} }

        SubsectionLabel {
            text: "Create a new space or room:"
        }

        create_room_form := RoundedView {
            width: Fill,
            height: Fit,
            margin: Inset{top: 6, bottom: 14, left: 5, right: 5}
            padding: 18
            flow: Down

            show_bg: true
            draw_bg +: {
                color: #xFCFDFF
                border_radius: 10.0
                border_size: 1.0
                border_color: (COLOR_BG_DISABLED)
            }

            create_room_form_content := View {
                width: Fill { max: 760 }
                height: Fit
                flow: Down
                spacing: 12

                create_form_intro := AddRoomFieldHint {
                    text: "Choose what you want to create, fill in a name, then decide whether it should be private or publicly discoverable."
                }

                create_mode_buttons := View {
                    width: Fill,
                    height: Fit,
                    flow: Flow.Right{wrap: true},
                    spacing: 10

                    create_workspace_mode := AddRoomCreateModeButton {
                        text: "Space"
                    }

                    create_room_mode := AddRoomCreateModeButton {
                        text: "Standalone Room"
                    }

                    create_room_in_workspace_mode := AddRoomCreateModeButton {
                        text: "Room In Space"
                    }
                }

                create_name_input := RobrixTextInput {
                    width: Fill,
                    height: Fit,
                    margin: Inset{top: 2}
                    padding: Inset{left: 12, right: 12, top: 11, bottom: 0}
                    empty_text: "Enter a room or space name..."
                }

                create_topic_input := RobrixTextInput {
                    width: Fill,
                    height: Fit,
                    padding: Inset{left: 12, right: 12, top: 11, bottom: 0}
                    empty_text: "Topic (optional)..."
                }

                create_visibility_view := View {
                    width: Fill,
                    height: Fit,
                    flow: Down
                    spacing: 6

                    visibility_label := AddRoomFieldLabel {
                        text: "Visibility"
                    }

                    visibility_hint := AddRoomFieldHint {
                        text: "Private rooms are invite-only. Public rooms can be found and joined by others."
                    }

                    create_visibility_dropdown := AddRoomFormDropdown {
                        width: Fill { max: 240 }
                    }
                }

                parent_space_view := View {
                    visible: false
                    width: Fill,
                    height: Fit,
                    flow: Down
                    spacing: 6

                    parent_space_label := AddRoomFieldLabel {
                        text: "Parent space"
                    }

                    parent_space_dropdown := AddRoomFormDropdown {
                        width: Fill
                    }

                    parent_space_status := AddRoomFieldHint {
                        text: ""
                    }
                }

                create_status_view := View {
                    width: Fill,
                    height: Fit,
                    flow: Right,
                    align: Align{y: 0.5}
                    spacing: 8

                    create_status_spinner := LoadingSpinner {
                        visible: false
                        width: 18,
                        height: 18,
                        draw_bg +: {
                            color: (COLOR_ACTIVE_PRIMARY)
                            border_size: 3.0
                        }
                    }

                    create_status_text := Label {
                        width: Fill, height: Fit
                        flow: Flow.Right{wrap: true},
                        draw_text +: {
                            color: (MESSAGE_TEXT_COLOR),
                            text_style: MESSAGE_TEXT_STYLE { font_size: 11 },
                        }
                        text: ""
                    }
                }

                create_room_button := RobrixPositiveIconButton {
                    width: Fit,
                    padding: Inset{top: 12, bottom: 12, left: 15, right: 15}
                    draw_icon.svg: (ICON_ADD)
                    icon_walk: Walk{width: 16, height: 16, margin: Inset{left: -2, right: -1} }
                    text: "Create Space"
                }
            }
        }

        LineH { padding: 10, margin: Inset{top: 2, right: 2} }

        SubsectionLabel {
            text: "Join an existing room or space:"
        }

        // TODO: support showing/hiding this help with a collapsible widget wrapper
        //       (Accordion widget, once it's added to Makepad upstream)

        help_info := MessageHtml {
            padding: 7
            width: Fill, height: Fit
            font_size: 10.
            font_color: #3
            body: "<p>You can enter a room/space address using either:</p>
                <ul>
                  <li> An <i>alias</i>, starting with <code>#</code>, like <code>#robrix:matrix.org</code>.</li>
                  <li> An <i>ID</i>, starting with <code>!</code>, like <code>!moVNEIUPxJZpxRHDUv:matrix.org</code>.</li>
                  <li> A Matrix link, like <code>https:matrix.to/...</code> or <code>matrix:...</code>.</li>
                </ul>
            "
        }

        join_room_view := View {
            width: Fill,
            height: Fit,
            margin: Inset{ top: 3, bottom: 4 }
            align: Align{y: 0.5}
            spacing: 5
            flow: Right

            room_alias_id_input := RobrixTextInput {
                align: Align{y: 0.5}
                margin: Inset{top: 0, left: 5, right: 5, bottom: 0},
                padding: Inset{left: 12, right: 12, top: 11, bottom: 0}
                width: Fill { max: 400 } // same width as the above `help_info`
                height: 40
                empty_text: "Enter alias, ID, or Matrix link..."
            }

            search_for_room_button := RobrixIconButton {
                padding: Inset{top: 10, bottom: 10, left: 12, right: 14}
                height: 40
                draw_icon.svg: (ICON_SEARCH)
                icon_walk: Walk{width: 16, height: 16}
                text: "Go"
            }
        }

        loading_room_view := View {
            visible: false
            spacing: 5,
            padding: 10,
            width: Fill
            height: Fit
            align: Align{y: 0.5}
            flow: Right

            loading_spinner := LoadingSpinner {
                width: 25,
                height: 25,
                draw_bg +: {
                    color: (COLOR_ACTIVE_PRIMARY)
                    border_size: 3.0
                }
            }

            loading_text := Label {
                width: Fill, height: Fit
                flow: Flow.Right{wrap: true},
                margin: Inset { top: 4 }
                draw_text +: {
                    color: (MESSAGE_TEXT_COLOR),
                    text_style: MESSAGE_TEXT_STYLE { font_size: 11 },
                }
            }
        }

        error_view := View {
            padding: 10
            error_text := Label {
                width: Fill, height: Fit
                flow: Flow.Right{wrap: true},
                draw_text +: {
                    color: (COLOR_FG_DANGER_RED),
                    text_style: MESSAGE_TEXT_STYLE { font_size: 11 },
                }
            }
        }

        fetched_room_summary := RoundedView {
            visible: false
            padding: 15
            margin: Inset{top: 10, bottom: 5, left: 5, right: 5}
            flow: Down
            width: Fill, height: Fit

            show_bg: true
            draw_bg +: {
                color: (COLOR_PRIMARY)
                border_radius: 4.0
                border_size: 1.0
                border_color: (COLOR_BG_DISABLED)
                // shadow_color: #0005
                // shadow_radius: 15.0
                // shadow_offset: vec2(1.0, 0.0), //5.0,5.0)
            }

            room_name_avatar_view := View {
                width: Fill, height: Fit
                spacing: 10
                align: Align{y: 0.5}
                flow: Right,

                room_avatar := Avatar {
                    width: 45, height: 45,
                    cursor: MouseCursor.Default,
                    text_view +: {
                        text +: {
                            draw_text +: {
                                text_style: TITLE_TEXT { font_size: 16.0 }
                            }
                        }
                    }
                }

                room_name := Label {
                    width: Fill, height: Fit,
                    margin: Inset{top: 3} // align it with the above room_avatar
                    flow: Flow.Right{wrap: true},
                    draw_text +: {
                        text_style: TITLE_TEXT { font_size: 16 }
                        color: (COLOR_TEXT)
                    }
                }
            }

            // Something like "This is a [regular|direct] [room|space] with N members."
            room_summary := Label {
                width: Fill, height: Fit
                flow: Flow.Right{wrap: true},
                margin: Inset{top: 10}
                draw_text +: {
                    color: (MESSAGE_TEXT_COLOR),
                    text_style: MESSAGE_TEXT_STYLE { font_size: 11 },
                }
            }

            subsection_alias_id := SubsectionLabel {
                draw_text +: { text_style: theme.font_regular { font_size: 12 } }
            }

            room_alias_and_id_view := View {
                padding: Inset{left: 15}
                width: Fill, height: Fit
                spacing: 8.4 // to line up the colons if the ID wraps to the next line
                align: Align{y: 0.5}
                flow: Flow.Right{wrap: true},

                room_alias := Label {
                    width: Fit, height: Fit
                    flow: Flow.Right{wrap: true},
                    draw_text +: {
                        color: (MESSAGE_TEXT_COLOR),
                        text_style: MESSAGE_TEXT_STYLE { font_size: 11 },
                    }
                }

                room_id := Label {
                    width: Fit, height: Fit
                    flow: Flow.Right{wrap: true},
                    draw_text +: {
                        color: (SMALL_STATE_TEXT_COLOR),
                        text_style: MESSAGE_TEXT_STYLE { font_size: 11 },
                    }
                }
            }

            subsection_topic := SubsectionLabel {
                draw_text +: { text_style: theme.font_regular { font_size: 12 } }
            }

            room_topic := MessageHtml {
                padding: Inset{left: 20, top: 5, right: 10, bottom: 10}
                width: Fill,
                height: Fit,
                font_size: 11
                font_color: (MESSAGE_TEXT_COLOR)
            }

            buttons_view := View {
                width: Fill
                height: Fit,
                flow: Flow.Right{wrap: true},
                align: Align{y: 0.5}
                spacing: 15
                margin: Inset{top: 15}

                // This button's text is based on the room state (e.g., joined, left, invited)
                // the room's join rules (e.g., public, can knock, invite-only (in which we disable it)).
                join_room_button := RobrixPositiveIconButton {
                    padding: 15,
                    draw_icon.svg: (ICON_JOIN_ROOM)
                    icon_walk: Walk{width: 17, height: 17, margin: Inset{left: -2, right: -1} }
                }

                cancel_button := RobrixNegativeIconButton {
                    align: Align{x: 0.5, y: 0.5}
                    padding: 15
                    draw_icon.svg: (ICON_FORBIDDEN)
                    icon_walk: Walk{width: 16, height: 16, margin: Inset{left: -2, right: -1} }
                    text: "Cancel"
                }
            }
        }

    }
}

#[derive(Script, Widget)]
pub struct AddRoomScreen {
    #[deref]
    view: View,
    #[rust]
    state: AddRoomState,
    #[rust]
    create_mode: CreateMode,
    #[rust]
    create_state: CreateState,
    #[rust]
    editable_spaces_state: EditableSpacesState,
    #[rust]
    editable_spaces: Vec<EditableSpaceOption>,
    #[rust]
    selected_parent_space: Option<OwnedRoomId>,
    /// The function to perform when the user clicks the `join_room_button`.
    #[rust(JoinButtonFunction::None)]
    join_function: JoinButtonFunction,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditableSpaceOption {
    pub room_name_id: RoomNameId,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CreateRoomVisibility {
    #[default]
    Private,
    Public,
}
impl CreateRoomVisibility {
    fn from_dropdown_index(index: usize) -> Self {
        if index == 1 {
            Self::Public
        } else {
            Self::Private
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CreateRoomRequestOrigin {
    #[default]
    AddRoomScreen,
    SpaceLobbyModal,
}

#[derive(Clone, Debug)]
pub struct CreateRoomRequest {
    pub name: String,
    pub topic: Option<String>,
    pub visibility: CreateRoomVisibility,
    pub parent_space_id: Option<OwnedRoomId>,
    pub is_space: bool,
    pub invite_user_ids: Vec<ruma::OwnedUserId>,
    pub origin: CreateRoomRequestOrigin,
}
impl CreateRoomRequest {
    fn is_room_in_space(&self) -> bool {
        self.parent_space_id.is_some() && !self.is_space
    }
}

#[derive(Debug)]
pub enum EditableSpacesAction {
    Loaded(Result<Vec<EditableSpaceOption>, String>),
}

#[derive(Debug)]
pub enum CreateRoomResultAction {
    Created {
        room_name_id: RoomNameId,
        is_space: bool,
        parent_space_id: Option<OwnedRoomId>,
        warning: Option<String>,
        origin: CreateRoomRequestOrigin,
    },
    Failed {
        is_space: bool,
        parent_space_id: Option<OwnedRoomId>,
        error: String,
        origin: CreateRoomRequestOrigin,
    },
}

#[derive(Clone, Debug)]
pub enum AddRoomScreenAction {
    PrefillJoinLookup(String),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum CreateMode {
    #[default]
    Space,
    StandaloneRoom,
    RoomInSpace,
}
impl CreateMode {
    fn from_radio_index(index: usize) -> Self {
        match index {
            1 => Self::StandaloneRoom,
            2 => Self::RoomInSpace,
            _ => Self::Space,
        }
    }

    fn is_space(self) -> bool {
        matches!(self, Self::Space)
    }

    fn requires_parent_space(self) -> bool {
        matches!(self, Self::RoomInSpace)
    }

    fn create_button_text(self) -> &'static str {
        match self {
            Self::Space => "Create Space",
            Self::StandaloneRoom => "Create Room",
            Self::RoomInSpace => "Create Room In Space",
        }
    }

    fn idle_status_text(self) -> &'static str {
        match self {
            Self::Space => "Create a top-level Matrix space.",
            Self::StandaloneRoom => "Create a new standalone room.",
            Self::RoomInSpace => {
                "Create a new room and attach it under one of your editable spaces."
            }
        }
    }
}

#[derive(Clone, Debug, Default)]
enum CreateState {
    #[default]
    Idle,
    Creating {
        request: CreateRoomRequest,
    },
    CreatedPendingLoad {
        room_name_id: RoomNameId,
    },
    CreatedLoaded {
        room_name_id: RoomNameId,
        is_space: bool,
    },
    Error(String),
}

#[derive(Clone, Debug, Default)]
enum EditableSpacesState {
    #[default]
    Unrequested,
    Loading,
    Loaded,
    Error(String),
}

#[derive(Default)]
#[allow(clippy::large_enum_variant)]
enum AddRoomState {
    /// We're waiting for the user to input a room ID, alias, or matrix link.
    #[default]
    WaitingOnUserInput,
    /// We successfully parsed the user input and have sent a request.
    /// We're now waiting for the room preview to be fetched and returned.
    Parsed {
        room_or_alias_id: OwnedRoomOrAliasId,
        via: Vec<OwnedServerName>,
    },
    /// The user entered invalid input that we couldn't parse into a room address.
    ParseError(String),
    /// We successfully fetched the room preview and have displayed it,
    /// and are waiting for the user to join the room.
    FetchedRoomPreview {
        frp: FetchedRoomPreview,
        room_or_alias_id: OwnedRoomOrAliasId,
        via: Vec<OwnedServerName>,
    },
    /// We failed to fetch the room preview, likely because it couldn't be found
    /// or because of connectivity issues or something else.
    FetchError(String),
    /// We successfully knocked on the room or space, and are waiting for
    /// a member of that room/space to acknowledge our knock by inviting us.
    Knocked { frp: FetchedRoomPreview },
    /// We successfully joined the room or space, and are waiting for it
    /// to be loaded from the homeserver.
    Joined { frp: FetchedRoomPreview },
    /// The fetched room or space has been loaded from the homeserver,
    /// so we can allow the user to jump to it via the `join_room_button`.
    Loaded {
        frp: FetchedRoomPreview,
        is_invite: bool,
    },
}
impl AddRoomState {
    fn fetched_room_preview(&self) -> Option<&FetchedRoomPreview> {
        match self {
            Self::FetchedRoomPreview { frp, .. }
            | Self::Knocked { frp }
            | Self::Joined { frp }
            | Self::Loaded { frp, .. } => Some(frp),
            _ => None,
        }
    }

    fn transition_to_knocked(&mut self) {
        let prev = std::mem::take(self);
        if let Self::FetchedRoomPreview { frp, .. } = prev {
            *self = Self::Knocked { frp };
        } else {
            *self = prev;
        }
    }

    fn transition_to_joined(&mut self) {
        let prev = std::mem::take(self);
        if let Self::FetchedRoomPreview { frp, .. } = prev {
            *self = Self::Joined { frp };
        } else {
            *self = prev;
        }
    }

    fn transition_to_loaded(&mut self, is_invite: bool) {
        let prev = std::mem::take(self);
        match prev {
            Self::FetchedRoomPreview { frp, .. } | Self::Joined { frp } | Self::Knocked { frp } => {
                *self = Self::Loaded { frp, is_invite };
            }
            _ => {
                *self = prev;
            }
        }
    }
}

impl ScriptHook for AddRoomScreen {
    fn on_after_new(&mut self, vm: &mut ScriptVm) {
        vm.with_cx_mut(|cx| {
            let create_visibility_dropdown =
                self.view.drop_down(cx, ids!(create_visibility_dropdown));
            create_visibility_dropdown
                .set_labels(cx, vec![String::from("Private"), String::from("Public")]);
            create_visibility_dropdown.set_selected_item(cx, 0);
            self.view
                .radio_button(cx, ids!(create_workspace_mode))
                .set_active(cx, true);
            self.view
                .radio_button(cx, ids!(create_room_mode))
                .set_active(cx, false);
            self.view
                .radio_button(cx, ids!(create_room_in_workspace_mode))
                .set_active(cx, false);
            self.view
                .button(cx, ids!(search_for_room_button))
                .set_enabled(cx, false);
            self.sync_create_form(cx);
        });
    }
}

impl AddRoomScreen {
    fn reset_finished_create_state(&mut self) {
        if !matches!(self.create_state, CreateState::Creating { .. }) {
            self.create_state = CreateState::Idle;
        }
    }

    fn request_editable_spaces(&mut self, cx: &mut Cx) {
        self.editable_spaces_state = EditableSpacesState::Loading;
        submit_async_request(MatrixRequest::GetEditableSpaces);
        self.sync_create_form(cx);
    }

    fn select_preferred_parent_space(&mut self, cx: &mut Cx) {
        let preferred_space_id = cx
            .get_global::<RoomsListRef>()
            .get_selected_space_id()
            .filter(|selected| {
                self.editable_spaces
                    .iter()
                    .any(|space| space.room_name_id.room_id() == selected)
            });
        self.selected_parent_space = preferred_space_id.or_else(|| {
            self.editable_spaces
                .first()
                .map(|space| space.room_name_id.room_id().clone())
        });
    }

    fn sync_parent_space_dropdown(&mut self, cx: &mut Cx) {
        let parent_space_dropdown = self.view.drop_down(cx, ids!(parent_space_dropdown));
        let parent_space_widget = self.view.widget(cx, ids!(parent_space_dropdown));

        let labels = match &self.editable_spaces_state {
            EditableSpacesState::Loading => vec![String::from("Loading editable spaces...")],
            EditableSpacesState::Error(_) => {
                vec![String::from("Unable to load editable spaces")]
            }
            EditableSpacesState::Loaded if self.editable_spaces.is_empty() => {
                vec![String::from("No editable spaces available")]
            }
            _ => self
                .editable_spaces
                .iter()
                .map(|space| space.room_name_id.to_string())
                .collect(),
        };
        parent_space_dropdown.set_labels(cx, labels);

        let selected_index = self
            .selected_parent_space
            .as_ref()
            .and_then(|selected| {
                self.editable_spaces
                    .iter()
                    .position(|space| space.room_name_id.room_id() == selected)
            })
            .unwrap_or(0);
        parent_space_dropdown.set_selected_item(cx, selected_index);

        let is_enabled = matches!(self.editable_spaces_state, EditableSpacesState::Loaded)
            && !self.editable_spaces.is_empty();
        parent_space_widget.set_disabled(cx, !is_enabled);
    }

    fn sync_create_form(&mut self, cx: &mut Cx) {
        self.view
            .radio_button(cx, ids!(create_workspace_mode))
            .set_active(cx, matches!(self.create_mode, CreateMode::Space));
        self.view
            .radio_button(cx, ids!(create_room_mode))
            .set_active(cx, matches!(self.create_mode, CreateMode::StandaloneRoom));
        self.view
            .radio_button(cx, ids!(create_room_in_workspace_mode))
            .set_active(cx, matches!(self.create_mode, CreateMode::RoomInSpace));

        let parent_space_view = self.view.view(cx, ids!(parent_space_view));
        parent_space_view.set_visible(cx, self.create_mode.requires_parent_space());
        self.sync_parent_space_dropdown(cx);

        let parent_space_status = self.view.label(cx, ids!(parent_space_status));
        let parent_status_text: String = match &self.editable_spaces_state {
            EditableSpacesState::Unrequested => {
                String::from("Choose a joined space where you can manage child rooms.")
            }
            EditableSpacesState::Loading => String::from("Loading spaces you can manage..."),
            EditableSpacesState::Loaded if self.editable_spaces.is_empty() => String::from(
                "You do not currently have permission to create child rooms in any joined space.",
            ),
            EditableSpacesState::Loaded => self
                .selected_parent_space
                .as_ref()
                .and_then(|selected| {
                    self.editable_spaces
                        .iter()
                        .find(|space| space.room_name_id.room_id() == selected)
                })
                .map(|space| format!("Selected space: {}", space.room_name_id))
                .unwrap_or_else(|| String::from("Select a parent space.")),
            EditableSpacesState::Error(error) => error.clone(),
        };
        parent_space_status.set_text(cx, &parent_status_text);

        let create_button = self.view.button(cx, ids!(create_room_button));
        let create_status_spinner = self.view.widget(cx, ids!(create_status_spinner));
        let create_status_text = self.view.label(cx, ids!(create_status_text));
        let create_name_input = self.view.text_input(cx, ids!(create_name_input));

        let (button_text, button_enabled, spinner_visible, status_text) = match &self.create_state {
            CreateState::Idle => (
                self.create_mode.create_button_text().to_string(),
                !create_name_input.text().trim().is_empty()
                    && (!self.create_mode.requires_parent_space()
                        || self.selected_parent_space.is_some()),
                false,
                self.create_mode.idle_status_text().to_string(),
            ),
            CreateState::Creating { request } => (
                if request.is_space {
                    String::from("Creating Space...")
                } else if request.is_room_in_space() {
                    String::from("Creating Room In Space...")
                } else {
                    String::from("Creating Room...")
                },
                false,
                true,
                if request.is_space {
                    String::from("Creating space...")
                } else if request.is_room_in_space() {
                    String::from("Creating room and linking it into the selected space...")
                } else {
                    String::from("Creating room...")
                },
            ),
            CreateState::CreatedPendingLoad { room_name_id, .. } => (
                String::from("Waiting For Sync..."),
                false,
                true,
                format!(
                    "Created {}. Waiting for it to finish syncing...",
                    room_name_id
                ),
            ),
            CreateState::CreatedLoaded {
                room_name_id,
                is_space,
            } => (
                if *is_space {
                    String::from("Go To Created Space")
                } else {
                    String::from("Go To Created Room")
                },
                true,
                false,
                if *is_space {
                    format!("Created space {}.", room_name_id)
                } else {
                    format!("Created room {}.", room_name_id)
                },
            ),
            CreateState::Error(error) => (
                self.create_mode.create_button_text().to_string(),
                !create_name_input.text().trim().is_empty()
                    && (!self.create_mode.requires_parent_space()
                        || self.selected_parent_space.is_some()),
                false,
                error.clone(),
            ),
        };

        create_button.set_text(cx, &button_text);
        create_button.set_enabled(cx, button_enabled);
        create_status_spinner.set_visible(cx, spinner_visible);
        create_status_text.set_text(cx, &status_text);
    }

    fn navigate_to_created_destination(
        &self,
        cx: &mut Cx,
        room_name_id: &RoomNameId,
        is_space: bool,
    ) {
        if is_space {
            cx.action(NavigationBarAction::GoToSpace {
                space_name_id: room_name_id.clone(),
            });
        } else {
            cx.action(AppStateAction::NavigateToRoom {
                room_to_close: None,
                destination_room: crate::room::BasicRoomDetails::Name(room_name_id.clone()),
            });
        }
    }
}

impl Widget for AddRoomScreen {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);

        if let Event::Actions(actions) = event {
            let room_alias_id_input = self.view.text_input(cx, ids!(room_alias_id_input));
            let search_for_room_button = self.view.button(cx, ids!(search_for_room_button));
            let create_name_input = self.view.text_input(cx, ids!(create_name_input));
            let create_topic_input = self.view.text_input(cx, ids!(create_topic_input));
            let create_visibility_dropdown =
                self.view.drop_down(cx, ids!(create_visibility_dropdown));
            let parent_space_dropdown = self.view.drop_down(cx, ids!(parent_space_dropdown));
            let create_room_button = self.view.button(cx, ids!(create_room_button));
            let create_mode_buttons = self.view.radio_button_set(
                cx,
                ids_array!(
                    create_workspace_mode,
                    create_room_mode,
                    create_room_in_workspace_mode,
                ),
            );
            let cancel_button = self
                .view
                .button(cx, ids!(fetched_room_summary.buttons_view.cancel_button));
            let join_room_button = self
                .view
                .button(cx, ids!(fetched_room_summary.buttons_view.join_room_button));

            // Enable or disable the button based on if the text input is empty.
            if let Some(text) = room_alias_id_input.changed(actions) {
                search_for_room_button.set_enabled(cx, !text.trim().is_empty());
            }

            let mut create_form_changed = false;
            if let Some(index) = create_mode_buttons.selected(cx, actions) {
                let new_mode = CreateMode::from_radio_index(index);
                if self.create_mode != new_mode {
                    self.create_mode = new_mode;
                    self.reset_finished_create_state();
                    if self.create_mode.requires_parent_space()
                        && matches!(
                            self.editable_spaces_state,
                            EditableSpacesState::Unrequested | EditableSpacesState::Error(_)
                        )
                    {
                        self.request_editable_spaces(cx);
                    }
                    create_form_changed = true;
                }
            }

            if create_name_input.changed(actions).is_some()
                || create_topic_input.changed(actions).is_some()
                || create_visibility_dropdown.changed(actions).is_some()
            {
                self.reset_finished_create_state();
                create_form_changed = true;
            }

            if let Some(index) = parent_space_dropdown.changed(actions) {
                self.selected_parent_space = self
                    .editable_spaces
                    .get(index)
                    .map(|space| space.room_name_id.room_id().clone());
                self.reset_finished_create_state();
                create_form_changed = true;
            }

            for action in actions {
                if let Some(AddRoomScreenAction::PrefillJoinLookup(query)) = action.downcast_ref() {
                    self.state = AddRoomState::WaitingOnUserInput;
                    room_alias_id_input.set_text(cx, query);
                    search_for_room_button.set_enabled(cx, !query.trim().is_empty());
                    room_alias_id_input.set_key_focus(cx);
                    self.redraw(cx);
                }

                match action.downcast_ref() {
                    Some(EditableSpacesAction::Loaded(Ok(spaces))) => {
                        self.editable_spaces = spaces.clone();
                        self.editable_spaces_state = EditableSpacesState::Loaded;
                        self.select_preferred_parent_space(cx);
                        create_form_changed = true;
                    }
                    Some(EditableSpacesAction::Loaded(Err(error))) => {
                        self.editable_spaces.clear();
                        self.selected_parent_space = None;
                        self.editable_spaces_state = EditableSpacesState::Error(error.clone());
                        create_form_changed = true;
                    }
                    _ => {}
                }

                if let Some(CreateRoomResultAction::Created {
                    room_name_id,
                    is_space,
                    parent_space_id,
                    warning,
                    origin,
                }) = action.downcast_ref()
                    && *origin == CreateRoomRequestOrigin::AddRoomScreen
                {
                    let success_message = if *is_space {
                        format!("Successfully created space {}.", room_name_id)
                    } else if parent_space_id.is_some() {
                        format!(
                            "Successfully created room {} in the selected space.",
                            room_name_id
                        )
                    } else {
                        format!("Successfully created room {}.", room_name_id)
                    };
                    enqueue_popup_notification(success_message, PopupKind::Success, Some(4.0));
                    if let Some(warning) = warning {
                        enqueue_popup_notification(warning.clone(), PopupKind::Warning, Some(6.0));
                    }
                    self.create_state = if *is_space {
                        CreateState::CreatedLoaded {
                            room_name_id: room_name_id.clone(),
                            is_space: true,
                        }
                    } else {
                        CreateState::CreatedPendingLoad {
                            room_name_id: room_name_id.clone(),
                        }
                    };
                    create_form_changed = true;
                }

                if let Some(CreateRoomResultAction::Failed { error, origin, .. }) =
                    action.downcast_ref()
                    && *origin == CreateRoomRequestOrigin::AddRoomScreen
                {
                    enqueue_popup_notification(error.clone(), PopupKind::Error, None);
                    self.create_state = CreateState::Error(error.clone());
                    create_form_changed = true;
                }

                if let Some(AppStateAction::RoomLoadedSuccessfully { room_name_id, .. }) =
                    action.downcast_ref()
                    && let CreateState::CreatedPendingLoad {
                        room_name_id: pending_room_name_id,
                        ..
                    } = &self.create_state
                    && pending_room_name_id.room_id() == room_name_id.room_id()
                {
                    self.create_state = CreateState::CreatedLoaded {
                        room_name_id: room_name_id.clone(),
                        is_space: false,
                    };
                    create_form_changed = true;
                }
            }

            if create_room_button.clicked(actions) {
                match &self.create_state {
                    CreateState::CreatedLoaded {
                        room_name_id,
                        is_space,
                    } => {
                        self.navigate_to_created_destination(cx, room_name_id, *is_space);
                    }
                    CreateState::Creating { .. } | CreateState::CreatedPendingLoad { .. } => {}
                    _ => {
                        let room_name = create_name_input.text().trim().to_string();
                        if room_name.is_empty() {
                            let err = String::from("Please enter a name for the room or space.");
                            enqueue_popup_notification(err.clone(), PopupKind::Error, None);
                            self.create_state = CreateState::Error(err);
                            create_name_input.set_key_focus(cx);
                            create_form_changed = true;
                        } else {
                            let parent_space_id = if self.create_mode.requires_parent_space() {
                                match self.selected_parent_space.clone() {
                                    Some(parent_space_id) => Some(parent_space_id),
                                    None => {
                                        let err = String::from(
                                            "Please choose a space where the new room should be created.",
                                        );
                                        enqueue_popup_notification(
                                            err.clone(),
                                            PopupKind::Error,
                                            None,
                                        );
                                        self.create_state = CreateState::Error(err);
                                        create_form_changed = true;
                                        None
                                    }
                                }
                            } else {
                                None
                            };

                            if !self.create_mode.requires_parent_space()
                                || parent_space_id.is_some()
                            {
                                let topic = create_topic_input.text().trim().to_string();
                                let request = CreateRoomRequest {
                                    name: room_name,
                                    topic: (!topic.is_empty()).then_some(topic),
                                    visibility: CreateRoomVisibility::from_dropdown_index(
                                        create_visibility_dropdown.selected_item(),
                                    ),
                                    parent_space_id,
                                    is_space: self.create_mode.is_space(),
                                    invite_user_ids: Vec::new(),
                                    origin: CreateRoomRequestOrigin::AddRoomScreen,
                                };
                                submit_async_request(MatrixRequest::CreateRoom(request.clone()));
                                self.create_state = CreateState::Creating { request };
                                create_form_changed = true;
                            }
                        }
                    }
                }
            }

            if create_form_changed {
                self.sync_create_form(cx);
                self.redraw(cx);
            }

            // If the cancel button was clicked, hide the room preview and return to default state.
            if cancel_button.clicked(actions) {
                self.state = AddRoomState::WaitingOnUserInput;
                room_alias_id_input.set_text(cx, "");
                room_alias_id_input.set_key_focus(cx);
                self.redraw(cx);
            }

            // If the join button was clicked, perform the appropriate action.
            if join_room_button.clicked(actions) {
                match (&self.join_function, &self.state) {
                    (
                        JoinButtonFunction::NavigateOrJoin,
                        AddRoomState::FetchedRoomPreview { frp, .. },
                    ) if matches!(frp.join_rule, Some(JoinRuleSummary::Public))
                        && !matches!(
                            frp.state,
                            Some(RoomState::Joined)
                                | Some(RoomState::Invited)
                                | Some(RoomState::Knocked)
                                | Some(RoomState::Banned)
                        ) =>
                    {
                        submit_async_request(MatrixRequest::JoinRoom {
                            room_id: frp.room_name_id.room_id().clone(),
                        });
                    }
                    (
                        JoinButtonFunction::NavigateOrJoin,
                        AddRoomState::FetchedRoomPreview { frp, .. }
                        | AddRoomState::Loaded { frp, .. },
                    ) => {
                        cx.action(AppStateAction::NavigateToRoom {
                            room_to_close: None,
                            destination_room: frp.clone().into(),
                        });
                    }
                    (
                        JoinButtonFunction::Knock,
                        AddRoomState::FetchedRoomPreview {
                            frp,
                            room_or_alias_id,
                            via,
                        },
                    ) => {
                        submit_async_request(MatrixRequest::Knock {
                            room_or_alias_id: frp
                                .canonical_alias
                                .clone()
                                .map_or_else(|| room_or_alias_id.clone(), Into::into),
                            reason: None,
                            server_names: via.clone(),
                        });
                    }
                    _ => {}
                }
            }

            // If the button was clicked or enter was pressed, try to parse the room address.
            let new_room_query = search_for_room_button
                .clicked(actions)
                .then(|| room_alias_id_input.text())
                .or_else(|| room_alias_id_input.returned(actions).map(|(t, _)| t));
            if let Some(t) = new_room_query {
                match parse_address(t.trim()) {
                    Ok((room_or_alias_id, via)) => {
                        self.state = AddRoomState::Parsed {
                            room_or_alias_id: room_or_alias_id.clone(),
                            via: via.clone(),
                        };
                        submit_async_request(MatrixRequest::GetRoomPreview {
                            room_or_alias_id,
                            via,
                        });
                    }
                    Err(e) => {
                        let err_str = format!(
                            "Could not parse the text as a valid room address.\nError: {e}."
                        );
                        enqueue_popup_notification(err_str.clone(), PopupKind::Error, None);
                        self.state = AddRoomState::ParseError(err_str);
                        room_alias_id_input.set_key_focus(cx);
                    }
                }
                self.redraw(cx);
            }

            // If we're waiting for the room preview to be fetched (i.e., in the Parsed state),
            // then check if we've received it via an action.
            if let AddRoomState::Parsed {
                room_or_alias_id,
                via,
            } = &self.state
            {
                for action in actions {
                    match action.downcast_ref() {
                        Some(RoomPreviewAction::Fetched(Ok(frp))) => {
                            let room_or_alias_id = room_or_alias_id.clone();
                            let via = via.clone();
                            self.state = AddRoomState::FetchedRoomPreview {
                                frp: frp.clone(),
                                room_or_alias_id,
                                via,
                            };
                            // Reset the buttons' hover states when they are first shown.
                            join_room_button.reset_hover(cx);
                            cancel_button.reset_hover(cx);
                            self.redraw(cx);
                            break;
                        }
                        Some(RoomPreviewAction::Fetched(Err(e))) => {
                            let err_str = format!("Failed to fetch room info.\n\nError: {e}.");
                            enqueue_popup_notification(err_str.clone(), PopupKind::Error, None);
                            self.state = AddRoomState::FetchError(err_str);
                            self.redraw(cx);
                            break;
                        }
                        _ => {}
                    }
                }
            }

            // If we've fetched and displayed the room preview, handle any responses to
            // the user clicking the join button (e.g., knocked on or joined the room/space).
            let mut transition_to_knocked = false;
            let mut transition_to_joined = false;
            if let AddRoomState::FetchedRoomPreview {
                frp,
                room_or_alias_id,
                ..
            } = &self.state
            {
                for action in actions {
                    match action.downcast_ref() {
                        Some(KnockResultAction::Knocked { room, .. })
                            if room.room_id() == frp.room_name_id.room_id() =>
                        {
                            let room_type = match room.room_type() {
                                Some(RoomType::Space) => "space",
                                _ => "room",
                            };
                            enqueue_popup_notification(
                                format!(
                                    "Successfully knocked on {room_type} {}.",
                                    frp.room_name_id
                                ),
                                PopupKind::Success,
                                Some(4.0),
                            );
                            transition_to_knocked = true;
                            break;
                        }
                        Some(KnockResultAction::Failed {
                            error,
                            room_or_alias_id: roai,
                        }) if room_or_alias_id == roai => {
                            enqueue_popup_notification(
                                format!("Failed to knock on room.\n\nError: {error}."),
                                PopupKind::Error,
                                None,
                            );
                            break;
                        }
                        _ => {}
                    }

                    match action.downcast_ref() {
                        Some(JoinRoomResultAction::Joined { room_id })
                            if room_id == frp.room_name_id.room_id() =>
                        {
                            let room_type = match &frp.room_type {
                                Some(RoomType::Space) => "space",
                                _ => "room",
                            };
                            enqueue_popup_notification(
                                format!("Successfully joined {room_type} {}.", frp.room_name_id),
                                PopupKind::Success,
                                Some(4.0),
                            );
                            transition_to_joined = true;
                            break;
                        }
                        Some(JoinRoomResultAction::Failed { room_id, error })
                            if room_id == frp.room_name_id.room_id() =>
                        {
                            enqueue_popup_notification(
                                format!("Failed to join room.\n\nError: {error}."),
                                PopupKind::Error,
                                None,
                            );
                            break;
                        }
                        _ => {}
                    }
                }
            }
            if transition_to_knocked {
                self.state.transition_to_knocked();
                self.redraw(cx);
            }
            if transition_to_joined {
                self.state.transition_to_joined();
                self.redraw(cx);
            }

            for action in actions {
                // If the room/space the user is searching for has been loaded from the homeserver
                // (e.g., by getting invited to it, or joining it in another client),
                // then update the state of
                if let Some(AppStateAction::RoomLoadedSuccessfully {
                    room_name_id,
                    is_invite,
                }) = action.downcast_ref()
                {
                    if self
                        .state
                        .fetched_room_preview()
                        .is_some_and(|frp| frp.room_name_id.room_id() == room_name_id.room_id())
                    {
                        self.state.transition_to_loaded(*is_invite);
                        self.redraw(cx);
                    }
                }
            }
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        let loading_room_view = self.view.view(cx, ids!(loading_room_view));
        let fetched_room_summary = self.view.view(cx, ids!(fetched_room_summary));
        let error_view = self.view.view(cx, ids!(error_view));

        match &self.state {
            AddRoomState::WaitingOnUserInput => {
                loading_room_view.set_visible(cx, false);
                fetched_room_summary.set_visible(cx, false);
                error_view.set_visible(cx, false);
            }
            AddRoomState::ParseError(err_str) | AddRoomState::FetchError(err_str) => {
                loading_room_view.set_visible(cx, false);
                fetched_room_summary.set_visible(cx, false);
                error_view.set_visible(cx, true);
                error_view.label(cx, ids!(error_text)).set_text(cx, err_str);
            }
            AddRoomState::Parsed {
                room_or_alias_id, ..
            } => {
                loading_room_view.set_visible(cx, true);
                loading_room_view
                    .label(cx, ids!(loading_text))
                    .set_text(cx, &format!("Fetching {room_or_alias_id}..."));
                fetched_room_summary.set_visible(cx, false);
                error_view.set_visible(cx, false);
            }
            ars @ AddRoomState::FetchedRoomPreview { frp, .. }
            | ars @ AddRoomState::Knocked { frp }
            | ars @ AddRoomState::Joined { frp }
            | ars @ AddRoomState::Loaded { frp, .. } => {
                loading_room_view.set_visible(cx, false);
                fetched_room_summary.set_visible(cx, true);
                error_view.set_visible(cx, false);

                // Populate the content of the fetched room preview.
                let room_avatar = fetched_room_summary.avatar(cx, ids!(room_avatar));
                match &frp.room_avatar {
                    FetchedRoomAvatar::Text(text) => {
                        room_avatar.show_text(cx, None, None, text);
                    }
                    FetchedRoomAvatar::Image(image_data) => {
                        let res = room_avatar.show_image(cx, None, |cx, img_ref| {
                            utils::load_png_or_jpg(&img_ref, cx, image_data)
                        });
                        if res.is_err() {
                            room_avatar.show_text(
                                cx,
                                None,
                                None,
                                frp.room_name_id.name_for_avatar().unwrap_or("?"),
                            );
                        }
                    }
                }

                let (room_or_space_lc, room_or_space_uc) = match &frp.room_type {
                    Some(RoomType::Space) => ("space", "Space"),
                    _ => ("room", "Room"),
                };
                let room_name = fetched_room_summary.label(cx, ids!(room_name));
                match frp.room_name_id.name_for_avatar() {
                    Some(n) => room_name.set_text(cx, n),
                    _ => room_name.set_text(
                        cx,
                        &format!(
                            "Unnamed {room_or_space_uc}, ID: {}",
                            frp.room_name_id.room_id()
                        ),
                    ),
                }

                fetched_room_summary
                    .label(cx, ids!(subsection_alias_id))
                    .set_text(cx, &format!("Main {room_or_space_uc} Alias and ID"));
                fetched_room_summary.label(cx, ids!(room_alias)).set_text(
                    cx,
                    &format!(
                        "Alias: {}",
                        frp.canonical_alias
                            .as_ref()
                            .map_or("not set", |a| a.as_str())
                    ),
                );
                fetched_room_summary
                    .label(cx, ids!(room_id))
                    .set_text(cx, &format!("ID: {}", frp.room_name_id.room_id().as_str()));
                fetched_room_summary
                    .label(cx, ids!(subsection_topic))
                    .set_text(cx, &format!("{room_or_space_uc} Topic"));
                fetched_room_summary
                    .html(cx, ids!(room_topic))
                    .set_text(cx, frp.topic.as_deref().unwrap_or("<i>No topic set</i>"));

                let room_summary = fetched_room_summary.label(cx, ids!(room_summary));
                let join_room_button = fetched_room_summary.button(cx, ids!(join_room_button));
                let join_function = match (&frp.state, &frp.join_rule) {
                    (Some(RoomState::Joined), _) => {
                        room_summary.set_text(
                            cx,
                            &format!("You have already joined this {room_or_space_lc}."),
                        );
                        join_room_button.set_text(cx, &format!("Go to {room_or_space_lc}"));
                        JoinButtonFunction::NavigateOrJoin
                    }
                    (Some(RoomState::Banned), _) => {
                        room_summary.set_text(
                            cx,
                            &format!("You have been banned from this {room_or_space_lc}."),
                        );
                        join_room_button.set_text(cx, "Cannot join until un-banned");
                        JoinButtonFunction::None
                    }
                    (Some(RoomState::Invited), _) => {
                        room_summary.set_text(
                            cx,
                            &format!("You have already been invited to this {room_or_space_lc}."),
                        );
                        join_room_button.set_text(cx, "Go to invitation");
                        JoinButtonFunction::NavigateOrJoin
                    }
                    (Some(RoomState::Knocked), _) => {
                        room_summary.set_text(
                            cx,
                            &format!("You have already knocked on this {room_or_space_lc}."),
                        );
                        join_room_button.set_text(cx, "Knock again (be nice!)");
                        JoinButtonFunction::Knock
                    }
                    (Some(RoomState::Left), join_rule) => {
                        room_summary
                            .set_text(cx, &format!("You previously left this {room_or_space_lc}."));
                        let (join_room_text, join_function) = match join_rule {
                            Some(JoinRuleSummary::Public) => (
                                format!("Re-join this {room_or_space_lc}"),
                                JoinButtonFunction::NavigateOrJoin,
                            ),
                            Some(JoinRuleSummary::Invite) => (
                                format!("Re-joining {room_or_space_lc} requires an invite"),
                                JoinButtonFunction::None,
                            ),
                            Some(JoinRuleSummary::Knock | JoinRuleSummary::KnockRestricted(_)) => (
                                format!("Knock to re-join {room_or_space_lc}"),
                                JoinButtonFunction::Knock,
                            ),
                            // TODO: handle this after we update matrix-sdk to the new `JoinRule` enum.
                            Some(JoinRuleSummary::Restricted(_)) => (
                                format!(
                                    "Re-joining {room_or_space_lc} requires an invite or other room membership"
                                ),
                                JoinButtonFunction::None,
                            ),
                            _ => (
                                format!("Not allowed to re-join this {room_or_space_lc}"),
                                JoinButtonFunction::None,
                            ),
                        };
                        join_room_button.set_text(cx, &join_room_text);
                        join_function
                    }
                    // This room is not yet known to the user.
                    (None, join_rule) => {
                        let direct = if frp.is_direct == Some(true) {
                            "direct"
                        } else {
                            "regular"
                        };
                        room_summary.set_text(
                            cx,
                            &format!(
                                "This is a {direct} {room_or_space_lc} with {} {}.",
                                frp.num_joined_members,
                                match frp.num_joined_members {
                                    1 => "member",
                                    _ => "members",
                                },
                            ),
                        );

                        let (join_room_text, join_function) = match join_rule {
                            Some(JoinRuleSummary::Public) => (
                                format!("Join this {room_or_space_lc}"),
                                JoinButtonFunction::NavigateOrJoin,
                            ),
                            Some(JoinRuleSummary::Invite) => (
                                format!("Joining {room_or_space_lc} requires an invite"),
                                JoinButtonFunction::None,
                            ),
                            Some(JoinRuleSummary::Knock | JoinRuleSummary::KnockRestricted(_)) => (
                                format!("Knock to join {room_or_space_lc}"),
                                JoinButtonFunction::Knock,
                            ),
                            // TODO: handle this after we update matrix-sdk to the new `JoinRule` enum.
                            Some(JoinRuleSummary::Restricted(_)) => (
                                format!(
                                    "Joining {room_or_space_lc} requires an invite or other room membership"
                                ),
                                JoinButtonFunction::None,
                            ),
                            _ => (
                                format!("Not allowed to join this {room_or_space_lc}"),
                                JoinButtonFunction::None,
                            ),
                        };
                        join_room_button.set_text(cx, &join_room_text);
                        join_function
                    }
                };

                match ars {
                    AddRoomState::FetchedRoomPreview { .. } => {
                        join_room_button
                            .set_enabled(cx, !matches!(join_function, JoinButtonFunction::None));
                        self.join_function = join_function;
                    }
                    AddRoomState::Knocked { .. } => {
                        room_summary.set_text(cx, &format!("You have knocked on this {room_or_space_lc} and must now wait for someone to invite you in."));
                        join_room_button.set_text(cx, "Successfully knocked!");
                        join_room_button.set_enabled(cx, false);
                    }
                    AddRoomState::Joined { .. } => {
                        room_summary.set_text(cx, &format!("You have joined this {room_or_space_lc}. It is now being loaded from the homeserver; please wait..."));
                        join_room_button.set_text(cx, "Successfully joined!");
                        join_room_button.set_enabled(cx, false);
                    }
                    AddRoomState::Loaded { is_invite, .. } => {
                        let verb = if *is_invite {
                            "been invited to"
                        } else {
                            "fully joined"
                        };
                        room_summary
                            .set_text(cx, &format!("You have {verb} this {room_or_space_lc}."));
                        let adj = if *is_invite { "invited" } else { "joined" };
                        join_room_button.set_text(cx, &format!("Go to {adj} {room_or_space_lc}"));
                        join_room_button.set_enabled(cx, true);
                        self.join_function = JoinButtonFunction::NavigateOrJoin;
                    }
                    _ => {}
                }
            }
        }

        self.view.draw_walk(cx, scope, walk)
    }
}

/// The function to perform when the user clicks the join button in the fetched room preview.
enum JoinButtonFunction {
    None,
    /// Navigate to an already-known room/space, or join it if possible.
    NavigateOrJoin,
    /// Knock on (request to join) a room/space.
    Knock,
}

/// Actions sent from the backend task as a result of a [`MatrixRequest::Knock`].
#[derive(Debug)]
pub enum KnockResultAction {
    /// The user successfully knocked on the room/space.
    Knocked {
        /// The room alias/ID that was originally sent with the knock request.
        room_or_alias_id: OwnedRoomOrAliasId,
        /// The room that was knocked on.
        room: matrix_sdk::Room,
    },
    /// There was an error attempting to knock on the room.
    Failed {
        /// The room alias/ID that was originally sent with the knock request.
        room_or_alias_id: OwnedRoomOrAliasId,
        error: matrix_sdk::Error,
    },
}

/// Tries to extract a room address (Alias or ID) from the given text.
///
/// This function is quite flexible and will attempt to parse `text` as:
/// * A Room ID (with a leading `!`).
/// * A Room Alias (with a leading `#`).
/// * A `https://matrix.to` URI, which includes either a room alias, or a room ID plus `via` servers.
/// * A `matrix:` scheme URI, which is similar to above.
fn parse_address(text: &str) -> Result<(OwnedRoomOrAliasId, Vec<OwnedServerName>), IdParseError> {
    match OwnedRoomOrAliasId::try_from(text) {
        Ok(room_or_alias_id) => Ok((room_or_alias_id, Vec::new())),
        Err(e) => {
            let uri_result = MatrixToUri::parse(text)
                .map(|uri| (uri.id().clone(), uri.via().to_owned()))
                .or_else(|_| {
                    MatrixUri::parse(text).map(|uri| (uri.id().clone(), uri.via().to_owned()))
                });

            if let Ok((matrix_id, via)) = uri_result {
                if let Some(room_or_alias_id) = match matrix_id {
                    MatrixId::Room(room_id) => Some(room_id.into()),
                    MatrixId::RoomAlias(alias) => Some(alias.into()),
                    MatrixId::Event(room_or_alias_id, _) => Some(room_or_alias_id),
                    _ => None,
                } {
                    return Ok((room_or_alias_id, via));
                }
            }
            Err(e)
        }
    }
}
