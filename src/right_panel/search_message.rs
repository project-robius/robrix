use std::{collections::BTreeMap, ops::DerefMut};

use imbl::Vector;
use makepad_widgets::*;
use matrix_sdk::ruma::{events::{room::message::{FormattedBody, MessageType, Relation, RoomMessageEventContent, TextMessageEventContent}, AnyMessageLikeEventContent, AnyTimelineEvent}, OwnedRoomId, OwnedUserId, RoomId};
use matrix_sdk_ui::timeline::{Profile, TimelineDetails};
use rangemap::RangeSet;

use crate::{app::AppState, home::{room_screen::{populate_audio_message_content, populate_file_message_content, populate_image_message_content, populate_text_message_content, ItemDrawnStatus}, rooms_list::RoomsListRef}, media_cache::MediaCache, shared::{avatar::AvatarWidgetRefExt, html_or_plaintext::HtmlOrPlaintextWidgetRefExt, message_search_input_bar::MessageSearchAction, popup_list::enqueue_popup_notification, text_or_image::TextOrImageWidgetRefExt, timestamp::TimestampWidgetRefExt}, sliding_sync::{submit_async_request, MatrixRequest}, utils::unix_time_millis_to_datetime};


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
    ICON_SEARCH = dep("crate://self/resources/icons/search.svg")
    // The top space is used to display a loading message while the room is being paginated.
    TopSpace = <View> {
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
            text: "Loading search results..."
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
    pub SearchResult = {{SearchResult}} {
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
                    width: Fill,
                    height: Fill,
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
                cancel_button = <RobrixIconButton> {
                    width: Fit,
                    height: Fit,
                    padding: 10,
                    spacing: 0,
                    margin: {left: 0, right: 10, top: -2},

                    draw_bg: {
                        border_color: (COLOR_DANGER_RED),
                        color: #fff0f0 // light red
                        border_radius: 5
                    }
                    draw_icon: {
                        svg_file: (ICON_CLOSE),
                        color: (COLOR_DANGER_RED)
                    }
                    icon_walk: {width: 16, height: 16, margin: 0}
                }
            }
            top_space = <TopSpace> {
                visible: false
            }
        }
    }
    pub MessageCard = <Message> {
        draw_bg: {
            instance highlight: 0.0
            instance hover: 0.0
            color: #ffffff  // default color
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
    pub TimelineSearch = <View> {
        width: Fill,
        height: Fill,
        align: {x: 0.5, y: 0.0} // center horizontally, align to top vertically
        flow: Overlay,
        list = <PortalList> {
            height: Fill,
            width: Fill
            flow: Down

            auto_tail: true, // set to `true` to lock the view to the last item.
            max_pull_down: 0.0, // set to `0.0` to disable the pulldown bounce animation.

            // Below, we must place all of the possible templates (views) that can be used in the portal list.
            Message = <Message> {}
            MessageCard = <MessageCard> {}
            ImageMessage = <ImageMessage> {}
            Empty = <Empty> {}
            RoomHeader = <Label> {
                margin: {left: 10},
                draw_text: {
                    text_style: <REGULAR_TEXT> {
                        font_size: 12.5,
                    },
                    color: #000,
                }
                text: "??"
            }
            NoMoreMessages = <Label> {
                margin: {left: 10, top: 30},
                draw_text: {
                    text_style: <REGULAR_TEXT> {
                        font_size: 16.5,
                    },
                    color: #000,
                }
                text: "??"
            }
            
        }
    }
    
    pub SearchScreen = {{SearchScreen}} {
        <View> {
            width: Fill,
            height: Fill,
            flow: Down,
            message_search_input_view = <View> {
                width: Fill, height: Fit,
                visible: true,
                <CachedWidget> {
                    message_search_input_bar = <MessageSearchInputBar> {
                        width: Fill,
                    }
                }
            }
            <View> {
                width: Fill,
                height: Fill,
                flow: Overlay,
                search_timeline = <TimelineSearch> {
                    width: Fill,
                    height: Fill,
                }
                search_result_plane = <SearchResult> {
                    width: Fill,
                    height: Fill,
                    visible: true
                }
            }
        }
    }
}

//Yellow
const SEARCH_HIGHLIGHT: Vec3 = Vec3 { x: 1.0, y: 0.87, z: 0.127 };

/// States that are necessary to display search results.
#[derive(Default)]
pub struct SearchState {
    /// The list of events in the search results.
    pub items: Vector<SearchResultItem>,
    /// The list of strings that should be highlighted in the search results.
    pub highlighted_strings: Vec<String>,
    /// See [`TimelineUiState.content_drawn_since_last_update`].
    pub content_drawn_since_last_update: RangeSet<usize>,
    /// Same as `content_drawn_since_last_update`, but for the event **profiles** (avatar, username).
    pub profile_drawn_since_last_update: RangeSet<usize>,
    /// All profile infos for the search results.
    pub profile_infos: BTreeMap<OwnedUserId, TimelineDetails<Profile>>,
    pub fully_paginated: bool,
    /// The index of the timeline item that was most recently scrolled up past it.
    pub last_scrolled_index: usize,
    /// Token to be use for pagination of earlier search results.
    pub next_batch_token: Option<String>,
}

/// The main widget that displays a single Matrix room.
#[derive(Live, LiveHook, Widget)]
pub struct SearchScreen {
    #[deref] 
    pub view: View,
    #[layout]
    layout: Layout,
    #[walk]
    walk: Walk,
    #[rust]
    pub search_state: SearchState,
    #[live]
    pub no_more_template: Option<LivePtr>,
    #[rust]
    pub room_id: Option<OwnedRoomId>,
}

impl Widget for SearchScreen {
    // Handle events and actions for the SearchScreen widget and its inner Timeline view.
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        self.widget_match_event(cx, event, scope);
    }
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        search_result_draw_walk(self, cx, scope, walk)
    }
}
impl WidgetMatchEvent for SearchScreen {
    fn handle_actions(&mut self, cx: &mut Cx, actions:&Actions, scope: &mut Scope) {
        for action in actions.iter() {
            handle_search_input(self, cx, action, scope);
            if let Some(SearchResultAction::Ok(SearchResultReceived {
                items,
                profile_infos,
                search_term,
                count,
                highlights,
                next_batch
            })) = action.downcast_ref() {
                self.view
                    .search_result(id!(search_result_plane)).hide_top_space(cx);
                let mut criteria = self.view
                    .search_result(id!(search_result_plane))
                    .get_search_criteria();
                if criteria.search_term != *search_term {
                    self.search_state.items = Vector::new();
                }
                self.search_state.profile_infos = profile_infos.clone();
                cx.action(MessageSearchAction::SetText(search_term.clone()));
                criteria.search_term = search_term.clone();
                self.view
                    .search_result(id!(search_result_plane))
                    .set_search_criteria(cx, criteria);
                self.view
                    .search_result(id!(search_result_plane))
                    .set_result_count(cx, *count);
                self.view.view(id!(search_timeline)).set_visible(cx, true);
                self.search_state
                    .content_drawn_since_last_update
                    .clear();
                self.search_state
                    .profile_drawn_since_last_update
                    .clear();
                for item in items {
                    self.search_state.items.push_front(item.clone());
                }
                let search_portal_list = self.portal_list(id!(search_timeline.list));
                if let Some(mut search_portal_list) = search_portal_list.borrow_mut() {
                    search_portal_list.set_item_range(cx, 0, self.search_state.items.len());
                }
                search_portal_list.set_first_id_and_scroll(
                    self.search_state.items.len().saturating_sub(1),
                    0.0,
                );
                search_portal_list.set_tail_range(true);
                self.search_state.highlighted_strings = highlights.to_vec();
                self.search_state.next_batch_token = next_batch.to_owned();
                self.redraw(cx);
            }
            if self.view.button(id!(search_all_rooms_button)).clicked(actions) {
                let mut criteria = self.search_result(id!(search_result_plane)).get_search_criteria();
                self.search_result(id!(search_result_plane)).reset(cx);
                criteria.include_all_rooms = true;
                self.search_result(id!(search_result_plane)).set_search_criteria(cx, criteria.clone());
                self.search_state = SearchState::default();
                submit_async_request(MatrixRequest::SearchMessages { room_id: None, include_all_rooms: true, search_term: criteria.search_term, next_batch: None, abort_previous_search: true });
            }
        }
        
    }
}

#[derive(Clone, Debug, DefaultNone)]
pub enum SearchResultAction{
    Ok(SearchResultReceived),
    None
}

#[derive(Default, Debug, Clone)]
pub struct SearchResultReceived {
    pub items: Vec<SearchResultItem>,
    pub profile_infos: BTreeMap<OwnedUserId, TimelineDetails<Profile>>,
    pub count: u32,
    pub highlights: Vec<String>,
    pub search_term: String,
    pub next_batch: Option<String>,
}


// The widget that displays an overlay of the summary for search results.
#[derive(Live, LiveHook, Widget)]
pub struct SearchResult {
    #[deref]
    pub view: View,
    #[rust]
    pub search_criteria: Criteria,
    #[rust]
    pub result_count: u32,
}

#[derive(Clone, Default)]
pub struct Criteria {
    pub search_term: String,
    pub include_all_rooms: bool,
    pub is_encrypted: bool,
}

impl Widget for SearchResult {
    // Handle events and actions for the SearchResult widget and its inner Timeline view.
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if !self.visible {
            return;
        }
        self.match_event(cx, event);
        self.view.handle_event(cx, event, scope);
    }
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        if !self.visible {
            return DrawStep::done();
        }
        self.view.draw_walk(cx, scope, walk)
    }
}
impl MatchEvent for SearchResult {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        let cancel_button_clicked = self.view.button(id!(cancel_button)).clicked(actions);
        if cancel_button_clicked {
            cx.widget_action(
                self.widget_uid(),
                &Scope::empty().path,
                MessageSearchAction::Clear,
            );
            submit_async_request(MatrixRequest::SearchMessages {
                room_id: None,
                include_all_rooms: false,
                search_term: "".to_string(),
                next_batch: None,
                abort_previous_search: true
            });
        }
    }
}
impl SearchResult {
    /// Display search summary.
    ///
    /// This is used to display the number of search results and the search criteria
    /// in the top-right of the room screen.
    fn set_result_count(&mut self, cx: &mut Cx, search_result_count: u32) {
        self.result_count = search_result_count;
        self.view.markdown(id!(summary_label)).set_text(
            cx,
            &format!(
                "{} results for **'{}'**",
                self.result_count, truncate_to_50(&self.search_criteria.search_term)
            ),
        );
        self.view.view(id!(loading_view)).set_visible(cx, false);
    }
    fn set_search_criteria(&mut self, cx: &mut Cx, search_criteria: Criteria) {
        self.view.markdown(id!(summary_label)).set_text(
            cx,
            &format!("Searching for **'{}'**", truncate_to_50(&search_criteria.search_term)),
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
    /// Displays the loading view for backwards pagination for search result.
    fn display_top_space(&mut self, cx: &mut Cx) {
        self.view.view(id!(top_space)).set_visible(cx, true);
    }
    /// Hides the loading view for backwards pagination for search result.
    fn hide_top_space(&mut self, cx: &mut Cx) {
        self.view.view(id!(top_space)).set_visible(cx, false);
    }
}
impl SearchResultRef {
    /// See [`SearchResult::set_result_count()`].
    pub fn set_result_count(&mut self, cx: &mut Cx, search_result_count: u32) {
        let Some(mut inner) = self.borrow_mut() else {
            return;
        };
        inner.set_result_count(cx, search_result_count);
    }
    /// See [`SearchResult::set_search_criteria()`].
    pub fn set_search_criteria(&self, cx: &mut Cx, search_criteria: Criteria) {
        let Some(mut inner) = self.borrow_mut() else {
            return;
        };
        inner.set_search_criteria(cx, search_criteria);
    }
    /// See [`SearchResult::reset()`].
    pub fn reset(&self, cx: &mut Cx) {
        let Some(mut inner) = self.borrow_mut() else {
            return;
        };
        inner.reset(cx);
    }
    /// See [`SearchResult::display_top_space()`].
    pub fn display_top_space(&self, cx: &mut Cx) {
        let Some(mut inner) = self.borrow_mut() else {
            return;
        };
        inner.display_top_space(cx);
    }
    /// See [`SearchResult::hide_top_space()`].
    pub fn hide_top_space(&self, cx: &mut Cx) {
        let Some(mut inner) = self.borrow_mut() else {
            return;
        };
        inner.hide_top_space(cx);
    }
    /// See [`SearchResult::get_search_criteria()`].
    pub fn get_search_criteria(&self) -> Criteria {
        let Some(inner) = self.borrow() else {
            return Criteria::default();
        };
        inner.search_criteria.clone()
    }
}

/// This is a specialized version of `RoomScreen::draw_walk()` that is specific to rendering the timeline of a search result.
///
/// It takes a `RoomScreen` widget and iterates through the `PortalList` widget inside it. For each item in the list, it checks if the item is a `DateDivider`, `ContextEvent` or `Event` and renders it accordingly.
///
/// The rendering of the timeline items is done by calling `populate_message_view()` for messages and `populate_small_state_event()` for state events.
///
/// This function is used in the `RoomScreen` widget's `draw_walk()` method when the timeline is being rendered as a search result.
pub fn search_result_draw_walk(
    room_screen: &mut SearchScreen,
    cx: &mut Cx2d,
    scope: &mut Scope,
    walk: Walk,
) -> DrawStep {
    while let Some(subview) = room_screen.view.draw_walk(cx, scope, walk).step() {
        // We only care about drawing the portal list.
        let portal_list_ref = subview.as_portal_list();
        let Some(mut list_ref) = portal_list_ref.borrow_mut() else {
            error!("!!! RoomScreen::draw_walk(): BUG: expected a PortalList widget, but got something else");
            continue;
        };
        let tl_items = &room_screen.search_state.items;
        // Set the portal list's range based on the number of timeline items.
        let last_item_id = tl_items.len();
        let list = list_ref.deref_mut();
        list.set_item_range(cx, 0, last_item_id);

        while let Some(item_id) = list.next_visible_item(cx) {
            if item_id == 0 && room_screen.search_state.next_batch_token.is_none() && last_item_id > 0 {
                WidgetRef::new_from_ptr(cx, room_screen.no_more_template)
                    .as_label()
                    .draw_all(cx, &mut Scope::empty());
            }
            let item = {
                let tl_idx = item_id;
                let Some(timeline_item) = tl_items.get(tl_idx) else {
                    // This shouldn't happen (unless the timeline gets corrupted or some other weird error),
                    // but we can always safely fill the item with an empty widget that takes up no space.
                    list.item(cx, item_id, live_id!(Empty));
                    continue;
                };
                let item_drawn_status = ItemDrawnStatus {
                    content_drawn: room_screen
                        .search_state
                        .content_drawn_since_last_update
                        .contains(&tl_idx),
                    profile_drawn: room_screen
                        .search_state
                        .profile_drawn_since_last_update
                        .contains(&tl_idx),
                };
                let (item, item_new_draw_status) = {
                    let current_item = timeline_item;

                    match &current_item {
                        SearchResultItem::Event(event) => match &**event {
                            AnyTimelineEvent::MessageLike(msg) => {
                                let mut content = msg.original_content();
                                if let Some(replace) = msg.relations().replace {
                                    content = replace.original_content();
                                }
                                match content {
                                    Some(AnyMessageLikeEventContent::RoomMessage(message)) => {
                                        let mut message = message.clone();
                                        if let Some(Relation::Replacement(replace)) = &message.relates_to {
                                            let new_content = &replace.new_content;
                                            message.msgtype = new_content.msgtype.clone();
                                        }

                                        if let MessageType::Text(text) = &mut message.msgtype {
                                            if let Some(ref mut formatted) = text.formatted {
                                                for highlight in room_screen
                                                    .search_state
                                                    .highlighted_strings
                                                    .iter()
                                                {
                                                    formatted.body = formatted.body.replace(
                                                        highlight,
                                                        &format!("<code>{}</code>", highlight),
                                                    );
                                                }
                                            } else {
                                                let mut formatted_string = text.body.clone();
                                                for highlight in room_screen
                                                    .search_state
                                                    .highlighted_strings
                                                    .iter()
                                                {
                                                    formatted_string = formatted_string.replace(
                                                        highlight,
                                                        &format!("<code>{}</code>", highlight),
                                                    );
                                                }
                                                text.formatted =
                                                    Some(FormattedBody::html(formatted_string));
                                            }
                                        }
                                        let mut media_cache = MediaCache::new(None);
                                        populate_message_search_view(
                                            cx,
                                            list,
                                            item_id,
                                            event,
                                            &message,
                                            &room_screen.search_state.profile_infos,
                                            &mut media_cache,
                                            item_drawn_status,
                                        )
                                    }
                                    _ => {
                                        list.item(cx, item_id, live_id!(Empty))
                                            .draw_all(cx, &mut Scope::empty());
                                        continue;
                                    }
                                }
                            },
                            _ => {
                                list.item(cx, item_id, live_id!(Empty))
                                    .draw_all(cx, &mut Scope::empty());
                                continue;
                            }
                        },

                        SearchResultItem::RoomHeader(room_id) => {
                            let rooms_list_ref = cx.get_global::<RoomsListRef>();
                            let room_name = rooms_list_ref
                                .get_room_name(room_id)
                                .unwrap_or(room_id.to_string());
                            let item = list.item(cx, item_id, live_id!(RoomHeader));
                            item.set_text(cx, &format!("Room {}", room_name));
                            (item, ItemDrawnStatus::both_drawn())
                        }
                    }
                };
                if item_new_draw_status.content_drawn {
                    room_screen
                        .search_state
                        .content_drawn_since_last_update
                        .insert(tl_idx..tl_idx + 1);
                }
                if item_new_draw_status.profile_drawn {
                    room_screen
                        .search_state
                        .profile_drawn_since_last_update
                        .insert(tl_idx..tl_idx + 1);
                }
                item
            };
            item.draw_all(cx, &mut Scope::empty());
        }
    }
    DrawStep::done()
}

/// Handles any search-related actions received by this RoomScreen.
///
/// See `MessageSearchAction` for the possible actions.
pub fn handle_search_input(
    room_screen: &mut SearchScreen,
    cx: &mut Cx,
    action: &Action,
    scope: &mut Scope,
) {
    let widget_action = action.as_widget_action();
    match widget_action.cast() {
        MessageSearchAction::Changed(search_term) => {
            if search_term.is_empty() {
                room_screen
                    .view(id!(search_timeline))
                    .set_visible(cx, false);
                room_screen
                    .search_result(id!(search_result_plane))
                    .reset(cx);
                room_screen
                    .search_result(id!(search_result_plane))
                    .set_visible(cx, false);
                room_screen.search_state = SearchState::default();
                // Abort previous inflight search request.
                submit_async_request(MatrixRequest::SearchMessages {
                    room_id: None,
                    include_all_rooms: false,
                    search_term: "".to_string(),
                    next_batch: None,
                    abort_previous_search: true
                });
                return;
            }
            if let Some(selected_room) = {
                let app_state = scope.data.get::<AppState>().unwrap();
                app_state.selected_room.clone()
            } {
                let mut criteria = room_screen
                    .search_result(id!(search_result_plane))
                    .get_search_criteria();
                criteria.search_term = search_term;
                criteria.include_all_rooms = false;
                room_screen
                    .search_result(id!(search_result_plane))
                    .set_search_criteria(cx, criteria.clone());
                room_screen.view(id!(search_timeline)).set_visible(cx, false);
                let room_id = selected_room.room_id();
                let rooms_list_ref = cx.get_global::<RoomsListRef>();
                let is_encrypted = rooms_list_ref.is_room_encrypted(room_id);
                if is_encrypted && !criteria.include_all_rooms {
                    enqueue_popup_notification(String::from("Searching for encrypted messages is not supported yet. You may want to try searching all rooms instead."));
                    return;
                }
                room_screen.search_result(id!(search_result_plane)).display_top_space(cx);
                submit_async_request(MatrixRequest::SearchMessages {
                    room_id: Some(room_id.to_owned()),
                    include_all_rooms: criteria.include_all_rooms,
                    search_term: criteria.search_term.clone(),
                    next_batch: None,
                    abort_previous_search: true
                });
            }
        }
        MessageSearchAction::Click(search_term) => {
            if let Some(_selected_room) = {
                let app_state = scope.data.get::<AppState>().unwrap();
                app_state.selected_room.clone()
            } {
                let mut criteria = room_screen
                    .search_result(id!(search_result_plane))
                    .get_search_criteria();
                // if search_term == criteria.search_term && !search_term.is_empty() {
                //     return;
                // }
                println!("criteria.search_term: {:#?}, search_term: {:#?}", criteria.search_term, search_term);
                criteria.search_term = search_term.clone();
                room_screen
                    .search_result(id!(search_result_plane))
                    .set_search_criteria(cx, criteria);

            }
        }
        MessageSearchAction::Clear => {
            room_screen
                .view(id!(search_timeline))
                .set_visible(cx, false);
            room_screen
                .search_result(id!(search_result_plane))
                .reset(cx);
            room_screen
                .search_result(id!(search_result_plane))
                .set_visible(cx, false);
            room_screen.search_state = SearchState::default();
        }
        _ => {}
    }
}

pub fn send_pagination_request_based_on_scroll_pos_for_search_result(
    room_screen: &mut SearchScreen,
    cx: &mut Cx,
    room_id: &RoomId,
    actions: &ActionsBuf,
    portal_list: &PortalListRef,
    search_result_plane: &SearchResultRef
) {
    let search_state = &mut room_screen.search_state;
    if search_state.fully_paginated { return };

    if !portal_list.scrolled(actions) { return };

    let first_index = portal_list.first_id();
    if first_index == 0 && search_state.last_scrolled_index > 0 {
        if let Some(next_batch_token) = &search_state.next_batch_token.take() {
            log!("Scrolled up from item {} --> 0, sending search request for room {} with backward_pagination_batch {:?}",
                search_state.last_scrolled_index, room_id, next_batch_token
            );
            search_result_plane.display_top_space(cx);
            let criteria = search_result_plane.get_search_criteria();
            submit_async_request(MatrixRequest::SearchMessages {
                room_id: Some(room_id.into()),
                include_all_rooms: criteria.include_all_rooms,
                search_term: criteria.search_term.clone(),
                next_batch: Some(next_batch_token.clone()),
                abort_previous_search: false
            });
        }
    }
    room_screen.search_state.last_scrolled_index = first_index;
}

/// Search result as timeline item
#[derive(Clone, Debug)]
pub enum SearchResultItem {
    /// The event that matches the search criteria.
    Event(Box<AnyTimelineEvent>),
    /// The room id used for displaying room header for all searched messages in a screen.
    RoomHeader(OwnedRoomId),
}

pub fn populate_message_search_view(
    cx: &mut Cx2d,
    list: &mut PortalList,
    item_id: usize,
    event_tl_item: &AnyTimelineEvent,
    message: &RoomMessageEventContent,
    user_profiles: &BTreeMap<OwnedUserId, TimelineDetails<Profile>>,
    media_cache: &mut MediaCache,
    item_drawn_status: ItemDrawnStatus,
) -> (WidgetRef, ItemDrawnStatus) {
    let mut new_drawn_status = item_drawn_status;
    let ts_millis = event_tl_item.origin_server_ts();

    // Sometimes we need to call this up-front, so we save the result in this variable
    // to avoid having to call it twice.
    let (item, used_cached_item) = match &message.msgtype {
        MessageType::Text(TextMessageEventContent { body, formatted, .. }) => {
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
                                    code_color: (SEARCH_HIGHLIGHT)
                                }
                            }
                        }
                    ),
                );
                populate_text_message_content(cx, &html_or_plaintext_ref, body, formatted.as_ref());
                new_drawn_status.content_drawn = true;
                (item, false)
            }
        }
        _mtype @ MessageType::Image(image) => {
            let template = live_id!(ImageMessage);
            let (item, existed) = list.item_with_existed(cx, item_id, template);

            if existed && item_drawn_status.content_drawn {
                (item, true)
            } else {
                let image_info = Some((image.info.clone().map(|info| *info),
                image.source.clone()));
                let is_image_fully_drawn = populate_image_message_content(
                    cx,
                    &item.text_or_image(id!(content.message)),
                    image_info,
                    message.body(),
                    media_cache,
                );
                new_drawn_status.content_drawn = is_image_fully_drawn;
                (item, false)
            }
        }
        MessageType::File(file_content) => {
            let template = live_id!(MessageCard);
            let (item, existed) = list.item_with_existed(cx, item_id, template);
            if existed && item_drawn_status.content_drawn {
                (item, true)
            } else {
                new_drawn_status.content_drawn = populate_file_message_content(
                    cx,
                    &item.html_or_plaintext(id!(content.message)),
                    file_content,
                );
                (item, false)
            }
        }
        MessageType::Audio(audio) => {
            let template = live_id!(MessageCard);
            let (item, existed) = list.item_with_existed(cx, item_id, template);
            if existed && item_drawn_status.content_drawn {
                (item, true)
            } else {
                new_drawn_status.content_drawn = populate_audio_message_content(
                    cx,
                    &item.html_or_plaintext(id!(content.message)),
                    audio,
                );
                (item, false)
            }
        }
        other => {
            let (item, existed) = list.item_with_existed(cx, item_id, live_id!(MessageCard));
            if existed && item_drawn_status.content_drawn {
                (item, true)
            } else {
                let kind = other.msgtype();
                item.label(id!(content.message)).set_text(
                    cx,
                    &format!("[Unsupported ({kind})] {}", message.body()),
                );
                new_drawn_status.content_drawn = true;
                (item, false)
            }
        }
    };

    // If `used_cached_item` is false, we should always redraw the profile, even if profile_drawn is true.
    let skip_draw_profile =
        used_cached_item && item_drawn_status.profile_drawn;
    if skip_draw_profile {
        // log!("\t --> populate_message_view(): SKIPPING profile draw for item_id: {item_id}");
        new_drawn_status.profile_drawn = true;
    } else {
        // log!("\t --> populate_message_view(): DRAWING  profile draw for item_id: {item_id}");
        let username_label = item.label(id!(content.username));
        let (username, profile_drawn) = item.avatar(id!(profile.avatar)).set_avatar_and_get_username(
                cx,
                event_tl_item.room_id(),
                event_tl_item.sender(),
                user_profiles.get(event_tl_item.sender()),
                Some(event_tl_item.event_id()),
            );
            username_label.set_text(cx, &username);
            new_drawn_status.profile_drawn = profile_drawn;
    }

    // If we've previously drawn the item content, skip all other steps.
    if used_cached_item && item_drawn_status.content_drawn && item_drawn_status.profile_drawn {
        return (item, new_drawn_status);
    }

    // Set the timestamp.
    if let Some(dt) = unix_time_millis_to_datetime(&ts_millis) {
        item.timestamp(id!(profile.timestamp)).set_date_time(cx, dt);
    } else {
        item.label(id!(profile.timestamp))
            .set_text(cx, &format!("{}", ts_millis.get()));
    }
    (item, new_drawn_status)
}
fn truncate_to_50(s: &str) -> String {
    let n = 10;
    if s.chars().count() > n {
        let mut string: String = s.chars().take(n).collect();
        string.push_str("..");
        string
    } else {
        s.to_string()
    }
}
