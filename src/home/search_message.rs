//! The SearchResult is the widget that contains search results interface for Desktop's right drawer.
//!
//! The SearchResultStackView is the stack navigation view that contains the `SearchResult` widget for Mobile view.
use std::{borrow::Cow, collections::BTreeMap, ops::DerefMut};
use regex::Regex;

use imbl::Vector;
use makepad_widgets::*;
use matrix_sdk::ruma::{
    api::client::{filter::RoomEventFilter, search::search_events::v3::{Criteria as MatrixCriteria, EventContext, OrderBy}}, events::{
        room::message::{
            AudioMessageEventContent, EmoteMessageEventContent, FileMessageEventContent, FormattedBody, ImageMessageEventContent, KeyVerificationRequestEventContent, MessageType, NoticeMessageEventContent, RoomMessageEventContent, TextMessageEventContent, VideoMessageEventContent
        }, AnyTimelineEvent
    }, uint, OwnedRoomId, OwnedUserId
};
use matrix_sdk_ui::timeline::{Profile, TimelineDetails};
use rangemap::RangeSet;

use crate::{
    app::{AppState, AppStateAction},
    home::{
        room_screen::{
            populate_text_message_content, ItemDrawnStatus, JumpToMessageRequest, MessageWidgetRefExt,
        },
        rooms_list::RoomsListRef,
    },
    shared::{
        avatar::AvatarWidgetRefExt, html_or_plaintext::HtmlOrPlaintextWidgetRefExt, message_search_input_bar::{MessageSearchAction, MessageSearchInputBarRef}, popup_list::{enqueue_popup_notification, PopupItem, PopupKind}, timestamp::TimestampWidgetRefExt
    },
    sliding_sync::{submit_async_request, MatrixRequest},
    utils::unix_time_millis_to_datetime,
};

// Consider extracting this constant
const SEARCH_HIGHLIGHT_COLOR: &str = "#fcdb03";

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;
    use crate::shared::styles::*;
    use crate::shared::helpers::*;
    use crate::shared::icon_button::RobrixIconButton;
    use crate::home::room_screen::*;

    COLOR_BUTTON_GREY = #B6BABF

    // The bottom space is used to display a loading message while the room is being paginated.
    BottomSpace = <View> {
        width: Fill, height: Fit,
        visible: true,
        align: {x: 0.5, y: 0}
        show_bg: true,
        draw_bg: {
            color: #xDAF5E5F0, // mostly opaque light green
        }
        flow: Right,

        label = <Label> {
            width: Fit, height: Fit,
            align: {x: 0.5, y: 0.5},
            padding: { top: 10.0, bottom: 7.0, left: 15.0, right: 15.0 }
            draw_text: {
                text_style: <MESSAGE_TEXT_STYLE> { font_size: 10 },
                color: (TIMESTAMP_TEXT_COLOR)
            }
            text: "Loading older search results..."
        }
        <LoadingSpinner> {
            width: 20,
            height: Fill,
            visible: true,
            draw_bg: {
                color: (COLOR_ACTIVE_PRIMARY)
                border_size: 3.0,
            }
        }
    }

    SearchIcon = <Icon> {
        width: Fit, height: Fit,
        margin: {top: 0, left: 10},
        align: {x: 0.0}
        spacing: 10,
        padding: 10,
        draw_bg: {
            instance color: (COLOR_BUTTON_GREY)
            instance color_hover: #fef65b
            uniform border_width: 1.5
            uniform border_radius: 4.0
            uniform hover: 0.0
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
                    max(1.0, self.border_radius)
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

    SearchResultSummary = {{SearchResultSummary}} {
        width: Fill, height: 60,
        show_bg: true,
        align: {y: 0.5}
        draw_bg: {
            color: (COLOR_SECONDARY)
        }

        <SearchIcon> {}
        summary_label = <Label> {
            width:Fill, height: Fit,
            margin: {left: 10, top: 0},
            align: {x: 0.0, y: 0.5}
            padding: 5,
            draw_text: {
                color: (MESSAGE_TEXT_COLOR),
                text_style: <REGULAR_TEXT> { font_size: (MESSAGE_FONT_SIZE) }
            }
            text: "Type to search."
        }
        search_all_rooms_button = <RobrixIconButton> {
            flow: RightWrap,
            width: 90,
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
            width: 70,
            visible: false,
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

    // White rounded message card against a grey backdrop.
    MessageCard = <Message> {
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

    SearchedMessages = <View> {
        width: Fill, height: Fill,
        align: {x: 0.5, y: 0.0} // center horizontally, align to top vertically
        flow: Overlay,

        list = <PortalList> {
            width: Fill, height: Fill,
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

    // SearchResult widget displays the main search results interface.
    pub SearchResult = {{SearchResult}} {
        no_more_template: <Label> {
            draw_text: {
                text_style: <REGULAR_TEXT>{
                    font_size: 11.5,
                },
                color: (COLOR_TEXT),
            }
            text: "No more results."
        }

        <View> {
            width: Fill, height: Fill,
            flow: Down,

            search_result_plane = <SearchResultSummary> {
                visible: true
            }
            searched_messages = <SearchedMessages> { }
            bottom_space = <BottomSpace> {
                visible: false
            }
        }
    }
    // SearchResultStackView is the main container widget for the search results screen.
    //
    // This widget provides the overall layout structure for displaying search results
    // within a navigation view. It's designed as a StackNavigationView that includes:
    // - A header with the title "Search Results"
    // - A body containing the SearchResult widget
    //
    pub SearchResultStackView = <StackNavigationView> {
        width: Fill, height: Fill
        full_screen: false
        padding: 0,
        draw_bg: {
            color: (COLOR_SECONDARY)
        }
        flow: Down

        body = {
            margin: {top: 0.0 },
            <SearchResult> {}
        }

        header = {
            height: 30.0,
            padding: {bottom: 10., top: 10.}
            content = {
                title_container = {
                    title = {
                        draw_text: {
                            wrap: Ellipsis,
                            text_style: { font_size: 10. }
                            color: #B,
                        }
                        text: "Search Results"
                    }
                }
            }
        }
    }
}

/// Apply highlights to a string by wrapping matching terms in HTML spans.
fn apply_highlights(text: String, highlights: &[String]) -> String {
    let mut result = text;
    for highlight in highlights {
        let re = Regex::new(&format!(r"(?i){}", regex::escape(highlight))).unwrap();
        result = re
            .replace_all(&result, |caps: &regex::Captures| {
                format!(
                    "<span data-mx-bg-color=\"{}\">{}</span>",
                    SEARCH_HIGHLIGHT_COLOR,
                    &caps[0].to_string() // Preserves original case
                )
            })
            .to_string();
    }
    result
}

/// Injects search term highlights into message content by wrapping matching terms in HTML spans.
///
/// This function modifies the message's formatted content to include visual highlighting
/// for search terms. It processes only text messages and preserves the original case
/// of matched terms while adding background color styling.
///
/// # Arguments
/// * `message` - The room message event content to modify
/// * `highlights` - Array of search terms to highlight in the message content
///
/// # Behavior
/// - Uses case-insensitive matching to find search terms
/// - Wraps matches in `<span data-mx-bg-color="#fcdb03">` tags for yellow highlighting
/// - Preserves original case and formatting of the matched text
/// - Creates or updates the message's `formatted` field with highlighted HTML content
pub fn highlight_search_terms_in_message(
    message: &mut RoomMessageEventContent,
    highlights: &[String],
) {
    /// Macro to apply highlights and set formatted content for message types with formatted fields
    macro_rules! apply_highlights_to_content {
        ($text:expr) => {{
            let formatted = if let Some(ref mut formatted) = $text.formatted {
                apply_highlights(formatted.body.clone(), highlights)
            } else {
                apply_highlights($text.body.clone(), highlights)
            };
            $text.formatted = Some(FormattedBody::html(formatted));
        }};
    }

    match &mut message.msgtype {
        MessageType::Text(content) => {
            apply_highlights_to_content!(content);
        }
        MessageType::Notice(content) => {
            apply_highlights_to_content!(content);
        }
        MessageType::Emote(content) => {
            apply_highlights_to_content!(content);
        }
        MessageType::File(content) => {
            apply_highlights_to_content!(content);
        }
        MessageType::Image(content) => {
            apply_highlights_to_content!(content);
        }
        MessageType::Video(content) => {
            apply_highlights_to_content!(content);
        }
        MessageType::Audio(content) => {
            apply_highlights_to_content!(content);
        }
        MessageType::VerificationRequest(content) => {
            apply_highlights_to_content!(content);
        }
        _ => {}
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
    /// Whether all search results have been fully paginated.
    is_fully_paginated: bool,
    /// This index is used when back pagination of the searched results fails.
    /// We will restore the scroll position to this index with enough offset
    /// so that the user can scroll down to try back pagination again.
    scroll_restore_checkpoint: usize,
}

/// The main widget that displays a list of search results.
///
/// SearchResult is a complex widget that manages the display and interaction
/// with Matrix message search results. It provides:
///
/// ## Key Features:
/// - **Infinite scrolling**: Automatically loads more results when scrolling
/// - **Message highlighting**: Highlights search terms in message content
/// - **Room filtering**: Can search in specific rooms or all rooms
/// - **Pagination**: Handles Matrix server pagination tokens efficiently
/// - **Loading states**: Shows loading indicators during search operations
///
/// ## State Management:
/// The widget maintains a `SearchState` that tracks:
/// - Search result items and their drawing status
/// - User profile information for message authors
/// - Pagination tokens for loading additional results
/// - Search parameters and room context
///
/// ## UI Components:
/// - Search summary bar showing result counts and search terms
/// - Scrollable message list with rich content rendering
/// - Loading indicators for ongoing operations
/// - "Search All Rooms" and "Search Again" action buttons
///
/// ## Event Handling:
/// The widget responds to scroll events to trigger pagination,
/// search input changes to update results, and button clicks
/// for room-specific vs global search operations.
#[derive(Live, LiveHook, Widget)]
struct SearchResult {
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

impl Widget for SearchResult {
    /// Handles events and actions for the SearchResult widget and its inner Timeline view.
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
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
        self.view.handle_event(cx, event, scope);
        self.widget_match_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        let tl_items = &self.search_state.items;
        while let Some(subview) = self.view.draw_walk(cx, scope, walk).step() {
            // We only care about drawing the portal list.
            let portal_list_ref = subview.as_portal_list();
            let Some(mut list_ref) = portal_list_ref.borrow_mut() else {
                error!("!!! SearchResult::draw_walk(): BUG: expected a PortalList widget, but got something else");
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

impl SearchResult {
    /// Sends a pagination request when the user is scrolling down and approaching the bottom of the search results.
    /// The request is sent with the `next_batch` token from the last search result received.
    fn paginate_search_results_based_on_scroll_pos(
        &mut self,
        cx: &mut Cx,
        actions: &ActionsBuf,
        portal_list: &PortalListRef,
        _scope: &mut Scope,
    ) {
        let total_items = self.search_state.items.len();
        if total_items == 0 {
            return;
        };
        if self.search_state.is_fully_paginated {
            return;
        };
        if !portal_list.scrolled(actions) {
            return;
        };
        let first_index = portal_list.first_id();
        let visible_items: usize = portal_list.visible_items();
        log!(
            "Scrolled down from item {} --> {:?}, before sending room {:?} last index {:?}",
            self.search_state.scroll_restore_checkpoint,
            total_items,
            self.room_id,
            first_index + visible_items
        );
        if first_index + visible_items >= total_items.saturating_sub(1)
            && self.search_state.scroll_restore_checkpoint < first_index
        {
            if let Some(next_batch_token) = self.search_state.next_batch_token.take() {
                log!("Scrolled down from item {} --> {:?}, sending back pagination request for search result in room {:?}",
                    self.search_state.scroll_restore_checkpoint, total_items, self.room_id,
                );
                let search_result_summary_ref =
                    self.view.search_result_summary(id!(search_result_plane));
                let includes_all_rooms = search_result_summary_ref.includes_all_rooms();
                self.display_bottom_space(cx);
                let message_search_choice =
                    if let (false, Some(room_id)) = (includes_all_rooms, &self.room_id) {
                        MessageSearchChoice::OneRoom(room_id.clone())
                    } else {
                        MessageSearchChoice::AllRooms
                    };
                let message_search_input_bar_ref = cx.get_global::<MessageSearchInputBarRef>();
                let search_term = message_search_input_bar_ref.get_text();
                submit_async_request(MatrixRequest::SearchMessages {
                    criteria: create_search_criteria(search_term, message_search_choice),
                    next_batch: Some(next_batch_token.clone()),
                    abort_previous_search: false,
                });
            }
        }
    }
}

impl SearchResult {
    /// Processes a new batch of search results and updates the UI and state.
    /// Optimized to take ownership of results to avoid clones.
    fn process_search_results(
        &mut self,
        cx: &mut Cx,
        scope: &mut Scope,
        results: SearchResultReceived,
    ) {
        let message_search_input_bar_ref = cx.get_global::<MessageSearchInputBarRef>();
        let search_term = message_search_input_bar_ref.get_text();
        let search_result_summary_ref = self.view.search_result_summary(id!(search_result_plane));
        // If the search input text has changed, reset everything.
        if search_term != results.search_term {
            cx.widget_action(
                self.widget_uid(),
                &scope.path,
                MessageSearchAction::Changed(search_term),
            );
            return;
        }
        self.hide_bottom_space(cx);
        // Re-enable the search all rooms button when results are received
        self.view
            .button(id!(search_all_rooms_button))
            .set_enabled(cx, true);

        search_result_summary_ref.display_result_summary(cx, &results);
        // Take ownership of profile infos instead of cloning
        self.search_state.profile_infos = results.profile_infos;
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
                search_portal_list.smooth_scroll_to(cx, 0, 0.0, None);
            }
        }
        self.search_state.next_batch_token = results.next_batch.clone();
        self.search_state.is_fully_paginated = results.next_batch.is_none();
        self.search_state.prev_search_term = Some(results.search_term);
        self.search_state.scroll_restore_checkpoint = search_portal_list.first_id();
        self.redraw(cx);
    }

    /// Handles actions from the MessageSearchInputBar.
    fn handle_search_bar_action(&mut self, cx: &mut Cx, scope: &mut Scope, action: &Action) {
        match action.as_widget_action().cast() {
            MessageSearchAction::Changed(search_term) => {
                self.hide_bottom_space(cx);
                let search_result_summary_ref =
                    self.search_result_summary(id!(search_result_plane));
                self.search_state = SearchState::default();
                if search_term.is_empty() {
                    self.view
                        .button_set(ids!(search_all_rooms_button, search_again_button))
                        .set_enabled(cx, false);
                    // Abort previous inflight search request regardless of message choice if search term is empty.
                    submit_async_request(MatrixRequest::SearchMessages {
                        criteria: create_search_criteria(
                            search_term,
                            MessageSearchChoice::AllRooms,
                        ),
                        next_batch: None,
                        abort_previous_search: true,
                    });
                    search_result_summary_ref.display_instruction(cx);
                    return;
                }
                self.view
                    .button_set(ids!(search_all_rooms_button, search_again_button))
                    .set_enabled(cx, true);
                if let Some(selected_room) = {
                    let app_state = scope.data.get::<AppState>().unwrap();
                    app_state.selected_room.clone()
                } {
                    let room_id = selected_room.room_id();
                    let criteria = create_search_criteria(
                        search_term.clone(),
                        MessageSearchChoice::OneRoom(room_id.clone()),
                    );
                    search_result_summary_ref.display_search_criteria(cx, scope, criteria.clone());
                    self.room_id = Some(room_id.clone());
                    let rooms_list_ref = cx.get_global::<RoomsListRef>();
                    let is_encrypted = rooms_list_ref.is_room_encrypted(room_id);
                    if is_encrypted && criteria.filter.rooms.is_none() {
                        enqueue_popup_notification(PopupItem {
                            message: String::from(
                                "Searching for encrypted messages is not supported yet.",
                            ),
                            auto_dismissal_duration: None,
                            kind: PopupKind::Info,
                        });
                        return;
                    }
                    self.display_bottom_space(cx);
                    // Disable the search all rooms button during search
                    self.view
                        .button(id!(search_all_rooms_button))
                        .set_enabled(cx, false);
                    submit_async_request(MatrixRequest::SearchMessages {
                        criteria,
                        next_batch: None,
                        abort_previous_search: true,
                    });
                }
            }
            MessageSearchAction::Clicked => {
                let search_result_summary_ref =
                    self.search_result_summary(id!(search_result_plane));
                let message_search_input_bar_ref = cx.get_global::<MessageSearchInputBarRef>();
                let search_term = message_search_input_bar_ref.get_text();
                if search_term.is_empty() {
                    search_result_summary_ref.display_instruction(cx);
                }
            }
            _ => {}
        }
    }

    fn handle_room_focus_changed_action(&mut self, cx: &mut Cx, action: &Action) {
        if let AppStateAction::RoomFocused(_room_id) = action.as_widget_action().cast() {
            self.view
                .button(id!(search_again_button))
                .set_visible(cx, true);
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

impl WidgetMatchEvent for SearchResult {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions, scope: &mut Scope) {
        for action in actions.iter() {
            self.handle_search_bar_action(cx, scope, action);
            self.handle_room_focus_changed_action(cx, action);
            match action.downcast_ref() {
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
                    // Leave an Y offset 100.0, so that the user can still scroll down to trigger back pagination again.
                    search_portal_list.set_first_id_and_scroll(
                        self.search_state.scroll_restore_checkpoint,
                        100.0,
                    );
                }
                _ => {}
            }
            let search_all_rooms_button = self.view.button(id!(search_all_rooms_button));
            if search_all_rooms_button.clicked(actions) {
                let message_search_input_bar_ref = cx.get_global::<MessageSearchInputBarRef>();
                let search_term = message_search_input_bar_ref.get_text();
                if search_term.is_empty() {
                    continue;
                }
                self.search_state = SearchState::default();
                // Disable the button during search
                search_all_rooms_button.set_enabled(cx, false);
                let criteria = create_search_criteria(search_term, MessageSearchChoice::AllRooms);
                let search_result_summary_ref =
                    self.search_result_summary(id!(search_result_plane));
                search_result_summary_ref.set_includes_all_rooms(cx, true);
                search_result_summary_ref.display_search_criteria(cx, scope, criteria.clone());
                self.display_bottom_space(cx);
                submit_async_request(MatrixRequest::SearchMessages {
                    criteria,
                    next_batch: None,
                    abort_previous_search: true,
                });
            }
            let search_again_button_ref = self.view.button(id!(search_again_button));
            if search_again_button_ref.clicked(actions) {
                search_again_button_ref.set_visible(cx, false);
                if let Some(selected_room) = {
                    let app_state = scope.data.get::<AppState>().unwrap();
                    app_state.selected_room.clone()
                } {
                    let search_result_summary_ref =
                        self.search_result_summary(id!(search_result_plane));
                    let Some(previous_room_id) = &self.room_id else {
                        continue;
                    };
                    if previous_room_id != selected_room.room_id() {
                        self.search_state = SearchState::default();
                    }
                    let includes_all_rooms = search_result_summary_ref.includes_all_rooms();
                    let room_id = selected_room.room_id();
                    self.room_id = Some(room_id.clone());
                    let rooms_list_ref = cx.get_global::<RoomsListRef>();
                    let is_encrypted = rooms_list_ref.is_room_encrypted(room_id);
                    if is_encrypted && !includes_all_rooms {
                        enqueue_popup_notification(PopupItem {
                            message: String::from(
                                "Searching for encrypted messages is not supported yet.",
                            ),
                            auto_dismissal_duration: None,
                            kind: PopupKind::Info,
                        });
                        return;
                    }
                    self.display_bottom_space(cx);
                    // Disable the search all rooms button during search
                    search_all_rooms_button.set_enabled(cx, false);
                    let message_search_input_bar_ref = cx.get_global::<MessageSearchInputBarRef>();
                    let search_term = message_search_input_bar_ref.get_text();
                    let criteria = create_search_criteria(
                        search_term,
                        if includes_all_rooms {
                            MessageSearchChoice::AllRooms
                        } else {
                            MessageSearchChoice::OneRoom(room_id.clone())
                        },
                    );
                    search_result_summary_ref.display_search_criteria(cx, scope, criteria.clone());
                    submit_async_request(MatrixRequest::SearchMessages {
                        criteria,
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
            includes_all_rooms: false,
            room_name: None,
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
    pub room_name: Option<String>,
    pub includes_all_rooms: bool,
}

// The widget that displays an overlay of the summary for search results.
#[derive(Live, LiveHook, Widget)]
pub struct SearchResultSummary {
    #[deref]
    pub view: View,
    #[rust]
    pub room_name: Option<String>,
    #[rust]
    pub includes_all_rooms: bool,
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
        let message_search_input_bar_ref = cx.get_global::<MessageSearchInputBarRef>();
        let search_term = message_search_input_bar_ref.get_text();
        self.view
            .button(id!(search_again_button))
            .set_visible(cx, !search_term.is_empty());
        self.view.draw_walk(cx, scope, walk)
    }
}

impl SearchResultSummary {
    /// Display the search criteria for the SearchResultSummary widget.
    ///
    /// This function is used to display the search criteria in the top-right of the room screen.
    /// It is typically used when a new search is initiated.
    ///
    pub fn display_search_criteria(
        &mut self,
        cx: &mut Cx,
        scope: &mut Scope,
        search_criteria: MatrixCriteria,
    ) {
        self.room_name = scope.data.get::<AppState>().and_then(|f| {
            f.selected_room
                .as_ref()
                .and_then(|f| f.room_name().cloned())
        });
        let location_text = if search_criteria.filter.rooms.is_none() {
            "in all rooms".to_string()
        } else {
            if let Some(room_name) = &self.room_name {
                format!("in {}", room_name.clone())
            } else {
                "".to_string()
            }
        };
        self.view.label(id!(summary_label)).set_text(
            cx,
            &format!(
                "Searching for '{}' {}",
                truncate_to_50(&search_criteria.search_term),
                location_text
            ),
        );
        self.visible = true;
    }

    /// Display result summary.
    ///
    /// This is used to display the number of search results and the search term at the top of the search result.
    pub fn display_result_summary(&mut self, cx: &mut Cx, results: &SearchResultReceived) {
        let location_text = if results.includes_all_rooms {
            Cow::Borrowed("in all rooms")
        } else {
            Cow::Owned(format!(
                "in {}",
                results.room_name.as_deref().unwrap_or_default()
            ))
        };

        self.view.label(id!(summary_label)).set_text(
            cx,
            &format!(
                "{} result{} for '{}' {}",
                results.count,
                if results.count <= 1 { "" } else { "s" },
                truncate_to_50(&results.search_term),
                location_text
            ),
        );
    }

    /// Set whether the search criteria to include all rooms.
    pub fn set_includes_all_rooms(&mut self, _cx: &mut Cx, includes_all_rooms: bool) {
        self.includes_all_rooms = includes_all_rooms;
    }
}

impl SearchResultSummaryRef {
    /// Display instruction to type to search.
    pub fn display_instruction(&self, cx: &mut Cx) {
        let Some(mut inner) = self.borrow_mut() else {
            return;
        };
        inner
            .view
            .label(id!(summary_label))
            .set_text(cx, "Type to search.");
        inner.visible = true;
    }

    /// See [`SearchResultSummary::display_search_criteria()`].
    pub fn display_search_criteria(
        &self,
        cx: &mut Cx,
        scope: &mut Scope,
        search_criteria: MatrixCriteria,
    ) {
        let Some(mut inner) = self.borrow_mut() else {
            return;
        };
        inner.display_search_criteria(cx, scope, search_criteria);
    }

    /// See [`SearchResultSummary::display_result_summary()`].
    pub fn display_result_summary(&self, cx: &mut Cx, results: &SearchResultReceived) {
        let Some(mut inner) = self.borrow_mut() else {
            return;
        };
        inner.display_result_summary(cx, results);
    }

    /// See [`SearchResultSummary::includes_all_rooms()`].
    pub fn includes_all_rooms(&self) -> bool {
        let Some(inner) = self.borrow() else {
            return false;
        };
        inner.includes_all_rooms
    }

    /// See [`SearchResultSummary::set_includes_all_rooms()`].
    pub fn set_includes_all_rooms(&self, cx: &mut Cx, includes_all_rooms: bool) {
        let Some(mut inner) = self.borrow_mut() else {
            return;
        };
        inner.set_includes_all_rooms(cx, includes_all_rooms);
    }
}

/// Search result as timeline item
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug)]
pub enum SearchResultItem {
    /// The event that matches the search criteria with precomputed formatted content.
    Event {
        event: AnyTimelineEvent,
        formatted_content: Option<RoomMessageEventContent>,
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
    let (item, used_cached_item) = if let Some(content) = formatted_content {
        match &content.msgtype {
            MessageType::Text(TextMessageEventContent {
                body, formatted, ..
            })
            | MessageType::Notice(NoticeMessageEventContent {
                body, formatted, ..
            })
            | MessageType::Emote(EmoteMessageEventContent {
                body, formatted, ..
            })
            | MessageType::Image(ImageMessageEventContent {
                body, formatted, ..
            })
            | MessageType::File(FileMessageEventContent {
                body, formatted, ..
            })
            | MessageType::Audio(AudioMessageEventContent {
                body, formatted, ..
            })
            | MessageType::Video(VideoMessageEventContent {
                body, formatted, ..
            })
            | MessageType::VerificationRequest(KeyVerificationRequestEventContent {
                body,
                formatted,
                ..
            }) => {
                let template = live_id!(MessageCard);
                let (item, existed) = list.item_with_existed(cx, item_id, template);
                if existed && item_drawn_status.content_drawn {
                    (item, true)
                } else {
                    populate_text_message_content(
                        cx,
                        &item.html_or_plaintext(id!(content.message)),
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
fn truncate_to_50(s: &str) -> Cow<'_, str> {
    if s.len() <= 50 {
        Cow::Borrowed(s)
    } else {
        Cow::Owned(format!("{}...", &s[..47]))
    }
}

/// Creates a search criteria object based on the search parameters.
fn create_search_criteria(
    search_term: String,
    message_search_choice: MessageSearchChoice,
) -> MatrixCriteria {
    let mut room_filter = RoomEventFilter::empty();
    if let MessageSearchChoice::OneRoom(room_id) = &message_search_choice {
        room_filter.rooms = Some(vec![room_id.to_owned()]);
    } else {
        room_filter.rooms = None;
    }
    let mut criteria = MatrixCriteria::new(search_term);
    criteria.filter = room_filter;
    criteria.order_by = Some(OrderBy::Recent);
    criteria.event_context = EventContext::new();
    criteria.event_context.after_limit = uint!(0);
    criteria.event_context.before_limit = uint!(0);
    criteria.event_context.include_profile = true;
    criteria
}
