//! A top-level view for adding (joining) or exploring new rooms and spaces.


use makepad_widgets::*;
use matrix_sdk::RoomState;
use ruma::{IdParseError, MatrixToUri, MatrixUri, OwnedRoomId, OwnedRoomOrAliasId, OwnedServerName, OwnedUserId, matrix_uri::MatrixId, room::{JoinRuleSummary, RoomType}};

use crate::{
    app::{AppState, AppStateAction},
    home::{invite_screen::JoinRoomResultAction, rooms_list::RoomsListRef},
    i18n::{AppLanguage, tr_fmt, tr_key},
    profile::user_profile::UserProfile,
    room::{BasicRoomDetails, FetchedRoomAvatar, FetchedRoomPreview, RoomPreviewAction},
    shared::{
        avatar::{AvatarState, AvatarWidgetRefExt},
        popup_list::{PopupKind, enqueue_popup_notification},
        styles::COLOR_FG_DANGER_RED,
    },
    sliding_sync::{DirectMessageRoomAction, MatrixRequest, RoomPreviewResponseMode, current_user_id, submit_async_request},
    space_service_sync::SpaceRequest,
    utils::{self, RoomNameId},
};

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*


    mod.widgets.CreateRoomForm = set_type_default() do #(CreateRoomForm::register_widget(vm)) {
        ..mod.widgets.View

        width: Fill
        height: Fit
        flow: Down

        create_room_help := Label {
            width: Fill, height: Fit
            flow: Flow.Right{wrap: true}
            draw_text +: {
                color: (MESSAGE_TEXT_COLOR),
                text_style: MESSAGE_TEXT_STYLE { font_size: 11 },
            }
            text: "Set a room name, topic, privacy, and encryption options."
        }

        create_room_view := View {
            width: Fill
            height: Fit
            margin: Inset{ top: 6, bottom: 10 }
            spacing: 8
            flow: Down

            create_room_name_input := RobrixTextInput {
                margin: Inset{left: 5, right: 5}
                padding: Inset{left: 12, right: 12, top: 11, bottom: 0}
                width: Fill
                height: 40
                empty_text: "Enter the new room name..."
            }

            create_room_topic_input := RobrixTextInput {
                margin: Inset{left: 5, right: 5}
                padding: Inset{left: 12, right: 12, top: 11, bottom: 0}
                width: Fill
                height: 40
                empty_text: "Topic (optional)"
            }

            create_room_visibility_selector_button := RoundedView {
                width: Fill, height: Fit
                flow: Right
                align: Align{y: 0.5}
                padding: Inset{left: 12, right: 8, top: 8, bottom: 8}
                margin: Inset{left: 5, right: 5, top: 2, bottom: 2}
                cursor: MouseCursor.Hand
                show_bg: true
                draw_bg +: {
                    color: (COLOR_PRIMARY)
                    border_radius: (RADIUS_SM)
                    border_size: 1.0
                    border_color: (COLOR_DROPDOWN_BORDER)
                }

                create_room_visibility_selector_label := Label {
                    width: Fill, height: Fit
                    draw_text +: {
                        color: (COLOR_DROPDOWN_TEXT)
                        text_style: REGULAR_TEXT { font_size: 11 }
                    }
                    text: "Private room (invite only)"
                }

                create_room_visibility_arrow := ExpandArrow {
                    width: 14, height: 14
                    draw_bg +: {
                        color: instance((COLOR_DROPDOWN_ARROW))
                    }
                }
            }

            create_room_visibility_popup := RoundedView {
                visible: false
                width: Fill, height: Fit
                flow: Down
                padding: Inset{top: 4, bottom: 4}
                margin: Inset{left: 5, right: 5}
                show_bg: true
                new_batch: true
                draw_bg +: {
                    color: (COLOR_PRIMARY)
                    border_radius: (RADIUS_MD)
                    border_size: 1.0
                    border_color: (COLOR_DROPDOWN_POPUP_BORDER)
                }

                create_room_visibility_private_option := View {
                    width: Fill, height: 36
                    flow: Right
                    align: Align{y: 0.5}
                    padding: Inset{left: 12, right: 12}
                    cursor: MouseCursor.Hand
                    show_bg: true
                    draw_bg +: { color: #0000 }

                    create_room_visibility_private_option_label := Label {
                        width: Fit, height: Fit
                        draw_text +: {
                            color: (COLOR_DROPDOWN_TEXT)
                            text_style: REGULAR_TEXT { font_size: 11 }
                        }
                        text: "Private room (invite only)"
                    }
                }

                create_room_visibility_public_option := View {
                    width: Fill, height: 36
                    flow: Right
                    align: Align{y: 0.5}
                    padding: Inset{left: 12, right: 12}
                    cursor: MouseCursor.Hand
                    show_bg: true
                    draw_bg +: { color: #0000 }

                    create_room_visibility_public_option_label := Label {
                        width: Fit, height: Fit
                        draw_text +: {
                            color: (COLOR_DROPDOWN_TEXT)
                            text_style: REGULAR_TEXT { font_size: 11 }
                        }
                        text: "Public room"
                    }
                }
            }

            create_room_visibility_hint := Label {
                width: Fill, height: Fit
                margin: Inset{left: 5, right: 5}
                flow: Flow.Right{wrap: true}
                draw_text +: {
                    color: (MESSAGE_TEXT_COLOR),
                    text_style: MESSAGE_TEXT_STYLE { font_size: 11 },
                }
                text: "Only people invited will be able to find and join this room."
            }

            create_room_encryption_row := View {
                width: Fill
                height: Fit
                margin: Inset{left: 5, right: 5, top: 2}
                spacing: 10
                align: Align{y: 0.5}
                flow: Right

                create_room_encrypted_toggle := Toggle {
                    width: Fit
                    height: Fit
                    padding: Inset{top: 4, right: 4, bottom: 4, left: 4}
                    text: ""
                    active: true
                    draw_bg +: {
                        size: 20.0
                        color_active: (COLOR_ACTIVE_PRIMARY)
                        border_color_active: (COLOR_ACTIVE_PRIMARY)
                        mark_color_active: #fff
                    }
                }

                create_room_encryption_text := View {
                    width: Fill
                    height: Fit
                    flow: Down
                    spacing: 4

                    create_room_encryption_label := Label {
                        width: Fill, height: Fit
                        draw_text +: {
                            color: (COLOR_TEXT),
                            text_style: TITLE_TEXT { font_size: 11.5 },
                        }
                        text: "Enable end-to-end encryption"
                    }

                    create_room_encryption_hint := Label {
                        width: Fill, height: Fit
                        flow: Flow.Right{wrap: true}
                        draw_text +: {
                            color: (MESSAGE_TEXT_COLOR),
                            text_style: MESSAGE_TEXT_STYLE { font_size: 11 },
                        }
                        text: "You can't disable this later."
                    }
                }
            }

            create_room_feedback := View {
                visible: false
                width: Fill
                height: Fit
                margin: Inset{left: 5, right: 5, top: 6}
                spacing: 8
                align: Align{y: 0.5}
                flow: Right

                create_room_feedback_spinner_wrap := View {
                    width: Fit
                    height: Fit

                    create_room_feedback_spinner := LoadingSpinner {
                        width: 16
                        height: 16
                        draw_bg +: {
                            color: (COLOR_ACTIVE_PRIMARY)
                            border_size: 2.0
                        }
                    }
                }

                create_room_feedback_label := Label {
                    width: Fill
                    height: Fit
                    flow: Flow.Right{wrap: true}
                    draw_text +: {
                        color: (MESSAGE_TEXT_COLOR),
                        text_style: MESSAGE_TEXT_STYLE { font_size: 11 },
                    }
                }
            }

            create_room_button_row := View {
                width: Fill
                height: Fit
                margin: Inset{top: 4}
                padding: Inset{left: 5}
                flow: Right

                create_room_button := RobrixPositiveIconButton {
                    width: Fit
                    padding: Inset{top: 10, bottom: 10, left: 12, right: 14}
                    height: 40
                    draw_icon.svg: (ICON_ADD)
                    icon_walk: Walk{width: 16, height: 16}
                    text: "Create room"
                }
            }
        }
    }


    // The main view that allows the user to add (join) or explore new rooms/spaces.
    mod.widgets.AddRoomScreen = #(AddRoomScreen::register_widget(vm)) {
        ..mod.widgets.ScrollYView

        width: Fill, height: Fill,
        flow: Down,
        padding: Inset{top: 5, left: 15, right: 15, bottom: 0},

        title := TitleLabel {
            text: "Add/Explore Rooms and Spaces"
            draw_text +: {
                text_style: theme.font_regular {font_size: 18},
            }
        }
        
        LineH { padding: 10, margin: Inset{top: 10, right: 2} }

        quick_actions_view := View {
            width: Fill
            height: Fit
            margin: Inset{ top: 8, bottom: 10 }
            padding: Inset{left: 5, right: 5}
            flow: Flow.Right{wrap: true}
            spacing: 10

            new_room_button := RobrixPositiveIconButton {
                width: Fit
                padding: Inset{top: 10, bottom: 10, left: 12, right: 14}
                height: 40
                draw_icon.svg: (ICON_ADD)
                icon_walk: Walk{width: 16, height: 16}
                text: "New room"
            }

            start_chat_button := RobrixPositiveIconButton {
                width: Fit
                padding: Inset{top: 10, bottom: 10, left: 12, right: 14}
                height: 40
                draw_icon.svg: (ICON_ADD_USER)
                icon_walk: Walk{width: 16, height: 16}
                text: "Start chat"
            }
        }

        LineH { padding: 10, margin: Inset{right: 2} }

        join_existing_label := SubsectionLabel {
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

        View {
            width: Fill
            height: 20
        }
        
    }

    mod.widgets.CreateRoomModal = #(CreateRoomModal::register_widget(vm)) {
        width: Fit
        height: Fit

        RoundedView {
            width: 400
            height: Fit
            align: Align{x: 0.5}
            flow: Down
            padding: Inset{top: 14, right: 14, bottom: 14, left: 14}

            show_bg: true
            draw_bg +: {
                color: #fff
                border_radius: 24.0
            }

            title_view := View {
                width: Fill
                height: Fit
                padding: Inset{top: 4, bottom: 14}
                align: Align{y: 0.5}
                flow: Right

                title := Label {
                    width: Fill
                    height: Fit
                    flow: Flow.Right{wrap: true}
                    draw_text +: {
                        text_style: TITLE_TEXT {font_size: 15}
                        color: #000
                    }
                    text: "Create New Room"
                }
            }

            subtitle := Label {
                width: Fill
                height: Fit
                margin: Inset{bottom: 10}
                flow: Flow.Right{wrap: true}
                draw_text +: {
                    color: (MESSAGE_TEXT_COLOR)
                    text_style: MESSAGE_TEXT_STYLE { font_size: 11 }
                }
                text: "Set a room name, topic, privacy, and encryption options."
            }

            create_room_form := mod.widgets.CreateRoomForm {}

            buttons_view := View {
                width: Fill
                height: Fit
                flow: Flow.Right{wrap: true}
                padding: Inset{top: 16, bottom: 5}
                align: Align{x: 1.0, y: 0.5}
                spacing: 10

                cancel_button := RobrixNeutralIconButton {
                    width: 132
                    align: Align{x: 0.5, y: 0.5}
                    padding: 12
                    draw_icon.svg: (ICON_FORBIDDEN)
                    icon_walk: Walk{width: 16, height: 16, margin: Inset{left: -2, right: -1} }
                    text: "Cancel"
                }

                create_button := RobrixPositiveIconButton {
                    width: 152
                    align: Align{x: 0.5, y: 0.5}
                    padding: 12
                    draw_icon.svg: (ICON_ADD)
                    icon_walk: Walk{width: 16, height: 16, margin: Inset{left: -2, right: -1} }
                    text: "Create room"
                }
            }
        }
    }

    mod.widgets.StartChatModal = #(StartChatModal::register_widget(vm)) {
        width: Fit
        height: Fit

        RoundedView {
            width: 400
            height: Fit
            align: Align{x: 0.5}
            flow: Down
            padding: Inset{top: 14, right: 14, bottom: 14, left: 14}

            show_bg: true
            draw_bg +: {
                color: #fff
                border_radius: 24.0
            }

            title_view := View {
                width: Fill
                height: Fit
                padding: Inset{top: 4, bottom: 14}
                align: Align{y: 0.5}
                flow: Right

                title := Label {
                    width: Fill
                    height: Fit
                    flow: Flow.Right{wrap: true}
                    draw_text +: {
                        text_style: TITLE_TEXT {font_size: 15}
                        color: #000
                    }
                    text: "Direct Messages"
                }
            }

            subtitle := Label {
                width: Fill
                height: Fit
                margin: Inset{bottom: 10}
                flow: Flow.Right{wrap: true}
                draw_text +: {
                    color: (MESSAGE_TEXT_COLOR)
                    text_style: MESSAGE_TEXT_STYLE { font_size: 11 }
                }
                text: "Start a conversation by entering a Matrix user ID."
            }

            search_row := View {
                width: Fill
                height: Fit
                spacing: 8
                align: Align{y: 0.5}
                margin: Inset{left: 5, right: 5}
                flow: Right

                chat_user_id_input := RobrixTextInput {
                    padding: Inset{left: 12, right: 12, top: 11, bottom: 0}
                    width: Fill
                    height: 40
                    empty_text: "Search by Matrix user ID..."
                }

                go_button := RobrixNeutralIconButton {
                    width: 82
                    height: 40
                    align: Align{x: 0.5, y: 0.5}
                    padding: 10
                    text: "Go"
                }
            }

            chat_hint := Label {
                width: Fill
                height: Fit
                margin: Inset{left: 5, right: 5, top: 8, bottom: 6}
                flow: Flow.Right{wrap: true}
                draw_text +: {
                    color: (MESSAGE_TEXT_COLOR)
                    text_style: MESSAGE_TEXT_STYLE { font_size: 11 }
                }
                text: "A direct message room will be created if one does not already exist."
            }

            buttons_view := View {
                width: Fill
                height: Fit
                flow: Right
                padding: Inset{top: 10, bottom: 5}
                align: Align{x: 1.0, y: 0.5}
                spacing: 10

                cancel_button := RobrixNeutralIconButton {
                    width: 132
                    align: Align{x: 0.5, y: 0.5}
                    padding: 12
                    draw_icon.svg: (ICON_FORBIDDEN)
                    icon_walk: Walk{width: 16, height: 16, margin: Inset{left: -2, right: -1} }
                    text: "Cancel"
                }
            }
        }
    }
}

#[derive(Script, ScriptHook, Widget)]
pub struct AddRoomScreen {
    #[deref] view: View,
    #[rust] state: AddRoomState,
    /// The function to perform when the user clicks the `join_room_button`.
    #[rust(JoinButtonFunction::None)] join_function: JoinButtonFunction,
    #[rust] app_language: AppLanguage,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CreateRoomContext {
    AddRoomPage,
    SpaceLobbyModal,
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
    Knocked {
        frp: FetchedRoomPreview,
    },
    /// We successfully joined the room or space, and are waiting for it
    /// to be loaded from the homeserver.
    Joined {
        frp: FetchedRoomPreview,
    },
    /// The fetched room or space has been loaded from the homeserver,
    /// so we can allow the user to jump to it via the `join_room_button`.
    Loaded {
        frp: FetchedRoomPreview,
        is_invite: bool,
    }
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
            Self::FetchedRoomPreview { frp, .. }
            | Self::Joined { frp }
            | Self::Knocked { frp } => {
                *self = Self::Loaded { frp, is_invite };
            }
            _ => {
                *self = prev;
            }
        }
    }
}

#[derive(Script, ScriptHook, Widget)]
pub struct CreateRoomForm {
    #[deref] view: View,
    #[rust(CreateRoomContext::AddRoomPage)] context: CreateRoomContext,
    #[rust(false)] creating_room: bool,
    #[rust(None)] pending_created_room: Option<RoomNameId>,
    #[rust(None)] fixed_parent_space_id: Option<OwnedRoomId>,
    #[rust(false)] visibility_popup_visible: bool,
    #[rust(false)] create_public_room: bool,
    #[rust(true)] create_encrypted_room: bool,
    #[rust] app_language: AppLanguage,
}

impl Widget for CreateRoomForm {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        let app_language = scope.data.get::<AppState>()
            .map(|app_state| app_state.app_language)
            .unwrap_or_default();
        if self.app_language != app_language {
            self.set_app_language(cx, app_language);
        }
        self.view.handle_event(cx, event, scope);
        self.handle_visibility_dropdown_event(cx, event);
        self.widget_match_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        let create_room_text_is_empty = self.view
            .text_input(cx, ids!(create_room_name_input))
            .text()
            .trim()
            .is_empty();
        self.view.button(cx, ids!(create_room_button))
            .set_enabled(cx, !self.is_busy() && !create_room_text_is_empty);

        self.sync_mode_views(cx);

        self.view.draw_walk(cx, scope, walk)
    }
}

impl WidgetMatchEvent for CreateRoomForm {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions, _scope: &mut Scope) {
        let create_room_name_input = self.view.text_input(cx, ids!(create_room_name_input));
        let create_room_topic_input = self.view.text_input(cx, ids!(create_room_topic_input));
        let create_room_button = self.view.button(cx, ids!(create_room_button));
        let create_room_encrypted_toggle = self.view.check_box(cx, ids!(create_room_encrypted_toggle));

        if let Some(text) = create_room_name_input.changed(actions) {
            if !self.is_busy() {
                self.clear_feedback(cx);
            }
            create_room_button.set_enabled(cx, !self.is_busy() && !text.trim().is_empty());
        }

        if let Some(enabled) = create_room_encrypted_toggle.changed(actions) {
            self.create_encrypted_room = enabled;
        }

        let create_room_request = create_room_button.clicked(actions)
            || create_room_name_input.returned(actions).is_some()
            || create_room_topic_input.returned(actions).is_some();
        if create_room_request {
            let _ = self.submit(cx);
        }

        for action in actions {
            if let Some(create_room_action) = action.downcast_ref() {
                match create_room_action {
                    CreateRoomAction::Created { room_name_id, parent_space_id, space_link_error, context }
                        if context == &self.context =>
                    {
                        self.creating_room = false;
                        create_room_name_input.set_text(cx, "");
                        create_room_topic_input.set_text(cx, "");
                        create_room_button.set_enabled(cx, false);

                        if let Some(space_id) = parent_space_id {
                            refresh_space_children(cx, space_id);
                        }

                        let room_name_text = room_name_id.to_string();
                        let mut popup_message = tr_fmt(self.app_language, "add_room.popup.created_room_success", &[
                            ("room_name", room_name_text.as_str()),
                        ]);
                        let popup_kind = if let Some(link_error) = space_link_error {
                            popup_message.push_str(&tr_fmt(self.app_language, "add_room.popup.created_room_space_link_suffix", &[
                                ("error", link_error.as_str()),
                            ]));
                            PopupKind::Warning
                        } else {
                            PopupKind::Success
                        };
                        enqueue_popup_notification(popup_message, popup_kind, Some(5.0));

                        if cx.has_global::<RoomsListRef>()
                            && cx.get_global::<RoomsListRef>().is_room_loaded(room_name_id.room_id())
                        {
                            self.clear_feedback(cx);
                            if self.context == CreateRoomContext::SpaceLobbyModal {
                                cx.action(CreateRoomModalAction::Close);
                            }
                            cx.action(AppStateAction::NavigateToRoom {
                                room_to_close: None,
                                destination_room: BasicRoomDetails::Name(room_name_id.clone()),
                            });
                        } else {
                            self.pending_created_room = Some(room_name_id.clone());
                            let feedback_text = match (parent_space_id.as_ref(), space_link_error.as_ref()) {
                                (Some(_), None) => tr_key(self.app_language, "add_room.feedback.room_created_syncing"),
                                (Some(_), Some(_)) => tr_key(self.app_language, "add_room.feedback.room_created_link_failed_opening"),
                                (None, _) => tr_key(self.app_language, "add_room.feedback.room_created_opening"),
                            };
                            self.set_feedback(cx, feedback_text, true, false);
                        }

                        self.view.redraw(cx);
                    }
                    CreateRoomAction::Failed { room_name, error, context }
                        if context == &self.context =>
                    {
                        self.creating_room = false;
                        create_room_button.set_enabled(cx, !create_room_name_input.text().trim().is_empty());
                        self.set_feedback(
                            cx,
                            &{
                                let error_text = error.to_string();
                                tr_fmt(self.app_language, "add_room.feedback.create_room_failed", &[
                                    ("error", error_text.as_str()),
                                ])
                            },
                            false,
                            true,
                        );
                        enqueue_popup_notification(
                            {
                                let error_text = error.to_string();
                                tr_fmt(self.app_language, "add_room.popup.create_room_failed", &[
                                    ("room_name", room_name.as_str()),
                                    ("error", error_text.as_str()),
                                ])
                            },
                            PopupKind::Error,
                            None,
                        );
                        self.view.redraw(cx);
                    }
                    _ => {}
                }
            }

            if let Some(AppStateAction::RoomLoadedSuccessfully { room_name_id, .. }) = action.downcast_ref()
                && self.pending_created_room.as_ref().is_some_and(|pending| pending.room_id() == room_name_id.room_id())
            {
                self.pending_created_room = None;
                self.clear_feedback(cx);
                if self.context == CreateRoomContext::SpaceLobbyModal {
                    cx.action(CreateRoomModalAction::Close);
                }
                cx.action(AppStateAction::NavigateToRoom {
                    room_to_close: None,
                    destination_room: BasicRoomDetails::Name(room_name_id.clone()),
                });
            }
        }
    }
}

impl CreateRoomForm {
    fn can_submit(&self, cx: &mut Cx) -> bool {
        !self.is_busy()
            && !self.view
                .text_input(cx, ids!(create_room_name_input))
                .text()
                .trim()
                .is_empty()
    }

    fn is_busy(&self) -> bool {
        self.creating_room || self.pending_created_room.is_some()
    }

    fn set_app_language(&mut self, cx: &mut Cx, app_language: AppLanguage) {
        self.app_language = app_language;
        self.view.text_input(cx, ids!(create_room_name_input))
            .set_empty_text(cx, tr_key(self.app_language, "add_room.create_room.input.placeholder").to_string());
        self.view.text_input(cx, ids!(create_room_topic_input))
            .set_empty_text(cx, tr_key(self.app_language, "add_room.create_room.topic.placeholder").to_string());
        self.view.label(cx, ids!(create_room_visibility_private_option_label))
            .set_text(cx, tr_key(self.app_language, "add_room.create_room.visibility.option.private"));
        self.view.label(cx, ids!(create_room_visibility_public_option_label))
            .set_text(cx, tr_key(self.app_language, "add_room.create_room.visibility.option.public"));
        self.view.label(cx, ids!(create_room_encryption_label))
            .set_text(cx, tr_key(self.app_language, "add_room.create_room.encryption.label"));
        self.view.label(cx, ids!(create_room_encryption_hint))
            .set_text(cx, tr_key(self.app_language, "add_room.create_room.encryption.hint"));
        self.view.button(cx, ids!(create_room_button))
            .set_text(cx, tr_key(self.app_language, "add_room.create_room.button.create"));
        self.update_visibility_selector_text(cx);
        self.sync_mode_views(cx);
    }

    fn set_visibility_popup_visible(&mut self, cx: &mut Cx, visible: bool) {
        self.visibility_popup_visible = visible;
        self.view.view(cx, ids!(create_room_visibility_popup)).set_visible(cx, visible);
    }

    fn set_create_room_public(&mut self, cx: &mut Cx, is_public: bool) {
        self.create_public_room = is_public;
        self.update_visibility_selector_text(cx);
    }

    fn update_visibility_selector_text(&mut self, cx: &mut Cx) {
        let selector_text = if self.create_public_room {
            tr_key(self.app_language, "add_room.create_room.visibility.option.public")
        } else {
            tr_key(self.app_language, "add_room.create_room.visibility.option.private")
        };
        self.view.label(cx, ids!(create_room_visibility_selector_label))
            .set_text(cx, selector_text);

        let hint_text = if self.create_public_room {
            tr_key(self.app_language, "add_room.create_room.visibility.hint.public")
        } else {
            tr_key(self.app_language, "add_room.create_room.visibility.hint.private")
        };
        self.view.label(cx, ids!(create_room_visibility_hint))
            .set_text(cx, hint_text);
    }

    fn handle_visibility_dropdown_event(&mut self, cx: &mut Cx, event: &Event) {
        let selector = self.view.view(cx, ids!(create_room_visibility_selector_button));
        if let Hit::FingerUp(fe) = event.hits(cx, selector.area()) {
            if fe.is_over && fe.was_tap() {
                self.set_visibility_popup_visible(cx, !self.visibility_popup_visible);
                self.view.redraw(cx);
            }
        }

        if !self.visibility_popup_visible {
            return;
        }

        let visibility_options: &[(&[LiveId], bool)] = &[
            (&[live_id!(create_room_visibility_private_option)], false),
            (&[live_id!(create_room_visibility_public_option)], true),
        ];

        for &(id_path, is_public) in visibility_options {
            let item_view = self.view.view(cx, id_path);
            if let Hit::FingerUp(fe) = event.hits(cx, item_view.area()) {
                if fe.is_over && fe.was_tap() {
                    self.set_create_room_public(cx, is_public);
                    self.set_visibility_popup_visible(cx, false);
                    self.view.redraw(cx);
                    break;
                }
            }
        }
    }

    fn set_feedback(&mut self, cx: &mut Cx, text: &str, show_spinner: bool, is_error: bool) {
        self.view.view(cx, ids!(create_room_feedback)).set_visible(cx, true);
        self.view.view(cx, ids!(create_room_feedback_spinner_wrap))
            .set_visible(cx, show_spinner);
        let mut feedback_label = self.view.label(cx, ids!(create_room_feedback_label));
        feedback_label.set_text(cx, text);
        script_apply_eval!(cx, feedback_label, {
            draw_text +: {
                color: #(
                    if is_error {
                        COLOR_FG_DANGER_RED
                    } else {
                        vec4(0.2, 0.2, 0.2, 1.0)
                    }
                )
            }
        });
    }

    fn clear_feedback(&mut self, cx: &mut Cx) {
        self.view.view(cx, ids!(create_room_feedback)).set_visible(cx, false);
        self.view.label(cx, ids!(create_room_feedback_label)).set_text(cx, "");
    }

    fn submit(&mut self, cx: &mut Cx) -> bool {
        if !self.can_submit(cx) {
            return false;
        }

        let room_name = self.view.text_input(cx, ids!(create_room_name_input)).text();
        let room_name = room_name.trim();
        let room_topic = self.view.text_input(cx, ids!(create_room_topic_input)).text();
        let room_topic = room_topic.trim();
        let parent_space_id = self.selected_parent_space_id();

        self.creating_room = true;
        self.set_feedback(cx, tr_key(self.app_language, "add_room.feedback.creating_room"), true, false);
        submit_async_request(MatrixRequest::CreateRoom {
            room_name: room_name.to_owned(),
            topic: (!room_topic.is_empty()).then_some(room_topic.to_owned()),
            is_public: self.create_public_room,
            is_encrypted: self.create_encrypted_room,
            parent_space_id,
            context: self.context.clone(),
        });
        self.view.redraw(cx);
        true
    }

    pub fn prepare(
        &mut self,
        cx: &mut Cx,
        preferred_parent_space_id: Option<OwnedRoomId>,
        context: CreateRoomContext,
        clear_room_name: bool,
    ) {
        self.context = context;
        self.creating_room = false;
        self.pending_created_room = None;
        self.fixed_parent_space_id = (self.context == CreateRoomContext::SpaceLobbyModal)
            .then_some(preferred_parent_space_id)
            .flatten();
        self.visibility_popup_visible = false;
        self.create_public_room = false;
        self.create_encrypted_room = true;

        let create_room_name_input = self.view.text_input(cx, ids!(create_room_name_input));
        let create_room_topic_input = self.view.text_input(cx, ids!(create_room_topic_input));
        let create_room_button = self.view.button(cx, ids!(create_room_button));
        let create_room_encrypted_toggle = self.view.check_box(cx, ids!(create_room_encrypted_toggle));

        if clear_room_name {
            create_room_name_input.set_text(cx, "");
            create_room_topic_input.set_text(cx, "");
        }
        self.clear_feedback(cx);
        create_room_button.set_enabled(cx, !create_room_name_input.text().trim().is_empty());
        create_room_button.set_text(cx, tr_key(self.app_language, "add_room.create_room.button.create"));
        create_room_button.reset_hover(cx);
        create_room_encrypted_toggle.set_active(cx, self.create_encrypted_room);
        self.set_create_room_public(cx, self.create_public_room);
        self.set_visibility_popup_visible(cx, false);
        self.sync_mode_views(cx);

        create_room_name_input.set_key_focus(cx);
        self.view.redraw(cx);
    }

    fn selected_parent_space_id(&self) -> Option<OwnedRoomId> {
        self.fixed_parent_space_id.clone()
    }

    fn sync_mode_views(&mut self, cx: &mut Cx) {
        self.view.view(cx, ids!(create_room_button_row))
            .set_visible(cx, self.context == CreateRoomContext::AddRoomPage);
        self.view.view(cx, ids!(create_room_visibility_popup))
            .set_visible(cx, self.visibility_popup_visible);
        self.view.label(cx, ids!(create_room_help))
            .set_visible(cx, self.context == CreateRoomContext::AddRoomPage);

        let help_text = if self.fixed_parent_space_id.is_some() {
            tr_key(self.app_language, "add_room.create_room.help.fixed_parent")
        } else {
            tr_key(self.app_language, "add_room.create_room.help.default")
        };
        self.view.label(cx, ids!(create_room_help)).set_text(cx, help_text);
    }
}

impl CreateRoomFormRef {
    pub fn set_app_language(&self, cx: &mut Cx, app_language: AppLanguage) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.set_app_language(cx, app_language);
    }

    pub fn can_submit(&self, cx: &mut Cx) -> bool {
        self.borrow().is_some_and(|inner| inner.can_submit(cx))
    }

    pub fn is_busy(&self) -> bool {
        self.borrow().is_some_and(|inner| inner.is_busy())
    }

    pub fn submit(&self, cx: &mut Cx) -> bool {
        self.borrow_mut().is_some_and(|mut inner| inner.submit(cx))
    }

    pub fn prepare(
        &self,
        cx: &mut Cx,
        preferred_parent_space_id: Option<OwnedRoomId>,
        context: CreateRoomContext,
        clear_room_name: bool,
    ) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.prepare(cx, preferred_parent_space_id, context, clear_room_name);
    }
}

#[derive(Script, ScriptHook, Widget)]
pub struct CreateRoomModal {
    #[deref] view: View,
    #[rust] app_language: AppLanguage,
    #[rust(false)] has_fixed_parent: bool,
}

impl Widget for CreateRoomModal {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        let app_language = scope.data.get::<AppState>()
            .map(|app_state| app_state.app_language)
            .unwrap_or_default();
        if self.app_language != app_language {
            self.set_app_language(cx, app_language);
        }
        self.view.handle_event(cx, event, scope);
        self.widget_match_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        let app_language = scope.data.get::<AppState>()
            .map(|app_state| app_state.app_language)
            .unwrap_or_default();
        if self.app_language != app_language {
            self.set_app_language(cx, app_language);
        }
        self.view.draw_walk(cx, scope, walk)
    }
}

impl WidgetMatchEvent for CreateRoomModal {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions, _scope: &mut Scope) {
        let create_room_form = self.view.create_room_form(cx, ids!(create_room_form));
        let create_button = self.view.button(cx, ids!(create_button));
        let cancel_button = self.view.button(cx, ids!(cancel_button));

        if create_button.clicked(actions) {
            let _ = create_room_form.submit(cx);
        }

        // Allow cancel anytime
        if cancel_button.clicked(actions) {
            cx.action(CreateRoomModalAction::Close);
        }

        // Update button states based on form state
        let is_busy = create_room_form.is_busy();
        let can_submit = create_room_form.can_submit(cx);
        create_button.set_enabled(cx, can_submit);
        create_button.set_text(cx, if is_busy {
            tr_key(self.app_language, "add_room.create_room.button.syncing")
        } else {
            tr_key(self.app_language, "add_room.create_room.button.create")
        });
        // cancel_button stays enabled always
    }
}

impl CreateRoomModal {
    fn set_app_language(&mut self, cx: &mut Cx, app_language: AppLanguage) {
        self.app_language = app_language;
        self.view.label(cx, ids!(title))
            .set_text(cx, tr_key(self.app_language, "add_room.create_room.modal.title"));
        self.view.label(cx, ids!(subtitle))
            .set_text(cx, if self.has_fixed_parent {
                tr_key(self.app_language, "add_room.create_room.modal.subtitle")
            } else {
                tr_key(self.app_language, "add_room.create_room.help.default")
            });
        self.view.button(cx, ids!(create_button))
            .set_text(cx, tr_key(self.app_language, "add_room.create_room.button.create"));
        self.view.button(cx, ids!(cancel_button))
            .set_text(cx, tr_key(self.app_language, "add_room.button.cancel"));
        self.view.create_room_form(cx, ids!(create_room_form))
            .set_app_language(cx, app_language);
        self.view.redraw(cx);
    }

    pub fn show(&mut self, cx: &mut Cx, preferred_parent_space_id: Option<OwnedRoomId>) {
        self.has_fixed_parent = preferred_parent_space_id.is_some();
        let create_room_form = self.view.create_room_form(cx, ids!(create_room_form));
        create_room_form.set_app_language(cx, self.app_language);
        create_room_form.prepare(
            cx,
            preferred_parent_space_id,
            CreateRoomContext::SpaceLobbyModal,
            true,
        );
        self.view.label(cx, ids!(subtitle))
            .set_text(cx, if self.has_fixed_parent {
                tr_key(self.app_language, "add_room.create_room.modal.subtitle")
            } else {
                tr_key(self.app_language, "add_room.create_room.help.default")
            });
        let can_submit = create_room_form.can_submit(cx);
        let create_button = self.view.button(cx, ids!(create_button));
        let cancel_button = self.view.button(cx, ids!(cancel_button));
        create_button.set_text(cx, tr_key(self.app_language, "add_room.create_room.button.create"));
        create_button.reset_hover(cx);
        cancel_button.reset_hover(cx);
        // Initial button states: form is empty so create disabled, cancel always enabled
        create_button.set_enabled(cx, can_submit);
        cancel_button.set_enabled(cx, true);
        self.view.redraw(cx);
    }
}

impl CreateRoomModalRef {
    pub fn show(&self, cx: &mut Cx, preferred_parent_space_id: Option<OwnedRoomId>) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.show(cx, preferred_parent_space_id);
    }
}

#[derive(Script, ScriptHook, Widget)]
pub struct StartChatModal {
    #[deref] view: View,
    #[rust(false)] submitting: bool,
    #[rust] app_language: AppLanguage,
}

impl Widget for StartChatModal {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        let app_language = scope.data.get::<AppState>()
            .map(|app_state| app_state.app_language)
            .unwrap_or_default();
        if self.app_language != app_language {
            self.set_app_language(cx, app_language);
        }
        self.view.handle_event(cx, event, scope);
        self.widget_match_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        let app_language = scope.data.get::<AppState>()
            .map(|app_state| app_state.app_language)
            .unwrap_or_default();
        if self.app_language != app_language {
            self.set_app_language(cx, app_language);
        }
        self.view.draw_walk(cx, scope, walk)
    }
}

impl WidgetMatchEvent for StartChatModal {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions, _scope: &mut Scope) {
        let chat_user_id_input = self.view.text_input(cx, ids!(chat_user_id_input));
        let go_button = self.view.button(cx, ids!(go_button));
        let cancel_button = self.view.button(cx, ids!(cancel_button));

        if chat_user_id_input.changed(actions).is_some() {
            let user_id_text_is_empty = chat_user_id_input.text().trim().is_empty();
            go_button.set_enabled(cx, !self.submitting && !user_id_text_is_empty);
        }

        let submit_chat_request = go_button.clicked(actions)
            || chat_user_id_input.returned(actions).is_some();
        if submit_chat_request {
            self.submit(cx);
        }

        if cancel_button.clicked(actions) {
            self.submitting = false;
            cx.action(StartChatModalAction::Close);
        }

        for action in actions {
            if matches!(action.downcast_ref(), Some(DirectMessageRoomAction::FoundExisting { .. } | DirectMessageRoomAction::NewlyCreated { .. })) {
                self.submitting = false;
                cx.action(StartChatModalAction::Close);
            } else if matches!(action.downcast_ref(), Some(DirectMessageRoomAction::FailedToCreate { .. } | DirectMessageRoomAction::DidNotExist { .. })) {
                self.submitting = false;
                let user_id_text_is_empty = chat_user_id_input.text().trim().is_empty();
                go_button.set_enabled(cx, !user_id_text_is_empty);
                cancel_button.set_enabled(cx, true);
                self.view.redraw(cx);
            }
        }
    }
}

impl StartChatModal {
    fn set_app_language(&mut self, cx: &mut Cx, app_language: AppLanguage) {
        self.app_language = app_language;
        self.view.label(cx, ids!(title))
            .set_text(cx, tr_key(self.app_language, "add_room.start_chat.modal.title"));
        self.view.label(cx, ids!(subtitle))
            .set_text(cx, tr_key(self.app_language, "add_room.start_chat.modal.subtitle"));
        self.view.text_input(cx, ids!(chat_user_id_input))
            .set_empty_text(cx, tr_key(self.app_language, "add_room.start_chat.modal.input.placeholder").to_string());
        self.view.label(cx, ids!(chat_hint))
            .set_text(cx, tr_key(self.app_language, "add_room.start_chat.modal.hint"));
        self.view.button(cx, ids!(go_button))
            .set_text(cx, tr_key(self.app_language, "add_room.start_chat.modal.button.go"));
        self.view.button(cx, ids!(cancel_button))
            .set_text(cx, tr_key(self.app_language, "add_room.button.cancel"));
        self.view.redraw(cx);
    }

    fn submit(&mut self, cx: &mut Cx) {
        let user_id_str = self.view.text_input(cx, ids!(chat_user_id_input)).text();
        let user_id_str = user_id_str.trim();
        if user_id_str.is_empty() {
            return;
        }

        match user_id_str.parse::<OwnedUserId>() {
            Ok(user_id) => {
                if current_user_id().as_ref().is_some_and(|current| current == &user_id) {
                    enqueue_popup_notification(
                        tr_key(self.app_language, "add_room.popup.cannot_add_self").to_string(),
                        PopupKind::Warning,
                        Some(4.0),
                    );
                    return;
                }

                self.submitting = true;
                self.view.button(cx, ids!(go_button)).set_enabled(cx, false);
                // cancel_button stays enabled so user can cancel during submission
                submit_async_request(MatrixRequest::OpenOrCreateDirectMessage {
                    create_encrypted: false,
                    user_profile: UserProfile {
                        user_id,
                        username: None,
                        avatar_state: AvatarState::Unknown,
                    },
                    allow_create: true,
                });
                self.view.redraw(cx);
            }
            Err(e) => {
                let error_text = e.to_string();
                enqueue_popup_notification(
                    tr_fmt(self.app_language, "add_room.popup.invalid_user_id", &[
                        ("error", error_text.as_str()),
                    ]),
                    PopupKind::Error,
                    None,
                );
                self.view.text_input(cx, ids!(chat_user_id_input)).set_key_focus(cx);
            }
        }
    }

    pub fn show(&mut self, cx: &mut Cx) {
        self.submitting = false;
        self.view.text_input(cx, ids!(chat_user_id_input)).set_text(cx, "");
        self.set_app_language(cx, self.app_language);
        let go_button = self.view.button(cx, ids!(go_button));
        let cancel_button = self.view.button(cx, ids!(cancel_button));
        go_button.reset_hover(cx);
        cancel_button.reset_hover(cx);
        go_button.set_enabled(cx, false); // Text input is empty
        cancel_button.set_enabled(cx, true);
        self.view.text_input(cx, ids!(chat_user_id_input)).set_key_focus(cx);
        self.view.redraw(cx);
    }
}

impl StartChatModalRef {
    pub fn show(&self, cx: &mut Cx) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.show(cx);
    }
}

impl AddRoomScreen {
    fn set_app_language(&mut self, cx: &mut Cx, app_language: AppLanguage) {
        self.app_language = app_language;
        self.view.label(cx, ids!(title))
            .set_text(cx, tr_key(self.app_language, "add_room.title"));
        self.view.button(cx, ids!(new_room_button))
            .set_text(cx, tr_key(self.app_language, "add_room.create_room.button.new_room"));
        self.view.button(cx, ids!(start_chat_button))
            .set_text(cx, tr_key(self.app_language, "add_room.start_chat.button"));
        self.view.label(cx, ids!(join_existing_label))
            .set_text(cx, tr_key(self.app_language, "add_room.section.join_existing"));
        self.view.html(cx, ids!(help_info))
            .set_text(cx, tr_key(self.app_language, "add_room.join.help_html"));
        self.view.text_input(cx, ids!(room_alias_id_input))
            .set_empty_text(cx, tr_key(self.app_language, "add_room.join.input.placeholder").to_string());
        self.view.button(cx, ids!(search_for_room_button))
            .set_text(cx, tr_key(self.app_language, "add_room.join.button.go"));
        self.view.button(cx, ids!(fetched_room_summary.buttons_view.cancel_button))
            .set_text(cx, tr_key(self.app_language, "add_room.button.cancel"));
        self.view.redraw(cx);
    }
}

impl Widget for AddRoomScreen {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        let app_language = scope.data.get::<AppState>()
            .map(|app_state| app_state.app_language)
            .unwrap_or_default();
        if self.app_language != app_language {
            self.set_app_language(cx, app_language);
        }
        self.view.handle_event(cx, event, scope);
        
        if let Event::Actions(actions) = event {
            let room_alias_id_input = self.view.text_input(cx, ids!(room_alias_id_input));
            let search_for_room_button = self.view.button(cx, ids!(search_for_room_button));
            let new_room_button = self.view.button(cx, ids!(new_room_button));
            let start_chat_button = self.view.button(cx, ids!(start_chat_button));
            let cancel_button = self.view.button(cx, ids!(fetched_room_summary.buttons_view.cancel_button));
            let join_room_button = self.view.button(cx, ids!(fetched_room_summary.buttons_view.join_room_button));

            if new_room_button.clicked(actions) {
                cx.action(CreateRoomModalAction::Open {
                    parent_space_id: None,
                });
            }
            if start_chat_button.clicked(actions) {
                cx.action(StartChatModalAction::Open);
            }

            // Enable or disable the button based on if the text input is empty.
            if let Some(text) = room_alias_id_input.changed(actions) {
                search_for_room_button.set_enabled(cx, !text.trim().is_empty());
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
                        AddRoomState::FetchedRoomPreview { frp, .. } | AddRoomState::Loaded { frp, .. }
                    ) => {
                        cx.action(AppStateAction::NavigateToRoom {
                            room_to_close: None,
                            destination_room: frp.clone().into(),
                        });
                    }
                    (
                        JoinButtonFunction::Knock,
                        AddRoomState::FetchedRoomPreview { frp, room_or_alias_id, via }
                    ) => {
                        submit_async_request(MatrixRequest::Knock {
                            room_or_alias_id: frp.canonical_alias.clone().map_or_else(
                                || room_or_alias_id.clone(),
                                Into::into
                            ),
                            reason: None,
                            server_names: via.clone(),
                        });
                    }
                    _ => { }
                }
            }

            // If the button was clicked or enter was pressed, try to parse the room address.
            let new_room_query = search_for_room_button.clicked(actions)
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
                            response_mode: RoomPreviewResponseMode::Action,
                        });
                    }
                    Err(e) => {
                        let error_text = e.to_string();
                        let err_str = tr_fmt(self.app_language, "add_room.popup.parse_error", &[
                            ("error", error_text.as_str()),
                        ]);
                        enqueue_popup_notification(
                            err_str.clone(),
                            PopupKind::Error,
                            None,
                        );
                        self.state = AddRoomState::ParseError(err_str);
                        room_alias_id_input.set_key_focus(cx);
                    }
                }
                self.redraw(cx);
            }

            // If we're waiting for the room preview to be fetched (i.e., in the Parsed state),
            // then check if we've received it via an action.
            if let AddRoomState::Parsed { room_or_alias_id, via } = &self.state {
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
                            let error_text = e.to_string();
                            let err_str = tr_fmt(self.app_language, "add_room.popup.fetch_error", &[
                                ("error", error_text.as_str()),
                            ]);
                            enqueue_popup_notification(
                                err_str.clone(),
                                PopupKind::Error,
                                None,
                            );
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
            let mut transition_to_joined  = false;
            if let AddRoomState::FetchedRoomPreview { frp, room_or_alias_id, .. } = &self.state {
                for action in actions {
                    match action.downcast_ref() {
                        Some(KnockResultAction::Knocked { room, .. }) if room.room_id() == frp.room_name_id.room_id() => {
                            let room_type = match room.room_type() {
                                Some(RoomType::Space) => tr_key(self.app_language, "add_room.word.space_lc"),
                                _ => tr_key(self.app_language, "add_room.word.room_lc"),
                            };
                            let room_name_text = frp.room_name_id.to_string();
                            enqueue_popup_notification(
                                tr_fmt(self.app_language, "add_room.popup.knock_success", &[
                                    ("room_type", room_type),
                                    ("room_name", room_name_text.as_str()),
                                ]),
                                PopupKind::Success,
                                Some(4.0),
                            );
                            transition_to_knocked = true;
                            break;
                        }
                        Some(KnockResultAction::Failed { error, room_or_alias_id: roai }) if room_or_alias_id == roai => {
                            let error_text = error.to_string();
                            enqueue_popup_notification(
                                tr_fmt(self.app_language, "add_room.popup.knock_failed", &[
                                    ("error", error_text.as_str()),
                                ]),
                                PopupKind::Error,
                                None,
                            );
                            break;
                        }
                        _ => { }
                    }

                    match action.downcast_ref() {
                        Some(JoinRoomResultAction::Joined { room_id }) if room_id == frp.room_name_id.room_id() => {
                            let room_type = match &frp.room_type {
                                Some(RoomType::Space) => tr_key(self.app_language, "add_room.word.space_lc"),
                                _ => tr_key(self.app_language, "add_room.word.room_lc"),
                            };
                            let room_name_text = frp.room_name_id.to_string();
                            enqueue_popup_notification(
                                tr_fmt(self.app_language, "add_room.popup.join_success", &[
                                    ("room_type", room_type),
                                    ("room_name", room_name_text.as_str()),
                                ]),
                                PopupKind::Success,
                                Some(4.0),
                            );
                            transition_to_joined = true;
                            break;
                        }
                        Some(JoinRoomResultAction::Failed { room_id, error }) if room_id == frp.room_name_id.room_id() => {
                            let error_text = error.to_string();
                            enqueue_popup_notification(
                                tr_fmt(self.app_language, "add_room.popup.join_failed", &[
                                    ("error", error_text.as_str()),
                                ]),
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
                if let Some(AppStateAction::RoomLoadedSuccessfully { room_name_id, is_invite }) = action.downcast_ref() {
                    if self.state.fetched_room_preview().is_some_and(|frp| frp.room_name_id.room_id() == room_name_id.room_id()) {
                        self.state.transition_to_loaded(*is_invite);
                        self.redraw(cx);
                    }
                }
            }
        }
    }


    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        let app_language = scope.data.get::<AppState>()
            .map(|app_state| app_state.app_language)
            .unwrap_or_default();
        if self.app_language != app_language {
            self.set_app_language(cx, app_language);
        }

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
            AddRoomState::Parsed { room_or_alias_id, .. } => {
                loading_room_view.set_visible(cx, true);
                loading_room_view.label(cx, ids!(loading_text)).set_text(
                    cx,
                    &tr_fmt(self.app_language, "add_room.loading.fetching", &[
                        ("target", room_or_alias_id.as_str()),
                    ]),
                );
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
                        let res = room_avatar.show_image(
                            cx,
                            None,
                            |cx, img_ref| utils::load_png_or_jpg(&img_ref, cx, image_data),
                        );
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
                    Some(RoomType::Space) => (
                        tr_key(self.app_language, "add_room.word.space_lc"),
                        tr_key(self.app_language, "add_room.word.space_uc"),
                    ),
                    _ => (
                        tr_key(self.app_language, "add_room.word.room_lc"),
                        tr_key(self.app_language, "add_room.word.room_uc"),
                    ),
                };
                let room_name = fetched_room_summary.label(cx, ids!(room_name));
                match frp.room_name_id.name_for_avatar() {
                    Some(n) => room_name.set_text(cx, n),
                    _ => room_name.set_text(cx, &tr_fmt(self.app_language, "add_room.fetched.room_name.unnamed", &[
                        ("room_or_space_uc", room_or_space_uc),
                        ("room_id", frp.room_name_id.room_id().as_str()),
                    ])),
                }

                fetched_room_summary.label(cx, ids!(subsection_alias_id)).set_text(
                    cx,
                    &tr_fmt(self.app_language, "add_room.fetched.main_alias_and_id", &[
                        ("room_or_space_uc", room_or_space_uc),
                    ]),
                );
                fetched_room_summary.label(cx, ids!(room_alias)).set_text(
                    cx,
                    &tr_fmt(self.app_language, "add_room.fetched.alias", &[
                        ("alias", frp.canonical_alias.as_ref().map_or(
                            tr_key(self.app_language, "add_room.fetched.alias.not_set"),
                            |a| a.as_str()
                        )),
                    ]),
                );
                fetched_room_summary.label(cx, ids!(room_id)).set_text(
                    cx,
                    &tr_fmt(self.app_language, "add_room.fetched.id", &[
                        ("room_id", frp.room_name_id.room_id().as_str()),
                    ]),
                );
                fetched_room_summary.label(cx, ids!(subsection_topic)).set_text(
                    cx,
                    &tr_fmt(self.app_language, "add_room.fetched.topic_title", &[
                        ("room_or_space_uc", room_or_space_uc),
                    ]),
                );
                fetched_room_summary.html(cx, ids!(room_topic)).set_text(
                    cx,
                    frp.topic.as_deref().unwrap_or(tr_key(self.app_language, "add_room.fetched.topic.not_set_html")),
                );

                let room_summary = fetched_room_summary.label(cx, ids!(room_summary));
                let join_room_button = fetched_room_summary.button(cx, ids!(join_room_button));
                let join_function = match (&frp.state, &frp.join_rule) {
                    (Some(RoomState::Joined), _) => {
                        room_summary.set_text(cx, &tr_fmt(self.app_language, "add_room.summary.already_joined", &[
                            ("room_or_space_lc", room_or_space_lc),
                        ]));
                        join_room_button.set_text(cx, &tr_fmt(self.app_language, "add_room.button.go_to", &[
                            ("room_or_space_lc", room_or_space_lc),
                        ]));
                        JoinButtonFunction::NavigateOrJoin
                    }
                    (Some(RoomState::Banned), _) => {
                        room_summary.set_text(cx, &tr_fmt(self.app_language, "add_room.summary.banned", &[
                            ("room_or_space_lc", room_or_space_lc),
                        ]));
                        join_room_button.set_text(cx, tr_key(self.app_language, "add_room.button.cannot_join_until_unbanned"));
                        JoinButtonFunction::None
                    }
                    (Some(RoomState::Invited), _) => {
                        room_summary.set_text(cx, &tr_fmt(self.app_language, "add_room.summary.already_invited", &[
                            ("room_or_space_lc", room_or_space_lc),
                        ]));
                        join_room_button.set_text(cx, tr_key(self.app_language, "add_room.button.go_to_invitation"));
                        JoinButtonFunction::NavigateOrJoin
                    }
                    (Some(RoomState::Knocked), _) => {
                        room_summary.set_text(cx, &tr_fmt(self.app_language, "add_room.summary.already_knocked", &[
                            ("room_or_space_lc", room_or_space_lc),
                        ]));
                        join_room_button.set_text(cx, tr_key(self.app_language, "add_room.button.knock_again"));
                        JoinButtonFunction::Knock
                    }
                    (Some(RoomState::Left), join_rule) => {
                        room_summary.set_text(cx, &tr_fmt(self.app_language, "add_room.summary.previously_left", &[
                            ("room_or_space_lc", room_or_space_lc),
                        ]));
                        let (join_room_text, join_function) = match join_rule {
                            Some(JoinRuleSummary::Public) => (
                                tr_fmt(self.app_language, "add_room.button.rejoin", &[("room_or_space_lc", room_or_space_lc)]),
                                JoinButtonFunction::NavigateOrJoin,
                            ),
                            Some(JoinRuleSummary::Invite) => (
                                tr_fmt(self.app_language, "add_room.button.rejoin_requires_invite", &[("room_or_space_lc", room_or_space_lc)]),
                                JoinButtonFunction::None,
                            ),
                            Some(JoinRuleSummary::Knock | JoinRuleSummary::KnockRestricted(_)) => (
                                tr_fmt(self.app_language, "add_room.button.knock_to_rejoin", &[("room_or_space_lc", room_or_space_lc)]),
                                JoinButtonFunction::Knock,
                            ),
                            // TODO: handle this after we update matrix-sdk to the new `JoinRule` enum.
                            Some(JoinRuleSummary::Restricted(_)) => (
                                tr_fmt(self.app_language, "add_room.button.rejoin_requires_other_membership", &[("room_or_space_lc", room_or_space_lc)]),
                                JoinButtonFunction::None,
                            ),
                            _ => (
                                tr_fmt(self.app_language, "add_room.button.not_allowed_to_rejoin", &[("room_or_space_lc", room_or_space_lc)]),
                                JoinButtonFunction::None,
                            ),
                        };
                        join_room_button.set_text(cx, &join_room_text);
                        join_function
                    }
                    // This room is not yet known to the user.
                    (None, join_rule) => {
                        let directness = if frp.is_direct == Some(true) {
                            tr_key(self.app_language, "add_room.word.direct")
                        } else {
                            tr_key(self.app_language, "add_room.word.regular")
                        };
                        let num_members = frp.num_joined_members.to_string();
                        let member_word = match frp.num_joined_members {
                            1 => tr_key(self.app_language, "add_room.word.member"),
                            _ => tr_key(self.app_language, "add_room.word.members"),
                        };
                        room_summary.set_text(cx, &tr_fmt(self.app_language, "add_room.summary.member_count", &[
                            ("directness", directness),
                            ("room_or_space_lc", room_or_space_lc),
                            ("num_members", num_members.as_str()),
                            ("member_word", member_word),
                        ]));

                        let (join_room_text, join_function) = match join_rule {
                            Some(JoinRuleSummary::Public) => (
                                tr_fmt(self.app_language, "add_room.button.join", &[("room_or_space_lc", room_or_space_lc)]),
                                JoinButtonFunction::NavigateOrJoin,
                            ),
                            Some(JoinRuleSummary::Invite) => (
                                tr_fmt(self.app_language, "add_room.button.join_requires_invite", &[("room_or_space_lc", room_or_space_lc)]),
                                JoinButtonFunction::None,
                            ),
                            Some(JoinRuleSummary::Knock | JoinRuleSummary::KnockRestricted(_)) => (
                                tr_fmt(self.app_language, "add_room.button.knock_to_join", &[("room_or_space_lc", room_or_space_lc)]),
                                JoinButtonFunction::Knock,
                            ),
                            // TODO: handle this after we update matrix-sdk to the new `JoinRule` enum.
                            Some(JoinRuleSummary::Restricted(_)) => (
                                tr_fmt(self.app_language, "add_room.button.join_requires_other_membership", &[("room_or_space_lc", room_or_space_lc)]),
                                JoinButtonFunction::None,
                            ),
                            _ => ( 
                                tr_fmt(self.app_language, "add_room.button.not_allowed_to_join", &[("room_or_space_lc", room_or_space_lc)]),
                                JoinButtonFunction::None,
                            ),
                        };
                        join_room_button.set_text(cx, &join_room_text);
                        join_function
                    }
                };

                match ars {
                    AddRoomState::FetchedRoomPreview { .. } => {
                        join_room_button.set_enabled(cx, !matches!(join_function, JoinButtonFunction::None));
                        self.join_function = join_function;
                    }
                    AddRoomState::Knocked { .. } => {
                        room_summary.set_text(cx, &tr_fmt(self.app_language, "add_room.summary.knocked_waiting", &[
                            ("room_or_space_lc", room_or_space_lc),
                        ]));
                        join_room_button.set_text(cx, tr_key(self.app_language, "add_room.button.successfully_knocked"));
                        join_room_button.set_enabled(cx, false);
                    }
                    AddRoomState::Joined { .. } => {
                        room_summary.set_text(cx, &tr_fmt(self.app_language, "add_room.summary.joined_loading", &[
                            ("room_or_space_lc", room_or_space_lc),
                        ]));
                        join_room_button.set_text(cx, tr_key(self.app_language, "add_room.button.successfully_joined"));
                        join_room_button.set_enabled(cx, false);
                    }
                    AddRoomState::Loaded { is_invite, .. } => {
                        let verb = if *is_invite {
                            tr_key(self.app_language, "add_room.word.verb.invited")
                        } else {
                            tr_key(self.app_language, "add_room.word.verb.joined")
                        };
                        room_summary.set_text(cx, &tr_fmt(self.app_language, "add_room.summary.loaded", &[
                            ("verb", verb),
                            ("room_or_space_lc", room_or_space_lc),
                        ]));
                        let adj = if *is_invite {
                            tr_key(self.app_language, "add_room.word.adj.invited")
                        } else {
                            tr_key(self.app_language, "add_room.word.adj.joined")
                        };
                        join_room_button.set_text(cx, &tr_fmt(self.app_language, "add_room.button.go_to_loaded", &[
                            ("adj", adj),
                            ("room_or_space_lc", room_or_space_lc),
                        ]));
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

fn refresh_space_children(cx: &mut Cx, space_id: &OwnedRoomId) {
    let Some(rooms_list_ref) = cx.has_global::<RoomsListRef>().then(|| cx.get_global::<RoomsListRef>()) else {
        return;
    };
    let Some(space_request_sender) = rooms_list_ref.get_space_request_sender() else {
        return;
    };
    let parent_chain = rooms_list_ref.get_space_parent_chain(space_id).unwrap_or_default();
    if let Err(e) = space_request_sender.send(SpaceRequest::SubscribeToSpaceRoomList {
        space_id: space_id.clone(),
        parent_chain: parent_chain.clone(),
    }) {
        error!("Failed to subscribe to space room list for {space_id}: {e}");
        return;
    }
    if let Err(e) = space_request_sender.send(SpaceRequest::PaginateSpaceRoomList {
        space_id: space_id.clone(),
        parent_chain: parent_chain.clone(),
    }) {
        error!("Failed to paginate children for space {space_id}: {e}");
    }
    if let Err(e) = space_request_sender.send(SpaceRequest::GetChildren {
        space_id: space_id.clone(),
        parent_chain,
    }) {
        error!("Failed to refresh children for space {space_id}: {e}");
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
    }
}

/// Actions sent from the backend task as a result of a [`MatrixRequest::CreateRoom`].
#[derive(Debug)]
pub enum CreateRoomAction {
    /// A new room was created.
    Created {
        room_name_id: RoomNameId,
        parent_space_id: Option<OwnedRoomId>,
        /// If set, the room was created but couldn't be linked into the requested space.
        space_link_error: Option<String>,
        context: CreateRoomContext,
    },
    /// There was an error creating the room.
    Failed {
        room_name: String,
        error: matrix_sdk::Error,
        context: CreateRoomContext,
    },
}

/// Actions emitted by other widgets to show or hide the create-room modal.
#[derive(Debug)]
pub enum CreateRoomModalAction {
    Open {
        parent_space_id: Option<OwnedRoomId>,
    },
    Close,
}

/// Actions emitted by other widgets to show or hide the start-chat modal.
#[derive(Debug)]
pub enum StartChatModalAction {
    Open,
    Close,
}

/// Actions sent from the backend task containing the spaces where the current user
/// can create child rooms.
#[derive(Debug)]
pub enum CreatableSpacesAction {
    Loaded {
        spaces: Vec<RoomNameId>,
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
                .or_else(|_| MatrixUri::parse(text).map(|uri| (uri.id().clone(), uri.via().to_owned())));
            
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
