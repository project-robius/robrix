use std::{borrow::Cow, ops::DerefMut};

use indexmap::IndexMap;
use makepad_widgets::*;
use matrix_sdk_ui::timeline::{AnyOtherFullStateEventContent, Profile, ReactionsByKeyBySender, TimelineDetails, TimelineEventItemId, TimelineItemKind, VirtualTimelineItem};
use ruma::{events::{receipt::Receipt, relation::InReplyTo, room::message::{EmoteMessageEventContent, FormattedBody, MessageFormat, MessageType, NoticeMessageEventContent, Relation, RoomMessageEventContent, TextMessageEventContent}, sticker::StickerEventContent, AnyMessageLikeEventContent, AnyStateEventContent, AnyTimelineEvent, FullStateEventContent}, uint, EventId, MilliSecondsSinceUnixEpoch, OwnedRoomId, OwnedUserId, UserId};

use crate::{event_preview::text_preview_of_other_state_new, media_cache::MediaCache, shared::{avatar::AvatarWidgetRefExt, html_or_plaintext::HtmlOrPlaintextWidgetRefExt, text_or_image::TextOrImageWidgetRefExt}, sliding_sync::UserPowerLevels, utils::unix_time_millis_to_datetime};

use super::{new_message_context_menu::{MessageAbilities, MessageDetails}, room_screen::{populate_audio_message_content, populate_file_message_content, populate_image_message_content, populate_location_message_content, populate_message_view, populate_small_state_event, populate_text_message_content, populate_video_message_content, set_timestamp, Eventable, ItemDrawnStatus, MessageOrSticker, MessageOrStickerType, MessageWidgetRefExt, MsgTypeWrapperRMC, PreviousEventable, RoomScreen}};

const MESSAGE_NOTICE_TEXT_COLOR: Vec3 = Vec3 { x: 0.5, y: 0.5, z: 0.5 };
const COLOR_DANGER_RED: Vec3 = Vec3 { x: 0.862, y: 0.0, z: 0.02 };
const SEARCH_HIGHLIGHT: Vec4 = Vec4 {
    x: 0.89,
    y: 0.967,
    z: 0.929,
    w: 1.0,
}; // LightGreen
live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;
    use crate::shared::styles::*;
    use crate::shared::helpers::*;
    use crate::shared::icon_button::*;
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
    /// The room ID of the currently-shown room.
    #[rust] pub room_id: Option<OwnedRoomId>,
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
                Some(SearchResultAction::Success(result_length, search_criteria)) => {
                    self.set_summary(cx, *result_length, search_criteria.clone());
                }
                Some(SearchResultAction::Pending) => {
                    self.view.search_result(id!(search_result_overlay)).set_visible(cx, true);
                }
                _ => {}
            }
        }
    }
}
impl SearchResult {
    /// Sets the `search_result_count` and `search_criteria` fields of this `SearchResult`.
    ///
    /// This is used to display the number of search results and the search criteria
    /// in the top-right of the room screen.
    fn set_summary(&mut self, cx: &mut Cx, search_result_count: usize, search_criteria: String) {
        self.view.html(id!(summary_label)).set_text(cx, &format!("{} results for <b>'{}'</b>", search_result_count, search_criteria));
        self.view.view(id!(loading_view)).set_visible(cx, false);
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
    /// See [`SearchResult::set_summary()`].
    pub fn set_summary(&self, cx: &mut Cx, search_result_count: usize, search_criteria: String) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.set_summary(cx, search_result_count, search_criteria);
    }

    /// See [`SearchResult::reset_summary()`].
    pub fn reset_summary(&self, cx: &mut Cx) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.reset_summary(cx);
    }
}
/// Creates, populates, and adds a Message liveview widget to the given `PortalList`
/// with the given `item_id`.
///
/// The content of the returned `Message` widget is populated with data from a message
/// or sticker and its containing `EventTimelineItem`.
// pub fn populate_message_view2(
//     cx: &mut Cx2d,
//     list: &mut PortalList,
//     item_id: usize,
//     room_id: &OwnedRoomId,
//     current_event: &AnyTimelineEvent,
//     message: MessageOrSticker,
//     prev_event: Option<&AnyTimelineEvent>,
//     media_cache: &mut MediaCache,
//     user_power_levels: &UserPowerLevels,
//     item_drawn_status: ItemDrawnStatus,
//     sender_profile:  Option<&TimelineDetails<Profile>>,
//     is_contextual: bool,
//     room_screen_widget_uid: WidgetUid,
// ) -> (WidgetRef, ItemDrawnStatus) {
//     let mut new_drawn_status = item_drawn_status;
//     let ts_millis = current_event.origin_server_ts();
    
//     let mut is_notice = false; // whether this message is a Notice
//     let mut is_server_notice = false; // whether this message is a Server Notice

//     // Determine whether we can use a more compact UI view that hides the user's profile info
//     // if the previous message (including stickers) was sent by the same user within 10 minutes.
//     let use_compact_view = match prev_event {
//         Some(AnyTimelineEvent::MessageLike(prev_event_tl_item)) => match prev_event_tl_item.original_content() {
//             Some(AnyMessageLikeEventContent::RoomMessage(_)) | Some(AnyMessageLikeEventContent::Sticker(_)) => {
//                 let prev_msg_sender = prev_event_tl_item.sender();
//                 prev_msg_sender == current_event.sender()
//                     && ts_millis.0
//                         .checked_sub(prev_event_tl_item.origin_server_ts().0)
//                         .is_some_and(|d| d < uint!(600000)) // 10 mins in millis
//             }
//             _ => false,
//         },
//         _ => false,
//     };

//     let has_html_body: bool;

//     // Sometimes we need to call this up-front, so we save the result in this variable
//     // to avoid having to call it twice.
//     let mut set_username_and_get_avatar_retval = None;
//     let in_reply_to = message.in_reply_to();
//     let (item, used_cached_item) = match message.get_type() {
//         MessageOrStickerType::Text(TextMessageEventContent { body, formatted, .. }) => {
//             has_html_body = formatted.as_ref().is_some_and(|f| f.format == MessageFormat::Html);
//             let template = if use_compact_view {
//                 live_id!(CondensedMessage)
//             } else {
//                 live_id!(Message)
//             };
//             let (item, existed) = list.item_with_existed(cx, item_id, template);
//             if existed && item_drawn_status.content_drawn {
//                 (item, true)
//             } else {
//                 let html_or_plaintext_ref = item.html_or_plaintext(id!(content.message));
//                 html_or_plaintext_ref.apply_over(cx, live!(
//                     html_view = {
//                         html = {
//                             font_color: (vec3(0.0,0.0,0.0)),
//                             draw_block: {
//                                 code_color: (SEARCH_HIGHLIGHT)
//                             }
//                             font_size: (15.0),
//                         }
//                     }
//                 ));
//                 populate_text_message_content(
//                     cx,
//                     &item.html_or_plaintext(id!(content.message)),
//                     body,
//                     formatted.as_ref(),
//                 );
//                 new_drawn_status.content_drawn = true;
//                 (item, false)
//             }
//         }
//         // A notice message is just a message sent by an automated bot,
//         // so we treat it just like a message but use a different font color.
//         MessageOrStickerType::Notice(NoticeMessageEventContent { body, formatted, .. }) => {
//             is_notice = true;
//             has_html_body = formatted.as_ref().is_some_and(|f| f.format == MessageFormat::Html);
//             let template = if use_compact_view {
//                 live_id!(CondensedMessage)
//             } else {
//                 live_id!(Message)
//             };
//             let (item, existed) = list.item_with_existed(cx, item_id, template);
//             if existed && item_drawn_status.content_drawn {
//                 (item, true)
//             } else {
//                 let html_or_plaintext_ref = item.html_or_plaintext(id!(content.message));
//                 html_or_plaintext_ref.apply_over(cx, live!(
//                     html_view = {
//                         html = {
//                             font_color: (MESSAGE_NOTICE_TEXT_COLOR),
//                             draw_normal:      { color: (MESSAGE_NOTICE_TEXT_COLOR), }
//                             draw_italic:      { color: (MESSAGE_NOTICE_TEXT_COLOR), }
//                             draw_bold:        { color: (MESSAGE_NOTICE_TEXT_COLOR), }
//                             draw_bold_italic: { color: (MESSAGE_NOTICE_TEXT_COLOR), }
//                             draw_block: {
//                                 code_color: (SEARCH_HIGHLIGHT)
//                             }
//                         }
//                     }
//                 ));
//                 populate_text_message_content(
//                     cx,
//                     &html_or_plaintext_ref,
//                     body,
//                     formatted.as_ref(),
//                 );
//                 new_drawn_status.content_drawn = true;
//                 (item, false)
//             }
//         }
//         MessageOrStickerType::ServerNotice(sn) => {
//             is_server_notice = true;
//             has_html_body = false;
//             let (item, existed) = list.item_with_existed(cx, item_id, live_id!(Message));

//             if existed && item_drawn_status.content_drawn {
//                 (item, true)
//             } else {
//                 let html_or_plaintext_ref = item.html_or_plaintext(id!(content.message));
//                 html_or_plaintext_ref.apply_over(cx, live!(
//                     html_view = {
//                         html = {
//                             font_color: (COLOR_DANGER_RED),
//                             draw_normal:      { color: (COLOR_DANGER_RED), }
//                             draw_italic:      { color: (COLOR_DANGER_RED), }
//                             draw_bold:        { color: (COLOR_DANGER_RED), }
//                             draw_bold_italic: { color: (COLOR_DANGER_RED), }
//                             draw_block: {
//                                 code_color: (SEARCH_HIGHLIGHT)
//                             }
//                         }
//                     }
//                 ));
//                 let formatted = format!(
//                     "<b>Server notice:</b> {}\n\n<i>Notice type:</i>: {}{}{}",
//                     sn.body,
//                     sn.server_notice_type.as_str(),
//                     sn.limit_type.as_ref()
//                         .map(|l| format!("\n<i>Limit type:</i> {}", l.as_str()))
//                         .unwrap_or_default(),
//                     sn.admin_contact.as_ref()
//                         .map(|c| format!("\n<i>Admin contact:</i> {}", c))
//                         .unwrap_or_default(),
//                 );
//                 populate_text_message_content(
//                     cx,
//                     &html_or_plaintext_ref,
//                     &sn.body,
//                     Some(&FormattedBody {
//                         format: MessageFormat::Html,
//                         body: formatted,
//                     }),
//                 );
//                 new_drawn_status.content_drawn = true;
//                 (item, false)
//             }
//         }
//         // An emote is just like a message but is prepended with the user's name
//         // to indicate that it's an "action" that the user is performing.
//         MessageOrStickerType::Emote(EmoteMessageEventContent { body, formatted, .. }) => {
//             has_html_body = formatted.as_ref().is_some_and(|f| f.format == MessageFormat::Html);
//             let template = if use_compact_view {
//                 live_id!(CondensedMessage)
//             } else {
//                 live_id!(Message)
//             };
//             let (item, existed) = list.item_with_existed(cx, item_id, template);
//             if existed && item_drawn_status.content_drawn {
//                 (item, true)
//             } else {
//                 // Draw the profile up front here because we need the username for the emote body.
//                 let (username, profile_drawn) = item.avatar(id!(profile.avatar)).set_avatar_and_get_username(
//                     cx,
//                     room_id,
//                     current_event.sender(),
//                     sender_profile,
//                     Some(current_event.event_id()),
//                 );

//                 // Prepend a "* <username> " to the emote body, as suggested by the Matrix spec.
//                 let (body, formatted) = if let Some(fb) = formatted.as_ref() {
//                     (
//                         Cow::from(&fb.body),
//                         Some(FormattedBody {
//                             format: fb.format.clone(),
//                             body: format!("* {} {}", &username, &fb.body),
//                         })
//                     )
//                 } else {
//                     (Cow::from(format!("* {} {}", &username, body)), None)
//                 };
//                 populate_text_message_content(
//                     cx,
//                     &item.html_or_plaintext(id!(content.message)),
//                     &body,
//                     formatted.as_ref(),
//                 );
//                 set_username_and_get_avatar_retval = Some((username, profile_drawn));
//                 new_drawn_status.content_drawn = true;
//                 (item, false)
//             }
//         }
//         // Handle images and sticker messages that are static images.
//         mtype @ MessageOrStickerType::Image(_) | mtype @ MessageOrStickerType::Sticker(_) => {
//             has_html_body = match mtype {
//                 MessageOrStickerType::Image(image) => image.formatted.as_ref()
//                     .is_some_and(|f| f.format == MessageFormat::Html),
//                 _ => false,
//             };
//             let template = if use_compact_view {
//                 live_id!(CondensedImageMessage)
//             } else {
//                 live_id!(ImageMessage)
//             };
//             let (item, existed) = list.item_with_existed(cx, item_id, template);

//             if existed && item_drawn_status.content_drawn {
//                 (item, true)
//             } else {
//                 let image_info = mtype.get_image_info();
//                 let is_image_fully_drawn = populate_image_message_content(
//                     cx,
//                     &item.text_or_image(id!(content.message)),
//                     image_info,
//                     message.body(),
//                     media_cache,
//                 );
//                 new_drawn_status.content_drawn = is_image_fully_drawn;
//                 (item, false)
//             }
//         }
//         MessageOrStickerType::Location(location) => {
//             has_html_body = false;
//             let template = if use_compact_view {
//                 live_id!(CondensedMessage)
//             } else {
//                 live_id!(Message)
//             };
//             let (item, existed) = list.item_with_existed(cx, item_id, template);
//             if existed && item_drawn_status.content_drawn {
//                 (item, true)
//             } else {
//                 let is_location_fully_drawn = populate_location_message_content(
//                     cx,
//                     &item.html_or_plaintext(id!(content.message)),
//                     location,
//                 );
//                 new_drawn_status.content_drawn = is_location_fully_drawn;
//                 (item, false)
//             }
//         }
//         MessageOrStickerType::File(file_content) => {
//             has_html_body = file_content.formatted.as_ref().is_some_and(|f| f.format == MessageFormat::Html);
//             let template = if use_compact_view {
//                 live_id!(CondensedMessage)
//             } else {
//                 live_id!(Message)
//             };
//             let (item, existed) = list.item_with_existed(cx, item_id, template);
//             if existed && item_drawn_status.content_drawn {
//                 (item, true)
//             } else {
//                 new_drawn_status.content_drawn = populate_file_message_content(
//                     cx,
//                     &item.html_or_plaintext(id!(content.message)),
//                     file_content,
//                 );
//                 (item, false)
//             }
//         }
//         MessageOrStickerType::Audio(audio) => {
//             has_html_body = audio.formatted.as_ref().is_some_and(|f| f.format == MessageFormat::Html);
//             let template = if use_compact_view {
//                 live_id!(CondensedMessage)
//             } else {
//                 live_id!(Message)
//             };
//             let (item, existed) = list.item_with_existed(cx, item_id, template);
//             if existed && item_drawn_status.content_drawn {
//                 (item, true)
//             } else {
//                 new_drawn_status.content_drawn = populate_audio_message_content(
//                     cx,
//                     &item.html_or_plaintext(id!(content.message)),
//                     audio,
//                 );
//                 (item, false)
//             }
//         }
//         MessageOrStickerType::Video(video) => {
//             has_html_body = video.formatted.as_ref().is_some_and(|f| f.format == MessageFormat::Html);
//             let template = if use_compact_view {
//                 live_id!(CondensedMessage)
//             } else {
//                 live_id!(Message)
//             };
//             let (item, existed) = list.item_with_existed(cx, item_id, template);
//             if existed && item_drawn_status.content_drawn {
//                 (item, true)
//             } else {
//                 new_drawn_status.content_drawn = populate_video_message_content(
//                     cx,
//                     &item.html_or_plaintext(id!(content.message)),
//                     video,
//                 );
//                 (item, false)
//             }
//         }
//         MessageOrStickerType::VerificationRequest(verification) => {
//             has_html_body = verification.formatted.as_ref().is_some_and(|f| f.format == MessageFormat::Html);
//             let template = live_id!(Message);
//             let (item, existed) = list.item_with_existed(cx, item_id, template);
//             if existed && item_drawn_status.content_drawn {
//                 (item, true)
//             } else {
//                 // Use `FormattedBody` to hold our custom summary of this verification request.
//                 let formatted = FormattedBody {
//                     format: MessageFormat::Html,
//                     body: format!(
//                         "<i>Sent a <b>verification request</b> to {}.<br>(Supported methods: {})</i>",
//                         verification.to,
//                         verification.methods
//                             .iter()
//                             .map(|m| m.as_str())
//                             .collect::<Vec<_>>()
//                             .join(", "),
//                     ),
//                 };

//                 populate_text_message_content(
//                     cx,
//                     &item.html_or_plaintext(id!(content.message)),
//                     &verification.body,
//                     Some(&formatted),
//                 );
//                 new_drawn_status.content_drawn = true;
//                 (item, false)
//             }
//         }
//         other => {
//             has_html_body = false;
//             let (item, existed) = list.item_with_existed(cx, item_id, live_id!(Message));
//             if existed && item_drawn_status.content_drawn {
//                 (item, true)
//             } else {
//                 let kind = other.as_str();
//                 item.label(id!(content.message)).set_text(
//                     cx,
//                     &format!("[Unsupported ({kind})] {}", message.body()),
//                 );
//                 new_drawn_status.content_drawn = true;
//                 (item, false)
//             }
//         }
//     };

//     // If `used_cached_item` is false, we should always redraw the profile, even if profile_drawn is true.
//     let skip_draw_profile =
//         use_compact_view || (used_cached_item && item_drawn_status.profile_drawn);
//     if skip_draw_profile {
//         // log!("\t --> populate_message_view(): SKIPPING profile draw for item_id: {item_id}");
//         new_drawn_status.profile_drawn = true;
//     } else {
//         // log!("\t --> populate_message_view(): DRAWING  profile draw for item_id: {item_id}");
//         let username_label = item.label(id!(content.username));

//         if !is_server_notice { // the normal case
//             let (username, profile_drawn) = set_username_and_get_avatar_retval.unwrap_or_else(||
//                 item.avatar(id!(profile.avatar)).set_avatar_and_get_username(
//                     cx,
//                     room_id,
//                     current_event.sender(),
//                     sender_profile,
//                     Some(current_event.event_id()),
//                 )
//             );
//             if is_notice {
//                 username_label.apply_over(cx, live!(
//                     draw_text: {
//                         color: (MESSAGE_NOTICE_TEXT_COLOR),
//                     }
//                 ));
//             }
//             username_label.set_text(cx, &username);
//             new_drawn_status.profile_drawn = profile_drawn;
//         }
//         else {
//             // Server notices are drawn with a red color avatar background and username.
//             let avatar = item.avatar(id!(profile.avatar));
//             avatar.show_text(cx, None, "⚠");
//             avatar.apply_over(cx, live!(
//                 text_view = {
//                     draw_bg: { background_color: (COLOR_DANGER_RED), }
//                 }
//             ));
//             username_label.set_text(cx, "Server notice");
//             username_label.apply_over(cx, live!(
//                 draw_text: {
//                     color: (COLOR_DANGER_RED),
//                 }
//             ));
//             new_drawn_status.profile_drawn = true;
//         }
//     }

//     // If we've previously drawn the item content, skip all other steps.
//     if used_cached_item && item_drawn_status.content_drawn && item_drawn_status.profile_drawn {
//         return (item, new_drawn_status);
//     }
    
//     // Set the Message widget's metadata for reply-handling purposes.
//     item.as_message().set_data(MessageDetails {
//         event_id: Some(current_event.event_id().to_owned()),
//         item_id,
//         related_event_id: in_reply_to.map(|f| f.event_id),
//         room_screen_widget_uid,
//         abilities: MessageAbilities::from_user_power_and_any_event(
//             user_power_levels,
//             current_event,
//             &message,
//             has_html_body,
//         ),
//         should_be_highlighted: false
//     });

//     // Set the timestamp.
//     if let Some(dt) = unix_time_millis_to_datetime(&ts_millis) {
//         // format as AM/PM 12-hour time
//         item.label(id!(profile.timestamp))
//             .set_text(cx, &format!("{}", dt.time().format("%l:%M %P")));
//         if !use_compact_view {
//             item.label(id!(profile.datestamp))
//                 .set_text(cx, &format!("{}", dt.date_naive()));
//         }
//     } else {
//         item.label(id!(profile.timestamp))
//             .set_text(cx, &format!("{}", ts_millis.get()));
//     }
//     if is_contextual {
//         item.view(id!(overlay_message)).set_visible(cx, true);
//     }
//     (item, new_drawn_status)
// }

// /// Abstracts over a message or sticker that can be displayed in a timeline.
// pub enum MessageOrSticker<'e> {
//     //Message(&'e timeline::Message),
//     Message(&'e RoomMessageEventContent),
//     Sticker(&'e StickerEventContent),
// }
// impl MessageOrSticker<'_> {
//     /// Returns the type of this message or sticker.
//     pub fn get_type(&self) -> MessageOrStickerType {
        
//         match self {
//             Self::Message(msg) => match &msg.msgtype {
//                 MessageType::Audio(audio) => MessageOrStickerType::Audio(audio),
//                 MessageType::Emote(emote) => MessageOrStickerType::Emote(emote),
//                 MessageType::File(file) => MessageOrStickerType::File(file),
//                 MessageType::Image(image) => MessageOrStickerType::Image(image),
//                 MessageType::Location(location) => MessageOrStickerType::Location(location),
//                 MessageType::Notice(notice) => MessageOrStickerType::Notice(notice),
//                 MessageType::ServerNotice(server_notice) => MessageOrStickerType::ServerNotice(server_notice),
//                 MessageType::Text(text) => MessageOrStickerType::Text(text),
//                 MessageType::Video(video) => MessageOrStickerType::Video(video),
//                 MessageType::VerificationRequest(verification_request) => MessageOrStickerType::VerificationRequest(verification_request),
//                 MessageType::_Custom(custom) => MessageOrStickerType::_Custom(custom),
//                 _ => MessageOrStickerType::Unknown,
//             },
//             Self::Sticker(sticker) => MessageOrStickerType::Sticker(sticker),
//         }
//     }

//     /// Returns the body of this message or sticker, which is a text representation of its content.
//     pub fn body(&self) -> &str {
//         match self {
//             Self::Message(msg) => msg.body(),
//             Self::Sticker(sticker) => sticker.body.as_str(),
//         }
//     }
//     pub fn in_reply_to(&self) -> Option<InReplyTo> {
//         match self {
//             Self::Message(msg) => msg.relates_to.as_ref().and_then(|f|{
//                 match f {
//                     Relation::Reply{
//                         in_reply_to
//                     } => {
//                         Some(in_reply_to.clone())
//                     },
//                     _ => None
//                 }
//             }),
//             Self::Sticker(_) => None,
//         }
//     }
// }
pub trait SmallStateEventContent {
    /// Populates the *content* (not the profile) of the given `item` with data from
    /// the given `event_tl_item` and `self` (the specific type of event content).
    ///
    /// ## Arguments
    /// * `item`: a `SmallStateEvent` widget that has already been added to
    ///   the given `PortalList` at the given `item_id`.
    ///   This function may either modify that item or completely replace it
    ///   with a different widget if needed.
    /// * `item_drawn_status`: the old (prior) drawn status of the item.
    /// * `new_drawn_status`: the new drawn status of the item, which may have already
    ///   been updated to reflect the item's profile having been drawn right before this function.
    ///
    /// ## Return
    /// Returns a tuple of the drawn `item` and its `new_drawn_status`.
    fn populate_item_content(
        &self,
        cx: &mut Cx,
        list: &mut PortalList,
        item_id: usize,
        item: WidgetRef,
        event_tl_item: &AnyTimelineEvent,
        username: &str,
        item_drawn_status: ItemDrawnStatus,
        new_drawn_status: ItemDrawnStatus,
        state_key: &str,
    ) -> (WidgetRef, ItemDrawnStatus);
}
impl SmallStateEventContent for AnyStateEventContentWrapper {
    fn populate_item_content(
        &self,
        cx: &mut Cx,
        list: &mut PortalList,
        item_id: usize,
        item: WidgetRef,
        _event_tl_item: &AnyTimelineEvent,
        username: &str,
        _item_drawn_status: ItemDrawnStatus,
        mut new_drawn_status: ItemDrawnStatus,
        state_key: &str,
    ) -> (WidgetRef, ItemDrawnStatus) {
        let Some(other_state) = self.into() else { return (list.item(cx, item_id, live_id!(Empty)), ItemDrawnStatus::new()) };
        let item = if let Some(text_preview) = text_preview_of_other_state_new(other_state, state_key) {
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
fn populate_small_state_event2(
    cx: &mut Cx,
    list: &mut PortalList,
    item_id: usize,
    room_id: &OwnedRoomId,
    event_tl_item: &AnyTimelineEvent,
    event_content: &impl SmallStateEventContent,
    item_drawn_status: ItemDrawnStatus,
    state_key: &str,
) -> (WidgetRef, ItemDrawnStatus) {
    let mut new_drawn_status = item_drawn_status;
    let (item, existed) = list.item_with_existed(cx, item_id, live_id!(SmallStateEvent));
    // The content of a small state event view may depend on the profile info,
    // so we can only mark the content as drawn after the profile has been fully drawn and cached.
    let skip_redrawing_profile = existed && item_drawn_status.profile_drawn;
    let skip_redrawing_content = skip_redrawing_profile && item_drawn_status.content_drawn;
    if skip_redrawing_content {
        return (item, new_drawn_status);
    }
    // If the profile has been drawn, we can just quickly grab the user's display name
    // instead of having to call `set_avatar_and_get_username` again.
    let username_opt = skip_redrawing_profile
        .then(|| None)
        .flatten();

    let username = username_opt.unwrap_or_else(|| {
        // As a fallback, call `set_avatar_and_get_username` to get the user's display name.
        let avatar_ref = item.avatar(id!(avatar));
        let (username, profile_drawn) = avatar_ref.set_avatar_and_get_username(
            cx,
            room_id,
            event_tl_item.sender(),
            None,
            Some(event_tl_item.event_id()),
        );
        // Draw the timestamp as part of the profile.
        set_timestamp(
            cx,
            &item,
            id!(left_container.timestamp),
            event_tl_item.origin_server_ts(),
        );
        new_drawn_status.profile_drawn = profile_drawn;
        username
    });

    // Proceed to draw the actual event content.
    event_content.populate_item_content(
        cx,
        list,
        item_id,
        item,
        event_tl_item,
        &username,
        item_drawn_status,
        new_drawn_status,
        state_key,
    )
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
                        .and_then(|f| match f.kind { SearchTimelineItemKind::ContextEvent(ref e)=> Some(e),
                            SearchTimelineItemKind::Event(ref e) => Some(e), 
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
                                            if !is_contextual {
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
                                    let wrapper = AnyStateEventContentWrapper(content);
                                    populate_small_state_event(
                                        cx,
                                        list,
                                        item_id,
                                        room_id,
                                        event,
                                        &wrapper,
                                        item_drawn_status,
                                        state_key,
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
    RoomHeader(String)
}

/// Actions related to a specific message within a room timeline.
#[derive(Clone, DefaultNone, Debug)]
pub enum SearchResultAction {
    /// Search result's length and the search criteria
    Success(usize, String),
    Pending,
    Close,
    None
}


pub struct AnyStateEventContentWrapper(AnyStateEventContent);

impl Into<Option<AnyOtherFullStateEventContent>> for &AnyStateEventContentWrapper {
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
    fn sender_profile(&self) -> &TimelineDetails<matrix_sdk_ui::timeline::Profile> {
        &TimelineDetails::Unavailable
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
}


pub struct PreviousWrapperAEI<'a>(pub &'a AnyTimelineEvent);
impl <'a> PreviousEventable for PreviousWrapperAEI<'a> {
    fn kind(&self) -> &TimelineItemKind {
        &TimelineItemKind::Virtual(VirtualTimelineItem::ReadMarker)
    }
}
