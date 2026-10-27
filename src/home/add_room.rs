//! A top-level view for adding (joining) or exploring new rooms and spaces, and for finding users.


use std::collections::HashSet;

use makepad_widgets::*;
use matrix_sdk::RoomState;
use ruma::{OwnedRoomOrAliasId, OwnedServerName, RoomId, room::{JoinRuleSummary, RoomType}};

use crate::{app::AppStateAction, avatar_cache::{self, AvatarCacheEntry}, directory_search::{DirectorySearchAction, RoomKind, RoomSearchResult, SEARCH_PAGE_SIZE, SearchQuery, UserSearchResult}, home::invite_screen::{JoinRoomResultAction, LeaveRoomResultAction}, logout::logout_confirm_modal::LogoutAction, profile::{user_profile::{UserProfilePaneInfo, UserProfileSlidingPaneWidgetExt, UserProfileSlidingPaneWidgetRefExt}, user_profile_cache}, room::{FetchedRoomAvatar, FetchedRoomPreview, RoomPreviewAction}, shared::{avatar::{AvatarWidgetRefExt, TextAvatarBgColor}, collapsible_header::{CollapsibleHeaderAction, CollapsibleHeaderWidgetExt}, list_rows::{draw_flat_list_status_row, handle_row_actions}, popup_list::{PopupKind, enqueue_popup_notification}, styles::set_view_bg_color}, sliding_sync::{MatrixRequest, RoomPreviewResponseMode, submit_async_request}, utils};

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*


    // A row with a search box and a button that searches right away.
    let SearchInputRow = View {
        width: Fill,
        height: Fit,
        margin: Inset{ top: 3, bottom: 4 }
        align: Align{y: 0.5}
        spacing: 5
        flow: Right
    }

    let SearchInput = RobrixTextInput {
        margin: Inset{top: 0, left: 5, right: 5, bottom: 0},
        padding: Inset{left: 12, right: 12, top: 0, bottom: 0}
        width: Fill { max: 400 } // same width as the above `help_info`
        height: 40
        autocapitalize: None,
        autocorrect: Disabled,
    }

    let SearchButton = RobrixIconButton {
        padding: Inset{top: 10, bottom: 10, left: 12, right: 14}
        height: 40
        draw_icon.svg: (ICON_SEARCH)
        icon_walk: Walk{width: 16, height: 16}
        text: "Go"
    }

    // Search results in sections, each with an optional header, its rows, and a "Load more" button.
    let SearchResultsList = View {
        width: Fill, height: Fit
        list := FlatList {
            width: Fill, height: Fit
            margin: Inset{left: 5, right: 5, bottom: 5}
            scroll_bars: ScrollBars { show_scroll_x: false, show_scroll_y: false }

            section_header := SubsectionLabel {
                margin: Inset{top: 12, bottom: 4}
                draw_text +: { text_style: theme.font_bold { font_size: 11.5 } }
            }
            result_row := mod.widgets.AvatarListRow {}
            // The rows below are left-aligned with the results' avatars.
            loading_row := mod.widgets.ListLoadingRow {
                align: Align{x: 0.0, y: 0.5}
                padding: Inset{left: 10, right: 10, top: 13, bottom: 13}
            }
            empty_row := mod.widgets.ListEmptyRow {
                padding: Inset{left: 10, right: 10, top: 13, bottom: 13}
                empty_label +: { padding: theme.mspace_1{left: 0.}, align: Align{x: 0.0, y: 0.0} }
            }
            load_more_row := View {
                width: Fill, height: Fit
                padding: Inset{left: 10, right: 10, top: 5, bottom: 5}
                load_more_button := RobrixNeutralIconButton {
                    padding: Inset{top: 8, bottom: 8, left: 14, right: 14}
                    text: "Load more"
                }
            }
        }
    }

    // The scrollable content of the AddRoomScreen, beneath its user profile pane.
    let AddRoomScreenContent = ScrollYView {
        width: Fill, height: Fill,
        flow: Down,
        padding: Inset{top: 5, left: 15, right: 15, bottom: 0},

        title := TitleLabel {
            text: "Add Rooms, Spaces, and People"
            draw_text +: {
                text_style: theme.font_regular {font_size: 18},
            }
        }
        
        LineH { padding: 10, margin: Inset{top: 10, right: 2} }

        SubsectionLabel {
            text: "Find and join rooms or spaces"
        }

        search_tips_header := CollapsibleHeader {
            width: Fit, height: 30
            margin: Inset{left: 5, right: 5, top: 3, bottom: 3}
            padding: Inset{left: 5, right: 12, top: 5, bottom: 5}
            starts_expanded: false
            draw_bg +: { color: (COLOR_BG_LAVENDER) }
            collapse_icon +: { draw_bg +: { color: (ROOM_NAME_TEXT_COLOR) } }
            label +: {
                width: Fit
                text: "What you can search for:"
                draw_text +: { color: (ROOM_NAME_TEXT_COLOR) }
            }
        }

        // An Html widget can't be hidden itself, so we hide this view around it.
        help_view := View {
            visible: false
            width: Fill, height: Fit
            help_info := MessageHtml {
                padding: 7
                width: Fill, height: Fit
                font_size: 10.
                font_color: (MESSAGE_TEXT_COLOR)
                body: "<ul>
                      <li> A room or space name like <code>Robrix</code>.</li>
                      <li> Keywords in a public room or space topic/description.</li>
                      <li> An <i>alias</i>, starting with <code>#</code>, like <code>#robrix:matrix.org</code>.</li>
                      <li> An <i>ID</i>, starting with <code>!</code>, like <code>!room_id:matrix.org</code>.</li>
                      <li> A Matrix link, like <code>matrix:...</code> or <code>https://matrix.to/...</code>.</li>
                    </ul>
                "
            }
        }

        join_room_view := SearchInputRow {
            room_alias_id_input := SearchInput {
                empty_text: "Enter a name, alias, ID, or Matrix link..."
            }
            search_for_room_button := SearchButton {}
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
                    stroke_width: 3.0
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
            width: Fill
            height: Fit
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

        // The results of searching for rooms and spaces, in separate lists.
        room_results := SearchResultsList {}

        LineH { padding: 10, margin: Inset{top: 15, right: 2} }

        SubsectionLabel {
            text: "Find people"
        }

        user_help_info := MessageHtml {
            padding: 7
            width: Fill, height: Fit
            font_size: 10.
            font_color: (MESSAGE_TEXT_COLOR)
            body: "<p>Search by name or user ID (<code>@alice:matrix.org</code>).</p>
            "
        }

        find_user_view := SearchInputRow {
            user_search_input := SearchInput {
                empty_text: "Enter a name or user ID..."
            }
            search_for_user_button := SearchButton {}
        }

        user_results := SearchResultsList {}

        View {
            width: Fill
            height: 20
        }
    }

    // The main view that allows the user to add (join) or explore new rooms/spaces, and find people.
    mod.widgets.AddRoomScreen = #(AddRoomScreen::register_widget(vm)) {
        width: Fill, height: Fill
        flow: Overlay
        result_row_color: (COLOR_TRANSPARENT)
        result_row_color_hover: (COLOR_LIST_ITEM_BG_HOVER)
        result_row_color_focus: (COLOR_LIST_ITEM_BG_FOCUS)

        content := AddRoomScreenContent {}

        // Shown upon clicking a person in the search results, on top of all other content.
        user_profile_sliding_pane := mod.widgets.UserProfileSlidingPane { }
    }
}

/// Delay after the last keystroke before we search for rooms, spaces, or people.
const SEARCH_DELAY: f64 = 0.5;

#[derive(Script, Widget)]
pub struct AddRoomScreen {
    #[deref] view: View,
    #[live] result_row_color: Vec4f,
    #[live] result_row_color_hover: Vec4f,
    #[live] result_row_color_focus: Vec4f,

    #[rust] state: AddRoomState,
    /// The function to perform when the user clicks the `join_room_button`.
    #[rust(JoinButtonFunction::None)] join_function: JoinButtonFunction,
    #[rust] room_search: Search<RoomResults>,
    #[rust] user_search: Search<SectionResults<UserSearchResult>>,
    /// The search box whose only match we'll open once we've looked it up, since the user pressed Enter before we had.
    #[rust] only_match_to_open: Option<SearchBox>,
    #[rust] result_rows: ResultRows,
    #[rust] was_profile_pane_shown: bool,
}

impl ScriptHook for AddRoomScreen {
    fn on_after_reload(&mut self, _vm: &mut ScriptVm) {
        // Some reloads reset the rows' avatars to their template, so we populate every row again.
        self.result_rows.populated.clear();
    }
}

#[derive(Default)]
#[allow(clippy::large_enum_variant)]
enum AddRoomState {
    /// We're waiting for the user to enter what to search for, or to pick a search result.
    #[default]
    WaitingOnUserInput,
    /// We successfully parsed the user input and have sent a request.
    /// We're now waiting for the room preview to be fetched and returned.
    Parsed {
        room_or_alias_id: OwnedRoomOrAliasId,
    },
    /// The user entered invalid input that we couldn't parse into a room address.
    ParseError(String),
    /// We successfully fetched the room preview and have displayed it,
    /// and are waiting for the user to join the room.
    FetchedRoomPreview {
        frp: FetchedRoomPreview,
        room_or_alias_id: OwnedRoomOrAliasId,
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

impl Widget for AddRoomScreen {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        let profile_pane = self.view.child(id!(user_profile_sliding_pane)).as_user_profile_sliding_pane();

        // The arrow keys move through the results of the search box that the user is typing in,
        // and Enter opens the result they moved to, so we handle those keys before that search box can.
        if let Event::KeyDown(key_event) = event
            && !profile_pane.is_currently_shown(cx)
        {
            let is_room_box_focused = cx.has_key_focus(self.view.text_input(cx, ids!(room_alias_id_input)).area());
            let is_people_box_focused = cx.has_key_focus(self.view.text_input(cx, ids!(user_search_input)).area());
            let is_enter = matches!(key_event.key_code, KeyCode::ReturnKey | KeyCode::NumpadEnter);
            // Holding down Enter shouldn't submit a search box or open a result over and over.
            if is_enter && key_event.is_repeat && (is_room_box_focused || is_people_box_focused) {
                return;
            }
            let search_box = if key_event.modifiers.any() {
                None
            } else if is_room_box_focused && self.are_room_results_shown() {
                Some(SearchBox::Rooms)
            } else if is_people_box_focused {
                Some(SearchBox::People)
            } else {
                None
            };
            let is_handled = match (search_box, key_event.key_code) {
                (Some(search_box), KeyCode::ArrowDown) => self.move_result_focus(cx, search_box, true),
                (Some(search_box), KeyCode::ArrowUp) => self.move_result_focus(cx, search_box, false),
                (Some(search_box), _) if is_enter => match self.result_rows.focused {
                    Some((focused_box, key)) if focused_box == search_box => {
                        match search_box {
                            SearchBox::Rooms => self.open_room_result(cx, key),
                            SearchBox::People => self.open_user_result(cx, key),
                        }
                        true
                    }
                    _ => false,
                },
                _ => false,
            };
            if is_handled {
                return;
            }
        }

        // While the profile pane is shown, it gets all of the user's input.
        if utils::is_interactive_hit_event(event) && profile_pane.is_currently_shown(cx) {
            profile_pane.handle_event(cx, event, scope);
        } else {
            self.view.handle_event(cx, event, scope);
        }

        // The profile pane doesn't always give key focus back when it closes, so we give it back
        // to the people search box, letting the user carry on moving through its results.
        let is_profile_pane_shown = profile_pane.is_currently_shown(cx);
        if self.was_profile_pane_shown
            && !is_profile_pane_shown
            && self.result_rows.focused.is_some_and(|(search_box, _)| search_box == SearchBox::People)
        {
            self.view.text_input(cx, ids!(user_search_input)).set_key_focus(cx);
        }
        self.was_profile_pane_shown = is_profile_pane_shown;

        if self.room_search.timer.is_event(event).is_some() {
            let query = self.view.text_input(cx, ids!(room_alias_id_input)).text();
            self.search_rooms(cx, query.trim());
        }
        if self.user_search.timer.is_event(event).is_some() {
            let query = self.view.text_input(cx, ids!(user_search_input)).text();
            self.search_users(cx, query.trim());
        }

        // An overlay can take a row's hover-out event, so we un-hover it ourselves.
        if let Event::ClearHover = event && self.result_rows.hovered_key.take().is_some() {
            self.redraw(cx);
        }

        if let Event::Actions(actions) = event {
            let room_alias_id_input = self.view.text_input(cx, ids!(room_alias_id_input));
            let search_for_room_button = self.view.button(cx, ids!(search_for_room_button));
            let user_search_input = self.view.text_input(cx, ids!(user_search_input));
            let search_for_user_button = self.view.button(cx, ids!(search_for_user_button));
            let cancel_button = self.view.button(cx, ids!(fetched_room_summary.buttons_view.cancel_button));
            let join_room_button = self.view.button(cx, ids!(fetched_room_summary.buttons_view.join_room_button));

            let search_tips_header = self.view.collapsible_header(cx, ids!(search_tips_header));
            if let Some(action) = actions.find_widget_action(search_tips_header.widget_uid())
                && let CollapsibleHeaderAction::Toggled { .. } = action.cast()
            {
                self.view.view(cx, ids!(help_view)).set_visible(cx, search_tips_header.is_expanded());
                self.redraw(cx);
            }

            // Enable or disable the button based on if the text input is empty.
            if let Some(text) = room_alias_id_input.changed(actions) {
                search_for_room_button.set_enabled(cx, !text.trim().is_empty());
                self.room_search.handle_text_changed(cx, &text);
                // Typing in either box means the user has moved on from any result they moved to or pressed Enter to open.
                self.only_match_to_open = None;
                self.result_rows.focused = None;
                // The user is looking for something else now, so we stop showing the room preview.
                self.state = AddRoomState::WaitingOnUserInput;
                self.redraw(cx);
            }
            if let Some(text) = user_search_input.changed(actions) {
                search_for_user_button.set_enabled(cx, !text.trim().is_empty());
                self.user_search.handle_text_changed(cx, &text);
                self.only_match_to_open = None;
                self.result_rows.focused = None;
                self.redraw(cx);
            }
            // We only show the arrow keys' focus while the user is in that result's search box,
            // or looking at the profile of a person they opened from it.
            if !profile_pane.is_currently_shown(cx)
                && ((room_alias_id_input.key_focus_lost(actions) && self.result_rows.unfocus(SearchBox::Rooms))
                    || (user_search_input.key_focus_lost(actions) && self.result_rows.unfocus(SearchBox::People)))
            {
                self.redraw(cx);
            }

            // If the cancel button was clicked, hide the room preview and return to default state.
            if cancel_button.clicked(actions) {
                self.state = AddRoomState::WaitingOnUserInput;
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
                        AddRoomState::FetchedRoomPreview { frp, room_or_alias_id }
                    ) => {
                        submit_async_request(MatrixRequest::Knock {
                            room_or_alias_id: frp.canonical_alias.clone().map_or_else(
                                || room_or_alias_id.clone(),
                                Into::into
                            ),
                            reason: None,
                            server_names: frp.via.clone(),
                        });
                    }
                    _ => { }
                }
            }

            // Submitting takes key focus away from a search box, but with a physical keyboard
            // the user can then keep typing or move through the results with the arrow keys.
            let should_keep_key_focus = !cfg!(any(target_os = "android", target_os = "ios")) || cx.keyboard.has_physical_keyboard();

            // If the button was clicked or enter was pressed, search right away.
            if let Some(query) = get_submitted_query(&room_alias_id_input, &search_for_room_button, actions) {
                if should_keep_key_focus {
                    room_alias_id_input.set_key_focus(cx);
                }
                self.search_rooms(cx, &query);
                let only_match = self.room_search.results.as_ref()
                    .and_then(|results| get_only_match(&query, &[&results.rooms, &results.spaces]));
                match (only_match, SearchQuery::parse(&query).and_then(|search_query| search_query.full_room_address())) {
                    // We already have the preview of the query's only match.
                    (Some(Some(key)), _) => self.open_room_result(cx, key),
                    (_, Some((room_or_alias_id, via))) => self.request_room_preview(cx, room_or_alias_id, via),
                    // Other text may match several rooms, so we wait to see whether it matches just one.
                    (None, None) => self.only_match_to_open = Some(SearchBox::Rooms),
                    // It matches no rooms, or several of them.
                    (Some(None), None) => self.only_match_to_open = None,
                }
            }
            if let Some(query) = get_submitted_query(&user_search_input, &search_for_user_button, actions) {
                if should_keep_key_focus {
                    user_search_input.set_key_focus(cx);
                }
                self.search_users(cx, &query);
                self.only_match_to_open = Some(SearchBox::People);
                self.open_only_match(cx);
            }

            // If we're waiting for the room preview to be fetched (i.e., in the Parsed state),
            // then check if we've received it via an action.
            if let AddRoomState::Parsed { room_or_alias_id } = &self.state {
                for action in actions {
                    match action.downcast_ref() {
                        Some(RoomPreviewAction::Fetched { room_or_alias_id: fetched_id, result: Ok(frp) })
                            if fetched_id == room_or_alias_id =>
                        {
                            let room_or_alias_id = room_or_alias_id.clone();
                            self.show_room_preview(cx, frp.clone(), room_or_alias_id);
                            break;
                        }
                        Some(RoomPreviewAction::Fetched { room_or_alias_id: fetched_id, result: Err(e) })
                            if fetched_id == room_or_alias_id =>
                        {
                            let err_str = format!("Failed to fetch room info.\n\nError: {e}.");
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
            if let AddRoomState::FetchedRoomPreview { frp, .. } = &self.state {
                for action in actions {
                    if let Some(KnockResultAction::Knocked { room, .. }) = action.downcast_ref()
                        && room.room_id() == frp.room_name_id.room_id()
                    {
                        transition_to_knocked = true;
                        break;
                    }

                    match action.downcast_ref() {
                        // Don't show success/failure popups here, they're shown in the backend task
                        // which is more consistent than doing it here, as the user may have left the AddRoom screen by then.
                        Some(JoinRoomResultAction::Joined { room_id }) if room_id == frp.room_name_id.room_id() => {
                            transition_to_joined = true;
                            break;
                        }
                        Some(JoinRoomResultAction::Failed { room_id, .. }) if room_id == frp.room_name_id.room_id() => {
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
                    self.record_room_state(cx, room_name_id.room_id(), if *is_invite { RoomState::Invited } else { RoomState::Joined });
                }
                // We show a found room's preview as soon as the user picks it, so we keep our membership in it up to date.
                if let Some(JoinRoomResultAction::Joined { room_id }) = action.downcast_ref() {
                    self.record_room_state(cx, room_id, RoomState::Joined);
                }
                if let Some(KnockResultAction::Knocked { room, .. }) = action.downcast_ref() {
                    self.record_room_state(cx, room.room_id(), RoomState::Knocked);
                }
                if let Some(LeaveRoomResultAction::Left { room_id }) = action.downcast_ref() {
                    self.record_room_state(cx, room_id, RoomState::Left);
                }

                match action.downcast_ref() {
                    Some(DirectorySearchAction::JoinedRooms { query, rooms }) if *query == self.room_search.query => {
                        if let Some(results) = self.room_search.results.as_mut() {
                            results.add_results(rooms, &mut self.result_rows);
                        }
                        self.redraw(cx);
                    }
                    Some(DirectorySearchAction::ExactRooms { query, rooms }) if *query == self.room_search.query => {
                        if let Some(results) = self.room_search.results.as_mut() {
                            results.add_results(rooms, &mut self.result_rows);
                            results.rooms.has_exact_matches = true;
                            results.spaces.has_exact_matches = true;
                        }
                        self.open_only_match(cx);
                        self.redraw(cx);
                    }
                    Some(DirectorySearchAction::ExactUsers { query, users }) if *query == self.user_search.query => {
                        if let Some(section) = self.user_search.results.as_mut() {
                            section.add_results(users, &mut self.result_rows);
                            section.has_exact_matches = true;
                        }
                        self.open_only_match(cx);
                        self.redraw(cx);
                    }
                    Some(DirectorySearchAction::DirectoryRooms { query, kind, rooms, next_batch, is_next_page, error })
                        if *query == self.room_search.query =>
                    {
                        let Some(results) = self.room_search.results.as_mut() else { continue };
                        let section = results.section_mut(*kind);
                        // We've started a new search since asking for this next page.
                        if *is_next_page && !section.has_directory_matches { continue }
                        section.add_results(rooms, &mut self.result_rows);
                        section.has_directory_matches = true;
                        // Upon an error, we keep the old token so the user can try again.
                        if error.is_none() {
                            section.more = next_batch.clone().map(MoreResults::NextBatch);
                        }
                        section.error = error.clone();
                        section.is_loading_more = false;
                        self.open_only_match(cx);
                        self.redraw(cx);
                    }
                    Some(DirectorySearchAction::DirectoryUsers { query, users, limit, is_limited, error })
                        if *query == self.user_search.query =>
                    {
                        let Some(section) = self.user_search.results.as_mut() else { continue };
                        // A bigger search gives us the users we already have plus some more.
                        section.add_results(users, &mut self.result_rows);
                        section.has_directory_matches = true;
                        // Upon an error, we keep the old limit so the user can try again.
                        if error.is_none() {
                            section.more = is_limited.then(|| MoreResults::Limit(limit + SEARCH_PAGE_SIZE));
                        }
                        section.error = error.clone();
                        section.is_loading_more = false;
                        self.open_only_match(cx);
                        self.redraw(cx);
                    }
                    _ => {}
                }

                if let Some(LogoutAction::ClearAppState { .. }) = action.downcast_ref() {
                    // The next account to log in shouldn't see this account's searches.
                    cx.stop_timer(self.room_search.timer);
                    cx.stop_timer(self.user_search.timer);
                    self.room_search = Search::default();
                    self.user_search = Search::default();
                    self.only_match_to_open = None;
                    self.result_rows = ResultRows::default();
                    self.state = AddRoomState::WaitingOnUserInput;
                    room_alias_id_input.set_text(cx, "");
                    user_search_input.set_text(cx, "");
                    profile_pane.reset(cx);
                    self.redraw(cx);
                }
            }

            let room_items = self.view.flat_list(cx, ids!(room_results.list)).items_with_actions(actions);
            for (key, item) in &room_items {
                let section = match *key {
                    key if key == ROOMS_SECTION.load_more_key => ResultSection::Rooms,
                    key if key == SPACES_SECTION.load_more_key => ResultSection::Spaces,
                    _ => continue,
                };
                if item.button(cx, ids!(load_more_button)).clicked(actions) {
                    self.load_more_results(cx, section);
                }
            }
            let (tapped_room, is_room_hover_changed) = handle_row_actions(room_items, false, actions, &mut self.result_rows.hovered_key);
            if let Some(key) = tapped_room {
                self.open_room_result(cx, key);
            }

            let user_items = self.view.flat_list(cx, ids!(user_results.list)).items_with_actions(actions);
            for (key, item) in &user_items {
                if *key == PEOPLE_SECTION.load_more_key && item.button(cx, ids!(load_more_button)).clicked(actions) {
                    self.load_more_results(cx, ResultSection::People);
                }
            }
            let (tapped_user, is_user_hover_changed) = handle_row_actions(user_items, false, actions, &mut self.result_rows.hovered_key);
            if let Some(key) = tapped_user {
                self.open_user_result(cx, key);
            }

            if is_room_hover_changed || is_user_hover_changed {
                self.redraw(cx);
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
            AddRoomState::Parsed { room_or_alias_id, .. } => {
                loading_room_view.set_visible(cx, true);
                loading_room_view.label(cx, ids!(loading_text)).set_text(
                    cx,
                    &format!("Fetching {room_or_alias_id}..."),
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
                    // We may not have this avatar's image yet, e.g., for a search result, so the avatar cache fetches it.
                    FetchedRoomAvatar::Text(_) => {
                        room_avatar.show_room(cx, frp.room_name_id.room_id(), frp.avatar_url.as_ref(), frp.room_name_id.name_for_avatar());
                    }
                    FetchedRoomAvatar::Image(avatar_image) => {
                        let res = room_avatar.show_image(
                            cx,
                            None,
                            |cx, img_ref| utils::load_avatar_image(&img_ref, cx, avatar_image),
                        );
                        if res.is_err() {
                            room_avatar.show_text(
                                cx,
                                Some(TextAvatarBgColor::UnknownRoom),
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
                    _ => room_name.set_text(cx, &format!("Unnamed {room_or_space_uc}, ID: {}", frp.room_name_id.room_id())),
                }

                fetched_room_summary.label(cx, ids!(subsection_alias_id)).set_text(
                    cx,
                    &format!("Main {room_or_space_uc} Alias and ID"),
                );
                fetched_room_summary.label(cx, ids!(room_alias)).set_text(
                    cx,
                    &format!("Alias: {}", frp.canonical_alias.as_ref().map_or("not set", |a| a.as_str())),
                );
                fetched_room_summary.label(cx, ids!(room_id)).set_text(
                    cx,
                    &format!("ID: {}", frp.room_name_id.room_id().as_str()),
                );
                fetched_room_summary.label(cx, ids!(subsection_topic)).set_text(
                    cx,
                    &format!("{room_or_space_uc} Topic"),
                );
                fetched_room_summary.html(cx, ids!(room_topic)).set_text(
                    cx,
                    frp.topic.as_deref().unwrap_or("<i>No topic set</i>"),
                );

                let room_summary = fetched_room_summary.label(cx, ids!(room_summary));
                let join_room_button = fetched_room_summary.button(cx, ids!(join_room_button));
                let join_function = match (&frp.state, &frp.join_rule) {
                    (Some(RoomState::Joined), _) => {
                        room_summary.set_text(cx, &format!("You have already joined this {room_or_space_lc}."));
                        join_room_button.set_text(cx, &format!("Go to {room_or_space_lc}"));
                        JoinButtonFunction::NavigateOrJoin
                    }
                    (Some(RoomState::Banned), _) => {
                        room_summary.set_text(cx, &format!("You have been banned from this {room_or_space_lc}."));
                        join_room_button.set_text(cx, "Cannot join until un-banned");
                        JoinButtonFunction::None
                    }
                    (Some(RoomState::Invited), _) => {
                        room_summary.set_text(cx, &format!("You have already been invited to this {room_or_space_lc}."));
                        join_room_button.set_text(cx, "Go to invitation");
                        JoinButtonFunction::NavigateOrJoin
                    }
                    (Some(RoomState::Knocked), _) => {
                        room_summary.set_text(cx, &format!("You have already knocked on this {room_or_space_lc}."));
                        join_room_button.set_text(cx, "Knock again (be nice!)");
                        JoinButtonFunction::Knock
                    }
                    (Some(RoomState::Left), join_rule) => {
                        room_summary.set_text(cx, &format!("You previously left this {room_or_space_lc}."));
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
                                format!("Re-joining {room_or_space_lc} requires an invite or other room membership"),
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
                        let direct = if frp.is_direct == Some(true) { "direct" } else { "regular" }; 
                        room_summary.set_text(cx, &format!(
                            "This is a {direct} {room_or_space_lc} with {} {}.",
                            frp.num_joined_members,
                            match frp.num_joined_members {
                                1 => "member",
                                _ => "members",
                            },
                        ));

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
                                format!("Joining {room_or_space_lc} requires an invite or other room membership"),
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
                        join_room_button.set_enabled(cx, !matches!(join_function, JoinButtonFunction::None));
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
                        let verb = if *is_invite { "been invited to" } else { "fully joined" };
                        room_summary.set_text(cx, &format!("You have {verb} this {room_or_space_lc}."));
                        let adj = if *is_invite { "invited" } else { "joined" };
                        join_room_button.set_text(cx, &format!("Go to {adj} {room_or_space_lc}"));
                        join_room_button.set_enabled(cx, true);
                        self.join_function = JoinButtonFunction::NavigateOrJoin;
                    }
                    _ => {}
                }
            }
        }

        let room_results_uid = self.view.child_by_path(ids!(content.room_results.list)).widget_uid();
        let are_room_results_shown = self.are_room_results_shown();
        let colors = (self.result_row_color, self.result_row_color_hover, self.result_row_color_focus);
        while let Some(step) = self.view.draw_walk(cx, scope, walk).step() {
            // A borrowed widget's uid is always 0, so we must get it before borrowing the list.
            let is_room_results = step.widget_uid() == room_results_uid;
            let list_ref = step.as_flat_list();
            let Some(mut list) = list_ref.borrow_mut() else { continue };
            let rows = &mut self.result_rows;
            if is_room_results {
                if are_room_results_shown {
                    match &self.room_search.results {
                        Some(results) => {
                            draw_search_section(cx, scope, &mut list, &results.rooms, &ROOMS_SECTION, rows, colors);
                            draw_search_section(cx, scope, &mut list, &results.spaces, &SPACES_SECTION, rows, colors);
                        }
                        None if !self.room_search.query.is_empty() => {
                            draw_flat_list_status_row(cx, scope, &mut list, id!(rooms_hint), false, "To find a person, search for them below.");
                        }
                        None => {}
                    }
                }
            } else {
                match &self.user_search.results {
                    Some(section) => draw_search_section(cx, scope, &mut list, section, &PEOPLE_SECTION, rows, colors),
                    None if !self.user_search.query.is_empty() => {
                        draw_flat_list_status_row(cx, scope, &mut list, id!(people_hint), false, "To find a room or space, search for it above.");
                    }
                    None => {}
                }
            }
            // Drop the rows that we didn't draw this time.
            list.items.retain_visible();
        }
        DrawStep::done()
    }
}

impl AddRoomScreen {
    /// Parses the given room address and starts fetching a preview of that room or space.
    fn search_for_room(&mut self, cx: &mut Cx, address: &str) {
        self.view.user_profile_sliding_pane(cx, ids!(user_profile_sliding_pane)).reset(cx);
        // We search for it too, so the search results match the input if the user cancels the room preview.
        self.search_rooms(cx, address.trim());
        match SearchQuery::parse(address).and_then(|search_query| search_query.full_room_address()) {
            Some((room_or_alias_id, via)) => self.request_room_preview(cx, room_or_alias_id, via),
            None => {
                let err_str = String::from("Could not parse the text as a valid room address.");
                enqueue_popup_notification(
                    err_str.clone(),
                    PopupKind::Error,
                    None,
                );
                self.state = AddRoomState::ParseError(err_str);
                self.view.text_input(cx, ids!(room_alias_id_input)).set_key_focus(cx);
            }
        }
        self.redraw(cx);
    }

    /// Starts fetching a preview of the given room or space, which we show once we get it.
    fn request_room_preview(&mut self, cx: &mut Cx, room_or_alias_id: OwnedRoomOrAliasId, via: Vec<OwnedServerName>) {
        self.only_match_to_open = None;
        self.state = AddRoomState::Parsed {
            room_or_alias_id: room_or_alias_id.clone(),
        };
        submit_async_request(MatrixRequest::GetRoomPreview {
            room_or_alias_id,
            via,
            response_mode: RoomPreviewResponseMode::Action,
        });
        self.redraw(cx);
    }

    /// Shows the given preview of a room or space, which we found by `room_or_alias_id`.
    fn show_room_preview(&mut self, cx: &mut Cx, frp: FetchedRoomPreview, room_or_alias_id: OwnedRoomOrAliasId) {
        self.state = AddRoomState::FetchedRoomPreview { frp, room_or_alias_id };
        // Reset the buttons' hover states when they are first shown.
        self.view.button(cx, ids!(fetched_room_summary.buttons_view.join_room_button)).reset_hover(cx);
        self.view.button(cx, ids!(fetched_room_summary.buttons_view.cancel_button)).reset_hover(cx);
        // The preview replaces the results, which the user may have scrolled down, so we scroll back up to it.
        self.view.child(id!(content)).as_view().set_scroll_pos(cx, Vec2d::default());
        self.redraw(cx);
    }

    /// Returns whether we're showing the room search results, which a room preview replaces but an error doesn't.
    fn are_room_results_shown(&self) -> bool {
        matches!(self.state, AddRoomState::WaitingOnUserInput | AddRoomState::ParseError(_) | AddRoomState::FetchError(_))
    }

    /// Moves the arrow keys' focus to the next or previous row of the given search box's results, scrolling to show it.
    ///
    /// Moving down from a list's last row loads more of that list instead, and moving up from the first row
    /// scrolls to the top of the screen. Returns `false` if there are no results to move through.
    fn move_result_focus(&mut self, cx: &mut Cx, search_box: SearchBox, is_down: bool) -> bool {
        // Each list of results in this search box, with its rows and whether it has more results to load.
        let lists: Vec<(ResultSection, Vec<LiveId>, bool)> = match search_box {
            SearchBox::Rooms => self.room_search.results.iter()
                .flat_map(|results| [
                    (ResultSection::Rooms, results.rooms.row_keys().collect(), results.rooms.more.is_some()),
                    (ResultSection::Spaces, results.spaces.row_keys().collect(), results.spaces.more.is_some()),
                ])
                .collect(),
            SearchBox::People => self.user_search.results.iter()
                .map(|section| (ResultSection::People, section.row_keys().collect(), section.more.is_some()))
                .collect(),
        };
        let list_path = match search_box {
            SearchBox::Rooms => ids!(room_results.list),
            SearchBox::People => ids!(user_results.list),
        };
        let rows: Vec<(LiveId, ResultSection)> = lists.iter()
            .flat_map(|(section, keys, _)| keys.iter().map(|key| (*key, *section)))
            .collect();
        if rows.is_empty() {
            return false;
        }
        let index = match self.result_rows.focused {
            Some((focused_box, key)) if focused_box == search_box => rows.iter().position(|(row_key, _)| *row_key == key),
            _ => None,
        };
        let new_index = match (index, is_down) {
            (None, true) => 0,
            (None | Some(0), false) => {
                self.view.child(id!(content)).as_view().set_scroll_pos(cx, Vec2d::default());
                return true;
            }
            (Some(index), false) => index - 1,
            (Some(index), true) => {
                let section = rows[index].1;
                let is_last_in_list = rows.get(index + 1).is_none_or(|(_, next_section)| *next_section != section);
                if is_last_in_list && lists.iter().any(|(list, _, has_more)| *list == section && *has_more) {
                    self.load_more_results(cx, section);
                    return true;
                }
                (index + 1).min(rows.len() - 1)
            }
        };
        let key = rows[new_index].0;
        self.result_rows.focused = Some((search_box, key));
        self.result_rows.hovered_key = None;
        // The user picked this result themselves, so we no longer open the query's only match for them.
        self.only_match_to_open = None;
        // Like with the Tab key, the content's scroll bars scroll this row into view upon this trigger.
        let row = self.view.flat_list(cx, list_path).borrow().and_then(|list| list.items.get(&key).map(|item| item.widget.clone()));
        if let Some(row) = row {
            cx.send_trigger(self.view.child(id!(content)).area(), Trigger { id: live_id!(scroll_focus_nav), from: row.area() });
        }
        self.redraw(cx);
        true
    }

    /// Starts getting more results for the given list, unless it has no more or we're already getting them.
    fn load_more_results(&mut self, cx: &mut Cx, section: ResultSection) {
        self.only_match_to_open = None;
        match section {
            ResultSection::Rooms | ResultSection::Spaces => {
                let kind = if section == ResultSection::Rooms { RoomKind::Room } else { RoomKind::Space };
                let Some(results) = self.room_search.results.as_mut() else { return };
                let section_results = results.section_mut(kind);
                if let Some(MoreResults::NextBatch(next_batch)) = &section_results.more
                    && !section_results.is_loading_more
                {
                    submit_async_request(MatrixRequest::SearchRoomDirectory {
                        query: self.room_search.query.clone(),
                        kind,
                        next_batch: Some(next_batch.clone()),
                    });
                    section_results.is_loading_more = true;
                }
            }
            ResultSection::People => {
                let Some(section_results) = self.user_search.results.as_mut() else { return };
                if let Some(MoreResults::Limit(limit)) = section_results.more
                    && !section_results.is_loading_more
                {
                    submit_async_request(MatrixRequest::SearchUserDirectory { query: self.user_search.query.clone(), limit });
                    section_results.is_loading_more = true;
                }
            }
        }
        self.redraw(cx);
    }

    /// Starts searching for rooms and spaces that match the given query, unless we already have its results.
    fn search_rooms(&mut self, cx: &mut Cx, query: &str) {
        cx.stop_timer(self.room_search.timer);
        let has_error = self.room_search.results.as_ref()
            .is_some_and(|results| results.rooms.error.is_some() || results.spaces.error.is_some());
        if query == self.room_search.query && !has_error {
            return;
        }
        let can_match_rooms = SearchQuery::parse(query).is_some_and(|search_query| search_query.can_match_rooms());
        if can_match_rooms {
            submit_async_request(MatrixRequest::ResolveRoomSearchQuery { query: query.to_owned() });
            for kind in [RoomKind::Room, RoomKind::Space] {
                submit_async_request(MatrixRequest::SearchRoomDirectory { query: query.to_owned(), kind, next_batch: None });
            }
        }
        self.room_search = Search {
            query: query.to_owned(),
            results: can_match_rooms.then(RoomResults::default),
            ..Default::default()
        };
        self.result_rows.unfocus(SearchBox::Rooms);
        self.redraw(cx);
    }

    /// Starts searching for people that match the given query, unless we already have its results.
    fn search_users(&mut self, cx: &mut Cx, query: &str) {
        cx.stop_timer(self.user_search.timer);
        let has_error = self.user_search.results.as_ref().is_some_and(|section| section.error.is_some());
        if query == self.user_search.query && !has_error {
            return;
        }
        let can_match_users = SearchQuery::parse(query).is_some_and(|search_query| search_query.can_match_users());
        if can_match_users {
            submit_async_request(MatrixRequest::ResolveUserSearchQuery { query: query.to_owned() });
            submit_async_request(MatrixRequest::SearchUserDirectory { query: query.to_owned(), limit: SEARCH_PAGE_SIZE });
        }
        self.user_search = Search {
            query: query.to_owned(),
            results: can_match_users.then(SectionResults::default),
            ..Default::default()
        };
        self.result_rows.unfocus(SearchBox::People);
        self.redraw(cx);
    }

    /// Opens the only match in the search box that the user pressed Enter in, once we've looked up its matches.
    fn open_only_match(&mut self, cx: &mut Cx) {
        let Some(search_box) = self.only_match_to_open else { return };
        let only_match = match search_box {
            SearchBox::Rooms => self.room_search.results.as_ref()
                .and_then(|results| get_only_match(&self.room_search.query, &[&results.rooms, &results.spaces])),
            SearchBox::People => self.user_search.results.as_ref()
                .and_then(|section| get_only_match(&self.user_search.query, &[section])),
        };
        // We're still looking up its matches.
        let Some(only_match) = only_match else { return };
        self.only_match_to_open = None;
        let Some(key) = only_match else { return };
        match search_box {
            SearchBox::Rooms => self.open_room_result(cx, key),
            SearchBox::People => self.open_user_result(cx, key),
        }
    }

    /// Shows the preview of the room or space search result with the given row key.
    ///
    /// We already have everything that its preview shows, so we needn't fetch it.
    fn open_room_result(&mut self, cx: &mut Cx, key: LiveId) {
        self.only_match_to_open = None;
        let Some(mut frp) = self.room_search.results.as_ref()
            .and_then(|results| results.rooms.items.iter().chain(&results.spaces.items).find(|room| room.row_key() == key))
            .map(|room| room.preview.clone())
        else { return };
        // Its row has usually fetched its avatar, which anything we open from the preview can show too.
        if let Some(avatar_url) = &frp.avatar_url
            && let AvatarCacheEntry::Loaded(data) = avatar_cache::get_or_fetch_avatar(cx, avatar_url)
        {
            frp.room_avatar = FetchedRoomAvatar::Image((avatar_url.clone(), data).into());
        }
        let room_or_alias_id = frp.alias.clone().map_or_else(|| frp.room_name_id.room_id().clone().into(), Into::into);
        self.show_room_preview(cx, frp, room_or_alias_id);
    }

    /// Records our new membership in the given room, if it's one of the rooms or spaces that we found.
    fn record_room_state(&mut self, cx: &mut Cx, room_id: &RoomId, state: RoomState) {
        let Some(results) = self.room_search.results.as_mut() else { return };
        let mut is_changed = false;
        for room in results.rooms.items.iter_mut().chain(&mut results.spaces.items) {
            if room.room_id() == room_id && room.preview.state != Some(state) {
                room.preview.state = Some(state);
                // Its row shows whether we've joined it.
                self.result_rows.populated.remove(&room.row_key());
                is_changed = true;
            }
        }
        if is_changed {
            self.redraw(cx);
        }
    }

    /// Opens the profile of the person search result with the given row key.
    fn open_user_result(&mut self, cx: &mut Cx, key: LiveId) {
        self.only_match_to_open = None;
        let Some(user_profile) = self.user_search.results.as_ref()
            .and_then(|section| section.items.iter().find(|user| user.row_key() == key))
            .map(|user| user.user_profile.clone())
        else { return };
        // We already have this user's profile, so the pane needn't fetch it again.
        user_profile_cache::insert_user_profile_if_missing(cx, user_profile.clone());
        // The pane covers the rows and takes their hover-out events, so no row stays hovered.
        self.result_rows.hovered_key = None;
        let profile_pane = self.view.user_profile_sliding_pane(cx, ids!(user_profile_sliding_pane));
        profile_pane.set_info(cx, UserProfilePaneInfo { user_profile, room: None });
        profile_pane.show(cx);
        self.redraw(cx);
    }
}

impl AddRoomScreenRef {
    /// Fills in the address input with the given room address and searches for it.
    pub fn search_for_room(&self, cx: &mut Cx, address: &str) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.view.text_input(cx, ids!(room_alias_id_input)).set_text(cx, address);
        inner.view.button(cx, ids!(search_for_room_button)).set_enabled(cx, true);
        inner.search_for_room(cx, address);
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


/// The text entered in one of the search boxes, and the results of searching for it.
#[derive(Default)]
struct Search<R> {
    /// The trimmed text that these results are for.
    query: String,
    /// This is `None` if the query can't match what this search box is for, e.g., a user ID in the room search.
    results: Option<R>,
    /// The countdown to searching after the user stops typing.
    timer: Timer,
}
impl<R: Default> Search<R> {
    /// Searches once the user stops typing, or clears the results if the search box is now empty.
    fn handle_text_changed(&mut self, cx: &mut Cx, text: &str) {
        cx.stop_timer(self.timer);
        if text.trim().is_empty() {
            *self = Self::default();
        } else {
            self.timer = cx.start_timeout(SEARCH_DELAY);
        }
    }
}

/// One of the two search boxes on this screen.
#[derive(Clone, Copy, PartialEq)]
enum SearchBox {
    Rooms,
    People,
}

/// One of the lists of search results, each with its own "Load more" button.
#[derive(Clone, Copy, PartialEq)]
enum ResultSection {
    Rooms,
    Spaces,
    People,
}

/// The results of searching for rooms and spaces, which we show in separate lists.
#[derive(Default)]
struct RoomResults {
    rooms: SectionResults<RoomSearchResult>,
    spaces: SectionResults<RoomSearchResult>,
}
impl RoomResults {
    fn section_mut(&mut self, kind: RoomKind) -> &mut SectionResults<RoomSearchResult> {
        match kind {
            RoomKind::Room => &mut self.rooms,
            RoomKind::Space => &mut self.spaces,
        }
    }

    /// Adds each of the given results to the rooms or spaces section.
    fn add_results(&mut self, results: &[RoomSearchResult], rows: &mut ResultRows) {
        let (spaces, rooms): (Vec<_>, Vec<_>) = results.iter().cloned().partition(RoomSearchResult::is_space);
        self.rooms.add_results(&rooms, rows);
        self.spaces.add_results(&spaces, rows);
    }
}

/// The results shown in one section of the search results, i.e., rooms, spaces, or people.
struct SectionResults<T> {
    /// The results in the order that we got them, so that a row never moves once it's shown.
    items: Vec<T>,
    /// Whether we've looked up the results that the query could exactly be.
    has_exact_matches: bool,
    /// Whether we've searched our homeserver's directory for the query.
    has_directory_matches: bool,
    error: Option<String>,
    /// How to get more matches from the directory, if it has more.
    more: Option<MoreResults>,
    is_loading_more: bool,
}
impl<T> Default for SectionResults<T> {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            has_exact_matches: false,
            has_directory_matches: false,
            error: None,
            more: None,
            is_loading_more: false,
        }
    }
}
impl<T: SearchResultRow> SectionResults<T> {
    /// Appends the given results, except for ones that we already have,
    /// which an exact match replaces in place.
    fn add_results(&mut self, results: &[T], rows: &mut ResultRows) {
        for result in results {
            let key = result.row_key();
            match self.items.iter_mut().find(|item| item.row_key() == key) {
                Some(item) if result.is_exact_match() => {
                    *item = result.clone();
                    rows.populated.remove(&key);
                }
                Some(_) => {}
                None => self.items.push(result.clone()),
            }
        }
    }

    fn row_keys(&self) -> impl Iterator<Item = LiveId> {
        self.items.iter().map(SearchResultRow::row_key)
    }

    fn exact_match_keys(&self) -> impl Iterator<Item = LiveId> {
        self.items.iter().filter(|result| result.is_exact_match()).map(SearchResultRow::row_key)
    }

    fn is_searching(&self) -> bool {
        !self.has_exact_matches || !self.has_directory_matches
    }
}

/// How to get more matches from a directory for one section of the search results.
enum MoreResults {
    /// Get the room directory's next page of matches, starting from this token.
    NextBatch(String),
    /// Search the user directory again for up to this many users.
    Limit(u32),
}

/// What we track about the rows that we draw in the list of search results.
#[derive(Default)]
struct ResultRows {
    hovered_key: Option<LiveId>,
    /// The row that the user moved to with the arrow keys, and the search box whose results it's in.
    focused: Option<(SearchBox, LiveId)>,
    /// The keys of the rows whose content is up to date, which needn't be set again when drawn.
    populated: HashSet<LiveId>,
}
impl ResultRows {
    /// Clears the arrow keys' focus if it's on one of the given search box's results, returning whether it was.
    fn unfocus(&mut self, search_box: SearchBox) -> bool {
        let was_focused = self.focused.is_some_and(|(focused_box, _)| focused_box == search_box);
        if was_focused {
            self.focused = None;
        }
        was_focused
    }
}

/// The texts and the list keys of one section of the search results.
struct SectionInfo {
    header: Option<&'static str>,
    searching_text: &'static str,
    empty_text: &'static str,
    header_key: LiveId,
    loading_key: LiveId,
    status_key: LiveId,
    load_more_key: LiveId,
}

const ROOMS_SECTION: SectionInfo = SectionInfo {
    header: Some("Rooms"),
    searching_text: "Searching for rooms...",
    empty_text: "No rooms found.",
    header_key: LiveId::from_str("rooms_header"),
    loading_key: LiveId::from_str("rooms_loading"),
    status_key: LiveId::from_str("rooms_status"),
    load_more_key: LiveId::from_str("rooms_load_more"),
};

const SPACES_SECTION: SectionInfo = SectionInfo {
    header: Some("Spaces"),
    searching_text: "Searching for spaces...",
    empty_text: "No spaces found.",
    header_key: LiveId::from_str("spaces_header"),
    loading_key: LiveId::from_str("spaces_loading"),
    status_key: LiveId::from_str("spaces_status"),
    load_more_key: LiveId::from_str("spaces_load_more"),
};

// The "Find people" label already says what this one section is.
const PEOPLE_SECTION: SectionInfo = SectionInfo {
    header: None,
    searching_text: "Searching for people...",
    empty_text: "No people found.",
    header_key: LiveId::from_str("people_header"),
    loading_key: LiveId::from_str("people_loading"),
    status_key: LiveId::from_str("people_status"),
    load_more_key: LiveId::from_str("people_load_more"),
};

/// A search result that we show as a row in the list of search results.
trait SearchResultRow: Clone {
    fn row_key(&self) -> LiveId;

    fn is_exact_match(&self) -> bool;

    /// Sets the content of the given row to show this result.
    ///
    /// Returns `false` if its avatar is still being fetched, in which case call this again later.
    fn populate_row(&self, cx: &mut Cx, row: &WidgetRef) -> bool;
}

impl SearchResultRow for RoomSearchResult {
    fn row_key(&self) -> LiveId {
        LiveId::from_str(self.room_id().as_str())
    }

    fn is_exact_match(&self) -> bool {
        self.is_exact_match
    }

    fn populate_row(&self, cx: &mut Cx, row: &WidgetRef) -> bool {
        let room_name = self.preview.room_name_id.name_for_avatar();
        let name = room_name.or(self.alias()).unwrap_or(self.room_id().as_str());
        row.label(cx, ids!(info.title)).set_text(cx, name);
        let members = match self.preview.num_joined_members {
            1 => "1 member".to_owned(),
            n => format!("{n} members"),
        };
        let mut subtitle = match self.alias() {
            Some(alias) if room_name.is_some_and(|name| name != alias) => format!("{alias} · {members}"),
            _ => members,
        };
        // Like the space lobby, we mark the rooms that we're already in.
        if self.preview.state == Some(RoomState::Joined) {
            subtitle.insert_str(0, "✅ Joined · ");
        }
        row.label(cx, ids!(info.subtitle)).set_text(cx, &subtitle);
        row.avatar(cx, ids!(avatar)).show_room(cx, self.room_id(), self.preview.avatar_url.as_ref(), Some(name))
    }
}

impl SearchResultRow for UserSearchResult {
    fn row_key(&self) -> LiveId {
        LiveId::from_str(self.user_profile.user_id.as_str())
    }

    fn is_exact_match(&self) -> bool {
        self.is_exact_match
    }

    fn populate_row(&self, cx: &mut Cx, row: &WidgetRef) -> bool {
        let name = self.user_profile.displayable_name();
        row.label(cx, ids!(info.title)).set_text(cx, name);
        row.label(cx, ids!(info.subtitle)).set_text(cx, self.user_profile.user_id.as_str());
        row.avatar(cx, ids!(avatar)).show_user(cx, self.user_profile.avatar_state.uri(), name)
    }
}

/// Draws one section of the search results into the given list:
/// its header, then its results or status, then a button to load more results.
fn draw_search_section<T: SearchResultRow>(
    cx: &mut Cx2d,
    scope: &mut Scope,
    list: &mut FlatList,
    section: &SectionResults<T>,
    info: &SectionInfo,
    rows: &mut ResultRows,
    (color, color_hover, color_focus): (Vec4f, Vec4f, Vec4f),
) {
    if let Some(header_text) = info.header
        && let Some(header) = list.item(cx, info.header_key, id!(section_header))
    {
        header.set_text(cx, header_text);
        header.draw_all(cx, scope);
    }

    for result in &section.items {
        let key = result.row_key();
        // The list drops rows that it didn't draw last time,
        // so a row that it creates again starts out empty and unhovered.
        if !list.items.contains_key(&key) {
            rows.populated.remove(&key);
            if rows.hovered_key == Some(key) {
                rows.hovered_key = None;
            }
        }
        let Some(row) = list.item(cx, key, id!(result_row)) else { continue };
        if !rows.populated.contains(&key) && result.populate_row(cx, &row) {
            rows.populated.insert(key);
        }
        let row_color = if rows.focused.is_some_and(|(_, focused_key)| focused_key == key) {
            color_focus
        } else if rows.hovered_key == Some(key) {
            color_hover
        } else {
            color
        };
        set_view_bg_color(cx, &row.as_view(), row_color);
        row.draw_all(cx, scope);
    }

    if let Some(error) = &section.error {
        draw_flat_list_status_row(cx, scope, list, info.status_key, false, error);
    } else if section.items.is_empty() && !section.is_searching() {
        draw_flat_list_status_row(cx, scope, list, info.status_key, false, info.empty_text);
    }
    if section.is_searching() || section.is_loading_more {
        draw_flat_list_status_row(cx, scope, list, info.loading_key, true, info.searching_text);
    } else if section.more.is_some()
        && let Some(load_more_row) = list.item(cx, info.load_more_key, id!(load_more_row))
    {
        load_more_row.draw_all(cx, scope);
    }
}

/// Returns the text that was submitted by clicking the given button or pressing Enter in the given input, if any.
fn get_submitted_query(input: &TextInputRef, button: &ButtonRef, actions: &Actions) -> Option<String> {
    button.clicked(actions).then(|| input.text())
        .or_else(|| input.returned(actions).map(|(text, _)| text))
        .map(|text| text.trim().to_owned())
        .filter(|text| !text.is_empty())
}

/// Returns the only item of the given iterator, if it has exactly one.
fn get_only_item<T>(mut items: impl Iterator<Item = T>) -> Option<T> {
    match (items.next(), items.next()) {
        (Some(item), None) => Some(item),
        _ => None,
    }
}

/// Returns `None` while we're still looking up matches for `query`, else the key of its only match in these sections, if any.
/// A fully-qualified query can only be one room or user, so for one we only need its exact matches.
fn get_only_match<T: SearchResultRow>(query: &str, sections: &[&SectionResults<T>]) -> Option<Option<LiveId>> {
    if SearchQuery::parse(query).is_some_and(|search_query| search_query.is_fully_qualified()) {
        if !sections.iter().all(|section| section.has_exact_matches) {
            return None;
        }
        return Some(get_only_item(sections.iter().flat_map(|section| section.exact_match_keys())));
    }
    if sections.iter().any(|section| section.is_searching()) {
        return None;
    }
    // A section that failed or that has more pages might have more matches.
    if sections.iter().any(|section| section.error.is_some() || section.more.is_some()) {
        return Some(None);
    }
    Some(get_only_item(sections.iter().flat_map(|section| section.row_keys())))
}

#[cfg(test)]
mod tests_section_results {
    use ruma::UserId;
    use crate::{profile::user_profile::UserProfile, shared::avatar::AvatarState};
    use super::*;

    fn user(raw_user_id: &str, is_exact_match: bool) -> UserSearchResult {
        UserSearchResult {
            user_profile: UserProfile {
                user_id: UserId::parse(raw_user_id).unwrap(),
                username: None,
                avatar_state: AvatarState::Unknown,
            },
            is_exact_match,
        }
    }

    fn user_ids(section: &SectionResults<UserSearchResult>) -> Vec<&str> {
        section.items.iter().map(|user| user.user_profile.user_id.as_str()).collect()
    }

    #[test]
    fn late_exact_matches_dont_move_rows() {
        let mut rows = ResultRows::default();
        let mut section = SectionResults::default();
        section.add_results(&[user("@bob:x.org", false), user("@alice:x.org", false)], &mut rows);
        section.add_results(&[user("@alice:x.org", true), user("@carol:x.org", true)], &mut rows);
        assert_eq!(user_ids(&section), ["@bob:x.org", "@alice:x.org", "@carol:x.org"]);
        assert!(section.items[1].is_exact_match);
    }

    #[test]
    fn early_exact_matches_come_first() {
        let mut rows = ResultRows::default();
        let mut section = SectionResults::default();
        section.add_results(&[user("@alice:x.org", true)], &mut rows);
        section.add_results(&[user("@bob:x.org", false), user("@alice:x.org", false)], &mut rows);
        assert_eq!(user_ids(&section), ["@alice:x.org", "@bob:x.org"]);
        // A directory match doesn't replace an exact match.
        assert!(section.items[0].is_exact_match);
    }

    #[test]
    fn replaced_rows_are_populated_again() {
        let mut rows = ResultRows::default();
        let mut section = SectionResults::default();
        section.add_results(&[user("@alice:x.org", false)], &mut rows);
        let key = section.items[0].row_key();
        rows.populated.insert(key);
        section.add_results(&[user("@alice:x.org", false)], &mut rows);
        assert!(rows.populated.contains(&key));
        section.add_results(&[user("@alice:x.org", true)], &mut rows);
        assert!(!rows.populated.contains(&key));
    }

    /// Returns a section that has finished searching and found the given users.
    fn searched(users: &[UserSearchResult]) -> SectionResults<UserSearchResult> {
        let mut section = SectionResults::default();
        section.add_results(users, &mut ResultRows::default());
        section.has_exact_matches = true;
        section.has_directory_matches = true;
        section
    }

    #[test]
    fn finds_the_only_match_of_a_partial_query() {
        let alice = || user("@alice:x.org", false);
        let alicia = || user("@alicia:x.org", false);
        let key = alice().row_key();
        assert_eq!(get_only_match("alice", &[&searched(&[alice()])]), Some(Some(key)));
        assert_eq!(get_only_match("alice", &[&searched(&[alice()]), &searched(&[])]), Some(Some(key)));
        assert_eq!(get_only_match("alice", &[&searched(&[alice(), alicia()])]), Some(None));
        assert_eq!(get_only_match("alice", &[&searched(&[alice()]), &searched(&[alicia()])]), Some(None));
        // There may be more matches that we haven't gotten.
        let mut section = searched(&[alice()]);
        section.more = Some(MoreResults::Limit(20));
        assert_eq!(get_only_match("alice", &[&section]), Some(None));
        let mut section = searched(&[alice()]);
        section.error = Some(String::from("failed"));
        assert_eq!(get_only_match("alice", &[&section]), Some(None));
        // We wait until we've looked up all of its matches.
        let mut section = searched(&[alice()]);
        section.has_directory_matches = false;
        assert_eq!(get_only_match("alice", &[&section]), None);
    }

    #[test]
    fn finds_the_exact_match_of_a_fully_qualified_query() {
        let mut section = searched(&[user("@alicia:x.org", false), user("@alice:x.org", true)]);
        let key = section.items[1].row_key();
        assert_eq!(get_only_match("@alice:x.org", &[&section]), Some(Some(key)));
        // The directory's other matches don't matter, so we needn't wait for them.
        section.has_directory_matches = false;
        assert_eq!(get_only_match("@alice:x.org", &[&section]), Some(Some(key)));
        section.has_exact_matches = false;
        assert_eq!(get_only_match("@alice:x.org", &[&section]), None);
        assert_eq!(get_only_match("@bob:x.org", &[&searched(&[user("@bob:x.org", false)])]), Some(None));
    }
}
