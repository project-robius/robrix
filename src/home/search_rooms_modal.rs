//! A modal dialog for searching joined rooms/spaces and homeserver users.

use makepad_widgets::*;
use ruma::{
    OwnedMxcUri, OwnedRoomAliasId, OwnedRoomId,
    room::{JoinRuleKind, RoomType},
};

use crate::{
    app::SelectedRoom,
    avatar_cache::{self, AvatarCacheEntry},
    home::{
        add_room::AddRoomScreenAction,
        navigation_tab_bar::NavigationBarAction,
        rooms_list::{RoomSearchResult, RoomsListAction, RoomsListRef},
        spaces_bar::{SpaceSearchResult, SpacesBarRef},
    },
    profile::user_profile::UserProfile,
    shared::avatar::{AvatarState, AvatarWidgetExt, AvatarWidgetRefExt},
    sliding_sync::{MatrixRequest, submit_async_request},
    utils,
};

const SEARCH_SPACE_CARD_BG: Vec4 = vec4(0.937, 0.965, 1.0, 1.0);
const SEARCH_SPACE_CARD_BORDER: Vec4 = vec4(0.769, 0.863, 0.969, 1.0);
const SEARCH_SPACE_BADGE_BG: Vec4 = vec4(0.875, 0.925, 1.0, 1.0);
const SEARCH_SPACE_BADGE_FG: Vec4 = vec4(0.157, 0.369, 0.659, 1.0);

const SEARCH_ROOM_CARD_BG: Vec4 = vec4(0.969, 0.976, 0.992, 1.0);
const SEARCH_ROOM_CARD_BORDER: Vec4 = vec4(0.898, 0.918, 0.953, 1.0);
const SEARCH_ROOM_BADGE_BG: Vec4 = vec4(0.933, 0.945, 0.969, 1.0);
const SEARCH_ROOM_BADGE_FG: Vec4 = vec4(0.286, 0.337, 0.42, 1.0);

const SEARCH_PEOPLE_CARD_BG: Vec4 = vec4(0.945, 0.984, 0.953, 1.0);
const SEARCH_PEOPLE_CARD_BORDER: Vec4 = vec4(0.816, 0.925, 0.839, 1.0);
const SEARCH_PEOPLE_BADGE_BG: Vec4 = vec4(0.866, 0.965, 0.89, 1.0);
const SEARCH_PEOPLE_BADGE_FG: Vec4 = vec4(0.102, 0.451, 0.192, 1.0);

const SEARCH_INVITE_CARD_BG: Vec4 = vec4(1.0, 0.969, 0.929, 1.0);
const SEARCH_INVITE_CARD_BORDER: Vec4 = vec4(0.953, 0.875, 0.757, 1.0);
const SEARCH_INVITE_BADGE_BG: Vec4 = vec4(0.992, 0.906, 0.773, 1.0);
const SEARCH_INVITE_BADGE_FG: Vec4 = vec4(0.631, 0.357, 0.0, 1.0);

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.SearchScopeToggleButton = Button {
        width: Fit
        height: Fit
        padding: Inset{left: 12, right: 12, top: 8, bottom: 8}
        spacing: 0
        text: ""
        draw_bg +: {
            color: #xF6F8FC
            color_hover: #xEDF2FA
            color_down: #xE2ECFF
            border_radius: 16.0
            border_size: 1.0
            border_color: #xDCE4F1
            border_color_hover: #xC8D7EB
            border_color_down: #xB9CDEF
        }
        draw_text +: {
            text_style: REGULAR_TEXT {font_size: 10.5}
            color: #x445064
            color_hover: #x223046
            color_down: #x223046
        }
    }

    mod.widgets.SearchResultActionButton = RobrixNeutralIconButton {
        width: Fit
        height: Fit
        padding: Inset{top: 7, bottom: 7, left: 10, right: 10}
        spacing: 0
        draw_text +: {
            text_style: REGULAR_TEXT {font_size: 9.5}
        }
    }

    mod.widgets.SearchResultEntry = set_type_default() do #(SearchResultEntry::register_widget(vm)) {
        ..mod.widgets.RoundedView

        width: Fill
        height: Fit
        flow: Down
        padding: Inset{top: 10, right: 12, bottom: 10, left: 12}
        spacing: 6
        cursor: MouseCursor.Hand

        show_bg: true
        draw_bg +: {
            color: #xF7F9FD
            border_radius: 8.0
            border_size: 1.0
            border_color: #xE5EAF3
        }

        title_row := View {
            width: Fill
            height: Fit
            flow: Right
            spacing: 8
            align: Align{y: 0.5}

            kind_label := Label {
                width: Fit
                height: Fit
                padding: Inset{left: 8, right: 8, top: 4, bottom: 4}
                draw_bg +: {
                    color: #xE6EEF9
                    border_radius: 10.0
                }
                draw_text +: {
                    text_style: REGULAR_TEXT {font_size: 9.5}
                    color: #x285EA8
                }
                text: "Room"
            }

            title := Label {
                width: Fill
                height: Fit
                flow: Flow.Right{wrap: true}
                draw_text +: {
                    text_style: TITLE_TEXT {font_size: 11.5}
                    color: #111
                }
                text: ""
            }

            action_hint := Label {
                width: Fit
                height: Fit
                draw_text +: {
                    text_style: REGULAR_TEXT {font_size: 9.5}
                    color: #x627086
                }
                text: ""
            }
        }

        subtitle := Label {
            width: Fill
            height: Fit
            flow: Flow.Right{wrap: true}
            draw_text +: {
                text_style: REGULAR_TEXT {font_size: 10}
                color: #666
            }
            text: ""
        }
    }

    mod.widgets.SearchUserEntry = set_type_default() do #(SearchUserEntry::register_widget(vm)) {
        ..mod.widgets.RoundedView

        width: Fill
        height: Fit
        flow: Right
        spacing: 10
        padding: Inset{top: 10, right: 12, bottom: 10, left: 12}
        align: Align{y: 0.5}
        cursor: MouseCursor.Hand

        show_bg: true
        draw_bg +: {
            color: #xF7F9FD
            border_radius: 8.0
            border_size: 1.0
            border_color: #xE5EAF3
        }

        avatar := Avatar {
            width: 34
            height: 34
            cursor: MouseCursor.Default
            text_view +: {
                text +: {
                    draw_text +: {
                        text_style: TITLE_TEXT {font_size: 13}
                    }
                }
            }
        }

        details := View {
            width: Fill
            height: Fit
            flow: Down
            spacing: 6

            title_row := View {
                width: Fill
                height: Fit
                flow: Right
                spacing: 8
                align: Align{y: 0.5}

                kind_label := Label {
                    width: Fit
                    height: Fit
                    padding: Inset{left: 8, right: 8, top: 4, bottom: 4}
                    draw_bg +: {
                        color: #xE6F6E8
                        border_radius: 10.0
                    }
                    draw_text +: {
                        text_style: REGULAR_TEXT {font_size: 9.5}
                        color: #x1A7331
                    }
                    text: "People"
                }

                title := Label {
                    width: Fill
                    height: Fit
                    flow: Flow.Right{wrap: true}
                    draw_text +: {
                        text_style: TITLE_TEXT {font_size: 11.5}
                        color: #111
                    }
                    text: ""
                }

                action_hint := Label {
                    width: Fit
                    height: Fit
                    draw_text +: {
                        text_style: REGULAR_TEXT {font_size: 9.5}
                        color: #x1A7331
                    }
                    text: "Message"
                }
            }

            subtitle := Label {
                width: Fill
                height: Fit
                flow: Flow.Right{wrap: true}
                draw_text +: {
                    text_style: REGULAR_TEXT {font_size: 10}
                    color: #666
                }
                text: ""
            }
        }
    }

    mod.widgets.SearchResultsList = #(SearchResultsList::register_widget(vm)) {
        width: Fill
        height: Fill

        list := PortalList {
            width: Fill
            height: Fill
            flow: Down
            spacing: 8
            keep_invisible: false
            auto_tail: false
            max_pull_down: 0.0
            clip_x: true
            clip_y: true
            scroll_bar: ScrollBar {
                drag_scrolling: true
                bar_size: 6.0
                min_handle_size: 24.0
            }

            ResultCard := CachedView {
                width: Fill
                height: Fit
                flow: Down

                card_root := RoundedView {
                    width: Fill
                    height: Fit
                    flow: Down
                    padding: Inset{top: 10, right: 12, bottom: 10, left: 12}
                    spacing: 6
                    cursor: MouseCursor.Hand

                    show_bg: true
                    draw_bg +: {
                        color: #xF7F9FD
                        border_radius: 8.0
                        border_size: 1.0
                        border_color: #xE5EAF3
                    }

                    title_row := View {
                        width: Fill
                        height: Fit
                        flow: Right
                        spacing: 8
                        align: Align{y: 0.5}

                        kind_label := Label {
                            width: Fit
                            height: Fit
                            padding: Inset{left: 8, right: 8, top: 4, bottom: 4}
                            draw_bg +: {
                                color: #xE6EEF9
                                border_radius: 10.0
                            }
                            draw_text +: {
                                text_style: REGULAR_TEXT {font_size: 9.5}
                                color: #x285EA8
                            }
                            text: "Room"
                        }

                        title := Label {
                            width: Fill
                            height: Fit
                            flow: Flow.Right{wrap: true}
                            draw_text +: {
                                text_style: TITLE_TEXT {font_size: 11.5}
                                color: #111
                            }
                            text: ""
                        }

                        action_button := SearchResultActionButton {
                            text: "Open"
                        }
                    }

                    subtitle := Label {
                        width: Fill
                        height: Fit
                        flow: Flow.Right{wrap: true}
                        draw_text +: {
                            text_style: REGULAR_TEXT {font_size: 10}
                            color: #666
                        }
                        text: ""
                    }
                }
            }

            UserCard := CachedView {
                width: Fill
                height: Fit
                flow: Down

                card_root := RoundedView {
                    width: Fill
                    height: Fit
                    flow: Right
                    spacing: 10
                    padding: Inset{top: 10, right: 12, bottom: 10, left: 12}
                    align: Align{y: 0.5}
                    cursor: MouseCursor.Hand

                    show_bg: true
                    draw_bg +: {
                        color: #xF7F9FD
                        border_radius: 8.0
                        border_size: 1.0
                        border_color: #xE5EAF3
                    }

                    avatar := Avatar {
                        width: 34
                        height: 34
                        cursor: MouseCursor.Default
                        text_view +: {
                            text +: {
                                draw_text +: {
                                    text_style: TITLE_TEXT {font_size: 13}
                                }
                            }
                        }
                    }

                    details := View {
                        width: Fill
                        height: Fit
                        flow: Down
                        spacing: 6

                        title_row := View {
                            width: Fill
                            height: Fit
                            flow: Right
                            spacing: 8
                            align: Align{y: 0.5}

                            kind_label := Label {
                                width: Fit
                                height: Fit
                                padding: Inset{left: 8, right: 8, top: 4, bottom: 4}
                                draw_bg +: {
                                    color: #xE6F6E8
                                    border_radius: 10.0
                                }
                                draw_text +: {
                                    text_style: REGULAR_TEXT {font_size: 9.5}
                                    color: #x1A7331
                                }
                                text: "People"
                            }

                            title := Label {
                                width: Fill
                                height: Fit
                                flow: Flow.Right{wrap: true}
                                draw_text +: {
                                    text_style: TITLE_TEXT {font_size: 11.5}
                                    color: #111
                                }
                                text: ""
                            }

                            action_button := SearchResultActionButton {
                                text: "Message"
                            }
                        }

                        subtitle := Label {
                            width: Fill
                            height: Fit
                            flow: Flow.Right{wrap: true}
                            draw_text +: {
                                text_style: REGULAR_TEXT {font_size: 10}
                                color: #666
                            }
                            text: ""
                        }
                    }
                }
            }
        }
    }

    mod.widgets.SearchRoomsModal = set_type_default() do #(SearchRoomsModal::register_widget(vm)) {
        ..mod.widgets.RoundedView

        width: 580
        height: 690
        align: Align{x: 0.5}
        flow: Down
        padding: Inset{top: 24, right: 24, bottom: 22, left: 24}
        spacing: 12
        clip_x: true
        clip_y: true

        show_bg: true
        draw_bg +: {
            color: (COLOR_PRIMARY)
            border_radius: 8.0
        }

            title_row := View {
                width: Fill
                height: Fit
                flow: Right
                align: Align{y: 0.5}

                title := Label {
                    width: Fill
                    height: Fit
                    draw_text +: {
                        text_style: TITLE_TEXT {font_size: 14}
                        color: #000
                    }
                    text: "Search Rooms And Spaces"
                }

                close_button := RobrixIconButton {
                    width: Fit
                    height: Fit
                    padding: 10
                    spacing: 0
                    align: Align{x: 0.5, y: 0.5}
                    icon_walk: Walk{width: 16, height: 16, margin: 0}
                    draw_icon.svg: (ICON_CLOSE)
                    draw_icon.color: #666
                    draw_bg +: {
                        border_size: 0
                        color: #0000
                        color_hover: #00000015
                        color_down: #00000025
                    }
                }
            }

            subtitle := Label {
                width: Fill
                height: Fit
                flow: Flow.Right{wrap: true}
                draw_text +: {
                    text_style: REGULAR_TEXT {font_size: 10.5}
                    color: #666
                }
                text: "Search joined spaces and rooms locally. Turn on People to search users on the current homeserver."
            }

            search_bar := RoundedView {
                width: Fill
                height: 35
                padding: Inset{top: 3, bottom: 3, left: 10, right: 4.5}
                spacing: 4
                align: Align{x: 0.0, y: 0.5}
                show_bg: true
                draw_bg +: {
                    color: (COLOR_PRIMARY)
                    border_radius: 4.0
                    border_color: (COLOR_SECONDARY)
                    border_size: 1.0
                }

                Icon {
                    draw_icon +: {
                        svg: (ICON_SEARCH)
                        color: (COLOR_TEXT_INPUT_IDLE)
                    }
                    icon_walk: Walk{width: 14, height: 14}
                }

                input := RobrixTextInput {
                    width: Fill
                    height: Fit
                    flow: Right
                    padding: 5
                    empty_text: "Search rooms, spaces, or people..."
                    draw_bg.border_size: 0.0
                    draw_text +: {
                        text_style: theme.font_regular {font_size: 10}
                    }
                }

                clear_button := RobrixNeutralIconButton {
                    visible: false
                    padding: Inset{top: 5, bottom: 5, left: 9, right: 9}
                    spacing: 0
                    align: Align{x: 0.5, y: 0.5}
                    draw_icon.svg: (ICON_CLOSE)
                    icon_walk: Walk{width: Fit, height: 10, margin: 0}
                }
            }

            scope_panel := RoundedView {
                width: Fill
                height: Fit
                flow: Down
                padding: 12
                spacing: 8
                show_bg: true
                draw_bg +: {
                    color: #xF8FAFD
                    border_radius: 8.0
                    border_size: 1.0
                    border_color: #xE5EAF3
                }

                scope_hint := Label {
                    width: Fill
                    height: Fit
                    flow: Flow.Right{wrap: true}
                    draw_text +: {
                        text_style: REGULAR_TEXT {font_size: 10}
                        color: #66758C
                    }
                    text: "No filter selected means search everything. Turn on People to search homeserver users."
                }

                scope_buttons := View {
                    width: Fill
                    height: Fit
                    flow: Flow.Right{wrap: true}
                    spacing: 8

                    scope_spaces_button := SearchScopeToggleButton {
                        text: "Spaces"
                    }

                    scope_rooms_button := SearchScopeToggleButton {
                        text: "Rooms"
                    }

                    scope_people_button := SearchScopeToggleButton {
                        text: "People"
                    }
                }
            }

            status_label := Label {
                width: Fill
                height: Fit
                flow: Flow.Right{wrap: true}
                draw_text +: {
                    text_style: REGULAR_TEXT {font_size: 10}
                    color: #777
                }
                text: "No filter selected means search all."
            }

            loading_state := View {
                visible: false
                width: Fill
                height: Fit
                flow: Right
                spacing: 8
                align: Align{y: 0.5}

                loading_spinner := LoadingSpinner {
                    width: 16
                    height: 16
                    draw_bg +: {
                        color: (COLOR_ACTIVE_PRIMARY)
                        border_size: 3.0
                    }
                }

                loading_label := Label {
                    width: Fill
                    height: Fit
                    draw_text +: {
                        text_style: REGULAR_TEXT {font_size: 10}
                        color: #66758C
                    }
                    text: "Searching people on the current homeserver..."
                }
            }

            empty_results_state := RoundedView {
                visible: false
                width: Fill
                height: Fit
                flow: Down
                spacing: 10
                padding: 14
                show_bg: true
                draw_bg +: {
                    color: #xFAFBFD
                    border_radius: 8.0
                    border_size: 1.0
                    border_color: #xE5EAF3
                }

                empty_title := Label {
                    width: Fill
                    height: Fit
                    draw_text +: {
                        text_style: TITLE_TEXT {font_size: 11.5}
                        color: #111
                    }
                    text: "No results found."
                }

                empty_body := Label {
                    width: Fill
                    height: Fit
                    flow: Flow.Right{wrap: true}
                    draw_text +: {
                        text_style: REGULAR_TEXT {font_size: 10.5}
                        color: #67758A
                    }
                    text: "Try another keyword, switch search options above, or open Add/Explore lookup."
                }

                empty_add_room_button := RobrixNeutralIconButton {
                    width: Fit
                    padding: Inset{top: 10, bottom: 10, left: 12, right: 14}
                    draw_icon.svg: (ICON_SEARCH)
                    icon_walk: Walk{width: 16, height: 16, margin: Inset{right: -2}}
                    text: "Open Add/Explore Lookup"
                }
            }

        search_results_list := SearchResultsList {}
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct SearchFilters {
    spaces: bool,
    rooms: bool,
    people: bool,
}

impl SearchFilters {
    fn is_all(self) -> bool {
        !self.spaces && !self.rooms && !self.people
    }

    fn searches_spaces(self) -> bool {
        self.spaces || self.is_all()
    }

    fn searches_rooms(self) -> bool {
        self.rooms || self.is_all()
    }

    fn searches_people(self) -> bool {
        self.people || self.is_all()
    }

    fn hint_text(self) -> &'static str {
        match (self.spaces, self.rooms, self.people) {
            (false, false, false) => {
                "No filter selected means search everything. Joined matches appear first, then public rooms/spaces and homeserver users."
            }
            (true, false, false) => "Searching joined spaces and public spaces.",
            (false, true, false) => "Searching joined rooms, invites, and public rooms.",
            (false, false, true) => {
                "Searching your local DM rooms and the current homeserver user directory."
            }
            (true, true, false) => "Searching joined spaces and rooms, plus the public directory.",
            (true, false, true) => {
                "Searching joined/public spaces plus homeserver people results."
            }
            (false, true, true) => {
                "Searching joined/public rooms plus homeserver people results."
            }
            (true, true, true) => "Searching everything, including public rooms/spaces and homeserver people results.",
        }
    }
}

#[derive(Clone, Debug)]
pub enum SearchUsersAction {
    Results {
        request_id: u64,
        query: String,
        results: Vec<UserProfile>,
        limited: bool,
    },
    Failed {
        request_id: u64,
        query: String,
        error: String,
    },
}

#[derive(Clone, Debug)]
pub enum SearchPublicRoomsAction {
    Results {
        request_id: u64,
        query: String,
        results: Vec<DirectorySearchResult>,
    },
    Failed {
        request_id: u64,
        query: String,
        error: String,
    },
}

#[derive(Clone, Debug)]
pub struct DirectorySearchResult {
    pub room_id: OwnedRoomId,
    pub canonical_alias: Option<OwnedRoomAliasId>,
    pub name: Option<String>,
    pub topic: Option<String>,
    pub avatar_url: Option<OwnedMxcUri>,
    pub room_type: Option<RoomType>,
    pub join_rule: JoinRuleKind,
    pub joined_members: u64,
}

impl DirectorySearchResult {
    fn display_name(&self) -> String {
        self.name
            .clone()
            .or_else(|| self.canonical_alias.as_ref().map(ToString::to_string))
            .unwrap_or_else(|| self.room_id.to_string())
    }

    fn subtitle(&self) -> String {
        self.topic
            .clone()
            .or_else(|| self.canonical_alias.as_ref().map(ToString::to_string))
            .unwrap_or_else(|| format!("{} members", self.joined_members))
    }

    fn is_space(&self) -> bool {
        matches!(self.room_type, Some(RoomType::Space))
    }

    fn is_joinable(&self) -> bool {
        matches!(self.join_rule, JoinRuleKind::Public)
    }

    fn lookup_query(&self) -> String {
        self.canonical_alias
            .as_ref()
            .map(ToString::to_string)
            .unwrap_or_else(|| self.room_id.to_string())
    }
}

#[derive(Clone, Debug)]
pub enum SearchResultNavTarget {
    JoinedRoom(RoomSearchResult),
    Space(SpaceSearchResult),
    DirectoryRoom(DirectorySearchResult),
    User(UserProfile),
}

impl SearchResultNavTarget {
    fn visual_style(&self) -> SearchResultVisualStyle {
        match self {
            Self::Space(_) => SearchResultVisualStyle {
                kind_label: "Space",
                action_hint: "Open space",
                card_bg: SEARCH_SPACE_CARD_BG,
                card_border: SEARCH_SPACE_CARD_BORDER,
                badge_bg: SEARCH_SPACE_BADGE_BG,
                badge_fg: SEARCH_SPACE_BADGE_FG,
                action_fg: SEARCH_SPACE_BADGE_FG,
            },
            Self::JoinedRoom(room) if room.is_invite && room.is_direct => SearchResultVisualStyle {
                kind_label: "DM Invite",
                action_hint: "Open invite",
                card_bg: SEARCH_INVITE_CARD_BG,
                card_border: SEARCH_INVITE_CARD_BORDER,
                badge_bg: SEARCH_INVITE_BADGE_BG,
                badge_fg: SEARCH_INVITE_BADGE_FG,
                action_fg: SEARCH_INVITE_BADGE_FG,
            },
            Self::JoinedRoom(room) if room.is_invite => SearchResultVisualStyle {
                kind_label: "Invite",
                action_hint: "Open invite",
                card_bg: SEARCH_INVITE_CARD_BG,
                card_border: SEARCH_INVITE_CARD_BORDER,
                badge_bg: SEARCH_INVITE_BADGE_BG,
                badge_fg: SEARCH_INVITE_BADGE_FG,
                action_fg: SEARCH_INVITE_BADGE_FG,
            },
            Self::JoinedRoom(room) if room.is_direct => SearchResultVisualStyle {
                kind_label: "People",
                action_hint: "Open DM",
                card_bg: SEARCH_PEOPLE_CARD_BG,
                card_border: SEARCH_PEOPLE_CARD_BORDER,
                badge_bg: SEARCH_PEOPLE_BADGE_BG,
                badge_fg: SEARCH_PEOPLE_BADGE_FG,
                action_fg: SEARCH_PEOPLE_BADGE_FG,
            },
            Self::JoinedRoom(_) => SearchResultVisualStyle {
                kind_label: "Room",
                action_hint: "Open room",
                card_bg: SEARCH_ROOM_CARD_BG,
                card_border: SEARCH_ROOM_CARD_BORDER,
                badge_bg: SEARCH_ROOM_BADGE_BG,
                badge_fg: SEARCH_ROOM_BADGE_FG,
                action_fg: SEARCH_ROOM_BADGE_FG,
            },
            Self::DirectoryRoom(directory_room) if directory_room.is_space() => {
                SearchResultVisualStyle {
                    kind_label: "Public Space",
                    action_hint: if directory_room.is_joinable() {
                        "Join space"
                    } else {
                        "Add space"
                    },
                    card_bg: SEARCH_SPACE_CARD_BG,
                    card_border: SEARCH_SPACE_CARD_BORDER,
                    badge_bg: SEARCH_SPACE_BADGE_BG,
                    badge_fg: SEARCH_SPACE_BADGE_FG,
                    action_fg: SEARCH_SPACE_BADGE_FG,
                }
            }
            Self::DirectoryRoom(directory_room) => SearchResultVisualStyle {
                kind_label: "Public Room",
                action_hint: if directory_room.is_joinable() {
                    "Join room"
                } else {
                    "Add room"
                },
                card_bg: SEARCH_ROOM_CARD_BG,
                card_border: SEARCH_ROOM_CARD_BORDER,
                badge_bg: SEARCH_ROOM_BADGE_BG,
                badge_fg: SEARCH_ROOM_BADGE_FG,
                action_fg: SEARCH_ROOM_BADGE_FG,
            },
            Self::User(_) => SearchResultVisualStyle {
                kind_label: "People",
                action_hint: "Message",
                card_bg: SEARCH_PEOPLE_CARD_BG,
                card_border: SEARCH_PEOPLE_CARD_BORDER,
                badge_bg: SEARCH_PEOPLE_BADGE_BG,
                badge_fg: SEARCH_PEOPLE_BADGE_FG,
                action_fg: SEARCH_PEOPLE_BADGE_FG,
            },
        }
    }
}

struct SearchResultVisualStyle {
    kind_label: &'static str,
    #[allow(dead_code)]
    action_hint: &'static str,
    card_bg: Vec4,
    card_border: Vec4,
    badge_bg: Vec4,
    badge_fg: Vec4,
    action_fg: Vec4,
}

impl SearchResultNavTarget {
    fn title_text(&self) -> String {
        match self {
            Self::JoinedRoom(room) => room.room_name_id.to_string(),
            Self::Space(space) => space.space_name_id.to_string(),
            Self::DirectoryRoom(room) => room.display_name(),
            Self::User(user) => user.displayable_name().to_string(),
        }
    }

    fn subtitle_text(&self) -> String {
        match self {
            Self::JoinedRoom(room) => room.subtitle.clone(),
            Self::Space(space) => space.subtitle.clone(),
            Self::DirectoryRoom(room) => room.subtitle(),
            Self::User(user) => user.user_id.to_string(),
        }
    }

    fn action_button_text(&self) -> &'static str {
        match self {
            Self::User(_) => "Message",
            Self::DirectoryRoom(room) if room.is_joinable() => "Join",
            Self::DirectoryRoom(_) => "Add",
            _ => "Open",
        }
    }

    fn room_id(&self) -> Option<&OwnedRoomId> {
        match self {
            Self::JoinedRoom(room) => Some(room.room_name_id.room_id()),
            Self::Space(space) => Some(space.space_name_id.room_id()),
            Self::DirectoryRoom(room) => Some(&room.room_id),
            Self::User(_) => None,
        }
    }

    #[allow(dead_code)]
    fn user_id(&self) -> Option<&str> {
        match self {
            Self::User(user) => Some(user.user_id.as_str()),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub enum SearchResultEntryAction {
    Clicked(SearchResultNavTarget),
    #[default]
    None,
}

impl ActionDefaultRef for SearchResultEntryAction {
    fn default_ref() -> &'static Self {
        static DEFAULT: SearchResultEntryAction = SearchResultEntryAction::None;
        &DEFAULT
    }
}

#[derive(Clone, Debug)]
pub enum SearchRoomsModalAction {
    Open,
    Close,
}

#[derive(Script, ScriptHook, Widget)]
pub struct SearchResultEntry {
    #[deref]
    view: View,
    #[rust]
    nav_target: Option<SearchResultNavTarget>,
}

impl Widget for SearchResultEntry {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        match event.hits(cx, self.view.area()) {
            Hit::FingerUp(fe) if fe.is_over && fe.is_primary_hit() && fe.was_tap() => {
                if let Some(nav_target) = self.nav_target.clone() {
                    cx.widget_action(
                        self.widget_uid(),
                        SearchResultEntryAction::Clicked(nav_target),
                    );
                }
            }
            _ => {}
        }
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl SearchResultEntry {
    #[allow(dead_code)]
    fn set_result(&mut self, cx: &mut Cx, result: SearchResultNavTarget) {
        let style = result.visual_style();
        let (title, subtitle) = match &result {
            SearchResultNavTarget::JoinedRoom(room) => {
                (room.room_name_id.to_string(), room.subtitle.clone())
            }
            SearchResultNavTarget::Space(space) => {
                (space.space_name_id.to_string(), space.subtitle.clone())
            }
            SearchResultNavTarget::DirectoryRoom(room) => {
                (room.display_name(), room.subtitle())
            }
            SearchResultNavTarget::User(_) => return,
        };

        self.nav_target = Some(result);
        self.view
            .label(cx, ids!(title_row.kind_label))
            .set_text(cx, style.kind_label);
        self.view.label(cx, ids!(title_row.title)).set_text(cx, &title);
        self.view
            .label(cx, ids!(title_row.action_hint))
            .set_text(cx, style.action_hint);
        self.view.label(cx, ids!(subtitle)).set_text(cx, &subtitle);

        let mut kind_label = self.view.label(cx, ids!(title_row.kind_label));
        let mut action_hint = self.view.label(cx, ids!(title_row.action_hint));
        script_apply_eval!(cx, self, {
            draw_bg +: {
                color: #(style.card_bg)
                border_color: #(style.card_border)
            }
        });
        script_apply_eval!(cx, kind_label, {
            draw_bg +: { color: #(style.badge_bg) }
            draw_text +: { color: #(style.badge_fg) }
        });
        script_apply_eval!(cx, action_hint, {
            draw_text +: { color: #(style.action_fg) }
        });
    }
}

impl SearchResultEntryRef {
    #[allow(dead_code)]
    fn set_result(&self, cx: &mut Cx, result: SearchResultNavTarget) {
        let Some(mut inner) = self.borrow_mut() else {
            return;
        };
        inner.set_result(cx, result);
    }
}

#[derive(Script, ScriptHook, Widget)]
pub struct SearchUserEntry {
    #[deref]
    view: View,
    #[rust]
    nav_target: Option<SearchResultNavTarget>,
}

impl Widget for SearchUserEntry {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        match event.hits(cx, self.view.area()) {
            Hit::FingerUp(fe) if fe.is_over && fe.is_primary_hit() && fe.was_tap() => {
                if let Some(nav_target) = self.nav_target.clone() {
                    cx.widget_action(
                        self.widget_uid(),
                        SearchResultEntryAction::Clicked(nav_target),
                    );
                }
            }
            _ => {}
        }
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl SearchUserEntry {
    #[allow(dead_code)]
    fn set_result(&mut self, cx: &mut Cx, profile: UserProfile) {
        let style = SearchResultNavTarget::User(profile.clone()).visual_style();
        self.nav_target = Some(SearchResultNavTarget::User(profile.clone()));
        self.view
            .label(cx, ids!(details.title_row.kind_label))
            .set_text(cx, style.kind_label);
        self.view
            .label(cx, ids!(details.title_row.title))
            .set_text(cx, profile.displayable_name());
        self.view
            .label(cx, ids!(details.title_row.action_hint))
            .set_text(cx, style.action_hint);
        self.view
            .label(cx, ids!(details.subtitle))
            .set_text(cx, profile.user_id.as_str());

        let avatar = self.view.avatar(cx, ids!(avatar));
        let display_name = profile.displayable_name().to_string();
        match &profile.avatar_state {
            AvatarState::Loaded(data) => {
                let _ = avatar.show_image(cx, None, |cx, img| {
                    utils::load_png_or_jpg(&img, cx, data)
                });
            }
            AvatarState::Known(Some(uri)) => match avatar_cache::get_or_fetch_avatar(cx, uri) {
                AvatarCacheEntry::Loaded(data) => {
                    let _ = avatar.show_image(cx, None, |cx, img| {
                        utils::load_png_or_jpg(&img, cx, &data)
                    });
                }
                AvatarCacheEntry::Requested | AvatarCacheEntry::Failed => {
                    avatar.show_text(cx, None, None, &display_name);
                }
            },
            AvatarState::Known(None) | AvatarState::Unknown | AvatarState::Failed => {
                avatar.show_text(cx, None, None, &display_name);
            }
        }

        let mut kind_label = self.view.label(cx, ids!(details.title_row.kind_label));
        let mut action_hint = self.view.label(cx, ids!(details.title_row.action_hint));
        script_apply_eval!(cx, self, {
            draw_bg +: {
                color: #(style.card_bg)
                border_color: #(style.card_border)
            }
        });
        script_apply_eval!(cx, kind_label, {
            draw_bg +: { color: #(style.badge_bg) }
            draw_text +: { color: #(style.badge_fg) }
        });
        script_apply_eval!(cx, action_hint, {
            draw_text +: { color: #(style.action_fg) }
        });
    }
}

impl SearchUserEntryRef {
    #[allow(dead_code)]
    fn set_result(&self, cx: &mut Cx, profile: UserProfile) {
        let Some(mut inner) = self.borrow_mut() else {
            return;
        };
        inner.set_result(cx, profile);
    }
}

#[derive(Clone, Debug, Default)]
pub enum SearchResultsListAction {
    Activated(SearchResultNavTarget),
    #[default]
    None,
}

impl ActionDefaultRef for SearchResultsListAction {
    fn default_ref() -> &'static Self {
        static DEFAULT: SearchResultsListAction = SearchResultsListAction::None;
        &DEFAULT
    }
}

#[derive(Script, ScriptHook, Widget)]
pub struct SearchResultsList {
    #[deref]
    view: View,
    #[rust]
    results: Vec<SearchResultNavTarget>,
}

impl Widget for SearchResultsList {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        self.widget_match_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        while let Some(widget_to_draw) = self.view.draw_walk(cx, scope, walk).step() {
            let portal_list_ref = widget_to_draw.as_portal_list();
            let Some(mut list) = portal_list_ref.borrow_mut() else {
                continue;
            };

            list.set_item_range(cx, 0, self.results.len());
            while let Some(item_id) = list.next_visible_item(cx) {
                let Some(result) = self.results.get(item_id).cloned() else {
                    continue;
                };
                let item = match &result {
                    SearchResultNavTarget::User(_) => list.item(cx, item_id, id!(UserCard)),
                    _ => list.item(cx, item_id, id!(ResultCard)),
                };

                match &result {
                    SearchResultNavTarget::User(profile) => {
                        self.populate_user_item(cx, &item, profile, result.clone());
                    }
                    _ => {
                        self.populate_result_item(cx, &item, &result);
                    }
                }

                item.draw_all(cx, &mut Scope::empty());
            }
        }
        DrawStep::done()
    }
}

impl WidgetMatchEvent for SearchResultsList {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions, _scope: &mut Scope) {
        for action in actions {
            let Some(widget_action) = action.as_widget_action() else {
                continue;
            };
            let Some(target) = widget_action
                .data
                .as_ref()
                .and_then(|data| data.downcast_ref::<SearchResultNavTarget>())
            else {
                continue;
            };

            if matches!(
                widget_action.action.downcast_ref::<ButtonAction>(),
                Some(ButtonAction::Clicked(_))
            ) {
                cx.widget_action(
                    self.widget_uid(),
                    SearchResultsListAction::Activated(target.clone()),
                );
                continue;
            }

            if let Some(ViewAction::FingerUp(fe)) =
                widget_action.action.downcast_ref::<ViewAction>()
                && fe.is_over
                && fe.is_primary_hit()
                && fe.was_tap()
            {
                cx.widget_action(
                    self.widget_uid(),
                    SearchResultsListAction::Activated(target.clone()),
                );
            }
        }
    }
}

impl SearchResultsList {
    fn set_results(&mut self, cx: &mut Cx, results: Vec<SearchResultNavTarget>) {
        self.results = results;
        self.notify_results_changed(cx);
    }

    fn notify_results_changed(&mut self, cx: &mut Cx) {
        self.view
            .portal_list(cx, ids!(list))
            .set_first_id_and_scroll(0, 0.0);
        self.redraw(cx);
    }

    fn style_result_card(
        &self,
        cx: &mut Cx,
        card_root: &WidgetRef,
        kind_label: &LabelRef,
        action_button: &ButtonRef,
        style: &SearchResultVisualStyle,
    ) {
        let mut card_root = card_root.clone();
        let mut kind_label = kind_label.clone();
        let mut action_button = action_button.clone();
        script_apply_eval!(cx, card_root, {
            draw_bg +: {
                color: #(style.card_bg)
                border_color: #(style.card_border)
            }
        });
        script_apply_eval!(cx, kind_label, {
            draw_bg +: { color: #(style.badge_bg) }
            draw_text +: { color: #(style.badge_fg) }
        });
        script_apply_eval!(cx, action_button, {
            draw_bg +: {
                color: #(style.badge_bg)
                color_hover: #(style.badge_bg)
                color_down: #(style.card_border)
                border_color: #(style.card_border)
                border_color_hover: #(style.card_border)
                border_color_down: #(style.action_fg)
            }
            draw_text +: {
                color: #(style.action_fg)
                color_hover: #(style.action_fg)
                color_down: #(style.action_fg)
            }
        });
    }

    fn populate_result_item(
        &self,
        cx: &mut Cx,
        item: &WidgetRef,
        result: &SearchResultNavTarget,
    ) {
        let style = result.visual_style();
        let card_root = item.child_by_path(ids!(card_root));
        card_root.set_action_data_always(result.clone());

        let kind_label = item
            .child_by_path(ids!(card_root.title_row.kind_label))
            .as_label();
        kind_label.set_text(cx, style.kind_label);

        item.child_by_path(ids!(card_root.title_row.title))
            .as_label()
            .set_text(cx, &result.title_text());
        item.child_by_path(ids!(card_root.subtitle))
            .as_label()
            .set_text(cx, &result.subtitle_text());

        let action_button_ref = item.child_by_path(ids!(card_root.title_row.action_button));
        action_button_ref.set_action_data_always(result.clone());
        let action_button = action_button_ref.as_button();
        action_button.set_text(cx, result.action_button_text());

        self.style_result_card(cx, &card_root, &kind_label, &action_button, &style);
    }

    fn populate_user_item(
        &self,
        cx: &mut Cx,
        item: &WidgetRef,
        profile: &UserProfile,
        result: SearchResultNavTarget,
    ) {
        let style = result.visual_style();
        let card_root = item.child_by_path(ids!(card_root));
        card_root.set_action_data_always(result.clone());

        let kind_label = item
            .child_by_path(ids!(card_root.details.title_row.kind_label))
            .as_label();
        kind_label.set_text(cx, style.kind_label);

        item.child_by_path(ids!(card_root.details.title_row.title))
            .as_label()
            .set_text(cx, profile.displayable_name());
        item.child_by_path(ids!(card_root.details.subtitle))
            .as_label()
            .set_text(cx, profile.user_id.as_str());

        let action_button_ref = item.child_by_path(ids!(card_root.details.title_row.action_button));
        action_button_ref.set_action_data_always(result.clone());
        let action_button = action_button_ref.as_button();
        action_button.set_text(cx, result.action_button_text());

        let avatar = item.child_by_path(ids!(card_root.avatar)).as_avatar();
        let display_name = profile.displayable_name().to_string();
        match &profile.avatar_state {
            AvatarState::Loaded(data) => {
                let _ = avatar.show_image(cx, None, |cx, img| {
                    utils::load_png_or_jpg(&img, cx, data)
                });
            }
            AvatarState::Known(Some(uri)) => match avatar_cache::get_or_fetch_avatar(cx, uri) {
                AvatarCacheEntry::Loaded(data) => {
                    let _ = avatar.show_image(cx, None, |cx, img| {
                        utils::load_png_or_jpg(&img, cx, &data)
                    });
                }
                AvatarCacheEntry::Requested | AvatarCacheEntry::Failed => {
                    avatar.show_text(cx, None, None, &display_name);
                }
            },
            AvatarState::Known(None) | AvatarState::Unknown | AvatarState::Failed => {
                avatar.show_text(cx, None, None, &display_name);
            }
        }

        self.style_result_card(cx, &card_root, &kind_label, &action_button, &style);
    }
}

impl SearchResultsListRef {
    fn set_results(&self, cx: &mut Cx, results: Vec<SearchResultNavTarget>) {
        let Some(mut inner) = self.borrow_mut() else {
            return;
        };
        inner.set_results(cx, results);
    }
}

#[derive(Script, ScriptHook, Widget)]
pub struct SearchRoomsModal {
    #[deref]
    view: View,
    #[rust]
    query: String,
    #[rust]
    filters: SearchFilters,
    #[rust]
    local_results: Vec<SearchResultNavTarget>,
    #[rust]
    directory_results: Vec<DirectorySearchResult>,
    #[rust]
    people_results: Vec<UserProfile>,
    #[rust]
    results: Vec<SearchResultNavTarget>,
    #[rust]
    directory_loading: bool,
    #[rust]
    people_loading: bool,
    #[rust]
    people_limited: bool,
    #[rust]
    directory_error: Option<String>,
    #[rust]
    people_error: Option<String>,
    #[rust]
    directory_request_id: u64,
    #[rust]
    people_request_id: u64,
}

impl Widget for SearchRoomsModal {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if matches!(event, Event::Signal) {
            avatar_cache::process_avatar_updates(cx);
            if !self.people_results.is_empty() || !self.directory_results.is_empty() {
                self.redraw(cx);
            }
        }

        self.view.handle_event(cx, event, scope);
        if matches!(
            event.hits_with_capture_overload(cx, self.view.area(), true),
            Hit::FingerScroll(_)
        ) {
            return;
        }
        self.widget_match_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl WidgetMatchEvent for SearchRoomsModal {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions, _scope: &mut Scope) {
        if self.view.button(cx, ids!(close_button)).clicked(actions) {
            self.reset(cx);
            cx.action(SearchRoomsModalAction::Close);
            return;
        }

        if actions
            .iter()
            .any(|action| matches!(action.downcast_ref(), Some(ModalAction::Dismissed)))
        {
            self.reset(cx);
            return;
        }

        if self
            .view
            .button(cx, ids!(scope_panel.scope_buttons.scope_spaces_button))
            .clicked(actions)
        {
            self.filters.spaces = !self.filters.spaces;
            self.refresh_search(cx);
        }
        if self
            .view
            .button(cx, ids!(scope_panel.scope_buttons.scope_rooms_button))
            .clicked(actions)
        {
            self.filters.rooms = !self.filters.rooms;
            self.refresh_search(cx);
        }
        if self
            .view
            .button(cx, ids!(scope_panel.scope_buttons.scope_people_button))
            .clicked(actions)
        {
            self.filters.people = !self.filters.people;
            self.refresh_search(cx);
        }

        let input = self.view.text_input(cx, ids!(search_bar.input));
        let clear_button = self.view.button(cx, ids!(search_bar.clear_button));
        if let Some(keywords) = input.changed(actions) {
            let keywords_trimmed = keywords.trim();
            self.query = if keywords_trimmed.len() < keywords.len() {
                keywords_trimmed.to_string()
            } else {
                keywords
            };
            clear_button.set_visible(cx, !self.query.is_empty());
            clear_button.reset_hover(cx);
            self.refresh_search(cx);
        }
        if clear_button.clicked(actions) {
            input.set_text(cx, "");
            clear_button.set_visible(cx, false);
            input.set_key_focus(cx);
            self.query.clear();
            self.refresh_search(cx);
        }

        if self.view.button(cx, ids!(empty_add_room_button)).clicked(actions) {
            self.open_add_room_lookup(cx);
            return;
        }

        for action in actions {
            if let Some(SearchUsersAction::Results {
                request_id,
                query,
                results,
                limited,
            }) = action.downcast_ref()
            {
                if *request_id != self.people_request_id || query != &self.query {
                    continue;
                }
                self.people_loading = false;
                self.people_limited = *limited;
                self.people_error = None;
                self.people_results = results.clone();
                self.rebuild_results(cx);
                continue;
            }

            if let Some(SearchUsersAction::Failed {
                request_id,
                query,
                error,
            }) = action.downcast_ref()
            {
                if *request_id != self.people_request_id || query != &self.query {
                    continue;
                }
                self.people_loading = false;
                self.people_limited = false;
                self.people_results.clear();
                self.people_error = Some(error.clone());
                self.rebuild_results(cx);
                continue;
            }

            if let Some(SearchPublicRoomsAction::Results {
                request_id,
                query,
                results,
            }) = action.downcast_ref()
            {
                if *request_id != self.directory_request_id || query != &self.query {
                    continue;
                }
                self.directory_loading = false;
                self.directory_error = None;
                self.directory_results = results.clone();
                self.rebuild_results(cx);
                continue;
            }

            if let Some(SearchPublicRoomsAction::Failed {
                request_id,
                query,
                error,
            }) = action.downcast_ref()
            {
                if *request_id != self.directory_request_id || query != &self.query {
                    continue;
                }
                self.directory_loading = false;
                self.directory_results.clear();
                self.directory_error = Some(error.clone());
                self.rebuild_results(cx);
                continue;
            }

            if let SearchResultsListAction::Activated(nav_target) =
                action.as_widget_action().cast_ref()
            {
                match nav_target {
                    SearchResultNavTarget::JoinedRoom(room) => {
                        cx.widget_action(
                            self.widget_uid(),
                            RoomsListAction::Selected(if room.is_invite {
                                SelectedRoom::InvitedRoom {
                                    room_name_id: room.room_name_id.clone(),
                                }
                            } else {
                                SelectedRoom::JoinedRoom {
                                    room_name_id: room.room_name_id.clone(),
                                }
                            }),
                        );
                        self.reset(cx);
                        cx.action(SearchRoomsModalAction::Close);
                        return;
                    }
                    SearchResultNavTarget::Space(space) => {
                        cx.action(NavigationBarAction::GoToSpace {
                            space_name_id: space.space_name_id.clone(),
                        });
                        self.reset(cx);
                        cx.action(SearchRoomsModalAction::Close);
                        return;
                    }
                    SearchResultNavTarget::DirectoryRoom(directory_room) => {
                        if directory_room.is_joinable() {
                            submit_async_request(MatrixRequest::JoinRoom {
                                room_id: directory_room.room_id.clone(),
                            });
                            self.reset(cx);
                            cx.action(SearchRoomsModalAction::Close);
                        } else {
                            self.open_add_room_lookup_query(cx, directory_room.lookup_query());
                        }
                        return;
                    }
                    SearchResultNavTarget::User(user_profile) => {
                        submit_async_request(MatrixRequest::OpenOrCreateDirectMessage {
                            user_profile: user_profile.clone(),
                            allow_create: true,
                        });
                        self.reset(cx);
                        cx.action(SearchRoomsModalAction::Close);
                        return;
                    }
                }
            }
        }
    }
}

impl SearchRoomsModal {
    fn build_local_results(&self, cx: &mut Cx, keywords: &str) -> Vec<SearchResultNavTarget> {
        if keywords.trim().is_empty() {
            return Vec::new();
        }

        let mut results = Vec::new();
        if cx.has_global::<RoomsListRef>() && (self.filters.searches_rooms() || self.filters.searches_people()) {
            let rooms_list = cx.get_global::<RoomsListRef>();
            results.extend(
                rooms_list
                    .search_results(keywords)
                    .into_iter()
                    .filter(|room| {
                        (room.is_direct && self.filters.searches_people())
                            || (!room.is_direct && self.filters.searches_rooms())
                    })
                    .map(SearchResultNavTarget::JoinedRoom),
            );
        }
        if self.filters.searches_spaces() && cx.has_global::<SpacesBarRef>() {
            let spaces_bar = cx.get_global::<SpacesBarRef>();
            results.extend(
                spaces_bar
                    .search_results(keywords)
                    .into_iter()
                    .map(SearchResultNavTarget::Space),
            );
        }
        results
    }

    fn rebuild_results(&mut self, cx: &mut Cx) {
        let mut seen_room_ids = std::collections::HashSet::new();
        let mut seen_user_ids = std::collections::HashSet::new();

        self.results.clear();
        for result in self.local_results.iter().cloned() {
            match &result {
                SearchResultNavTarget::User(user) => {
                    if seen_user_ids.insert(user.user_id.to_string()) {
                        self.results.push(result);
                    }
                }
                _ => {
                    if result
                        .room_id()
                        .is_none_or(|room_id| seen_room_ids.insert(room_id.clone()))
                    {
                        self.results.push(result);
                    }
                }
            }
        }

        for directory_room in self.directory_results.iter().cloned() {
            if seen_room_ids.insert(directory_room.room_id.clone()) {
                self.results
                    .push(SearchResultNavTarget::DirectoryRoom(directory_room));
            }
        }

        for user in self.people_results.iter().cloned() {
            if seen_user_ids.insert(user.user_id.to_string()) {
                self.results.push(SearchResultNavTarget::User(user));
            }
        }
        self.results.truncate(50);
        self.sync_scope_buttons(cx);
        self.sync_text(cx);
        self.sync_visibility(cx);
        self.view
            .child_by_path(ids!(search_results_list))
            .as_search_results_list()
            .set_results(cx, self.results.clone());
        self.redraw(cx);
    }

    fn refresh_search(&mut self, cx: &mut Cx) {
        self.local_results = self.build_local_results(cx, &self.query);
        self.directory_results.clear();
        self.directory_error = None;
        self.people_results.clear();
        self.people_error = None;
        self.people_limited = false;
        self.directory_request_id = self.directory_request_id.wrapping_add(1);
        self.people_request_id = self.people_request_id.wrapping_add(1);

        if !self.query.is_empty() && (self.filters.searches_spaces() || self.filters.searches_rooms()) {
            self.directory_loading = true;
            submit_async_request(MatrixRequest::SearchPublicRooms {
                search_term: self.query.clone(),
                limit: 20,
                include_spaces: self.filters.searches_spaces(),
                include_rooms: self.filters.searches_rooms(),
                request_id: self.directory_request_id,
            });
        } else {
            self.directory_loading = false;
        }

        if !self.query.is_empty() && self.filters.searches_people() {
            self.people_loading = true;
            submit_async_request(MatrixRequest::SearchUsers {
                search_term: self.query.clone(),
                limit: 20,
                request_id: self.people_request_id,
            });
        } else {
            self.people_loading = false;
        }

        self.rebuild_results(cx);
    }

    fn status_text(&self) -> String {
        if self.query.is_empty() {
            return self.filters.hint_text().to_string();
        }

        if (self.people_loading || self.directory_loading) && self.results.is_empty() {
            return format!("Searching \"{}\"...", self.query);
        }

        if self.results.is_empty() {
            if let Some(error) = &self.directory_error {
                return format!("Public room search failed: {error}");
            }
            if let Some(error) = &self.people_error {
                return format!("People search failed: {error}");
            }
            return format!("No results found for \"{}\".", self.query);
        }

        let mut suffix = String::new();
        if self.directory_loading || self.people_loading {
            suffix.push_str(" More server results are still loading.");
        }
        if self.people_limited {
            suffix.push_str(" Showing a limited set of people results.");
        }
        format!(
            "Found {} result(s) for \"{}\".{}",
            self.results.len(),
            self.query,
            suffix
        )
    }

    fn sync_visibility(&mut self, cx: &mut Cx) {
        self.view
            .view(cx, ids!(loading_state))
            .set_visible(cx, self.people_loading || self.directory_loading);
        self.view
            .view(cx, ids!(empty_results_state))
            .set_visible(
                cx,
                !self.query.is_empty()
                    && !self.people_loading
                    && !self.directory_loading
                    && self.results.is_empty(),
            );
    }

    fn apply_scope_button_style(&mut self, cx: &mut Cx, id: &[LiveId], active: bool) {
        let mut button = self.view.button(cx, id);
        if active {
            script_apply_eval!(cx, button, {
                draw_bg +: {
                    color: mod.widgets.COLOR_ACTIVE_PRIMARY
                    color_hover: mod.widgets.COLOR_ACTIVE_PRIMARY_DARKER
                    color_down: #x0c5daa
                    border_color: mod.widgets.COLOR_ACTIVE_PRIMARY
                    border_color_hover: mod.widgets.COLOR_ACTIVE_PRIMARY_DARKER
                    border_color_down: #x0c5daa
                }
                draw_text +: {
                    color: mod.widgets.COLOR_PRIMARY
                    color_hover: mod.widgets.COLOR_PRIMARY
                    color_down: mod.widgets.COLOR_PRIMARY
                }
            });
        } else {
            script_apply_eval!(cx, button, {
                draw_bg +: {
                    color: #xF6F8FC
                    color_hover: #xEDF2FA
                    color_down: #xE2ECFF
                    border_color: #xDCE4F1
                    border_color_hover: #xC8D7EB
                    border_color_down: #xB9CDEF
                }
                draw_text +: {
                    color: #x445064
                    color_hover: #x223046
                    color_down: #x223046
                }
            });
        }
    }

    fn sync_scope_buttons(&mut self, cx: &mut Cx) {
        self.apply_scope_button_style(
            cx,
            ids!(scope_panel.scope_buttons.scope_spaces_button),
            self.filters.spaces,
        );
        self.apply_scope_button_style(
            cx,
            ids!(scope_panel.scope_buttons.scope_rooms_button),
            self.filters.rooms,
        );
        self.apply_scope_button_style(
            cx,
            ids!(scope_panel.scope_buttons.scope_people_button),
            self.filters.people,
        );
    }

    fn sync_text(&mut self, cx: &mut Cx) {
        self.view
            .label(cx, ids!(scope_panel.scope_hint))
            .set_text(cx, self.filters.hint_text());
        self.view
            .label(cx, ids!(status_label))
            .set_text(cx, &self.status_text());

        let loading_label = if self.people_loading && self.directory_loading {
            "Searching public rooms, spaces, and homeserver users..."
        } else if self.directory_loading {
            "Searching public rooms and spaces on the current homeserver..."
        } else {
            "Searching people on the current homeserver..."
        };
        self.view
            .label(cx, ids!(loading_state.loading_label))
            .set_text(cx, loading_label);

        let empty_body = if let Some(error) = &self.directory_error {
            format!(
                "Public room search failed: {error}. You can still try another keyword, switch search options above, or open Add/Explore lookup."
            )
        } else if let Some(error) = &self.people_error {
            format!(
                "People search failed: {error}. You can still try another keyword, turn off People, or open Add/Explore lookup."
            )
        } else {
            String::from(
                "Try another keyword, switch search options above, or open Add/Explore lookup.",
            )
        };
        self.view
            .label(cx, ids!(empty_body))
            .set_text(cx, &empty_body);
    }

    fn open_add_room_lookup(&mut self, cx: &mut Cx) {
        self.open_add_room_lookup_query(cx, self.query.clone());
    }

    fn open_add_room_lookup_query(&mut self, cx: &mut Cx, query: String) {
        cx.action(NavigationBarAction::GoToAddRoom);
        cx.action(AddRoomScreenAction::PrefillJoinLookup(query));
        self.reset(cx);
        cx.action(SearchRoomsModalAction::Close);
    }

    fn reset(&mut self, cx: &mut Cx) {
        self.query.clear();
        self.filters = SearchFilters::default();
        self.local_results.clear();
        self.directory_results.clear();
        self.people_results.clear();
        self.results.clear();
        self.directory_loading = false;
        self.people_loading = false;
        self.people_limited = false;
        self.directory_error = None;
        self.people_error = None;
        self.directory_request_id = self.directory_request_id.wrapping_add(1);
        self.people_request_id = self.people_request_id.wrapping_add(1);
        self.view
            .text_input(cx, ids!(search_bar.input))
            .set_text(cx, "");
        self.view
            .button(cx, ids!(search_bar.clear_button))
            .set_visible(cx, false);
        self.rebuild_results(cx);
    }

    pub fn show(&mut self, cx: &mut Cx) {
        self.reset(cx);
        self.view
            .text_input(cx, ids!(search_bar.input))
            .set_key_focus(cx);
    }
}

impl SearchRoomsModalRef {
    pub fn show(&self, cx: &mut Cx) {
        let Some(mut inner) = self.borrow_mut() else {
            return;
        };
        inner.show(cx);
    }
}
