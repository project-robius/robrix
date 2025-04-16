use std::ops::DerefMut;

use indexmap::IndexMap;
use makepad_widgets::*;
use matrix_sdk_ui::timeline::{AnyOtherFullStateEventContent, InReplyToDetails, ReactionsByKeyBySender, TimelineDetails, TimelineEventItemId, VirtualTimelineItem};
use ruma::{events::{receipt::Receipt, room::message::{FormattedBody, MessageType, RoomMessageEventContent}, AnyMessageLikeEventContent, AnyStateEventContent, AnyTimelineEvent, FullStateEventContent}, uint, EventId, MilliSecondsSinceUnixEpoch, OwnedUserId, UserId};

use crate::{event_preview::text_preview_of_other_state,  utils::unix_time_millis_to_datetime};

use super::room_screen::{populate_message_view, populate_small_state_event, Eventable, ItemDrawnStatus, MessageOrSticker, MsgTypeAble, PreviousEventable, RoomScreen, SmallStateEventContent};

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;
    use crate::shared::styles::*;
    use crate::shared::helpers::*;
    use crate::shared::icon_button::*;
    use crate::home::rooms_list::RoomsList;

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
    #[rust] pub next_batch: Option<String>
}

impl Widget for SearchResult {
    // Handle events and actions for the SearchResult widget and its inner Timeline view.
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.match_event(cx, event);
        self.view.handle_event(cx, event, scope);
    }
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
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
                    next_batch
                }) => {
                    self.set_result_count(cx, *count);
                    self.next_batch = next_batch.clone();
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
    fn set_result_count(&self, cx: &mut Cx, search_result_count: usize) {
        self.view.html(id!(summary_label)).set_text(cx, &format!("{} results for <b>'{}'</b>", search_result_count, self.search_criteria));
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
    pub fn set_result_count(&self, cx: &mut Cx, search_result_count: usize) {
        let Some(inner) = self.borrow() else { return };
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

pub fn search_result_draw_walk(room_screen: &mut RoomScreen, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
    let room_screen_widget_uid = room_screen.widget_uid();
    while let Some(subview) = room_screen.view.draw_walk(cx, scope, walk).step() {
        // We only care about drawing the portal list.
        let portal_list_ref = subview.as_portal_list();
        let Some(mut list_ref) = portal_list_ref.borrow_mut() else {
            error!("!!! RoomScreen::draw_walk(): BUG: expected a PortalList widget, but got something else");
            continue;
        };
        let Some(tl_state) = room_screen.tl_state.as_mut() else {
            return DrawStep::done();
        };
        let room_id = &tl_state.room_id;
        let tl_items = &tl_state.searched_results;

        // Set the portal list's range based on the number of timeline items.
        let last_item_id = tl_items.len();
        let list = list_ref.deref_mut();
        list.set_item_range(cx, 0, last_item_id);

        while let Some(item_id) = list.next_visible_item(cx) {
            let item = {
                let tl_idx = item_id;
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
                                                for highlight in tl_state.searched_results_highlighted_strings.iter() {
                                                    formatted.body = formatted.body.replace(highlight, &format!("<code>{}</code>", highlight));
                                                }
                                            } else {
                                                let mut formated_string = text.body.clone();
                                                for highlight in tl_state.searched_results_highlighted_strings.iter() {
                                                    formated_string = formated_string.replace(highlight, &format!("<code>{}</code>", highlight));
                                                }
                                                text.formatted = Some(FormattedBody::html(formated_string));
                                            }
                                        }
                                        let event = &EventableWrapperAEI(event);
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
        count: usize,
        next_batch: Option<String>
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