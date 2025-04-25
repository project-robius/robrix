use std::sync::Arc;

use imbl::Vector;
use indexmap::IndexMap;
use makepad_widgets::*;
use matrix_sdk_ui::timeline::{AnyOtherFullStateEventContent, InReplyToDetails, ReactionsByKeyBySender, TimelineDetails, TimelineEventItemId, TimelineItem, VirtualTimelineItem};
use ruma::{api::client::search::search_events::v3::ResultCategories, events::{receipt::Receipt, room::message::{FormattedBody, MessageType, RoomMessageEventContent}, AnyMessageLikeEvent, AnyMessageLikeEventContent, AnyStateEventContent, AnyTimelineEvent, FullStateEventContent}, uint, EventId, MilliSecondsSinceUnixEpoch, OwnedRoomId, OwnedUserId, UserId};

use crate::{event_preview::text_preview_of_other_state, room, shared::search_bar::SearchBarAction, sliding_sync::{submit_async_request, MatrixRequest}, utils::unix_time_millis_to_datetime};

use super::{new_message_context_menu::ContextMenuFromEvent, room_screen::{self, populate_message_view, populate_small_state_event, ItemDrawnStatus, MessageOrSticker, MessageViewFromEvent, PreviousMessageViewFromEvent, RoomScreen, SearchResultState, SmallStateEventContent, TimelineUiState}, rooms_list::RoomsListRef};

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
            text: "Loading more search results..."
        }
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
            top_space = <TopSpace> {
                visible: true
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
    #[live(true)] visible: bool,
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
    fn handle_actions(&mut self, cx: &mut Cx, actions:&Actions) {
        let cancel_button_clicked = self.view.button(id!(cancel_button)).clicked(actions);
        if cancel_button_clicked {
            cx.widget_action(self.widget_uid(), &Scope::empty().path, SearchBarAction::ResetSearch);
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
        self.visible = true;
    }
    /// Resets the search result summary and set the loading view back to visible.
    ///
    /// This function clears the summary text and makes the loading indicator visible.
    /// It is typically used when a new search is initiated or search results are being cleared.
    fn reset(&mut self, cx: &mut Cx) {
        self.view.html(id!(summary_label)).set_text(cx, "");
        self.view.view(id!(loading_view)).set_visible(cx, true);
        self.search_criteria = String::from("");
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
    /// See [`SearchResult::set_visible()`].
    pub fn set_visible(&self, cx: &mut Cx, visible: bool) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.visible = visible;
            inner.redraw(cx);
        }
    }
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
    /// See [`SearchResult::reset()`].
    pub fn reset(&self, cx: &mut Cx) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.reset(cx);
    }
    /// See [`SearchResult::display_top_space()`].
    pub fn display_top_space(&self, cx: &mut Cx) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.display_top_space(cx);
    }
    /// See [`SearchResult::hide_top_space()`].
    pub fn hide_top_space(&self, cx: &mut Cx) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.hide_top_space(cx);
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
        if let Some(backward_pagination_batch) = &search_result_state.backward_pagination_batch {
            if !search_result_state.batch_list.contains(backward_pagination_batch) {
                log!("Scrolled up from item {} --> 0, sending back search_result request for room {} &search_result_state.backward_pagination_batch {:?}",
                    search_result_state.last_scrolled_index, tl.room_id, search_result_state.backward_pagination_batch
                );
                submit_async_request(MatrixRequest::SearchMessages {
                    room_id: tl.room_id.clone(),
                    include_all_rooms: search_result_state.include_all_rooms,
                    search_term: search_result_state.search_term.clone(),
                    backward_pagination_batch: search_result_state.backward_pagination_batch.clone(),
                });
            }
        }
    }
    tl.search_result_state.last_scrolled_index = first_index;
}

    /// Populates the item at `item_id` in `list` with data from `tl_state` and `search_result_draw_state`.
    /// 
    /// If `item_id` is 0 and `no_more_template` is present, it instead populates the item with the
    /// contents of `no_more_template`.
    /// 
    /// `media_cache` is used to cache avatars and other media.
    /// 
    /// `user_power` is used to determine whether or not to display some elements.
    /// 
    /// `room_list` is used to get the display name of the room.
    /// 
    /// `room_screen_widget_uid` is the widget uid of the room screen widget that this widget is inside.
    ///
// pub fn populate_search_result_item(
//     cx: &mut Cx2d, 
//     list: &mut PortalList, 
//     item_id: usize,
//     tl_state: &mut TimelineUiState, 
//     search_result_draw_state: &mut SearchResultDrawState,
//     room_list: &RoomsListRef, 
//     room_screen_widget_uid: WidgetUid,
//     no_more_template: Option<LivePtr>
// ) -> Option<WidgetRef> {
//     let media_cache = &mut tl_state.media_cache;
//     let search_result_state = &tl_state.search_result_state;
//     let user_power = &tl_state.user_power;
//     let tl_items = &search_result_state.items;
//     let room_id= &tl_state.room_id;
//     let item = {
//         let tl_idx = item_id;
//         if item_id == 0 {
//             WidgetRef::new_from_ptr(cx, no_more_template).as_label().draw_all(cx, &mut Scope::empty());
//         }
//         let Some(timeline_item) = tl_items.get(tl_idx) else {
//             // This shouldn't happen (unless the timeline gets corrupted or some other weird error),
//             // but we can always safely fill the item with an empty widget that takes up no space.
//             list.item(cx, item_id, live_id!(Empty));
//             return None
//         };
//         let item_drawn_status = ItemDrawnStatus {
//             content_drawn: tl_state.content_drawn_since_last_update.contains(&tl_idx),
//             profile_drawn: tl_state.profile_drawn_since_last_update.contains(&tl_idx),
//         };
//         let (item, item_new_draw_status) = {
//             let current_item = timeline_item;
//             let prev_event = tl_idx.checked_sub(1).and_then(|i| tl_items.get(i))
//                 .and_then(|f| match f.kind { 
//                     SearchTimelineItemKind::ContextEvent(ref e) | SearchTimelineItemKind::Event(ref e) => Some(e),
//                     _ => None });
            
//             match &current_item.kind {
//                 SearchTimelineItemKind::Virtual(virtual_item) => {
//                     match virtual_item {
//                         VirtualTimelineItem::DateDivider(millis) => {
//                             let item = list.item(cx, item_id, live_id!(DateDivider));
//                             let text = unix_time_millis_to_datetime(millis)
//                                 // format the time as a shortened date (Sat, Sept 5, 2021)
//                                 .map(|dt| format!("{}", dt.date_naive().format("%a %b %-d, %Y")))
//                                 .unwrap_or_else(|| format!("{:?}", millis));
//                             item.label(id!(date)).set_text(cx, &text);
//                             (item, ItemDrawnStatus::both_drawn())
//                         }
//                         VirtualTimelineItem::ReadMarker => {
//                             return None
//                         }
//                     }
                    
//                 }
//                 SearchTimelineItemKind::ContextEvent(event) | SearchTimelineItemKind::Event(event) => match event {
//                     AnyTimelineEvent::MessageLike(msg) => {
//                         // let date_text = unix_time_millis_to_datetime(&event.origin_server_ts())
//                         // // format the time as a shortened date (Sat, Sept 5, 2021)
//                         // .map(|dt| format!("{}", dt.date_naive().format("%a %b %-d, %Y")))
//                         // .unwrap_or_else(|| format!("{:?}", event.origin_server_ts()));
//                         // if !search_result_draw_state.last_date_divider_string.is_empty() {
//                         //     if search_result_draw_state.last_date_divider_string != date_text {
//                         //         let item = list.item(cx, item_id, live_id!(DateDivider));
//                         //         item.label(id!(date)).set_text(cx, &date_text);
//                         //         item.draw_all(cx, &mut Scope::empty());
//                         //         search_result_draw_state.last_date_divider_string = date_text;
//                         //     }
//                         // } else {
//                         //     let item = list.item(cx, item_id, live_id!(DateDivider));
//                         //     item.label(id!(date)).set_text(cx, &date_text);
//                         //     item.draw_all(cx, &mut Scope::empty());
//                         //     search_result_draw_state.last_date_divider_string = date_text;                            
//                         // }
//                         let room_id = event.room_id().to_owned();

//                         if let Some(ref mut last_room_id) = search_result_draw_state.last_room_id{
//                             if room_id != *last_room_id {
//                                 *last_room_id = room_id.clone().to_owned();
//                                 let item = list.item(cx, item_id, live_id!(RoomHeader));
//                                 let room_name = room_list.get_room_name(&room_id).unwrap_or(room_id.to_string());
//                                 item.set_text(cx, &format!("Room {}", room_name));
//                                 item.draw_all(cx, &mut Scope::empty());
//                             }
//                         } else {
//                             let item = list.item(cx, item_id, live_id!(RoomHeader));
//                             let room_name = room_list.get_room_name(&room_id).unwrap_or(room_id.to_string());
//                             item.set_text(cx, &format!("Room {}", room_name));
//                             item.draw_all(cx, &mut Scope::empty());
//                             search_result_draw_state.last_room_id = Some(room_id.to_owned());
//                         }
//                         match msg.original_content() {
//                             Some(AnyMessageLikeEventContent::RoomMessage(mut message)) => {
//                                 let is_contextual = matches!(&current_item.kind, SearchTimelineItemKind::ContextEvent(_));
//                                 if let MessageType::Text(text) = &mut message.msgtype {
//                                     if let Some(ref mut formatted) = text.formatted {
//                                         for highlight in search_result_state.highlighted_strings.iter() {
//                                             formatted.body = formatted.body.replace(highlight, &format!("<code>{}</code>", highlight));
//                                         }
//                                     } else {
//                                         let mut formated_string = text.body.clone();
//                                         for highlight in search_result_state.highlighted_strings.iter() {
//                                             formated_string = formated_string.replace(highlight, &format!("<code>{}</code>", highlight));
//                                         }
//                                         text.formatted = Some(FormattedBody::html(formated_string));
//                                     }
//                                 }
//                                 let event = &MessageViewFromEventWrapperAEI(event);
//                                 let prev_event = prev_event.map(PreviousWrapperAEI);
//                                 let message = RoomMessageWrapper(&message);
//                                 populate_message_view(
//                                     cx,
//                                     list,
//                                     item_id,
//                                     &room_id,
//                                     event,
//                                     MessageOrSticker::Message(&message),
//                                     prev_event.as_ref(),
//                                     media_cache,
//                                     user_power,
//                                     is_contextual,
//                                     item_drawn_status,
//                                     room_screen_widget_uid,
//                                 )
//                             }
//                             _ => return None
//                         }
//                     },
//                     AnyTimelineEvent::State(state) => {
//                         let state_key = state.state_key();
//                         if let Some(content) = state.original_content() {
//                             let wrapper = AnyStateEventContentWrapper(&content, state_key);
//                             let event = &MessageViewFromEventWrapperAEI(event);
//                             populate_small_state_event(
//                                 cx,
//                                 list,
//                                 item_id,
//                                 room_id,
//                                 event,
//                                 &wrapper,
//                                 item_drawn_status,
//                             )
//                         } else {
//                             return None
//                         }
//                     }
//                 }
//                 SearchTimelineItemKind::RoomHeader(room_id) => {
//                     let item = list.item(cx, item_id, live_id!(RoomHeader));
//                     let room_name = room_list.get_room_name(room_id).unwrap_or(room_id.to_string());
//                     item.set_text(cx, &format!("Room {}", room_name));
//                     (item, ItemDrawnStatus::both_drawn())
//                 }
//             }
//         };
//         if item_new_draw_status.content_drawn {
//             tl_state.content_drawn_since_last_update.insert(tl_idx .. tl_idx + 1);
//         }
//         if item_new_draw_status.profile_drawn {
//             tl_state.profile_drawn_since_last_update.insert(tl_idx .. tl_idx + 1);
//         }
//         item
//     };
//    Some(item)
// }
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
    pub fn with_room_header(room_id: OwnedRoomId) -> Self {
        SearchTimelineItem {
            kind: SearchTimelineItemKind::RoomHeader(room_id)
        }
    }
    pub fn body_of_timeline_item(&self) -> Option<String> {
        if let SearchTimelineItemKind::Event(AnyTimelineEvent::MessageLike(AnyMessageLikeEvent::RoomMessage(room_msg))) = &self.kind {
            if let Some(room_msg) = room_msg.as_original() {
                return Some(room_msg.content.body().to_string())
            }
        }
        None
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
    RoomHeader(OwnedRoomId),
}

/// Wrapper for `AnyStateEventContent` that provides the state key for the event.
pub struct AnyStateEventContentWrapper<'a>(pub &'a AnyStateEventContent, pub &'a str);

impl From<&AnyStateEventContentWrapper<'_>> for Option<AnyOtherFullStateEventContent> {
    fn from(val: &AnyStateEventContentWrapper<'_>) -> Self {
        match val.0.clone() {
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
/// A wrapper for `AnyTimelineEvent` that implements `MessageViewFromEvent`.
pub struct MessageViewFromEventWrapperAEI<'a>(pub &'a AnyTimelineEvent);

/// A trait for abstracting over the different types of timeline events for populating messages.
/// 
/// This is used for populating messages in the timeline and the search result.
impl MessageViewFromEvent for MessageViewFromEventWrapperAEI<'_> {
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


impl SmallStateEventContent<MessageViewFromEventWrapperAEI<'_>> for AnyStateEventContentWrapper<'_> {
    fn populate_item_content(
        &self,
        cx: &mut Cx,
        list: &mut PortalList,
        item_id: usize,
        item: WidgetRef,
        _event_tl_item: &MessageViewFromEventWrapperAEI,
        username: &str,
        _item_drawn_status: ItemDrawnStatus,
        mut new_drawn_status: ItemDrawnStatus,
    ) -> (WidgetRef, ItemDrawnStatus) {
        let Some(other_state) = self.into() else { return (list.item(cx, item_id, live_id!(Empty)), ItemDrawnStatus::new()) };
        let item = if let Some(text_preview) = text_preview_of_other_state(&other_state, self.1) {
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

/// A wrapper for `AnyTimelineEvent` that implements `PreviousMessageViewFromEvent`.
pub struct PreviousWrapperAEI<'a>(pub &'a AnyTimelineEvent);
impl <'a> PreviousMessageViewFromEvent<MessageViewFromEventWrapperAEI<'a>> for PreviousWrapperAEI<'a> {
    fn use_compact(&self, current: &MessageViewFromEventWrapperAEI<'a>) -> bool {
        let prev_msg_sender = self.0.sender();
        let current_sender = current.0.sender();
        prev_msg_sender == current_sender && current.0.origin_server_ts().0.checked_sub(self.0.origin_server_ts().0)
            // Use compact_view within a day
            .is_some_and(|d| d < uint!(86400000))
    }
}
/// A wrapper for `RoomMessageEventContent` that implements `ContextMenuFromEvent`.
pub struct RoomMessageWrapper<'a>(pub &'a RoomMessageEventContent);
impl ContextMenuFromEvent for RoomMessageWrapper<'_> {
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

pub fn handle_search_new_items(
    tl: &mut SearchResultState, 
    new_items: Vector<Arc<TimelineItem>>, 
    forward_pagination_batch: Option<String>, 
    backward_pagination_batch: Option<String>,
) {
    if let Some(forward_pagination_batch) = forward_pagination_batch.clone() {
        tl.batch_list.push(forward_pagination_batch);
    }
    tl.backward_pagination_batch = backward_pagination_batch;
    for item in new_items.iter().rev() {
        tl.items.push_front(item.clone());
    }
}

pub fn convert_result_categories_to_search_item(results: ResultCategories) -> Vector<SearchTimelineItem> {
    let mut timeline_events= Vector::new();
    let mut last_room_id: Option<OwnedRoomId> = None;
    for item in results.room_events.results.iter().rev() {
        let Some(event) = item.result.clone().and_then(|f|f.deserialize().ok()) else { continue };
        if let Some(ref mut last_room_id) = last_room_id {
            if last_room_id != event.room_id() {
                *last_room_id = event.room_id().to_owned();
                timeline_events.push_back(SearchTimelineItem::with_room_header(last_room_id.clone()));
            }
        } else {
            last_room_id = Some(event.room_id().to_owned());
            timeline_events.push_back(SearchTimelineItem::with_room_header(event.room_id().to_owned()));

        }
        timeline_events.push_back(SearchTimelineItem::with_virtual(VirtualTimelineItem::DateDivider(event.origin_server_ts())));
        item.context.events_after.iter().for_each(|f| {
            // if let Ok(timeline_event) = f.deserialize() {
            //     timeline_events.push_back(SearchTimelineItem::with_context_event(timeline_event));
            // }
            match f.deserialize() {
                Ok(timeline_event) => timeline_events.push_back(SearchTimelineItem::with_context_event(timeline_event)),
                Err(e) => println!("Error deserializing after event: {e:?}")
            }
        });
        timeline_events.push_back(SearchTimelineItem::with_event(event.clone()));
        item.context.events_before.iter().for_each(|f| {
            // if let Ok(timeline_event) = f.deserialize() {
            //     timeline_events.push_back(SearchTimelineItem::with_context_event(timeline_event));
            // } else if Err(e) {

            // }
            match f.deserialize() {
                Ok(timeline_event) => timeline_events.push_back(SearchTimelineItem::with_context_event(timeline_event)),
                Err(e) => println!("Error deserializing before event: {e:?}")
            }
        });
        
        
        
        
    }
    timeline_events
}

#[derive(Default)]
pub struct SearchResultDrawState {
    pub last_room_id: Option<OwnedRoomId>,
    pub done_loading: bool,
    pub last_date_divider_string: String,
}
