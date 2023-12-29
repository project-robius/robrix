//! A room screen is the UI page that displays a single Room's timeline of events/messages
//! along with a message input bar at the bottom.

use std::{ops::DerefMut, sync::Arc};

use eyeball_im::VectorDiff;
use imbl::Vector;
use makepad_widgets::*;
use matrix_sdk::ruma::{
    MilliSecondsSinceUnixEpoch,
    events::{
        AnySyncTimelineEvent,
        AnySyncMessageLikeEvent,
        FullStateEventContent,
        room::{
            guest_access::GuestAccess,
            history_visibility::HistoryVisibility,
            join_rules::JoinRule,
        },
        SyncMessageLikeEvent,
    },
    OwnedRoomId,
};
use matrix_sdk_ui::timeline::{
    self,
    AnyOtherFullStateEventContent,
    EventTimelineItem,
    MembershipChange,
    MemberProfileChange,
    RoomMembershipChange,
    VirtualTimelineItem,
    TimelineDetails,
    TimelineItemContent,
    TimelineItemKind, TimelineItem,
};

use crate::{
    sliding_sync::{get_timeline_items, submit_async_request, MatrixRequest, take_timeline_update_receiver},
    utils::unix_time_millis_to_datetime,
};

live_design! {
    import makepad_draw::shader::std::*;
    import makepad_widgets::base::*;
    import makepad_widgets::theme_desktop_dark::*;

    import crate::shared::styles::*;
    import crate::shared::helpers::*;
    import crate::shared::search_bar::SearchBar;

    IMG_PROFILE_A = dep("crate://self/resources/profile_1.jpg")
    ICO_FAV = dep("crate://self/resources/icon_favorite.svg")
    ICO_COMMENT = dep("crate://self/resources/icon_comment.svg")
    ICO_REPLY = dep("crate://self/resources/icon_reply.svg")
    ICO_LIKES = dep("crate://self/resources/icon_likes.svg")
    ICO_USER = dep("crate://self/resources/icon_user.svg")
    ICO_ADD = dep("crate://self/resources/icon_add.svg")

    FONT_SIZE_SUB = 9.5
    FONT_SIZE_P = 12.5
    
    TEXT_SUB = {
        font_size: (FONT_SIZE_SUB),
        font: {path: dep("crate://makepad-widgets/resources/GoNotoKurrent-Regular.ttf")}
    }
    
    TEXT_P = {
        font_size: (FONT_SIZE_P),
        height_factor: 1.65,
        font: {path: dep("crate://makepad-widgets/resources/GoNotoKurrent-Regular.ttf")}
    }
    
    COLOR_BG = #xfff8ee
    COLOR_BRAND = #xf88
    COLOR_BRAND_HOVER = #xf66
    COLOR_META_TEXT = #xaaa
    COLOR_META = #xccc
    COLOR_META_INV = #xfffa
    COLOR_OVERLAY_BG = #x000000d8
    COLOR_DIVIDER = #x00000018
    COLOR_DIVIDER_DARK = #x00000044
    COLOR_PROFILE_CIRCLE = #xfff8ee
    COLOR_P = #x999
    
    FillerY = <View> {width: Fill}
    
    FillerX = <View> {height: Fill}
    
    
    IconButton = <Button> {
        draw_text: {
            instance hover: 0.0
            instance pressed: 0.0
            text_style: {
                font_size: 11.0
            }
            fn get_color(self) -> vec4 {
                return mix(
                    mix(
                        (COLOR_META_TEXT),
                        (COLOR_BRAND),
                        self.hover
                    ),
                    (COLOR_BRAND_HOVER),
                    self.pressed
                )
            }
        }
        draw_icon: {
            svg_file: (ICO_FAV),
            fn get_color(self) -> vec4 {
                return mix(
                    mix(
                        (COLOR_META),
                        (COLOR_BRAND),
                        self.hover
                    ),
                    (COLOR_BRAND_HOVER),
                    self.pressed
                )
            }
        }
        icon_walk: {width: 7.5, height: Fit, margin: {left: 5.0}}
        draw_bg: {
            fn pixel(self) -> vec4 {
                let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                return sdf.result
            }
        }
        padding: 9.0
        text: "1"
    }
    
    LineH = <RoundedView> {
        width: Fill,
        height: 2,
        margin: 0.0
        padding: 0.0,
        spacing: 0.0
        draw_bg: {color: (COLOR_DIVIDER)}
    }

    Timestamp = <Label> {
        padding: { top: 10.0, bottom: 0.0, left: 0.0, right: 0.0 }
        draw_text: {
            text_style: <TEXT_SUB> {},
            color: (COLOR_META_TEXT)
        }
        text: " "
    }
    
    MessageMenu = <View> {
        width: Fill,
        height: Fit,
        margin: 0.0
        flow: Down,
        padding: 0.0,
        spacing: 0.0
        
        <View> {
            width: Fill,
            height: Fit,
            margin: 0.0
            flow: Right,
            padding: 0.0,
            spacing: 10.0
            
            likes = <IconButton> {draw_icon: {svg_file: (ICO_FAV)} icon_walk: {width: 15.0, height: Fit}}
            comments = <IconButton> {draw_icon: {svg_file: (ICO_COMMENT)} icon_walk: {width: 15.0, height: Fit}, text: "7"}
            <FillerX> {}
            reply = <IconButton> {draw_icon: {svg_file: (ICO_REPLY)} icon_walk: {width: 15.0, height: Fit}, text: ""}
        }
    }
    
    // An empty view that takes up no space in the portal list.
    Empty = <View> { }

    // The view used for each text-based message event in a room's timeline.
    Message = <View> {
        width: Fill,
        height: Fit,
        margin: 0.0
        flow: Down,
        padding: 0.0,
        spacing: 0.0
        
        body = <View> {
            width: Fill,
            height: Fit
            flow: Right,
            padding: 10.0,
            spacing: 10.0
            
            profile = <View> {
                align: {x: 0.5, y: 0.0} // centered horizontally, top aligned
                width: 65.0,
                height: Fit,
                margin: {top: 7.5}
                flow: Down,
                profile_img = <Image> {
                    source: (IMG_PROFILE_A)
                    margin: 0,
                    width: 50.,
                    height: 50.
                    draw_bg: {
                        fn pixel(self) -> vec4 {
                            let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                            let c = self.rect_size * 0.5;
                            sdf.circle(c.x, c.y, c.x - 2.)
                            sdf.fill_keep(self.get_color());
                            sdf.stroke((COLOR_PROFILE_CIRCLE), 1);
                            return sdf.result
                        }
                    }
                }
                timestamp = <Timestamp> { }
                datestamp = <Timestamp> {
                    padding: { top: 5.0 }
                }
            }
            content = <View> {
                width: Fill,
                height: Fit
                flow: Down,
                padding: 0.0
                
                username = <Label> {
                    margin: {bottom: 10.0, top: 10.0}
                    draw_text: {
                        text_style: <TEXT_SUB> {},
                        color: (COLOR_META_TEXT)
                    }
                    text: "<username>"
                }
                message = <Label> {
                    width: Fill,
                    height: Fit
                    draw_text: {
                        wrap: Word,
                        text_style: <TEXT_P> {},
                        color: (COLOR_P)
                    }
                    text: ""
                }
                
                <LineH> {
                    margin: {top: 13.0, bottom: 5.0}
                }
                
                <MessageMenu> {}
            }
        }
    }


    // The view used for each state event in a room's timeline.
    // The timestamp, profile picture, and text are all very small.
    SmallStateEvent = <View> {
        width: Fill,
        height: Fit,
        margin: 0.0
        flow: Right,
        padding: 0.0,
        spacing: 0.0
        
        body = <View> {
            width: Fill,
            height: Fit
            flow: Right,
            padding: { top: 2.0, bottom: 2.0 }
            spacing: 5.0
            
            left_container = <View> {
                align: {x: 0.5, y: 0.0} // centered horizontally, top aligned
                width: 70.0,
                // padding: {right: -5.0}
                height: Fit
                flow: Right,

                timestamp = <Timestamp> {
                    padding: {top: 5.0}
                    draw_text: {
                        text_style: <TEXT_SUB> {},
                        color: (COLOR_META_TEXT)
                    }
                }
            }

            profile_img = <Image> {
                source: (IMG_PROFILE_A)
                width: 19.0,
                height: 19.0,
                draw_bg: {
                    fn pixel(self) -> vec4 {
                        let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                        let c = self.rect_size * 0.5;
                        sdf.circle(c.x, c.y, c.x - 2.)
                        sdf.fill_keep(self.get_color());
                        sdf.stroke((COLOR_PROFILE_CIRCLE), 1);
                        return sdf.result
                    }
                }
            }

            content = <Label> {
                width: Fill,
                height: Fit
                padding: 4.0,
                draw_text: {
                    wrap: Word,
                    text_style: <TEXT_SUB> {},
                    color: (COLOR_P)
                }
                text: "<placeholder room state event>"
            }
        }
    }


    // The view used for each day divider in a room's timeline.
    // The date text is centered between two horizontal lines.
    DayDivider = <View> {
        width: Fill,
        height: Fit,
        margin: 0.0,
        flow: Right,
        padding: 0.0,
        spacing: 0.0,
        align: {x: 0.5, y: 0.5} // center horizontally and vertically

        left_line = <LineH> {
            margin: {top: 10.0, bottom: 10.0}
            draw_bg: {color: (COLOR_DIVIDER_DARK)}
        }

        date = <Label> {
            padding: {left: 7.0, right: 7.0}
            margin: {bottom: 10.0, top: 10.0}
            draw_text: {
                text_style: <TEXT_SUB> {},
                color: (COLOR_DIVIDER_DARK)
            }
            text: "<date>"
        }

        right_line = <LineH> {
            margin: {top: 10.0, bottom: 10.0}
            draw_bg: {color: (COLOR_DIVIDER_DARK)}
        }
    }

    // The top space is used to display a loading animation while the room is being paginated.
    TopSpace = <View> {
        width: Fill,
        height: 0.0,

        label = <Label> {
            text: "Loading..."
        }
    }

    Timeline = {{Timeline}} {
        width: Fill,
        height: Fill,
        align: {x: 0.5, y: 0.0} // center horizontally, align to top vertically

        list = <PortalList> {
            auto_tail: false, // set to `true` to lock the view to the last item.
            height: Fill,
            width: Fill
            flow: Down
    
            // Below, we must place all of the possible views that can be used in the portal list.
            TopSpace = <TopSpace> {}
            Message = <Message> {}
            SmallStateEvent = <SmallStateEvent> {}
            Empty = <Empty> {}
            DayDivider = <DayDivider> {}
            BottomSpace = <View> {height: 80}
        }    
    }


    IMG_DEFAULT_AVATAR = dep("crate://self/resources/img/default_avatar.png")
    IMG_SMILEY_FACE_BW = dep("crate://self/resources/img/smiley_face_bw.png")
    IMG_PLUS = dep("crate://self/resources/img/plus.png")
    IMG_KEYBOARD_ICON = dep("crate://self/resources/img/keyboard_icon.png")

    RoomScreen = <KeyboardView> {
        width: Fill, height: Fill
        flow: Down
        show_bg: true,
        draw_bg: {
            color: #fff
        }

        // First, display the timeline of all messages/events.
        timeline = <Timeline> {}
        
        // Below that, display a view that holds the message input bar.
        <View> {
            width: Fill, height: Fit
            flow: Right, align: {y: 0.5}, padding: 10.
            show_bg: true,
            draw_bg: {
                color: #fff
            }

            <Image> {
                source: (IMG_KEYBOARD_ICON),
                width: 36., height: 36.
            }
            message_input = <SearchBar> {
                show_bg: false
                input = {
                    width: Fill, height: Fit, margin: 0
                    empty_message: " "
                    draw_text:{
                        text_style:<REGULAR_TEXT>{font_size: 11},

                        fn get_color(self) -> vec4 {
                            return #0
                        }
                    }
                }
            }
            <Image> {
                source: (IMG_SMILEY_FACE_BW),
                width: 36., height: 36.
            }
            <Image> {
                source: (IMG_PLUS),
                width: 36., height: 36.
            }
        }
    }
}


#[derive(Live, Widget)]
pub struct Timeline {
    #[deref] view: View,

    // TODO: figure out how to remove the option whilst deriving `Live`.
    #[rust] room_id: Option<OwnedRoomId>,
    /// The index in [`RoomsList::all_rooms`] where this room is located.
    #[rust] room_index: usize,

    /// Whether this room's timeline has been fully paginated, which means
    /// that the oldest (first) event in the timeline is locally synced and available.
    /// When `true`, further backwards pagination requests will not be sent.
    #[rust] fully_paginated: bool,

    /// The list of currently-known items in this room's timeline,
    /// which doesn't necessarily contain all timeline items as known by the server.
    ///
    /// This list is only directly accessed by the UI thread.
    /// An async background task receives updates for this timeline
    /// and then enqueues these updates in a room-specific queue,
    /// which the UI task then dequeues and applies to this list
    /// *after* the UI task has finished drawing the current frame.
    #[rust] items: Vector<Arc<TimelineItem>>,

    /// The channel receiver for timeline updates for this room.
    /// Here we use a synchronous (non-async) channel because the receiver runs
    /// in a sync context and the sender runs in an async context,
    /// which is okay because a sender on an unbounded channel never needs to block.
    #[rust] update_receiver: Option<crossbeam_channel::Receiver<Vec<VectorDiff<Arc<TimelineItem>>>>>,
}

// This struct is auto-generated by deriving `Widget` on `Timeline`.
impl TimelineRef {
    pub fn set_room_info(&self, room_index: usize, room_id: OwnedRoomId) {
        if let Some(mut timeline) = self.borrow_mut() {
            timeline.room_id = Some(room_id.clone());
            timeline.room_index = room_index;

            if let Some((items, receiver)) = take_timeline_update_receiver(&room_id) {
                timeline.items = items;
                timeline.update_receiver = Some(receiver);
            }

            // kick off a back pagination request for this room
            if !timeline.fully_paginated {
                submit_async_request(MatrixRequest::PaginateRoomTimeline {
                    room_id,
                    batch_size: 50,
                    max_events: 50,
                })
            }
        }
    }
}

impl LiveHook for Timeline {
    fn after_new_from_doc(&mut self, _cx: &mut Cx) {
        println!("@@@@ Timeline::after_new_from_doc()");
    }
}

impl Widget for Timeline {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope)

        // TODO: handle actions upon an item being clicked.
        // for (item_id, item) in self.list.items_with_actions(&actions) {
        //     if item.button(id!(likes)).clicked(&actions) {
        //         log!("hello {}", item_id);
        //     }
        // }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        // Determing length of the portal list based on the number of timeline items.
        let timeline_items_owned = self.room_id.as_ref().and_then(|r| get_timeline_items(r));
        let timeline_items = timeline_items_owned.as_ref();
        let last_item_id = timeline_items
            .map(|(_tl, items)| items.len() as u64)
            .unwrap_or(0);
        let last_item_id = last_item_id + 1; // Add 1 for the TopSpace.
        
        // Start the actual drawing procedure.
        while let Some(list_item) = self.view.draw_walk(cx, scope, walk).step() {
            // We only care about drawing the portal list.
            let portal_list_ref = list_item.as_portal_list();
            let Some(mut list_ref) = portal_list_ref.borrow_mut() else { continue };
            let list = list_ref.deref_mut();
        
            // + 1 again because we use the last item for the BottomSpace.
            list.set_item_range(cx, 0, last_item_id + 1);

            while let Some(item_id) = list.next_visible_item(cx) {
                // println!("Drawing item {}", item_id);
                let item = if item_id == 0 {
                    list.item(cx, item_id, live_id!(TopSpace)).unwrap()
                } else if item_id >= last_item_id {
                    list.item(cx, item_id, live_id!(BottomSpace)).unwrap()
                } else {
                    let tl_idx = (item_id - 1) as usize;
                    let Some(timeline_item) = timeline_items.and_then(|(_tl, items)| items.get(tl_idx)) else {
                        // This shouldn't happen (unless the timeline gets corrupted or some other weird error),
                        // but we can always safely fill the item with an empty widget that takes up no space.
                        list.item(cx, item_id, live_id!(Empty)).unwrap();
                        continue;
                    };
                    match timeline_item.kind() {
                        TimelineItemKind::Event(event_tl_item) => {
                            // Choose to draw either a Message or SmallStateEvent based on the timeline event's content.
                            match event_tl_item.content() {
                                TimelineItemContent::Message(message) => populate_message_view(
                                    cx,
                                    list,
                                    item_id,
                                    event_tl_item,
                                    message,
                                ),
                                TimelineItemContent::RedactedMessage => populate_redacted_message_view(
                                    cx,
                                    list,
                                    item_id,
                                    event_tl_item,
                                    self.room_id.as_ref().unwrap(), // room must exist at this point
                                ),
                                TimelineItemContent::MembershipChange(membership_change) => populate_membership_change_view(
                                    cx,
                                    list,
                                    item_id,
                                    event_tl_item,
                                    membership_change,
                                ),
                                TimelineItemContent::ProfileChange(profile_change) => populate_profile_change_view(
                                    cx,
                                    list,
                                    item_id,
                                    event_tl_item,
                                    profile_change,
                                ),
                                TimelineItemContent::OtherState(other) => populate_other_state_view(
                                    cx,
                                    list,
                                    item_id,
                                    event_tl_item,
                                    other,
                                ),
                                unhandled => {
                                    let item = list.item(cx, item_id, live_id!(SmallStateEvent)).unwrap();
                                    item.label(id!(content)).set_text(&format!("[TODO] {:?}", unhandled));
                                    item
                                }
                            }
                        }
                        TimelineItemKind::Virtual(VirtualTimelineItem::DayDivider(millis)) => {
                            let item = list.item(cx, item_id, live_id!(DayDivider)).unwrap();
                            let text = unix_time_millis_to_datetime(millis)
                                // format the time as a shortened date (Sat, Sept 5, 2021)
                                .map(|dt| format!("{}", dt.date().format("%a %b %-d, %Y")))
                                .unwrap_or_else(|| format!("{:?}", millis));
                            item.label(id!(date)).set_text(&text);
                            item
                        }
                        TimelineItemKind::Virtual(VirtualTimelineItem::ReadMarker) => {
                            // reuse the DayDivider view for user read markers.
                            let item = list.item(cx, item_id, live_id!(DayDivider)).unwrap();
                            item.label(id!(date)).set_text(&format!("Read marker, {}", timeline_item.unique_id()));
                            item
                        }
                    }
                };
                item.draw_all(cx, &mut Scope::empty());
            }
        }
        DrawStep::done()
    }
}


/// Creates, populates, and adds a Message liveview widget to the given `PortalList`
/// with the given `item_id`.
///
/// The content of the returned `Message` widget is populated with data from the given `message`
/// and its parent `EventTimelineItem`.
fn populate_message_view(
    cx: &mut Cx,
    list: &mut PortalList,
    item_id: u64,
    event_tl_item: &EventTimelineItem,
    message: &timeline::Message,
) -> WidgetRef {
    let item = list.item(cx, item_id, live_id!(Message)).unwrap();
    item.label(id!(content.message)).set_text(message.body());

    // Set sender to the display name if available, otherwise the user id.
    let sender = match event_tl_item.sender_profile() {
        TimelineDetails::Ready(profile) => profile.display_name.as_deref(),
        _ => None,
    }.unwrap_or_else(|| event_tl_item.sender().as_str());
    item.label(id!(content.username)).set_text(sender);

    // Set the timestamp.
    let ts_millis = event_tl_item.timestamp();
    if let Some(dt) = unix_time_millis_to_datetime(&ts_millis) {
        // format as AM/PM 12-hour time
        item.label(id!(profile.timestamp)).set_text(
            &format!("{}", dt.time().format("%l:%M %P"))
        );
        item.label(id!(profile.datestamp)).set_text(
            &format!("{}", dt.date())
        );
    } else {
        item.label(id!(profile.timestamp)).set_text(
            &format!("{}", ts_millis.get())
        );
    }

    // Temp filler: set the likes and comments count to the item id, just for now.
    item.button(id!(likes)).set_text(&format!("{item_id}"));
    item.button(id!(comments)).set_text(&format!("{item_id}"));

    item
} 




/// Creates, populates, and adds a `SmallStateEvent` liveview widget to the given `PortalList`
/// with the given `item_id`.
///
/// The content of the returned widget is populated with metadata about the redacted message
/// the corresponds to the given `EventTimelineItem`.
fn populate_redacted_message_view(
    cx: &mut Cx,
    list: &mut PortalList,
    item_id: u64,
    event_tl_item: &EventTimelineItem,
    _room_id: &OwnedRoomId
) -> WidgetRef {
    let item = list.item(cx, item_id, live_id!(SmallStateEvent)).unwrap();
    let redactor_and_reason = if let Some(redacted_msg) = event_tl_item.latest_json() {
        if let Ok(old) = redacted_msg.deserialize() {
            match old {
                AnySyncTimelineEvent::MessageLike(AnySyncMessageLikeEvent::RoomMessage(SyncMessageLikeEvent::Redacted(redaction))) => {
                    Some((
                        redaction.unsigned.redacted_because.sender,
                        redaction.unsigned.redacted_because.content.reason,
                    ))
                }
                _ => None,
            }
        } else { None }
    } else { None };

    set_timestamp(&item, id!(left_container.timestamp), event_tl_item.timestamp());
    
    // Get the display name (or user ID) of the original sender of the now-redacted message.
    let original_sender = match event_tl_item.sender_profile() {
        TimelineDetails::Ready(profile) => profile.display_name.as_deref(),
        _ => None,
    }.unwrap_or_else(|| event_tl_item.sender().as_str());
    let text = match redactor_and_reason {
        Some((redactor, Some(reason))) => {
            format!("{} deleted {}'s message: {:?}.", redactor, original_sender, reason)
        }
        Some((redactor, None)) => {
            format!("{} deleted {}'s message.", redactor, original_sender)
        }
        None => {
            format!("{}'s message was deleted.", original_sender)
        }
    };
    item.label(id!(content)).set_text(&text);
    item
} 


/// Creates, populates, and adds a SmallStateEvent liveview widget to the given `PortalList`
/// with the given `item_id`.
///
/// The content of the returned widget is populated with data from the
/// given room membership change and its parent `EventTimelineItem`.
fn populate_membership_change_view(
    cx: &mut Cx,
    list: &mut PortalList,
    item_id: u64,
    event_tl_item: &EventTimelineItem,
    change: &RoomMembershipChange,
) -> WidgetRef {
    let item = list.item(cx, item_id, live_id!(SmallStateEvent)).unwrap();

    let text = match change.change() {
        None 
        | Some(MembershipChange::NotImplemented)
        | Some(MembershipChange::None) => {
            // Don't actually display anything for nonexistent/unimportant membership changes.
            return list.item(cx, item_id, live_id!(Empty)).unwrap();
        }
        Some(MembershipChange::Error) =>
            format!("{} had a membership change error.", event_tl_item.sender()),
        Some(MembershipChange::Joined) =>
            format!("{} joined this room.", event_tl_item.sender()),
        Some(MembershipChange::Left) =>
            format!("{} left this room.", event_tl_item.sender()),
        Some(MembershipChange::Banned) =>
            format!("{} banned {} from this room.", event_tl_item.sender(), change.user_id()),
        Some(MembershipChange::Unbanned) =>
            format!("{} unbanned {} from this room.", event_tl_item.sender(), change.user_id()),
        Some(MembershipChange::Kicked) =>
            format!("{} kicked {} from this room.", event_tl_item.sender(), change.user_id()),
        Some(MembershipChange::Invited) =>
            format!("{} invited {} to this room.", event_tl_item.sender(), change.user_id()),
        Some(MembershipChange::KickedAndBanned) =>
            format!("{} kicked and banned {} from this room.", event_tl_item.sender(), change.user_id()),
        Some(MembershipChange::InvitationAccepted) =>
            format!("{} accepted an invitation to this room.", event_tl_item.sender()),
        Some(MembershipChange::InvitationRejected) =>
            format!("{} rejected an invitation to this room.", event_tl_item.sender()),
        Some(MembershipChange::InvitationRevoked) =>
            format!("{} revoked {}'s invitation to this room.", event_tl_item.sender(), change.user_id()),
        Some(MembershipChange::Knocked) =>
            format!("{} requested to join this room.", event_tl_item.sender()),
        Some(MembershipChange::KnockAccepted) =>
            format!("{} accepted {}'s request to join this room.", event_tl_item.sender(), change.user_id()),
        Some(MembershipChange::KnockRetracted) =>
            format!("{} retracted their request to join this room.", event_tl_item.sender()),
        Some(MembershipChange::KnockDenied) =>
            format!("{} denied {}'s request to join this room.", event_tl_item.sender(), change.user_id()),
    };

    set_timestamp(&item, id!(left_container.timestamp), event_tl_item.timestamp());
    item.label(id!(content)).set_text(&text);
    item
}



/// Creates, populates, and adds a SmallStateEvent liveview widget to the given `PortalList`
/// with the given `item_id`.
///
/// The content of the returned `SmallStateEvent` widget is populated with data from the
/// given member profile change and its parent `EventTimelineItem`.
fn populate_profile_change_view(
    cx: &mut Cx,
    list: &mut PortalList,
    item_id: u64,
    event_tl_item: &EventTimelineItem,
    change: &MemberProfileChange,
) -> WidgetRef {
    let item = list.item(cx, item_id, live_id!(SmallStateEvent)).unwrap();

    let name_text = if let Some(name_change) = change.displayname_change() {
        let old = name_change.old.as_deref().unwrap_or(event_tl_item.sender().as_str());
        let new = name_change.new.as_deref().unwrap_or("");
        format!("{old} changed their display name to {new:?}")
    } else {
        String::new()
    };

    let avatar_text = if let Some(_avatar_change) = change.avatar_url_change() {
        if name_text.is_empty() {
            format!("{} changed their profile picture.", event_tl_item.sender().as_str())
        } else {
            format!(" and changed their profile picture.")
        }
        // TODO: handle actual avatar URI change.
    } else {
        String::from(".")
    };

    set_timestamp(&item, id!(left_container.timestamp), event_tl_item.timestamp());
    item.label(id!(content)).set_text(&format!("{}{}", name_text, avatar_text));
    item
}



/// Creates, populates, and adds a SmallStateEvent liveview widget to the given `PortalList`
/// with the given `item_id`.
///
/// The content of the returned `SmallStateEvent` widget is populated with data from the given `message`
/// and its parent `EventTimelineItem`.
fn populate_other_state_view(
    cx: &mut Cx,
    list: &mut PortalList,
    item_id: u64,
    event_tl_item: &EventTimelineItem,
    other_state: &timeline::OtherState,
) -> WidgetRef {  
    let text = match other_state.content() {
        AnyOtherFullStateEventContent::RoomAliases(FullStateEventContent::Original { content, .. }) => {
            let mut s = format!("set this room's aliases to ");
            for alias in &content.aliases {
                s.push_str(alias.as_str());
                s.push_str(", ");
            }
            s.truncate(s.len() - 2); // remove the last trailing ", "
            Some(s)
        }
        AnyOtherFullStateEventContent::RoomAvatar(_) => {
            // TODO: handle a changed room avatar (picture)
            None
        }
        AnyOtherFullStateEventContent::RoomCanonicalAlias(FullStateEventContent::Original { content, .. }) => {
            Some(format!("set the main address of this room to {}", 
                content.alias.as_ref().map(|a| a.as_str()).unwrap_or("<unknown>")
            ))
        }
        AnyOtherFullStateEventContent::RoomCreate(FullStateEventContent::Original { content, .. }) => {
            Some(format!("created this room (v{})", content.room_version.as_str()))
        }
        AnyOtherFullStateEventContent::RoomGuestAccess(FullStateEventContent::Original { content, .. }) => {
            Some(match content.guest_access {
                GuestAccess::CanJoin => format!("has allowed guests to join this room"),
                GuestAccess::Forbidden | _ => format!("has forbidden guests from joining this room"),
            })
        }
        AnyOtherFullStateEventContent::RoomHistoryVisibility(FullStateEventContent::Original { content, .. }) => {
            let visibility = match content.history_visibility {
                HistoryVisibility::Invited => "invited users, since they were invited",
                HistoryVisibility::Joined => "joined users, since they joined",
                HistoryVisibility::Shared => "joined users, for all of time",
                HistoryVisibility::WorldReadable | _ => "anyone for all time",
            };
            Some(format!("set this room's history to be visible by {}", visibility))
        }
        AnyOtherFullStateEventContent::RoomJoinRules(FullStateEventContent::Original { content, .. }) => {
            Some(match content.join_rule {
                JoinRule::Public => format!("set this room to be joinable by anyone"),
                JoinRule::Knock => format!("set this room to be joinable by invite only or by request"),
                JoinRule::Private => format!("set this room to be private"),
                JoinRule::Restricted(_) => format!("set this room to be joinable by invite only or with restrictions"),
                JoinRule::KnockRestricted(_) => format!("set this room to be joinable by invite only or requestable with restrictions"),
                JoinRule::Invite | _ => format!("set this room to be joinable by invite only"),
            })
        }
        AnyOtherFullStateEventContent::RoomName(FullStateEventContent::Original { content, .. }) => {
            Some(format!("changed this room's name to {:?}", content.name))
        }
        AnyOtherFullStateEventContent::RoomPowerLevels(_) => {
            None
        }
        AnyOtherFullStateEventContent::RoomTopic(FullStateEventContent::Original { content, .. }) => {
            Some(format!("changed this room's topic to {:?}", content.topic))
        }
        AnyOtherFullStateEventContent::SpaceParent(_)
        | AnyOtherFullStateEventContent::SpaceChild(_) => None,
        other => {
            println!("*** Unhandled: {:?}", other);
            None
        }
    };

    if let Some(text) = text {
        let item = list.item(cx, item_id, live_id!(SmallStateEvent)).unwrap();
        item.label(id!(content)).set_text(
            &format!("{} {}.", event_tl_item.sender(), text)
        );
        // Set the timestamp.
        set_timestamp(&item, id!(left_container.timestamp), event_tl_item.timestamp());
        item
    } else {
        list.item(cx, item_id, live_id!(Empty)).unwrap()
    }
}



/// Sets the text of the `Label` at the given `item`'s live ID path
/// to a typical 12-hour AM/PM timestamp format.
fn set_timestamp(
    item: &WidgetRef,
    live_id_path: &[LiveId],
    timestamp: MilliSecondsSinceUnixEpoch,
) {
    if let Some(dt) = unix_time_millis_to_datetime(&timestamp) {
        // format as AM/PM 12-hour time
        item.label(live_id_path).set_text(
            &format!("{}", dt.time().format("%l:%M %P"))
        );
    } else {
        item.label(live_id_path).set_text(
            &format!("{}", timestamp.get())
        );
    }
}
