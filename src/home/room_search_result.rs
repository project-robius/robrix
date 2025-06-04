use std::{borrow::Cow, collections::BTreeMap, ops::DerefMut, sync::Arc};

use indexmap::IndexMap;
use makepad_widgets::*;
use matrix_sdk_ui::timeline::{
    self, AnyOtherFullStateEventContent, InReplyToDetails, Profile, ReactionsByKeyBySender, TimelineDetails, TimelineEventItemId, TimelineItem
};
use matrix_sdk::ruma::{
    self, events::{
        receipt::Receipt, room::message::{
            AudioMessageEventContent, EmoteMessageEventContent, FileMessageEventContent, FormattedBody, ImageMessageEventContent, KeyVerificationRequestEventContent, MessageFormat, MessageType, NoticeMessageEventContent, Relation, RoomMessageEventContent, TextMessageEventContent, VideoMessageEventContent
        }, sticker::StickerEventContent, AnyMessageLikeEvent, AnyMessageLikeEventContent, AnyStateEventContent, AnyTimelineEvent, FullStateEventContent
    }, room_id, uint, EventId, MilliSecondsSinceUnixEpoch, OwnedRoomId, OwnedUserId, UserId
};

use crate::{
    app::AppState, event_preview::text_preview_of_other_state, home::{new_message_context_menu::MessageAbilities, room_screen::{draw_replied_to_message, populate_audio_message_content, populate_file_message_content, populate_image_message_content, populate_location_message_content, populate_text_message_content, populate_video_message_content, MessageOrStickerType, MessageWidgetRefExt, MESSAGE_NOTICE_TEXT_COLOR, SEARCH_HIGHLIGHT}}, media_cache::MediaCache, shared::{avatar::AvatarWidgetRefExt, html_or_plaintext::HtmlOrPlaintextWidgetRefExt, message_search_input_bar::MessageSearchAction, styles::COLOR_DANGER_RED, text_or_image::TextOrImageWidgetRefExt}, sliding_sync::{current_user_id, submit_async_request, MatrixRequest, UserPowerLevels}, utils::unix_time_millis_to_datetime
};

use crate::home::{
    new_message_context_menu::MessageDetails,
    room_screen::{
        populate_message_view, populate_small_state_event, MessageDisplay, ItemDrawnStatus,
        MessageOrSticker, ContextMenuFromEvent, PreviousMessageDisplay,
        RoomScreen, SmallStateEventContent, TimelineUiState,
    },
    rooms_list::RoomsListWidgetExt,
};

use super::SearchState;

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
                    height: 40
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
    #[live(true)]
    visible: bool,
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
                room_id: room_id!("!not_used:matrix.org").to_owned(), 
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
    room_screen: &mut RoomScreen,
    cx: &mut Cx2d,
    scope: &mut Scope,
    walk: Walk,
) -> DrawStep {
    let room_screen_widget_uid = room_screen.widget_uid();
    let Some(room_id) = &room_screen.room_id else {
        return DrawStep::done();
    };
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
        let tl_items = &tl_state.search_state.items;
        // Set the portal list's range based on the number of timeline items.
        let last_item_id = tl_items.len();
        let list = list_ref.deref_mut();
        list.set_item_range(cx, 0, last_item_id);

        while let Some(item_id) = list.next_visible_item(cx) {
            if item_id == 0 && tl_state.search_state.next_batch_token.is_none() && last_item_id > 0 {
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
                    content_drawn: tl_state
                        .search_state
                        .content_drawn_since_last_update
                        .contains(&tl_idx),
                    profile_drawn: tl_state
                        .search_state
                        .profile_drawn_since_last_update
                        .contains(&tl_idx),
                };
                let (item, item_new_draw_status) = {
                    let current_item = timeline_item;
                    let prev_event = tl_idx
                        .checked_sub(1)
                        .and_then(|i| tl_items.get(i))
                        .and_then(|f| match f {
                            SearchResultItem::Event(ref e) => Some(e),
                            _ => None,
                        });

                    match &current_item {
                        SearchResultItem::DateDivider(millis) => {
                            let item = list.item(cx, item_id, live_id!(DateDivider));
                            let text = unix_time_millis_to_datetime(millis)
                                // format the time as a shortened date (Sat, Sept 5, 2021)
                                .map(|dt| format!("{}", dt.date_naive().format("%a %b %-d, %Y")))
                                .unwrap_or_else(|| format!("{:?}", millis));
                            item.label(id!(date)).set_text(cx, &text);
                            (item, ItemDrawnStatus::both_drawn())
                        }
                        SearchResultItem::Event(event) => match event {
                            AnyTimelineEvent::MessageLike(msg) => {
                                let mut content = msg.original_content();
                                if let Some(replace) = msg.relations().replace {
                                    content = replace.original_content();
                                }
                                match content {
                                    Some(AnyMessageLikeEventContent::RoomMessage(message)) => {
                                        let mut message = message.clone();
                                        if let Some(relation) = &message.relates_to {
                                            match relation {
                                                Relation::Replacement(replace) => {
                                                    let new_content = &replace.new_content;
                                                    message.msgtype = new_content.msgtype.clone();
                                                }
                                                _ => {}
                                            }
                                        }

                                        if let MessageType::Text(text) = &mut message.msgtype {
                                            if let Some(ref mut formatted) = text.formatted {
                                                for highlight in tl_state
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
                                                for highlight in tl_state
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
                                        // Do not use compact view if previous event is state
                                        let prev_event = prev_event
                                            .and_then(|f| {
                                                if matches!(f, AnyTimelineEvent::State(_)) {
                                                    None
                                                } else {
                                                    Some(f)
                                                }
                                            });
                                        
                                        populate_message_search_view(
                                            cx,
                                            list,
                                            item_id,
                                            room_id,
                                            event,
                                            &message,
                                            prev_event,
                                            &tl_state.search_state.profile_infos,
                                            &mut tl_state.media_cache,
                                            &tl_state.user_power,
                                            item_drawn_status,
                                            room_screen_widget_uid,
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
                            let room_name = room_screen
                                .view
                                .rooms_list(id!(rooms_list))
                                .get_room_name(room_id)
                                .unwrap_or(room_id.to_string());
                            let item = list.item(cx, item_id, live_id!(RoomHeader));
                            item.set_text(cx, &format!("Room {}", room_name));
                            (item, ItemDrawnStatus::both_drawn())
                        }
                    }
                };
                if item_new_draw_status.content_drawn {
                    tl_state
                        .search_state
                        .content_drawn_since_last_update
                        .insert(tl_idx..tl_idx + 1);
                }
                if item_new_draw_status.profile_drawn {
                    tl_state
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

/// Copies the HTML content of a message to the clipboard if the message is a text message, notice, emote, image, file, audio, video, or verification request, and if it is formatted as HTML.
///
/// If the message is an edit of another message, the function will copy the content of the original message instead of the edited message.
///
/// Returns `true` if the content was copied successfully, and `false` otherwise.
///
/// The function takes as input the current Makepad context, a reference to a `MessageDetails` struct, a reference to a `TimelineUiState` struct, and a mutable reference to a boolean to set to `true` if the content was copied successfully.
pub fn search_copy_html(
    cx: &mut Cx,
    details: &MessageDetails,
    tl: &TimelineUiState,
    success: &mut bool,
) {
    let Some(SearchResultItem::Event(event)) =
        tl.search_state.items.get(details.item_id)
    else {
        return;
    };
    if let AnyTimelineEvent::MessageLike(msg) = event {
        let mut content = msg.original_content();
        if let Some(replace) = msg.relations().replace {
            content = replace.original_content();
        }
        if let Some(AnyMessageLikeEventContent::RoomMessage(mut message)) = content {
            if let Some(Relation::Replacement(replace)) = &message.relates_to {
                let new_content = &replace.new_content;
                message.msgtype = new_content.msgtype.clone();
            }
            match &message.msgtype {
                MessageType::Text(TextMessageEventContent {
                    formatted: Some(FormattedBody { body, .. }),
                    ..
                })
                | MessageType::Notice(NoticeMessageEventContent {
                    formatted: Some(FormattedBody { body, .. }),
                    ..
                })
                | MessageType::Emote(EmoteMessageEventContent {
                    formatted: Some(FormattedBody { body, .. }),
                    ..
                })
                | MessageType::Image(ImageMessageEventContent {
                    formatted: Some(FormattedBody { body, .. }),
                    ..
                })
                | MessageType::File(FileMessageEventContent {
                    formatted: Some(FormattedBody { body, .. }),
                    ..
                })
                | MessageType::Audio(AudioMessageEventContent {
                    formatted: Some(FormattedBody { body, .. }),
                    ..
                })
                | MessageType::Video(VideoMessageEventContent {
                    formatted: Some(FormattedBody { body, .. }),
                    ..
                })
                | MessageType::VerificationRequest(KeyVerificationRequestEventContent {
                    formatted: Some(FormattedBody { body, .. }),
                    ..
                }) => {
                    cx.copy_to_clipboard(body);
                    *success = true;
                }
                _ => {}
            }
        }
    }
}
/// Copies the text of the message at the given item id to the clipboard.
///
/// The item id must point to a message in the timeline, otherwise this function does nothing.
///
/// If the message is a reply-to message, the body of the replied-to message is copied instead.
pub fn search_result_copy_to_clipboard(
    cx: &mut Cx,
    details: &MessageDetails,
    tl: &TimelineUiState,
) {
    let Some(SearchResultItem::Event(event)) =
        tl.search_state.items.get(details.item_id)
    else {
        return;
    };
    if let AnyTimelineEvent::MessageLike(msg) = event {
        let mut content = msg.original_content();
        if let Some(replace) = msg.relations().replace {
            content = replace.original_content();
        }
        if let Some(AnyMessageLikeEventContent::RoomMessage(mut message)) = content {
            if let Some(Relation::Replacement(replace)) = &message.relates_to {
                let new_content = &replace.new_content;
                message.msgtype = new_content.msgtype.clone();
            }
            cx.copy_to_clipboard(message.body());
        }
    }
}
/// React to a message in the timeline.
///
/// The item id must point to a message in the timeline, otherwise this function does nothing.
///
/// If the message is a reply-to message, the reaction is sent to the replied-to message instead.
///
pub fn search_result_react(
    cx: &mut Cx,
    room_screen_view: &View,
    details: &MessageDetails,
    tl: &TimelineUiState,
    reaction: String,
    success: &mut bool,
) {
    let Some(SearchResultItem::Event(event)) =
        tl.search_state.items.get(details.item_id)
    else {
        return;
    };
    if Some(event.event_id().to_owned()) != details.event_id {
        return;
    }
    room_screen_view
        .view(id!(search_timeline))
        .set_visible(cx, false);
    if let Some(transaction_id) = event.transaction_id() {
        submit_async_request(MatrixRequest::ToggleReaction {
            room_id: event.room_id().to_owned(),
            timeline_event_id: TimelineEventItemId::TransactionId(transaction_id.to_owned()),
            reaction,
        });
    } else {
        submit_async_request(MatrixRequest::ToggleReaction {
            room_id: event.room_id().to_owned(),
            timeline_event_id: TimelineEventItemId::EventId(event.event_id().to_owned()),
            reaction,
        });
    }
    *success = true;
}

/// Reply to a message in the timeline.
///
/// The item id must point to a message in the timeline, otherwise this function does nothing.
///
/// If the message is a reply-to message, the reply is sent to the replied-to message instead.
///
/// This function also hides the search results view and clears the search filter.
pub fn search_result_reply(
    cx: &mut Cx,
    room_screen_view: &View,
    room_screen_widget_uid: WidgetUid,
    details: &MessageDetails,
    tl: &TimelineUiState,
    success: &mut bool,
) -> Option<MessageDetails> {
    let Some(SearchResultItem::Event(event)) =
        tl.search_state.items.get(details.item_id)
    else {
        return None;
    };
    if Some(event.event_id().to_owned()) != details.event_id {
        return None;
    }
    let mut timeline_details = details.clone();
    for (index, item) in tl.items.iter().enumerate() {
        if item
            .as_event()
            .and_then(|f| f.event_id())
            .map(|f| Some(f.to_owned()) == details.event_id)
            .unwrap_or(false)
        {
            timeline_details.item_id = index;
            break;
        }
    }
    room_screen_view.view(id!(search_timeline))
        .set_visible(cx, false);
    cx.widget_action(
        room_screen_widget_uid,
        &Scope::empty().path,
        MessageSearchAction::Clear,
    );
    *success = true;
    Some(timeline_details)
}

pub fn search_result_redact(
    cx: &mut Cx,
    room_screen_view: &View,
    room_screen_widget_uid: WidgetUid,
    details: &MessageDetails,
    tl: &TimelineUiState,
    success: &mut bool,
) -> Option<MessageDetails> {
    let Some(SearchResultItem::Event(event)) =
        tl.search_state.items.get(details.item_id)
    else {
        return None;
    };
    if Some(event.event_id().to_owned()) != details.event_id {
        return None;
    }
    let mut timeline_details = details.clone();
    for (index, item) in tl.items.iter().enumerate() {
        if item
            .as_event()
            .and_then(|f| f.event_id())
            .map(|f| Some(f.to_owned()) == details.event_id)
            .unwrap_or(false)
        {
            timeline_details.item_id = index;
            break;
        }
    }
    room_screen_view.view(id!(search_timeline))
        .set_visible(cx, false);
    cx.widget_action(
        room_screen_widget_uid,
        &Scope::empty().path,
        MessageSearchAction::Clear,
    );
    *success = true;
    Some(timeline_details)
}
/// Finds the index of the timeline item in the main timeline items list
/// that the given search result item is related to (i.e. is a reply to).
///
/// Returns `None` if the search result item is not a reply to a message
/// in the main timeline items list.
pub fn search_result_jump_to_related(
    _cx: &mut Cx,
    details: &MessageDetails,
    tl: &TimelineUiState,
) -> Option<usize> {
    let Some(SearchResultItem::Event(event)) =
        tl.search_state.items.get(details.item_id)
    else {
        return None;
    };
    if Some(event.event_id().to_owned()) != details.event_id {
        return None;
    }
    if let Some((pos, _elt)) = tl.items.iter().enumerate().find(|(_i, x)| {
        x.as_event()
            .and_then(|f| f.event_id())
            .map(|f| Some(f.to_owned()) == details.event_id)
            .unwrap_or(false)
    }) {
        return Some(pos);
    }
    None
}

/// Handles any search-related actions received by this RoomScreen.
///
/// See `MessageSearchAction` for the possible actions.
pub fn handle_search_input(
    room_screen: &mut RoomScreen,
    cx: &mut Cx,
    action: &Action,
    scope: &mut Scope,
) {
    let widget_action = action.as_widget_action();
    match widget_action.cast() {
        MessageSearchAction::Changed(search_term) => {
            if search_term.is_empty() {
                room_screen
                    .search_result(id!(search_result_plane))
                    .reset(cx);
                room_screen.view(id!(timeline)).set_visible(cx, true);
                room_screen
                    .view(id!(search_timeline))
                    .set_visible(cx, false);
                // Abort previous inflight search request.
                submit_async_request(MatrixRequest::SearchMessages { 
                    room_id: room_id!("!not_used:matrix.org").to_owned(), 
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
                if Some(selected_room.room_id()) == room_screen.room_id.as_ref() {
                    room_screen.search_debounce_timer = cx.start_timeout(3.0);
                    if let Some(ref mut tl_state) = room_screen.tl_state {
                        tl_state.search_state = SearchState::default();
                    }
                    let mut criteria = room_screen
                        .search_result(id!(search_result_plane))
                        .get_search_criteria();
                    criteria.search_term = search_term;
                    criteria.include_all_rooms = false;
                    room_screen
                        .search_result(id!(search_result_plane))
                        .set_search_criteria(cx, criteria);
                    room_screen.view(id!(timeline)).set_visible(cx, false);
                }
            }
        }
        MessageSearchAction::Click(search_term) => {
            if let Some(selected_room) = {
                let app_state = scope.data.get::<AppState>().unwrap();
                app_state.selected_room.clone()
            } {
                if Some(selected_room.room_id()) == room_screen.room_id.as_ref() {
                    let mut criteria = room_screen
                        .search_result(id!(search_result_plane))
                        .get_search_criteria();
                    if search_term == criteria.search_term && !search_term.is_empty() {
                        return;
                    }
                    criteria.search_term = search_term.clone();
                    room_screen
                        .search_result(id!(search_result_plane))
                        .set_search_criteria(cx, criteria);
                    room_screen.view(id!(timeline)).set_visible(cx, false);
                }
            }
        }
        MessageSearchAction::Clear => {
            cx.stop_timer(room_screen.search_debounce_timer);
            room_screen
                .view(id!(search_timeline))
                .set_visible(cx, false);
            room_screen.view(id!(timeline)).set_visible(cx, true);
            room_screen
                .search_result(id!(search_result_plane))
                .reset(cx);
            room_screen
                .search_result(id!(search_result_plane))
                .set_visible(cx, false);
            let Some(tl) = room_screen.tl_state.as_mut() else {
                return;
            };
            tl.search_state = SearchState::default();
        }
        _ => {}
    }
}

pub fn send_pagination_request_based_on_scroll_pos_for_search_result(
    room_screen: &mut RoomScreen,
    cx: &mut Cx,
    actions: &ActionsBuf,
    portal_list: &PortalListRef,
    search_result_plane: &SearchResultRef
) {
    let Some(tl) = room_screen.tl_state.as_mut() else { return };
    let search_state = &mut tl.search_state;
    if search_state.fully_paginated { return };
    
    if !portal_list.scrolled(actions) { return };

    let first_index = portal_list.first_id();
    if first_index == 0 && search_state.last_scrolled_index > 0 {
        if let Some(next_batch_token) = &search_state.next_batch_token.take() {
            log!("Scrolled up from item {} --> 0, sending search request for room {} with backward_pagination_batch {:?}",
                search_state.last_scrolled_index, tl.room_id, next_batch_token
            );
            search_result_plane.display_top_space(cx);
            let criteria = search_result_plane.get_search_criteria();
            submit_async_request(MatrixRequest::SearchMessages {
                room_id: tl.room_id.clone(),
                include_all_rooms: criteria.include_all_rooms,
                search_term: criteria.search_term.clone(),
                next_batch: Some(next_batch_token.clone()),
                abort_previous_search: false
            });
        }
    }
    tl.search_state.last_scrolled_index = first_index;
}

/// Search result as timeline item
#[derive(Clone, Debug)]
pub enum SearchResultItem {
    /// The event that matches the search criteria.
    Event(AnyTimelineEvent),
    /// A date divider used to separate each search results.
    DateDivider(MilliSecondsSinceUnixEpoch),
    /// The room id used for displaying room header for all searched messages in a screen.
    RoomHeader(OwnedRoomId),
}

pub struct AnyStateEventContentWrapper<'a>(pub &'a AnyStateEventContent, pub &'a str);

impl<'a> From<&AnyStateEventContentWrapper<'a>> for Option<AnyOtherFullStateEventContent> {
    fn from(val: &AnyStateEventContentWrapper<'a>) -> Self {
        match val.0 {
            AnyStateEventContent::RoomAliases(p) => Some(
                AnyOtherFullStateEventContent::RoomAliases(FullStateEventContent::Original {
                    content: p.clone(),
                    prev_content: None,
                }),
            ),
            AnyStateEventContent::RoomAvatar(p) => Some(AnyOtherFullStateEventContent::RoomAvatar(
                FullStateEventContent::Original {
                    content: p.clone(),
                    prev_content: None,
                },
            )),
            AnyStateEventContent::RoomCanonicalAlias(p) => {
                Some(AnyOtherFullStateEventContent::RoomCanonicalAlias(
                    FullStateEventContent::Original {
                        content: p.clone(),
                        prev_content: None,
                    },
                ))
            }
            AnyStateEventContent::RoomCreate(p) => Some(AnyOtherFullStateEventContent::RoomCreate(
                FullStateEventContent::Original {
                    content: p.clone(),
                    prev_content: None,
                },
            )),
            AnyStateEventContent::RoomEncryption(p) => Some(
                AnyOtherFullStateEventContent::RoomEncryption(FullStateEventContent::Original {
                    content: p.clone(),
                    prev_content: None,
                }),
            ),
            AnyStateEventContent::RoomGuestAccess(p) => Some(
                AnyOtherFullStateEventContent::RoomGuestAccess(FullStateEventContent::Original {
                    content: p.clone(),
                    prev_content: None,
                }),
            ),
            AnyStateEventContent::RoomHistoryVisibility(p) => {
                Some(AnyOtherFullStateEventContent::RoomHistoryVisibility(
                    FullStateEventContent::Original {
                        content: p.clone(),
                        prev_content: None,
                    },
                ))
            }
            AnyStateEventContent::RoomJoinRules(p) => Some(
                AnyOtherFullStateEventContent::RoomJoinRules(FullStateEventContent::Original {
                    content: p.clone(),
                    prev_content: None,
                }),
            ),
            AnyStateEventContent::RoomPinnedEvents(p) => Some(
                AnyOtherFullStateEventContent::RoomPinnedEvents(FullStateEventContent::Original {
                    content: p.clone(),
                    prev_content: None,
                }),
            ),
            AnyStateEventContent::RoomName(p) => Some(AnyOtherFullStateEventContent::RoomName(
                FullStateEventContent::Original {
                    content: p.clone(),
                    prev_content: None,
                },
            )),
            AnyStateEventContent::RoomPowerLevels(p) => Some(
                AnyOtherFullStateEventContent::RoomPowerLevels(FullStateEventContent::Original {
                    content: p.clone(),
                    prev_content: None,
                }),
            ),
            AnyStateEventContent::RoomServerAcl(p) => Some(
                AnyOtherFullStateEventContent::RoomServerAcl(FullStateEventContent::Original {
                    content: p.clone(),
                    prev_content: None,
                }),
            ),
            AnyStateEventContent::RoomTombstone(p) => Some(
                AnyOtherFullStateEventContent::RoomTombstone(FullStateEventContent::Original {
                    content: p.clone(),
                    prev_content: None,
                }),
            ),
            AnyStateEventContent::RoomTopic(p) => Some(AnyOtherFullStateEventContent::RoomTopic(
                FullStateEventContent::Original {
                    content: p.clone(),
                    prev_content: None,
                },
            )),
            AnyStateEventContent::SpaceParent(p) => Some(
                AnyOtherFullStateEventContent::SpaceParent(FullStateEventContent::Original {
                    content: p.clone(),
                    prev_content: None,
                }),
            ),
            AnyStateEventContent::SpaceChild(p) => Some(AnyOtherFullStateEventContent::SpaceChild(
                FullStateEventContent::Original {
                    content: p.clone(),
                    prev_content: None,
                },
            )),
            AnyStateEventContent::PolicyRuleRoom(p) => Some(
                AnyOtherFullStateEventContent::PolicyRuleRoom(FullStateEventContent::Original {
                    content: p.clone(),
                    prev_content: None,
                }),
            ),
            AnyStateEventContent::PolicyRuleServer(p) => Some(
                AnyOtherFullStateEventContent::PolicyRuleServer(FullStateEventContent::Original {
                    content: p.clone(),
                    prev_content: None,
                }),
            ),
            AnyStateEventContent::PolicyRuleUser(p) => Some(
                AnyOtherFullStateEventContent::PolicyRuleUser(FullStateEventContent::Original {
                    content: p.clone(),
                    prev_content: None,
                }),
            ),
            AnyStateEventContent::RoomThirdPartyInvite(p) => {
                Some(AnyOtherFullStateEventContent::RoomThirdPartyInvite(
                    FullStateEventContent::Original {
                        content: p.clone(),
                        prev_content: None,
                    },
                ))
            }
            AnyStateEventContent::BeaconInfo(_) => None,
            AnyStateEventContent::CallMember(_) => None,
            AnyStateEventContent::MemberHints(_) => None,
            AnyStateEventContent::RoomMember(_) => None,
            _ => None,
        }
    }
}

/// Wrapper for AnyTimelineEvent that implements `MessageDisplay` trait.
pub struct MessageDisplayWrapperAEI<'a>(
    pub &'a AnyTimelineEvent,
    pub &'a BTreeMap<OwnedUserId, TimelineDetails<Profile>>,
);

impl MessageDisplay for MessageDisplayWrapperAEI<'_> {
    fn timestamp(&self) -> MilliSecondsSinceUnixEpoch {
        self.0.origin_server_ts()
    }
    fn event_id(&self) -> Option<&EventId> {
        Some(self.0.event_id())
    }
    fn sender(&self) -> &UserId {
        self.0.sender()
    }
    fn sender_profile(&self) -> Option<&TimelineDetails<Profile>> {
        self.1.get(self.sender())
    }
    fn reactions(&self) -> Option<ReactionsByKeyBySender> {
        None
    }
    fn identifier(&self) -> TimelineEventItemId {
        if let Some(transaction_id) = self.0.transaction_id() {
            return TimelineEventItemId::TransactionId(transaction_id.to_owned());
        }
        TimelineEventItemId::EventId(self.0.event_id().to_owned())
    }
    fn is_highlighted(&self) -> bool {
        false
    }
    fn is_editable(&self) -> bool {
        if !self.is_own() {
            return false;
        }
        if let AnyTimelineEvent::MessageLike(AnyMessageLikeEvent::RoomMessage(msg)) = self.0 {
            if let Some(is_editable) = msg.as_original().map(|f| {
                matches!(
                    f.content.msgtype,
                    MessageType::Text(_)
                        | MessageType::Emote(_)
                        | MessageType::Audio(_)
                        | MessageType::File(_)
                        | MessageType::Image(_)
                        | MessageType::Video(_)
                )
            }) {
                return is_editable;
            }
        }
        false
    }

    fn is_own(&self) -> bool {
        if current_user_id() == Some(self.0.sender().to_owned()) {
            return true;
        }
        false
    }
    fn can_be_replied_to(&self) -> bool {
        if self.event_id().is_none() {
            false
        } else {
            matches!(
                self.0,
                AnyTimelineEvent::MessageLike(AnyMessageLikeEvent::RoomMessage(_))
            )
        }
    }
    fn read_receipts(&self) -> Option<&IndexMap<OwnedUserId, Receipt>> {
        None
    }

    fn room_id(&self) -> Option<&ruma::RoomId> {
        Some(self.0.room_id())
    }
}

impl SmallStateEventContent<MessageDisplayWrapperAEI<'_>>
    for AnyStateEventContentWrapper<'_>
{
    fn populate_item_content(
        &self,
        cx: &mut Cx,
        list: &mut PortalList,
        item_id: usize,
        item: WidgetRef,
        _event_tl_item: &MessageDisplayWrapperAEI,
        username: &str,
        _item_drawn_status: ItemDrawnStatus,
        mut new_drawn_status: ItemDrawnStatus,
    ) -> (WidgetRef, ItemDrawnStatus) {
        let Some(other_state) = self.into() else {
            return (
                list.item(cx, item_id, live_id!(Empty)),
                ItemDrawnStatus::new(),
            );
        };
        let item =
            if let Some(text_preview) = text_preview_of_other_state(&other_state, true, self.1) {
                item.label(id!(content))
                    .set_text(cx, &text_preview.format_with(username, true));
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

/// A wrapper for the `AnyTimelineEvent` that implements `PreviousMessageDisplay` trait for compact view.
pub struct PreviousWrapperAEI<'a>(pub &'a AnyTimelineEvent);
impl<'a> PreviousMessageDisplay<MessageDisplayWrapperAEI<'a>>
    for PreviousWrapperAEI<'a>
{
    fn use_compact(&self, current: &MessageDisplayWrapperAEI<'a>) -> bool {
        let prev_msg_sender = self.0.sender();
        let current_sender = current.0.sender();
        {
            prev_msg_sender == current_sender
                && current
                    .0
                    .origin_server_ts()
                    .0
                    .checked_sub(self.0.origin_server_ts().0)
                    .is_some_and(|d| d < uint!(86400000)) //within a day
        }
    }
}

/// A wrapper for the `RoomMessageEventContent` and `InReplyToDetails` that implements `ContextMenuFromEvent` trait.
pub struct MessageWrapperRMC<'a>(
    pub &'a RoomMessageEventContent,
    pub Option<&'a InReplyToDetails>,
);
impl ContextMenuFromEvent for MessageWrapperRMC<'_> {
    fn msgtype(&self) -> &MessageType {
        &self.0.msgtype
    }
    fn body(&self) -> &str {
        self.0.body()
    }
    fn in_reply_to(&self) -> Option<&InReplyToDetails> {
        self.1
    }
    fn is_searched_result(&self) -> bool {
        true
    }
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

pub enum MessageOrStickerSearchView<'e> {
    Message(&'e RoomMessageEventContent),
    Sticker(&'e StickerEventContent),
}

impl MessageOrStickerSearchView<'_>{
    /// Returns the type of this message or sticker.
    pub fn get_type(&self) -> MessageOrStickerType {
        match self {
            Self::Message(msg) => match &msg.msgtype {
                MessageType::Audio(audio) => MessageOrStickerType::Audio(&audio),
                MessageType::Emote(emote) => MessageOrStickerType::Emote(&emote),
                MessageType::File(file) => MessageOrStickerType::File(&file),
                MessageType::Image(image) => MessageOrStickerType::Image(&image),
                MessageType::Location(location) => MessageOrStickerType::Location(&location),
                MessageType::Notice(notice) => MessageOrStickerType::Notice(&notice),
                MessageType::ServerNotice(server_notice) => MessageOrStickerType::ServerNotice(&server_notice),
                MessageType::Text(text) => MessageOrStickerType::Text(&text),
                MessageType::Video(video) => MessageOrStickerType::Video(&video),
                MessageType::VerificationRequest(verification_request) => MessageOrStickerType::VerificationRequest(&verification_request),
                MessageType::_Custom(custom) => MessageOrStickerType::_Custom(&custom),
                _ => MessageOrStickerType::Unknown,
            },
            Self::Sticker(sticker) => MessageOrStickerType::Sticker(sticker),
        }
    }
    pub fn body(&self) -> &str {
        match self {
            Self::Message(msg) => msg.body(),
            Self::Sticker(sticker) => sticker.body.as_str(),
        }
    }
}
pub fn populate_message_search_view(
    cx: &mut Cx2d,
    list: &mut PortalList,
    item_id: usize,
    room_id: &OwnedRoomId,
    event_tl_item: &AnyTimelineEvent,
    message: &RoomMessageEventContent,
    prev_event: Option<&AnyTimelineEvent>,
    user_profiles: &BTreeMap<OwnedUserId, TimelineDetails<Profile>>,
    media_cache: &mut MediaCache,
    user_power_levels: &UserPowerLevels,
    item_drawn_status: ItemDrawnStatus,
    room_screen_widget_uid: WidgetUid,
) -> (WidgetRef, ItemDrawnStatus) {
    let mut new_drawn_status = item_drawn_status;
    let ts_millis = event_tl_item.origin_server_ts();
    let has_html_body: bool;

    // Sometimes we need to call this up-front, so we save the result in this variable
    // to avoid having to call it twice.
    let mut set_username_and_get_avatar_retval = None;
    let (item, used_cached_item) = match &message.msgtype {
        MessageType::Text(TextMessageEventContent { body, formatted, .. }) => {
            has_html_body = formatted.as_ref().is_some_and(|f| f.format == MessageFormat::Html);
            let template = live_id!(Message);
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
                populate_text_message_content(cx, &html_or_plaintext_ref, &body, formatted.as_ref());
                new_drawn_status.content_drawn = true;
                (item, false)
            }
        }
        mtype @ MessageType::Image(image) => {
            has_html_body = match mtype {
                MessageType::Image(image) => image.formatted.as_ref()
                    .is_some_and(|f| f.format == MessageFormat::Html),
                _ => false,
            };
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
            has_html_body = file_content.formatted.as_ref().is_some_and(|f| f.format == MessageFormat::Html);
            let template = live_id!(Message);
            let (item, existed) = list.item_with_existed(cx, item_id, template);
            if existed && item_drawn_status.content_drawn {
                (item, true)
            } else {
                new_drawn_status.content_drawn = populate_file_message_content(
                    cx,
                    &item.html_or_plaintext(id!(content.message)),
                    &file_content,
                );
                (item, false)
            }
        }
        MessageType::Audio(audio) => {
            has_html_body = audio.formatted.as_ref().is_some_and(|f| f.format == MessageFormat::Html);
            let template = live_id!(Message);
            let (item, existed) = list.item_with_existed(cx, item_id, template);
            if existed && item_drawn_status.content_drawn {
                (item, true)
            } else {
                new_drawn_status.content_drawn = populate_audio_message_content(
                    cx,
                    &item.html_or_plaintext(id!(content.message)),
                    &audio,
                );
                (item, false)
            }
        }
        other => {
            has_html_body = false;
            let (item, existed) = list.item_with_existed(cx, item_id, live_id!(Message));
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
        let (username, profile_drawn) = set_username_and_get_avatar_retval.unwrap_or_else(||
                item.avatar(id!(profile.avatar)).set_avatar_and_get_username(
                    cx,
                    event_tl_item.room_id(),
                    event_tl_item.sender(),
                    user_profiles.get(event_tl_item.sender()),
                    Some(event_tl_item.event_id()),
                )
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
        // format as AM/PM 12-hour time
        item.label(id!(profile.timestamp))
            .set_text(cx, &format!("{}", dt.time().format("%l:%M %P")));
        item.label(id!(profile.datestamp))
            .set_text(cx, &format!("{}", dt.date_naive()));
        
    } else {
        item.label(id!(profile.timestamp))
            .set_text(cx, &format!("{}", ts_millis.get()));
    }
    (item, new_drawn_status)
}