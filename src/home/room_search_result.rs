use std::{ops::{DerefMut, Range}, sync::Arc};

use imbl::Vector;
use indexmap::IndexMap;
use makepad_widgets::*;
use matrix_sdk_ui::timeline::{AnyOtherFullStateEventContent, InReplyToDetails, ReactionsByKeyBySender, TimelineDetails, TimelineEventItemId, VirtualTimelineItem};
use ruma::{events::{receipt::Receipt, room::message::{FormattedBody, MessageType, RoomMessageEventContent}, AnyMessageLikeEventContent, AnyStateEventContent, AnyTimelineEvent, FullStateEventContent}, uint, EventId, MilliSecondsSinceUnixEpoch, OwnedEventId, OwnedUserId, UserId};

use crate::{event_preview::text_preview_of_other_state, home::room_screen::RoomScreenTooltipActions, sliding_sync::{submit_async_request, MatrixRequest}, utils::unix_time_millis_to_datetime};

use super::{loading_pane::{LoadingPaneState, LoadingPaneWidgetExt}, room_screen::{populate_message_view, populate_small_state_event, Eventable, ItemDrawnStatus, MessageOrSticker, MsgTypeAble, PreviousEventable, RoomScreen, SmallStateEventContent, TimelineUiState}, room_screen_traits::Stateable};

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;
    use crate::shared::styles::*;
    use crate::shared::helpers::*;
    use crate::shared::icon_button::*;
    use crate::home::rooms_list::RoomsList;
    use crate::home::room_screen::*;
    COLOR_BUTTON_GREY = #B6BABF
    ICON_SEARCH = dep("crate://self/resources/icons/search.svg")
    SearchIcon = <Icon> {
        align: {x: 0.0} // Align to top-right
        spacing: 10,
        margin: {top: 0, left: 10},
        padding: {top: 10, bottom: 10, left: 8, right: 15}
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
        // draw_bg: {
        //     color: (COLOR_SECONDARY)
        // }
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
            height: 60,
            show_bg: true,
            align: {y: 0.5}
            draw_bg: {
                color: (COLOR_SECONDARY)
            }
            <SearchIcon> {}
            summary_label = <Html> {
                margin: {left: 10},
                align: {x: 0.3}  // Align to top-right
                width: Fill,
                height: Fit,
                padding: 0,
                font_color: (MESSAGE_TEXT_COLOR),
                font_size: (MESSAGE_FONT_SIZE),
                body: ""
            }
            search_all_rooms_button = <Button> {
                align: {x: 0.8},
                margin: {right:10, top: -2}
                draw_text:{color:#000}
                text: "Search All Rooms"
            }
            cancel_button = <RobrixIconButton> {
                align: {x: 1.0}
                margin: {right: 10, top:0},
                width: Fit,
                height: Fit,
                padding: {left: 15, right: 15}
                draw_bg: {
                    border_color: (COLOR_DANGER_RED),
                    color: #fff0f0 // light red
                }
                draw_icon: {
                    svg_file: (ICON_CLOSE),
                    color: (COLOR_DANGER_RED)
                }
                icon_walk: {width: 16, height: 16, margin: 0}
            }
        }
        
    }
}

// The main widget that displays a single Matrix room.
#[derive(Live, LiveHook, Widget)]
pub struct SearchResult {
    #[deref] pub view: View,
    #[rust] pub search_criteria: String,
    #[rust] pub result_count: u32,
    #[live] pub timeline_template: Option<LivePtr>,
    #[rust] pub timeline_view: Option<ViewRef>,
}

impl Widget for SearchResult {
    // Handle events and actions for the SearchResult widget and its inner Timeline view.
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.match_event(cx, event);
        self.view.handle_event(cx, event, scope);
    }
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
        // if let Some(timeline_view) = &self.timeline_view {
        //     while let Some(subview) = timeline_view.draw_walk(cx, scope, walk).step() {
        //         let portal_list_ref = subview.as_portal_list();
        //         let Some(mut list_ref) = portal_list_ref.borrow_mut() else {
        //             error!("!!! RoomScreen::draw_walk(): BUG: expected a PortalList widget, but got something else");
        //             continue;
        //         };
        //         let list = list_ref.deref_mut();
        //         list.set_item_range(cx, 0, 10);
        //         while let Some(item_id) = list.next_visible_item(cx) {
        //             let item = list.item(cx, item_id, live_id!(RoomHeader));
        //             item.as_label().set_text(cx, "ss");
        //             item.draw_all(cx, &mut Scope::empty());
        //         }
        //     }
        // }

        // DrawStep::done()
    }
}
impl MatchEvent for SearchResult {
    fn handle_actions(&mut self, cx: &mut Cx, actions:&Actions) {
        let cancel_button_clicked = self.view.button(id!(cancel_button)).clicked(actions);
        if cancel_button_clicked {
            cx.action(SearchResultAction::Close);
        }
        for action in actions {
            match action.downcast_ref() {
                Some(SearchResultAction::Success{
                    count,
                }) => {
                    self.set_result_count(cx, *count);
                }
                Some(SearchResultAction::Pending(search_criteria)) => {
                    self.set_search_criteria(cx, search_criteria.clone());
                }
                _ => {}
            }
        }
    }
}
impl SearchResult {
    /// Display search summary.
    ///
    /// This is used to display the number of search results and the search criteria
    /// in the top-right of the room screen.
    fn set_result_count(&mut self, cx: &mut Cx, search_result_count: u32) {
        self.result_count = self.result_count.saturating_add(search_result_count);
        self.view.html(id!(summary_label)).set_text(cx, &format!("{} results for <b>'{}'</b>", self.result_count, self.search_criteria));
        self.view.view(id!(loading_view)).set_visible(cx, false);
    }

    /// Set Search criteria.
    ///
    /// This is used to display the number of search results and the search criteria
    /// in the top-right of the room screen.
    fn set_search_criteria(&mut self, cx: &mut Cx, search_criteria: String) {
        self.view.html(id!(summary_label)).set_text(cx, &format!("Searching for <b>'{}'</b>", search_criteria));
        self.search_criteria = search_criteria;
        self.view.search_result(id!(search_result_overlay)).set_visible(cx, true);
        self.timeline_view = Some(WidgetRef::new_from_ptr(cx, self.timeline_template).as_view());
    }
    /// Resets the search result summary and displays the loading view.
    ///
    /// This function clears the summary text and makes the loading indicator visible.
    /// It is typically used when a new search is initiated or search results are being cleared.
    fn reset_summary(&mut self, cx: &mut Cx) {
        self.view.html(id!(summary_label)).set_text(cx, "");
        self.view.view(id!(loading_view)).set_visible(cx, true);
    }
}
impl SearchResultRef {
    /// See [`SearchResult::set_result_count()`].
    pub fn set_result_count(&mut self, cx: &mut Cx, search_result_count: u32) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.set_result_count(cx, search_result_count);
    }
    /// See [`SearchResult::set_search_criteria()`].
    pub fn set_search_criteria(&self, cx: &mut Cx, search_criteria: String) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.set_search_criteria(cx, search_criteria);
    }

    /// See [`SearchResult::reset_summary()`].
    pub fn reset_summary(&self, cx: &mut Cx) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.reset_summary(cx);
    }
}
pub fn send_pagination_request_based_on_scroll_pos_for_search_result(
    room_screen: &mut RoomScreen,
    _cx: &mut Cx,
    actions: &ActionsBuf,
    portal_list: &PortalListRef,
) {
    let Some(tl) = room_screen.tl_state.as_mut() else { return };
    let search_result_state = &mut tl.search_result_state;
    if search_result_state.fully_paginated { return };
    if !portal_list.scrolled(actions) { return };

    let first_index = portal_list.first_id();
    if first_index == 0 && search_result_state.last_scrolled_index > 0 {
        log!("Scrolled up from item {} --> 0, sending back pagination request for room {}",
            search_result_state.last_scrolled_index, tl.room_id,
        );
        if let Some(next_batch) = &search_result_state.next_batch {
            if !search_result_state.batch_tokens.contains(&next_batch) {
                submit_async_request(MatrixRequest::SearchMessages {
                    room_id: tl.room_id.clone(),
                    include_all_rooms: search_result_state.include_all_rooms,
                    search_term: search_result_state.search_term.clone(),
                    next_batch: search_result_state.next_batch.clone(),
                });
            }
        }
    }
    tl.search_result_state.last_scrolled_index = first_index;
}
pub fn search_result_draw_walk(room_screen: &mut RoomScreen, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
    let room_screen_widget_uid = room_screen.widget_uid();
    while let Some(subview) = room_screen.view.draw_walk(cx, scope, walk).step() {
        if subview.view(id!(search_timeline)).visible() {
            let portal_list_ref = subview.as_portal_list();
            let Some(mut list_ref) = portal_list_ref.borrow_mut() else {
                error!("!!! RoomScreen::draw_walk(): BUG: expected a PortalList widget, but got something else");
                continue;
            };
            let Some(tl_state) = room_screen.tl_state.as_mut() else {
                return DrawStep::done();
            };
            //room_screen.no_more_template
            let room_id = &tl_state.room_id;
            let tl_items = &tl_state.search_result_state.items;
            let list = list_ref.deref_mut();
            list.set_item_range(cx, 0, 10);
            while let Some(item_id) = list.next_visible_item(cx) {
                let item = {
                    let tl_idx = item_id;
                    if item_id == 0 {
                        let _ = WidgetRef::new_from_ptr(cx, room_screen.no_more_template).as_label().draw_all(cx, &mut Scope::empty());
                    }
                    let Some(timeline_item) = tl_items.get(tl_idx) else {
                        // This shouldn't happen (unless the timeline gets corrupted or some other weird error),
                        // but we can always safely fill the item with an empty widget that takes up no space.
                        list.item(cx, item_id, live_id!(Empty));
                        continue;
                    };
                    let item_drawn_status = ItemDrawnStatus {
                        content_drawn: tl_state.content_drawn_since_last_update.contains(&tl_idx),
                        profile_drawn: tl_state.profile_drawn_since_last_update.contains(&tl_idx),
                    };
                    let (item, item_new_draw_status) = {
                        let current_item = timeline_item;
                        let prev_event = tl_idx.checked_sub(1).and_then(|i| tl_items.get(i))
                            .and_then(|f| match f.kind { 
                                SearchTimelineItemKind::ContextEvent(ref e) | SearchTimelineItemKind::Event(ref e) => Some(e),
                                _ => None });
                        
                        match &current_item.kind {
                            SearchTimelineItemKind::Virtual(virtual_item) => {
                                match virtual_item {
                                    VirtualTimelineItem::DateDivider(millis) => {
                                        let item = list.item(cx, item_id, live_id!(DateDivider));
                                        let text = unix_time_millis_to_datetime(millis)
                                            // format the time as a shortened date (Sat, Sept 5, 2021)
                                            .map(|dt| format!("{}", dt.date_naive().format("%a %b %-d, %Y")))
                                            .unwrap_or_else(|| format!("{:?}", millis));
                                        item.label(id!(date)).set_text(cx, &text);
                                        (item, ItemDrawnStatus::both_drawn())
                                    }
                                    VirtualTimelineItem::ReadMarker => {
                                        continue
                                    }
                                }
                                
                            }
                            SearchTimelineItemKind::ContextEvent(event) | SearchTimelineItemKind::Event(event) => match event {
                                AnyTimelineEvent::MessageLike(msg) => {
                                    match msg.original_content() {
                                        Some(AnyMessageLikeEventContent::RoomMessage(mut message)) => {
                                            let is_contextual = matches!(&current_item.kind, SearchTimelineItemKind::ContextEvent(_));
                                            if let MessageType::Text(text) = &mut message.msgtype {
                                                
                                                if let Some(ref mut formatted) = text.formatted {
                                                    for highlight in tl_state.search_result_state.highlighted_strings.iter() {
                                                        formatted.body = formatted.body.replace(highlight, &format!("<code>{}</code>", highlight));
                                                    }
                                                } else {
                                                    let mut formated_string = text.body.clone();
                                                    for highlight in tl_state.search_result_state.highlighted_strings.iter() {
                                                        formated_string = formated_string.replace(highlight, &format!("<code>{}</code>", highlight));
                                                    }
                                                    text.formatted = Some(FormattedBody::html(formated_string));
                                                }
                                            }
                                            let event = &EventableWrapperAEI(&event);
                                            let prev_event = prev_event.map(|f| PreviousWrapperAEI(f));
                                            let message = MsgTypeWrapperRMC(&message);
                                            populate_message_view(
                                                cx,
                                                list,
                                                item_id,
                                                room_id,
                                                event,
                                                MessageOrSticker::Message(&message),
                                                prev_event.as_ref(),
                                                &mut tl_state.media_cache,
                                                &tl_state.user_power,
                                                is_contextual,
                                                item_drawn_status,
                                                room_screen_widget_uid,
                                            )
                                        }
                                        _ => continue
                                    }
                                },
                                AnyTimelineEvent::State(state) => {
                                    let state_key = state.state_key();
                                    if let Some(content) = state.original_content() {
                                        let wrapper = AnyStateEventContentWrapper(&content, state_key);
                                        let event = &EventableWrapperAEI(event);
                                        populate_small_state_event(
                                            cx,
                                            list,
                                            item_id,
                                            room_id,
                                            event,
                                            &wrapper,
                                            item_drawn_status,
                                        )
                                    } else {
                                        continue
                                    }
                                }
                            }
                            SearchTimelineItemKind::RoomHeader(room_name) => {
                                let item = list.item(cx, item_id, live_id!(RoomHeader));
                                item.set_text(cx, &format!("Room {}", room_name));
                                (item, ItemDrawnStatus::both_drawn())
                            }
                            SearchTimelineItemKind::NoMoreMessages => {
                                let item = list.item(cx, item_id, live_id!(NoMoreMessages));
                                item.set_text(cx, "No More");
                                (item, ItemDrawnStatus::both_drawn())
                            }
                        }
                    };
                    if item_new_draw_status.content_drawn {
                        tl_state.content_drawn_since_last_update.insert(tl_idx .. tl_idx + 1);
                    }
                    if item_new_draw_status.profile_drawn {
                        tl_state.profile_drawn_since_last_update.insert(tl_idx .. tl_idx + 1);
                    }
                    item
                };
                item.draw_all(cx, &mut Scope::empty());
            }
            return DrawStep::done();
        }
    }

    DrawStep::done()
}

#[derive(Clone)]
pub struct SearchTimelineItem{
    pub kind: SearchTimelineItemKind
}
impl SearchTimelineItem{
    pub fn with_context_event(event: AnyTimelineEvent) -> Self {
        SearchTimelineItem {
            kind: SearchTimelineItemKind::ContextEvent(event)
        }
    }
    pub fn with_event(event: AnyTimelineEvent) -> Self {
        SearchTimelineItem {
            kind: SearchTimelineItemKind::Event(event)
        }
    }
    pub fn with_virtual(virtual_item: VirtualTimelineItem) -> Self {
        SearchTimelineItem {
            kind: SearchTimelineItemKind::Virtual(virtual_item)
        }
    }
    pub fn with_room_header(room_name: String) -> Self {
        SearchTimelineItem {
            kind: SearchTimelineItemKind::RoomHeader(room_name)
        }
    }
    pub fn with_no_more_messages() -> Self {
        SearchTimelineItem {
            kind: SearchTimelineItemKind::NoMoreMessages
        }
    }
}
#[derive(Clone)]
pub enum SearchTimelineItemKind {
    /// The event that matches the search criteria 
    Event(AnyTimelineEvent),
    /// The events before or after the event that matches the search criteria
    ContextEvent(AnyTimelineEvent),
    /// An item that doesn't correspond to an event, for example the user's
    /// own read marker, or a date divider.
    Virtual(VirtualTimelineItem),
    /// The room header displaying room name for all found messages in a room.
    RoomHeader(String),
    /// The text to be displayed at the top of the search result to indicate end of results.
    NoMoreMessages
}

/// Actions related to a specific message within a room timeline.
#[derive(Clone, DefaultNone, Debug)]
pub enum SearchResultAction {
    /// Search result's success.
    Success{
        count: u32,
    },
    /// Pending search result and its search criteria.
    Pending(String),
    Close,
    None
}


pub struct AnyStateEventContentWrapper<'a>(pub &'a AnyStateEventContent, pub &'a str);

impl <'a>Into<Option<AnyOtherFullStateEventContent>> for &AnyStateEventContentWrapper<'a> {
    fn into(self) -> Option<AnyOtherFullStateEventContent> {
        match self.0.clone() {
            AnyStateEventContent::RoomAliases(p) => Some(AnyOtherFullStateEventContent::RoomAliases(FullStateEventContent::Original { content: p, prev_content: None})),
            AnyStateEventContent::RoomAvatar(p) => Some(AnyOtherFullStateEventContent::RoomAvatar(FullStateEventContent::Original { content: p, prev_content: None})),
            AnyStateEventContent::RoomCanonicalAlias(p) => Some(AnyOtherFullStateEventContent::RoomCanonicalAlias(FullStateEventContent::Original { content: p, prev_content: None})),
            AnyStateEventContent::RoomCreate(p) => Some(AnyOtherFullStateEventContent::RoomCreate(FullStateEventContent::Original { content: p, prev_content: None})),
            AnyStateEventContent::RoomEncryption(p) => Some(AnyOtherFullStateEventContent::RoomEncryption(FullStateEventContent::Original { content: p, prev_content: None})),
            AnyStateEventContent::RoomGuestAccess(p) => Some(AnyOtherFullStateEventContent::RoomGuestAccess(FullStateEventContent::Original { content: p, prev_content: None})),
            AnyStateEventContent::RoomHistoryVisibility(p) => Some(AnyOtherFullStateEventContent::RoomHistoryVisibility(FullStateEventContent::Original { content: p, prev_content: None})),
            AnyStateEventContent::RoomJoinRules(p) => Some(AnyOtherFullStateEventContent::RoomJoinRules(FullStateEventContent::Original { content: p, prev_content: None})),
            AnyStateEventContent::RoomPinnedEvents(p) => Some(AnyOtherFullStateEventContent::RoomPinnedEvents(FullStateEventContent::Original { content: p, prev_content: None})),
            AnyStateEventContent::RoomName(p) => Some(AnyOtherFullStateEventContent::RoomName(FullStateEventContent::Original { content: p, prev_content: None})),
            AnyStateEventContent::RoomPowerLevels(p) => Some(AnyOtherFullStateEventContent::RoomPowerLevels(FullStateEventContent::Original { content: p, prev_content: None})),
            AnyStateEventContent::RoomServerAcl(p) => Some(AnyOtherFullStateEventContent::RoomServerAcl(FullStateEventContent::Original { content: p, prev_content: None})),
            AnyStateEventContent::RoomTombstone(p) => Some(AnyOtherFullStateEventContent::RoomTombstone(FullStateEventContent::Original { content: p, prev_content: None})),
            AnyStateEventContent::RoomTopic(p) => Some(AnyOtherFullStateEventContent::RoomTopic(FullStateEventContent::Original { content: p, prev_content: None})),
            AnyStateEventContent::SpaceParent(p) => Some(AnyOtherFullStateEventContent::SpaceParent(FullStateEventContent::Original { content: p, prev_content: None})),
            AnyStateEventContent::SpaceChild(p) => Some(AnyOtherFullStateEventContent::SpaceChild(FullStateEventContent::Original { content: p, prev_content: None})),
            AnyStateEventContent::PolicyRuleRoom(p) => Some(AnyOtherFullStateEventContent::PolicyRuleRoom(FullStateEventContent::Original { content: p, prev_content: None})),
            AnyStateEventContent::PolicyRuleServer(p) => Some(AnyOtherFullStateEventContent::PolicyRuleServer(FullStateEventContent::Original { content: p, prev_content: None})),
            AnyStateEventContent::PolicyRuleUser(p) => Some(AnyOtherFullStateEventContent::PolicyRuleUser(FullStateEventContent::Original { content: p, prev_content: None})),
            AnyStateEventContent::RoomThirdPartyInvite(p) => Some(AnyOtherFullStateEventContent::RoomThirdPartyInvite(FullStateEventContent::Original { content: p, prev_content: None})),
            AnyStateEventContent::BeaconInfo(_) => None,
            AnyStateEventContent::CallMember(_) => None,
            AnyStateEventContent::MemberHints(_) => None,
            AnyStateEventContent::RoomMember(_) => None,
            _ => None,
        }
    }
}

pub struct EventableWrapperAEI<'a>(pub &'a AnyTimelineEvent);

impl <'a> Eventable for EventableWrapperAEI<'a> {
    fn timestamp(&self) -> MilliSecondsSinceUnixEpoch {
        self.0.origin_server_ts()
    }
    fn event_id(&self) -> Option<&EventId> {
        Some(self.0.event_id())
    }
    fn sender(&self) -> &UserId {
        self.0.sender()
    }
    fn sender_profile(&self) -> Option<&TimelineDetails<matrix_sdk_ui::timeline::Profile>> {
        None
    }
    fn reactions(&self) -> Option<ReactionsByKeyBySender> {
        None
    }
    fn identifier(&self) -> TimelineEventItemId {
        TimelineEventItemId::EventId(self.0.event_id().to_owned())
    }
    fn is_highlighted(&self) -> bool {
        false
    }
    fn is_editable(&self) -> bool {
        false
    }
    fn is_own(&self) -> bool {
        false
    }
    fn can_be_replied_to(&self) -> bool {
        false
    }
    fn read_receipts(&self) -> Option<&IndexMap<OwnedUserId, Receipt>> {
        None
    }
    fn latest_json(&self) -> Option<&ruma::serde::Raw<ruma::events::AnySyncTimelineEvent>> {
        None
    }
    fn room_id(&self) -> Option<&ruma::RoomId> {
        Some(self.0.room_id())
    }
}


impl  <'a> SmallStateEventContent<EventableWrapperAEI<'_>> for AnyStateEventContentWrapper<'a> {
    fn populate_item_content(
        &self,
        cx: &mut Cx,
        list: &mut PortalList,
        item_id: usize,
        item: WidgetRef,
        _event_tl_item: &EventableWrapperAEI,
        username: &str,
        _item_drawn_status: ItemDrawnStatus,
        mut new_drawn_status: ItemDrawnStatus,
    ) -> (WidgetRef, ItemDrawnStatus) {
        let Some(other_state) = self.into() else { return (list.item(cx, item_id, live_id!(Empty)), ItemDrawnStatus::new()) };
        let item = if let Some(text_preview) = text_preview_of_other_state(&other_state, &self.1) {
            item.label(id!(content))
                .set_text(cx, &text_preview.format_with(username));
            new_drawn_status.content_drawn = true;
            item
        } else {
            let item = list.item(cx, item_id, live_id!(Empty));
            new_drawn_status = ItemDrawnStatus::new();
            item
        };
        (item, new_drawn_status)
    }
}

pub struct PreviousWrapperAEI<'a>(pub &'a AnyTimelineEvent);
impl <'a> PreviousEventable<EventableWrapperAEI<'a>> for PreviousWrapperAEI<'a> {
    fn use_compact(&self, current: &EventableWrapperAEI<'a>) -> bool {
        let prev_msg_sender = self.0.sender();
        let current_sender = current.0.sender();
        let compact_view = {
            prev_msg_sender == current_sender && current.0.origin_server_ts().0.checked_sub(self.0.origin_server_ts().0)
                // Use compact_view within a day
                .is_some_and(|d| d < uint!(86400000))
        };
        compact_view
    }
}

pub struct MsgTypeWrapperRMC<'a>(pub &'a RoomMessageEventContent);
impl <'a>MsgTypeAble for MsgTypeWrapperRMC<'a> {
    fn msgtype(&self) -> &MessageType {
        &self.0.msgtype
    }
    fn body(&self) -> &str {
        self.0.body()
    }
    fn in_reply_to(&self) -> Option<&InReplyToDetails> {
        None
    }
    fn is_searched_result(&self) -> bool {
        true
    }
}

//pub fn handle_search_first_update<'a, S: Stateable<EventableWrapperAEI<'a>>>(tl: &mut S, portal_list: &PortalListRef,  cx: &mut Cx, initial_items: Vector<EventableWrapperAEI<'a>>, next_batch: Option<String>, done_loading: &mut bool) {
pub fn handle_search_first_update<'a, S: Stateable<T>, T:Eventable>(tl: &mut S, portal_list: &PortalListRef,  cx: &mut Cx, initial_items: Vector<Arc<T>>, next_batch: Option<String>, done_loading: &mut bool) {
    tl.get_content_drawn_since_last_update().clear();
    tl.get_profile_drawn_since_last_update().clear();
    *tl.get_fully_paginated() = false;
    // Set the portal list to the very bottom of the timeline.
    portal_list.set_first_id_and_scroll(initial_items.len().saturating_sub(1), 0.0);
    portal_list.set_tail_range(true);
 //   jump_to_bottom.update_visibility(cx, true);

    *tl.get_items() = initial_items;
    *done_loading = true;
}

pub fn handle_search_new_items<'a, S: Stateable<T>, T:Eventable>(
    view: View,
    tl: &mut S, 
    portal_list: &PortalListRef, 
    cx: &mut Cx, 
    ui: WidgetUid,
    new_items: Vector<Arc<T>>, 
    next_batch: Option<String>,
    changed_indices: Range<usize>,
    is_append: bool,
    clear_cache: bool,
    done_loading: &mut bool,
    should_continue_backwards_pagination: &mut bool
) {
    let curr_first_id = portal_list.first_id();
    if new_items.is_empty() {
        if !tl.get_items().is_empty() {
            log!("Timeline::handle_event(): timeline (had {} items) was cleared for room {}", tl.get_items().len(), tl.room_id());
            // For now, we paginate a cleared timeline in order to be able to show something at least.
            // A proper solution would be what's described below, which would be to save a few event IDs
            // and then either focus on them (if we're not close to the end of the timeline)
            // or paginate backwards until we find them (only if we are close the end of the timeline).
            *should_continue_backwards_pagination = true;
        }

        // If the bottom of the timeline (the last event) is visible, then we should
        // set the timeline to live mode.
        // If the bottom of the timeline is *not* visible, then we should
        // set the timeline to Focused mode.

        // TODO: Save the event IDs of the top 3 items before we apply this update,
        //       which indicates this timeline is in the process of being restored,
        //       such that we can jump back to that position later after applying this update.

        // TODO: here we need to re-build the timeline via TimelineBuilder
        //       and set the TimelineFocus to one of the above-saved event IDs.

        // TODO: the docs for `TimelineBuilder::with_focus()` claim that the timeline's focus mode
        //       can be changed after creation, but I do not see any methods to actually do that.
        //       <https://matrix-org.github.io/matrix-rust-sdk/matrix_sdk_ui/timeline/struct.TimelineBuilder.html#method.with_focus>
        //
        //       As such, we probably need to create a new async request enum variant
        //       that tells the background async task to build a new timeline
        //       (either in live mode or focused mode around one or more events)
        //       and then replaces the existing timeline in ALL_ROOMS_INFO with the new one.
    }

    // Maybe todo?: we can often avoid the following loops that iterate over the `items` list
    //       by only doing that if `clear_cache` is true, or if `changed_indices` range includes
    //       any index that comes before (is less than) the above `curr_first_id`.

    if new_items.len() == tl.get_items().len() {
        // log!("Timeline::handle_event(): no jump necessary for updated timeline of same length: {}", items.len());
    }
    else if curr_first_id > new_items.len() {
        log!("Timeline::handle_event(): jumping to bottom: curr_first_id {} is out of bounds for {} new items", curr_first_id, new_items.len());
        portal_list.set_first_id_and_scroll(new_items.len().saturating_sub(1), 0.0);
        portal_list.set_tail_range(true);
        //jump_to_bottom.update_visibility(cx, true);
    }
    else if let Some((curr_item_idx, new_item_idx, new_item_scroll, _event_id)) =
        find_new_item_matching_current_item(cx, portal_list, curr_first_id, &tl.get_items(), &new_items)
    {
        if curr_item_idx != new_item_idx {
            log!("Timeline::handle_event(): jumping view from event index {curr_item_idx} to new index {new_item_idx}, scroll {new_item_scroll}, event ID {_event_id}");
            portal_list.set_first_id_and_scroll(new_item_idx, new_item_scroll);
            *tl.get_prev_first_index() = Some(new_item_idx);
            // Set scrolled_past_read_marker false when we jump to a new event
            *tl.get_scrolled_past_read_marker() = false;
            // When the tooltip is up, the timeline may jump. This may take away the hover out event to required to clear the tooltip
            cx.widget_action(ui, &Scope::empty().path, RoomScreenTooltipActions::HoverOut);

        }
    }
    //
    // TODO: after an (un)ignore user event, all timelines are cleared. Handle that here.
    //
    else {
        warning!("!!! Couldn't find new event with matching ID for ANY event currently visible in the portal list");
    }

    // If new items were appended to the end of the timeline, show an unread messages badge on the jump to bottom button.
    if is_append && !portal_list.is_at_end() {
        // if let Some(room_id) = &self.room_id {
        //     // Immediately show the unread badge with no count while we fetch the actual count in the background.
        //     //jump_to_bottom.show_unread_message_badge(cx, UnreadMessageCount::Unknown);
        //     submit_async_request(MatrixRequest::GetNumberUnreadMessages{ room_id: room_id.clone() });
        // }
        
        submit_async_request(MatrixRequest::GetNumberUnreadMessages{ room_id: tl.room_id().to_owned() });
    }

    if clear_cache {
        tl.get_content_drawn_since_last_update().clear();
        tl.get_profile_drawn_since_last_update().clear();
        *tl.get_fully_paginated() = false;

        // If this RoomScreen is showing the loading pane and has an ongoing backwards pagination request,
        // then we should update the status message in that loading pane
        // and then continue paginating backwards until we find the target event.
        // Note that we do this here because `clear_cache` will always be true if backwards pagination occurred.
        let loading_pane = view.loading_pane(id!(loading_pane));
        let mut loading_pane_state = loading_pane.take_state();
        if let LoadingPaneState::BackwardsPaginateUntilEvent {
            ref mut events_paginated, target_event_id, ..
        } = &mut loading_pane_state {
            *events_paginated += new_items.len().saturating_sub(tl.get_items().len());
            log!("While finding target event {target_event_id}, loaded {events_paginated} messages...");
            // Here, we assume that we have not yet found the target event,
            // so we need to continue paginating backwards.
            // If the target event has already been found, it will be handled
            // in the `TargetEventFound` match arm below, which will set
            // `should_continue_backwards_pagination` to `false`.
            // So either way, it's okay to set this to `true` here.
            *should_continue_backwards_pagination = true;
        }
        loading_pane.set_state(cx, loading_pane_state);
    } else {
        tl.get_content_drawn_since_last_update().remove(changed_indices.clone());
        tl.get_profile_drawn_since_last_update().remove(changed_indices.clone());
        // log!("Timeline::handle_event(): changed_indices: {changed_indices:?}, items len: {}\ncontent drawn: {:#?}\nprofile drawn: {:#?}", items.len(), tl.content_drawn_since_last_update, tl.profile_drawn_since_last_update);
    }
    *tl.get_items() = new_items;
    *done_loading = true;
}

fn find_new_item_matching_current_item<T:Eventable>(
    cx: &mut Cx,
    portal_list: &PortalListRef,
    starting_at_curr_idx: usize,
    curr_items: &Vector<Arc<T>>,
    new_items: &Vector<Arc<T>>,
) -> Option<(usize, usize, f64, OwnedEventId)> {
    let mut curr_item_focus = curr_items.focus();
    let mut idx_curr = starting_at_curr_idx;
    let mut curr_items_with_ids: Vec<(usize, OwnedEventId)> = Vec::with_capacity(
        portal_list.visible_items()
    );

    // Find all items with real event IDs that are currently visible in the portal list.
    // TODO: if this is slow, we could limit it to 3-5 events at the most.
    if curr_items_with_ids.len() <= portal_list.visible_items() {
        while let Some(curr_item) = curr_item_focus.get(idx_curr) {
            if let Some(event_id) = curr_item.event_id() {
                curr_items_with_ids.push((idx_curr, event_id.to_owned()));
            }
            if curr_items_with_ids.len() >= portal_list.visible_items() {
                break;
            }
            idx_curr += 1;
        }
    }

    // Find a new item that has the same real event ID as any of the current items.
    for (idx_new, new_item) in new_items.iter().enumerate() {
        let Some(event_id) = new_item.event_id() else {
            continue;
        };
        if let Some((idx_curr, _)) = curr_items_with_ids
            .iter()
            .find(|(_, ev_id)| ev_id == event_id)
        {
            // Not all items in the portal list are guaranteed to have a position offset,
            // some may be zeroed-out, so we need to account for that possibility by only
            // using events that have a real non-zero area
            if let Some(pos_offset) = portal_list.position_of_item(cx, *idx_curr) {
                log!("Found matching event ID {event_id} at index {idx_new} in new items list, corresponding to current item index {idx_curr} at pos offset {pos_offset}");
                return Some((*idx_curr, idx_new, pos_offset, event_id.to_owned()));
            }
        }
    }

    None
}