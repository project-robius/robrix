use std::{borrow::Cow, collections::BTreeMap, ops::DerefMut};

use imbl::Vector;
use makepad_widgets::*;
use matrix_sdk::ruma::{
    events::{
        room::message::{
            FormattedBody, MessageType, RoomMessageEventContent, TextMessageEventContent,
        },
        AnyTimelineEvent,
    },
    OwnedRoomId, OwnedUserId,
};
use matrix_sdk_ui::timeline::{Profile, TimelineDetails};
use rangemap::RangeSet;

use crate::{
    app::AppState,
    home::{
        room_screen::{
            populate_text_message_content, ItemDrawnStatus, JumpToMessageRequest, MessageWidgetRefExt,
        },
        rooms_list::RoomsListRef,
    },
    shared::{
        avatar::AvatarWidgetRefExt,
        html_or_plaintext::HtmlOrPlaintextWidgetRefExt,
        message_search_input_bar::MessageSearchAction,
        popup_list::{enqueue_popup_notification, PopupItem, PopupKind},
        timestamp::TimestampWidgetRefExt,
        styles::COLOR_WARNING_YELLOW
    },
    sliding_sync::{submit_async_request, MatrixRequest},
    utils::unix_time_millis_to_datetime,
};

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;
    use crate::shared::styles::*;
    use crate::shared::helpers::*;
    use crate::shared::icon_button::*;
    use crate::shared::message_search_input_bar::*;
    use crate::home::rooms_list::RoomsList;
    use crate::home::room_screen::*;

    COLOR_BUTTON_GREY = #B6BABF

    // The bottom space is used to display a loading message while the room is being paginated.
    BottomSpace = <View> {
        visible: false,
        width: Fill,
        height: Fit,
        align: {x: 0.5, y: 0}
        show_bg: true,
        draw_bg: {
            color: #xDAF5E5F0, // mostly opaque light green
        }

        label = <Label> {
            width: Fill,
            height: Fit,
            align: {x: 0.5, y: 0.5},
            padding: { top: 10.0, bottom: 7.0, left: 15.0, right: 15.0 }
            draw_text: {
                text_style: <MESSAGE_TEXT_STYLE> { font_size: 10 },
                color: (TIMESTAMP_TEXT_COLOR)
            }
            text: "Loading older search results..."
        }
    }

    SearchIcon = <Icon> {
        align: {x: 0.0} // Align to top-right
        spacing: 10,
        margin: {top: 0, left: 10},
        padding: 10,
        width: Fit,
        height: Fit,
        draw_bg: {
            instance color: (COLOR_BUTTON_GREY)
            instance color_hover: #fef65b
            instance border_width: 1.5
            instance radius: 3.0
            instance hover: 0.0
            fn get_color(self) -> vec4 {
                return mix(self.color, mix(self.color, self.color_hover, 0.2), self.hover)
            }
            fn pixel(self) -> vec4 {
                let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                sdf.box(
                    self.border_width,
                    self.border_width,
                    self.rect_size.x - self.border_width * 2.0,
                    self.rect_size.y - self.border_width * 2.0,
                    max(1.0, self.radius)
                )
                sdf.fill(self.get_color());
                return sdf.result;
            }
        }
        draw_icon: {
            svg_file: (ICON_SEARCH),
            fn get_color(self) -> vec4 {
                return (COLOR_TEXT_INPUT_IDLE);
            }
        }
        icon_walk: {width: 16, height: 16}
    }

    pub SearchResultSummary = {{SearchResultSummary}} {
        width: Fill,
        height: Fill,
        show_bg: false,
        flow: Overlay,

        loading_view = <View> {
            width: Fill,
            height: Fill,
            show_bg: true,
            visible: true,
            draw_bg: {
                color: (COLOR_SECONDARY)
            }
            align: {x: 0.5, y: 0.5}

            <SearchIcon> {}
        }
        <View> {
            width: Fill,
            height: Fit,
            flow: Down,

            <View> {
                width: Fill,
                height: 60,
                show_bg: true,
                align: {y: 0.5}
                draw_bg: {
                    color: (COLOR_SECONDARY)
                }

                <SearchIcon> {}
                summary_label = <Markdown> {
                    margin: {left: 10, top:0},
                    align: {x: 0.3, y: 0.5}  // Align to top-right
                    height: Fit,
                    padding: 5,
                    font_color: (MESSAGE_TEXT_COLOR),
                    font_size: (MESSAGE_FONT_SIZE),
                    body: ""
                }
                search_all_rooms_button = <RobrixIconButton> {
                    flow: RightWrap,
                    width: 90,
                    height: Fit,
                    padding: { top:2, bottom:2, left: 10, right: 10}
                    margin: {top: 5, bottom: 10}
                    align: {x: 0.5, y: 0.5}
                    draw_bg: {
                        color: (COLOR_ACTIVE_PRIMARY)
                    }
                    draw_text: {
                        color: (COLOR_PRIMARY)
                        text_style: <REGULAR_TEXT> {}
                    }
                    text: "Search All Rooms"
                }
                search_again_button = <RobrixIconButton> {
                    flow: RightWrap,
                    visible: false,
                    width: 70,
                    height: Fit,
                    padding: { top:2, bottom:2, left: 10, right: 10}
                    margin: {top: 5, bottom: 10}
                    align: {x: 0.5, y: 0.5}
                    draw_bg: {
                        color: (COLOR_ACTIVE_PRIMARY)
                    }
                    draw_text: {
                        color: (COLOR_PRIMARY)
                        text_style: <REGULAR_TEXT> {}
                    }
                    text: "Search Again"
                }
            }
            
        }
    }
    // White rounded message card against a grey backdrop.
    pub MessageCard = <Message> {
        draw_bg: {
            instance border_radius: 4.0,
            instance border_size: 1.0,
            instance border_color: #000000,
            fn pixel(self) -> vec4 {
                let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                // draw bg
                sdf.box(
                    self.border_size,
                    self.border_size,
                    self.rect_size.x - self.border_size * 2.0,
                    self.rect_size.y - self.border_size * 2.0,
                    max(1.0, self.border_radius)
                )
                sdf.fill(self.color);
                sdf.stroke(
                    self.border_color,
                    self.border_size
                )
                return sdf.result;
            }
        }
    }

    pub SearchedMessages = <View> {
        width: Fill,
        height: Fill,
        align: {x: 0.5, y: 0.0} // center horizontally, align to top vertically
        flow: Overlay,

        list = <PortalList> {
            height: Fill,
            width: Fill
            flow: Down
            auto_tail: false, // set to `true` to lock the view to the last item.
            max_pull_down: 0.0, // set to `0.0` to disable the pulldown bounce animation.

            // Below, we must place all of the possible templates (views) that can be used in the portal list.
            MessageCard = <MessageCard> {}
            Empty = <Empty> {}
            RoomHeader = <Label> {
                margin: {left: 10},
                draw_text: {
                    text_style: <REGULAR_TEXT> {
                        font_size: 12.5,
                    },
                    color: #000,
                }
                text: "Unknown Room Name"
            }
        }
    }

    pub SearchResults = {{SearchResults}} {
        no_more_template: <Label> {
            draw_text: {
                text_style: <REGULAR_TEXT>{
                    font_size: 11.5,
                },
                color: (COLOR_TEXT),
            }
            text: "No More"
        }

        <View> {
            width: Fill,
            height: Fill,
            flow: Down,

            search_result_plane = <SearchResultSummary> {
                width: Fill,
                height: Fit,
                visible: true
            }
            searched_messages = <SearchedMessages> {
                width: Fill,
                height: Fill,
            }
            bottom_space = <BottomSpace> {
                visible: false
            }
        }
        search_context_menu = <RoundedView> {
            visible: false,
            flow: Down
            width: 180,
            height: Fit,
            padding: 8
            spacing: 0,
            align: {x: 0, y: 0}
            show_bg: true
            draw_bg: {
                color: #fff
                border_radius: 5.0
                border_size: 0.5
                border_color: #888
            }

            go_to_message_button = <RobrixIconButton> {
                height: 35
                width: Fill,
                margin: 0,
                icon_walk: {width: 16, height: 16, margin: {right: 3}}
                draw_icon: { svg_file: dep("crate://self/resources/icons/jump.svg") }
                text: "Go to Message"
            }
        }
    }
}

/// Precompute formatted message content with highlights to avoid repeated string operations during rendering.
pub fn format_message_content(message: &mut RoomMessageEventContent, highlights: &[String]) {
    if let MessageType::Text(text) = &mut message.msgtype {
        let formatted = if let Some(ref mut formatted) = text.formatted {
            let mut body = formatted.body.clone();
            // TODO: Remove all <code> </code> before appending them to highlights.
            for highlight in highlights {
                body = body.replace(highlight, &format!("<code>{}</code>", highlight));
            }
            body
        } else {
            let mut formatted_string = text.body.clone();
            for highlight in highlights {
                formatted_string =
                    formatted_string.replace(highlight, &format!("<code>{}</code>", highlight));
            }
            formatted_string
        };
        text.formatted = Some(FormattedBody::html(formatted));
    }
}

/// States that are necessary to display search results.
/// Contains all the data needed to render the search UI and manage pagination.
#[derive(Default)]
struct SearchState {
    /// The list of events in the search results.
    items: Vector<SearchResultItem>,
    /// See [`TimelineUiState.content_drawn_since_last_update`].
    content_drawn_since_last_update: RangeSet<usize>,
    /// Same as `content_drawn_since_last_update`, but for the event **profiles** (avatar, username).
    profile_drawn_since_last_update: RangeSet<usize>,
    /// All profile infos for the search results.
    profile_infos: BTreeMap<OwnedUserId, TimelineDetails<Profile>>,
    /// Token to be used for pagination of earlier search results.
    next_batch_token: Option<String>,
    /// The search term for the last search request.
    prev_search_term: Option<String>,
    /// Previous first_id to detect scroll direction.
    prev_first_index: Option<usize>,
    /// Whether all search results have been fully paginated.
    is_fully_paginated: bool,
}

/// The main widget that displays a list of search results.
#[derive(Live, LiveHook, Widget)]
struct SearchResults {
    #[deref]
    view: View,
    #[layout]
    layout: Layout,
    #[walk]
    walk: Walk,
    #[rust]
    search_state: SearchState,
    #[live]
    no_more_template: Option<LivePtr>,
    #[rust]
    room_id: Option<OwnedRoomId>,
}

impl Widget for SearchResults {
    /// Handles events and actions for the SearchResults widget and its inner Timeline view.
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);

        // Handle pagination when user scrolls to the top
        if let Event::Actions(actions) = event {
            let search_portal_list = self.portal_list(id!(searched_messages.list));
            self.paginate_search_results_based_on_scroll_pos(
                cx,
                actions,
                &search_portal_list,
                scope,
            );
        }

        self.widget_match_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        let tl_items = &self.search_state.items;
       
        while let Some(subview) = self.view.draw_walk(cx, scope, walk).step() {
            // We only care about drawing the portal list.
            let portal_list_ref = subview.as_portal_list();
            let Some(mut list_ref) = portal_list_ref.borrow_mut() else {
                error!("!!! SearchResults::draw_walk(): BUG: expected a PortalList widget, but got something else");
                continue;
            };
             if tl_items.is_empty() {
                continue;
            }
            // Set the portal list's range based on the number of searched items.
            let last_item_id = tl_items.len();
            let list = list_ref.deref_mut();
            list.set_item_range(cx, 0, last_item_id);

            while let Some(item_id) = list.next_visible_item(cx) {
                // Show "No More" template at the bottom when fully paginated

                let item = {
                    let tl_idx = item_id;
                    let Some(search_item) = tl_items.get(tl_idx) else {
                        // This shouldn't happen (unless the timeline gets corrupted or some other weird error),
                        // but we can always safely fill the item with an empty widget that takes up no space.
                        list.item(cx, item_id, live_id!(Empty));
                        continue;
                    };
                    let item_drawn_status = ItemDrawnStatus {
                        content_drawn: self
                            .search_state
                            .content_drawn_since_last_update
                            .contains(&tl_idx),
                        profile_drawn: self
                            .search_state
                            .profile_drawn_since_last_update
                            .contains(&tl_idx),
                    };
                    let (item, item_new_draw_status) = populate_message_search_view(
                        cx,
                        list,
                        item_id,
                        search_item,
                        &self.search_state.profile_infos,
                        item_drawn_status,
                    );
                    if item_new_draw_status.content_drawn {
                        self.search_state
                            .content_drawn_since_last_update
                            .insert(tl_idx..tl_idx + 1);
                    }
                    if item_new_draw_status.profile_drawn {
                        self.search_state
                            .profile_drawn_since_last_update
                            .insert(tl_idx..tl_idx + 1);
                    }
                    item
                };
                item.draw_all(cx, &mut Scope::empty());
                if item_id == last_item_id.saturating_sub(1)
                    && self.search_state.is_fully_paginated
                    && last_item_id > 0
                {
                    WidgetRef::new_from_ptr(cx, self.no_more_template)
                        .as_label()
                        .draw_all(cx, &mut Scope::empty());
                    continue;
                }
            }
        }
        DrawStep::done()
    }
}

impl SearchResults {
    /// Sends a pagination request when the user is scrolling down and approaching the bottom of the search results.
    /// The request is sent with the `next_batch` token from the last search result received.
    fn paginate_search_results_based_on_scroll_pos(
        &mut self,
        cx: &mut Cx,
        actions: &ActionsBuf,
        portal_list: &PortalListRef,
        _scope: &mut Scope,
    ) {
        if !portal_list.scrolled(actions) {
            return;
        };

        let first_index = portal_list.first_id();
        let total_items = self.search_state.items.len();

        // Detect scroll direction using prev_first_index
        let is_scrolling_down = if let Some(prev_first_index) = self.search_state.prev_first_index {
            first_index > prev_first_index
        } else {
            false // Default to false if no previous index
        };

        self.search_state.prev_first_index = Some(first_index);
        // Only trigger pagination when scrolling down, at the bottom, and not fully paginated
        // Tried to use is_filling_port, it does not work.
        if is_scrolling_down && total_items > 0 && !self.search_state.is_fully_paginated {
            let visible_items: usize = portal_list.visible_items();
            // Check if we've reached the bottom (first_id + visible_items >= total_items)
            if first_index + visible_items >= total_items.saturating_sub(1) {
                if let Some(next_batch_token) = self.search_state.next_batch_token.take() {
                    log!("Scrolling down reached bottom: first_id={}, visible_items={}, total={}, sending pagination request",
                        first_index, visible_items, total_items
                    );
                    let search_result_summary_ref =
                        self.view.search_result_summary(id!(search_result_plane));
                    let criteria = search_result_summary_ref.get_search_criteria();

                    self.display_bottom_space(cx);

                    submit_async_request(MatrixRequest::SearchMessages {
                        message_search_choice: if let (false, Some(room_id)) = (criteria.include_all_rooms, &self.room_id) {
                            MessageSearchChoice::OneRoom(room_id.clone())
                        } else {
                            MessageSearchChoice::AllRooms
                        },
                        search_term: criteria.search_term.clone(),
                        next_batch: Some(next_batch_token.clone()),
                        abort_previous_search: false,
                    });
                }
            }
        }
    }
}

impl SearchResults {
    /// Processes a new batch of search results and updates the UI and state.
    /// Optimized to take ownership of results to avoid clones.
    fn process_search_results(
        &mut self,
        cx: &mut Cx,
        scope: &mut Scope,
        results: SearchResultReceived,
    ) {
        let mut search_result_summary_ref =
            self.view.search_result_summary(id!(search_result_plane));
        let criteria = search_result_summary_ref.get_search_criteria();

        // If the search input text has changed, reset everything.
        if criteria.search_term != results.search_term
            || self
                .search_state
                .prev_search_term
                .as_ref()
                .is_some_and(|p| p != &results.search_term)
        {
            self.search_state = SearchState::default();
        }

        self.hide_bottom_space(cx);
        // Re-enable the search all rooms button when results are received
        self.view
            .button(id!(search_all_rooms_button))
            .set_enabled(cx, true);
        // Hide the search again button when successful results are received
        self.view
            .button(id!(search_again_button))
            .set_visible(cx, false);

        // Take ownership of profile infos instead of cloning
        self.search_state.profile_infos = results.profile_infos;

        // Update the search bar and summary widget
        cx.action(MessageSearchAction::SetText(results.search_term.clone()));
        search_result_summary_ref.set_search_criteria(cx, scope, criteria);
        search_result_summary_ref.set_result_count(cx, results.count);

        self.view.view(id!(searched_messages)).set_visible(cx, true);

        // Clear draw caches
        self.search_state.content_drawn_since_last_update.clear();
        self.search_state.profile_drawn_since_last_update.clear();

        // Append new items efficiently using Vector::append
        self.search_state.items.append(results.items);

        let search_portal_list = self.portal_list(id!(searched_messages.list));
        search_portal_list.set_tail_range(false);
        if let Some(mut search_portal_list) = search_portal_list.borrow_mut() {
            search_portal_list.set_item_range(cx, 0, self.search_state.items.len());
            if self.search_state.prev_search_term.is_none() {
                // After several testing, multiply the length of the list by 5, will ensure the the portal list is at the top.
                // This is a hacky way to ensure the portal list is at the top as it is easy to display the bottom of the portal list.
                search_portal_list.smooth_scroll_to(cx, 0, 0.0, None);
            }
        }
        
        self.search_state.next_batch_token = results.next_batch.clone();
        self.search_state.is_fully_paginated = results.next_batch.is_none();
        self.search_state.prev_search_term = Some(results.search_term);

        self.redraw(cx);
    }

    /// Handles actions from the MessageSearchInputBar.
    fn handle_search_bar_action(&mut self, cx: &mut Cx, scope: &mut Scope, action: &Action) {
        match action.as_widget_action().cast() {
            MessageSearchAction::Changed(search_term) => {
                let search_result_summary_ref =
                    self.search_result_summary(id!(search_result_plane));
                if search_term.is_empty() {
                    search_result_summary_ref.reset(cx);
                    self.search_state = SearchState::default();
                    // Abort previous inflight search request.
                    submit_async_request(MatrixRequest::SearchMessages {
                        message_search_choice: MessageSearchChoice::AllRooms,
                        search_term: String::default(),
                        next_batch: None,
                        abort_previous_search: true,
                    });
                    return;
                }
                if let Some(selected_room) = {
                    let app_state = scope.data.get::<AppState>().unwrap();
                    app_state.selected_room.clone()
                } {
                    let mut criteria = search_result_summary_ref.get_search_criteria();
                    if criteria.search_term == search_term {
                        return;
                    }
                    criteria.search_term = search_term;
                    search_result_summary_ref.set_search_criteria(cx, scope, criteria.clone());
                    let room_id = selected_room.room_id();
                    self.room_id = Some(room_id.clone());
                    let rooms_list_ref = cx.get_global::<RoomsListRef>();
                    let is_encrypted = rooms_list_ref.is_room_encrypted(room_id);
                    if is_encrypted && !criteria.include_all_rooms {
                        enqueue_popup_notification(PopupItem {
                            message: String::from("Searching for encrypted messages is not supported yet."),
                            auto_dismissal_duration: None,
                            kind: PopupKind::Info
                        });
                        return;
                    }
                    self.display_bottom_space(cx);
                    // Disable the search all rooms button during search
                    self.view
                        .button(id!(search_all_rooms_button))
                        .set_enabled(cx, false);
                    submit_async_request(MatrixRequest::SearchMessages {
                        message_search_choice: MessageSearchChoice::OneRoom(room_id.clone()),
                        search_term: criteria.search_term.clone(),
                        next_batch: None,
                        abort_previous_search: true,
                    });
                }
            }
            MessageSearchAction::Clicked(search_term) => {
                let search_result_summary_ref =
                    self.search_result_summary(id!(search_result_plane));
                let mut criteria = search_result_summary_ref.get_search_criteria();
                criteria.search_term = search_term.clone();
                search_result_summary_ref.set_search_criteria(cx, scope, criteria);
            }
            _ => {}
        }
    }

    /// Displays the loading view for backwards pagination for search result.
    fn display_bottom_space(&mut self, cx: &mut Cx) {
        self.view.view(id!(bottom_space)).set_visible(cx, true);
    }

    /// Hides the loading view for backwards pagination for search result.
    fn hide_bottom_space(&mut self, cx: &mut Cx) {
        self.view.view(id!(bottom_space)).set_visible(cx, false);
    }
}

impl WidgetMatchEvent for SearchResults {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions, scope: &mut Scope) {
        for action in actions.iter() {
            self.handle_search_bar_action(cx, scope, action);
            match action.downcast_ref(){
                Some(SearchResultAction::Received(results)) => {
                    self.process_search_results(cx, scope, results.clone());
                }
                Some(SearchResultAction::ErrorWithNextBatchToken(next_batch_token)) => {
                    self.search_state.next_batch_token = next_batch_token.clone();
                    self.hide_bottom_space(cx);
                    self.view
                        .button(id!(search_all_rooms_button))
                        .set_enabled(cx, true);
                    // Show the search again button
                    self.view
                        .button(id!(search_again_button))
                        .set_visible(cx, true);
                    
                    let search_portal_list = self.portal_list(id!(searched_messages.list));
                    search_portal_list.set_first_id_and_scroll(search_portal_list.first_id().saturating_sub(10), 5.0);
                }
                _ => {}
            }
            let search_all_rooms_button = self.view.button(id!(search_all_rooms_button));
            if search_all_rooms_button.clicked(actions) {
                // Disable the button during search
                search_all_rooms_button.set_enabled(cx, false);
                let search_result_summary_ref =
                    self.search_result_summary(id!(search_result_plane));
                let mut criteria = search_result_summary_ref.get_search_criteria();
                search_result_summary_ref.reset(cx);
                criteria.include_all_rooms = true;
                search_result_summary_ref.set_search_criteria(cx, scope, criteria.clone());
                self.display_bottom_space(cx);
                self.search_state = SearchState::default();
                submit_async_request(MatrixRequest::SearchMessages {
                    message_search_choice: MessageSearchChoice::AllRooms,
                    search_term: criteria.search_term,
                    next_batch: None,
                    abort_previous_search: true,
                });
            }
            
            if self
                .view
                .button(id!(search_again_button))
                .clicked(actions)
            {
                self.view
                    .button(id!(search_again_button))
                    .set_visible(cx, false);
                if let Some(selected_room) = {
                    let app_state = scope.data.get::<AppState>().unwrap();
                    app_state.selected_room.clone()
                } {
                    let search_result_summary_ref =
                        self.search_result_summary(id!(search_result_plane));
                    let criteria = search_result_summary_ref.get_search_criteria();
                    let room_id = selected_room.room_id();
                    self.room_id = Some(room_id.clone());
                    let rooms_list_ref = cx.get_global::<RoomsListRef>();
                    let is_encrypted = rooms_list_ref.is_room_encrypted(room_id);
                    if is_encrypted && !criteria.include_all_rooms {
                        enqueue_popup_notification(PopupItem {
                            message: String::from("Searching for encrypted messages is not supported yet."),
                            auto_dismissal_duration: None,
                            kind: PopupKind::Info
                        });
                        return;
                    }
                    self.display_bottom_space(cx);
                    // Disable the search all rooms button during search
                    search_all_rooms_button.set_enabled(cx, false);
                    submit_async_request(MatrixRequest::SearchMessages {
                        message_search_choice: MessageSearchChoice::OneRoom(room_id.clone()),
                        search_term: criteria.search_term.clone(),
                        next_batch: None,
                        abort_previous_search: true,
                    });
                }
            }
        }
    }
}

/// Actions related to search result processing.
#[derive(Clone, Debug, DefaultNone)]
pub enum SearchResultAction {
    /// Search results have been received from the Matrix server.
    Received(SearchResultReceived),
    /// An error occurred while processing search results.
    /// The previous token should be restored.
    ErrorWithNextBatchToken(Option<String>),
    None,
}

impl Default for SearchResultReceived {
    fn default() -> Self {
        Self {
            items: Vector::new(),
            profile_infos: BTreeMap::new(),
            count: 0,
            search_term: String::new(),
            next_batch: None,
        }
    }
}

/// Data structure containing search results received from the Matrix server.
#[derive(Debug, Clone)]
pub struct SearchResultReceived {
    pub items: Vector<SearchResultItem>,
    pub profile_infos: BTreeMap<OwnedUserId, TimelineDetails<Profile>>,
    pub count: u32,
    pub search_term: String,
    pub next_batch: Option<String>,
}

// The widget that displays an overlay of the summary for search results.
#[derive(Live, LiveHook, Widget)]
pub struct SearchResultSummary {
    #[deref]
    pub view: View,
    #[rust]
    pub search_criteria: Criteria,
    /// The number of search results.
    ///
    /// This number includes the contextual messages which are not displayed.
    #[rust]
    pub result_count: u32,
    #[rust]
    pub room_name: Option<String>,
}

#[derive(Clone, Default, Debug)]
pub struct Criteria {
    pub search_term: String,
    pub include_all_rooms: bool,
    pub is_encrypted: bool,
}

impl Widget for SearchResultSummary {
    /// Handles events and actions for the SearchResultSummary widget and its inner Timeline view.
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if !self.visible {
            return;
        }
        self.view.handle_event(cx, event, scope);
    }
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        if !self.visible {
            return DrawStep::done();
        }
        self.view.draw_walk(cx, scope, walk)
    }
}

impl SearchResultSummary {
    /// Display search summary.
    ///
    /// This is used to display the number of search results and the search criteria
    /// in the top-right of the room screen.
    /// Optimized to avoid unnecessary string operations.
    fn set_result_count(&mut self, cx: &mut Cx, search_result_count: u32) {
        if self.result_count == search_result_count {
            return;
        }

        self.result_count = search_result_count;
        let location_text = if self.search_criteria.include_all_rooms {
            Cow::Borrowed("in all rooms")
        } else {
            Cow::Owned(format!(
                "in {}",
                self.room_name.as_deref().unwrap_or_default()
            ))
        };

        self.view.markdown(id!(summary_label)).set_text(
            cx,
            &format!(
                "{} result{} for **'{}'** {}",
                self.result_count,
                if self.result_count <= 1 { "" } else { "s" },
                truncate_to_50(&self.search_criteria.search_term),
                location_text
            ),
        );
        self.view.view(id!(loading_view)).set_visible(cx, false);
    }

    /// Sets the search criteria for the SearchResultSummary widget.
    ///
    /// This function is used to display the search criteria in the top-right of the room screen.
    /// It is typically used when a new search is initiated.
    ///
    fn set_search_criteria(&mut self, cx: &mut Cx, scope: &mut Scope, search_criteria: Criteria) {
        self.room_name = scope.data.get::<AppState>().and_then(|f| {
            f.selected_room
                .as_ref()
                .and_then(|f| f.room_name().cloned())
        });
        let location_text = if search_criteria.include_all_rooms {
            "in all rooms".to_string()
        } else {
            if let Some(room_name) = &self.room_name {
                format!("in {}", room_name.clone())
            } else {
                "".to_string()
            }
        };
        self.view.markdown(id!(summary_label)).set_text(
            cx,
            &format!(
                "Searching for **'{}'** {}",
                truncate_to_50(&search_criteria.search_term),
                location_text
            ),
        );
        self.search_criteria = search_criteria;
        self.visible = true;
        self.view.view(id!(loading_view)).set_visible(cx, true);
    }

    /// Resets the search result summary and set the loading view back to visible.
    ///
    /// This function clears the summary text and makes the loading indicator visible.
    /// It is typically used when a new search is initiated or search results are being cleared.
    fn reset(&mut self, cx: &mut Cx) {
        self.view.html(id!(summary_label)).set_text(cx, "");
        self.view.view(id!(loading_view)).set_visible(cx, true);
        self.search_criteria = Criteria::default();
        self.visible = false;
        self.result_count = 0;
    }
    
}
impl SearchResultSummaryRef {
    /// See [`SearchResultSummary::set_result_count()`].
    pub fn set_result_count(&mut self, cx: &mut Cx, search_result_count: u32) {
        let Some(mut inner) = self.borrow_mut() else {
            return;
        };
        inner.set_result_count(cx, search_result_count);
    }

    /// See [`SearchResultSummary::set_search_criteria()`].
    pub fn set_search_criteria(&self, cx: &mut Cx, scope: &mut Scope, search_criteria: Criteria) {
        let Some(mut inner) = self.borrow_mut() else {
            return;
        };
        inner.set_search_criteria(cx, scope, search_criteria);
    }

    /// See [`SearchResultSummary::reset()`].
    pub fn reset(&self, cx: &mut Cx) {
        let Some(mut inner) = self.borrow_mut() else {
            return;
        };
        inner.reset(cx);
    }

    /// See [`SearchResultSummary::get_search_criteria()`].
    pub fn get_search_criteria(&self) -> Criteria {
        let Some(inner) = self.borrow() else {
            return Criteria::default();
        };
        inner.search_criteria.clone()
    }
}

/// Search result as timeline item
#[derive(Clone, Debug)]
pub enum SearchResultItem {
    /// The event that matches the search criteria with precomputed formatted content.
    Event {
        event: Box<AnyTimelineEvent>,
        formatted_content: Box<Option<RoomMessageEventContent>>,
    },
    /// The room id used for displaying room header for all searched messages in a screen.
    RoomHeader(OwnedRoomId),
}

/// Message search choice.
#[derive(Clone, Debug)]
pub enum MessageSearchChoice {
    /// Search in all rooms
    AllRooms,
    /// Search in one room
    OneRoom(OwnedRoomId),
}

fn populate_message_search_view(
    cx: &mut Cx2d,
    list: &mut PortalList,
    item_id: usize,
    search_item: &SearchResultItem,
    user_profiles: &BTreeMap<OwnedUserId, TimelineDetails<Profile>>,
    item_drawn_status: ItemDrawnStatus,
) -> (WidgetRef, ItemDrawnStatus) {
    let mut new_drawn_status = item_drawn_status;
    let (event, formatted_content) = match search_item {
        SearchResultItem::Event {
            event,
            formatted_content,
        } => (event, formatted_content),
        SearchResultItem::RoomHeader(room_id) => {
            // Handle room header case
            let item = list.item(cx, item_id, live_id!(RoomHeader));
            let rooms_list_ref = cx.get_global::<RoomsListRef>();
            let room_name = rooms_list_ref
                .get_room_name(room_id)
                .unwrap_or(room_id.to_string());
            item.set_text(cx, &format!("Room {}", room_name));
            return (item, ItemDrawnStatus::both_drawn());
        }
    };

    let ts_millis = event.origin_server_ts();

    // Use precomputed formatted content
    let (item, used_cached_item) = if let Some(content) = formatted_content.as_ref() {
        match &content.msgtype {
            MessageType::Text(TextMessageEventContent {
                body, formatted, ..
            }) => {
                let template = live_id!(MessageCard);
                let (item, existed) = list.item_with_existed(cx, item_id, template);
                if existed && item_drawn_status.content_drawn {
                    (item, true)
                } else {
                    let html_or_plaintext_ref = item.html_or_plaintext(id!(content.message));
                    html_or_plaintext_ref.apply_over(
                        cx,
                        live!(
                            html_view = {
                                html = {
                                    font_color: (vec3(0.0,0.0,0.0)),
                                    draw_block: {
                                        code_color: (COLOR_WARNING_YELLOW)
                                    }
                                }
                            }
                        ),
                    );
                    populate_text_message_content(
                        cx,
                        &html_or_plaintext_ref,
                        body,
                        formatted.as_ref(),
                    );
                    new_drawn_status.content_drawn = true;
                    (item, false)
                }
            }
            _ => {
                let item = list.item(cx, item_id, live_id!(Empty));
                (item, false)
            }
        }
    } else {
        let item = list.item(cx, item_id, live_id!(Empty));
        (item, false)
    };
    // If `used_cached_item` is false, we should always redraw the profile, even if profile_drawn is true.
    let skip_draw_profile = used_cached_item && item_drawn_status.profile_drawn;
    if skip_draw_profile {
        // log!("\t --> populate_message_view(): SKIPPING profile draw for item_id: {item_id}");
        new_drawn_status.profile_drawn = true;
    } else {
        // log!("\t --> populate_message_view(): DRAWING  profile draw for item_id: {item_id}");
        let username_label = item.label(id!(content.username));
        let (username, profile_drawn) = item
            .avatar(id!(profile.avatar))
            .set_avatar_and_get_username(
                cx,
                event.room_id(),
                event.sender(),
                user_profiles.get(event.sender()),
                Some(event.event_id()),
            );
        username_label.set_text(cx, &username);
        new_drawn_status.profile_drawn = profile_drawn;
    }

    // If we've previously drawn the item content, skip all other steps.
    if used_cached_item && item_drawn_status.content_drawn && item_drawn_status.profile_drawn {
        return (item, new_drawn_status);
    }
    // Set the timestamp with date and time format.
    if let Some(dt) = unix_time_millis_to_datetime(ts_millis) {
        item.timestamp(id!(profile.timestamp))
            .set_date_time_with_format(cx, dt, "%F\n%H:%M");
    } else {
        item.label(id!(profile.timestamp))
            .set_text(cx, &format!("{}", ts_millis.get()));
    }
    item.as_message().set_jump_option(
        cx,
        JumpToMessageRequest {
            room_id: event.room_id().to_owned(),
            event_id: event.event_id().to_owned(),
        },
    );

    (item, new_drawn_status)
}

/// Truncates a string to 50 characters and appends "..." if longer.
/// Used for displaying search terms in the UI summary.
/// Uses Cow for efficient string handling to avoid unnecessary allocations.
fn truncate_to_50(s: &str) -> Cow<str> {
    if s.len() <= 50 {
        Cow::Borrowed(s)
    } else {
        Cow::Owned(format!("{}...", &s[..47]))
    }
}
