//! A room screen is the UI page that displays a single Room's timeline of events/messages
//! along with a message input bar at the bottom.

use std::{collections::BTreeMap, ops::DerefMut, sync::{Arc, Mutex}};

use imbl::Vector;
use makepad_widgets::*;
use matrix_sdk::ruma::{
    events::{
        AnySyncTimelineEvent,
        AnySyncMessageLikeEvent,
        FullStateEventContent,
        room::{
            guest_access::GuestAccess,
            history_visibility::HistoryVisibility,
            join_rules::JoinRule, message::{MessageType, RoomMessageEventContent}, MediaSource,
        },
        SyncMessageLikeEvent,
    }, uint, MilliSecondsSinceUnixEpoch, OwnedRoomId,
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

use rangemap::RangeSet;
use unicode_segmentation::UnicodeSegmentation;
use crate::{
    media_cache::{MediaCache, MediaCacheEntry, AVATAR_CACHE},
    shared::{avatar::{AvatarRef, AvatarWidgetRefExt}, text_or_image::TextOrImageWidgetRefExt},
    sliding_sync::{submit_async_request, take_timeline_update_receiver, MatrixRequest},
    utils::{self, unix_time_millis_to_datetime, MediaFormatConst},
};

live_design! {
    import makepad_draw::shader::std::*;
    import makepad_widgets::base::*;
    import makepad_widgets::theme_desktop_dark::*;

    import crate::shared::styles::*;
    import crate::shared::helpers::*;
    import crate::shared::search_bar::SearchBar;
    import crate::shared::avatar::Avatar;
    import crate::shared::text_or_image::TextOrImage;

    IMG_DEFAULT_AVATAR = dep("crate://self/resources/img/default_avatar.png")
    IMG_LOADING = dep("crate://self/resources/img/loading.png")
    ICO_FAV = dep("crate://self/resources/icon_favorite.svg")
    ICO_COMMENT = dep("crate://self/resources/icon_comment.svg")
    ICO_REPLY = dep("crate://self/resources/icon_reply.svg")
    ICO_SEND = dep("crate://self/resources/icon_send.svg")
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
    COLOR_READ_MARKER = #xeb2733
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
        text: ""
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
                avatar = <Avatar> {
                    width: 50.,
                    height: 50.
                    // draw_bg: {
                    //     fn pixel(self) -> vec4 {
                    //         let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                    //         let c = self.rect_size * 0.5;
                    //         sdf.circle(c.x, c.y, c.x - 2.)
                    //         sdf.fill_keep(self.get_color());
                    //         sdf.stroke((COLOR_PROFILE_CIRCLE), 1);
                    //         return sdf.result
                    //     }
                    // }
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
                        wrap: Ellipsis,
                    }
                    text: "<Username not available>"
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
                
                // <LineH> {
                //     margin: {top: 13.0, bottom: 5.0}
                // }
                
                <MessageMenu> {}
            }
        }
    }

    // The view used for a condensed message that came right after another message
    // from the same sender, and thus doesn't need to display the sender's profile again.
    CondensedMessage = <Message> {
        body = {
            padding: { top: 5.0, bottom: 5.0, left: 10.0, right: 10.0 },
            profile = <View> {
                align: {x: 0.5, y: 0.0} // centered horizontally, top aligned
                width: 65.0,
                height: Fit,
                flow: Down,
                timestamp = <Timestamp> { padding: {top: 3.0} }
            }
            content = <View> {
                width: Fill,
                height: Fit,
                flow: Down,
                
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
            }
        }
    }

    // The view used for each static image-based message event in a room's timeline.
    // This excludes stickers and other animated GIFs, video clips, audio clips, etc.
    ImageMessage = <Message> {
        body = {
            content = {
                message = <TextOrImage> {
                    width: Fill, height: 300,
                    // img_view = { img = { fit: Horizontal } }
                }
            }
        }
    }

    // The view used for a condensed image message that came right after another message
    // from the same sender, and thus doesn't need to display the sender's profile again.
    // This excludes stickers and other animated GIFs, video clips, audio clips, etc.
    CondensedImageMessage = <CondensedMessage> {
        body = {
            content = {
                message = <TextOrImage> {
                    width: Fill, height: 300,
                    // img_view = { img = { fit: Horizontal } }
                }
            }
        }
    }


    // The view used for each state event (non-messages) in a room's timeline.
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

            avatar = <Avatar> {
                width: 19.,
                height: 19.,

                text_view = { text = { draw_text: {
                    text_style: <TITLE_TEXT>{ font_size: 7. }
                }}}
            }

            content = <Label> {
                width: Fill,
                height: Fit
                padding: {top: 5.0},
                draw_text: {
                    wrap: Word,
                    text_style: <TEXT_SUB> {},
                    color: (COLOR_P)
                }
                text: ""
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

    // The view used for the divider indicating where the user's last-viewed message is.
    // This is implemented as a DayDivider with a different color and a fixed text label.
    ReadMarker = <DayDivider> {
        left_line = {
            draw_bg: {color: (COLOR_READ_MARKER)}
        }

        date = {
            draw_text: {
                color: (COLOR_READ_MARKER)
            }
            text: "New Messages"
        }

        right_line = {
            draw_bg: {color: (COLOR_READ_MARKER)}
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
            auto_tail: true, // set to `true` to lock the view to the last item.
            height: Fill,
            width: Fill
            flow: Down
    
            // Below, we must place all of the possible templates (views) that can be used in the portal list.
            TopSpace = <TopSpace> {}
            Message = <Message> {}
            CondensedMessage = <CondensedMessage> {}
            ImageMessage = <ImageMessage> {}
            CondensedImageMessage = <CondensedImageMessage> {}
            SmallStateEvent = <SmallStateEvent> {}
            Empty = <Empty> {}
            DayDivider = <DayDivider> {}
            ReadMarker = <ReadMarker> {}
        }
    }


    IMG_SMILEY_FACE_BW = dep("crate://self/resources/img/smiley_face_bw.png")
    IMG_PLUS = dep("crate://self/resources/img/plus.png")
    IMG_KEYBOARD_ICON = dep("crate://self/resources/img/keyboard_icon.png")

    RoomScreen = {{RoomScreen}} {
        width: Fill, height: Fill,
        show_bg: true,
        draw_bg: {
            color: #fff
        }

        <KeyboardView> {
            width: Fill, height: Fill,
            flow: Down,

            // First, display the timeline of all messages/events.
            timeline = <Timeline> {}
            
            // Below that, display a view that holds the message input bar.
            <View> {
                width: Fill, height: Fit
                flow: Right, align: {y: 1.0}, padding: 10.
                show_bg: true,
                draw_bg: {
                    color: #fff
                }

                message_input = <TextInput> {
                    width: Fill, height: Fit, margin: 0
                    align: {y: 0.5}
                    empty_message: "Write a message..."
                    draw_bg: {
                        color: #fff
                    }
                    draw_text: {
                        text_style:<REGULAR_TEXT>{},
        
                        fn get_color(self) -> vec4 {
                            return #ccc
                        }
                    }

                    // TODO find a way to override colors
                    draw_cursor: {
                        instance focus: 0.0
                        uniform border_radius: 0.5
                        fn pixel(self) -> vec4 {
                            let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                            sdf.box(
                                0.,
                                0.,
                                self.rect_size.x,
                                self.rect_size.y,
                                self.border_radius
                            )
                            sdf.fill(mix(#0f0, #0b0, self.focus));
                            return sdf.result
                        }
                    }

                    // TODO find a way to override colors
                    draw_select: {
                        instance hover: 0.0
                        instance focus: 0.0
                        uniform border_radius: 2.0
                        fn pixel(self) -> vec4 {
                            let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                            sdf.box(
                                0.,
                                0.,
                                self.rect_size.x,
                                self.rect_size.y,
                                self.border_radius
                            )
                            sdf.fill(mix(#0e0, #0d0, self.focus)); // Pad color
                            return sdf.result
                        }
                    }
                }
                // <Image> {
                //     source: (IMG_SMILEY_FACE_BW),
                //     width: 36., height: 36.
                // }
                // <Image> {
                //     source: (IMG_PLUS),
                //     width: 36., height: 36.
                // }
                send_message_button = <IconButton> {
                    draw_icon: {svg_file: (ICO_SEND)},
                    icon_walk: {width: 15.0, height: Fit},
                }
            }
        }
    }
}

/// A simple deref wrapper around the `RoomScreen` widget that enables us to handle its events.
#[derive(Live, LiveHook, Widget)]
struct RoomScreen {
    #[deref] view: View,
    #[rust] room_id: Option<OwnedRoomId>,
}
impl Widget for RoomScreen {
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }

    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope){
        // Handle actions on this widget, e.g., it being hidden or shown.
        if let Event::Actions(actions) = event {
            if self.button(id!(send_message_button)).clicked(&actions) {
                let msg_input_widget = self.text_input(id!(message_input));
                let entered_text = msg_input_widget.text();
                msg_input_widget.set_text_and_redraw(cx, "");
                if !entered_text.is_empty() {
                    let room_id = self.room_id.clone().unwrap();
                    log!("Sending message to room {}: {:?}", room_id, entered_text);
                    submit_async_request(MatrixRequest::SendMessage {
                        room_id,
                        message: RoomMessageEventContent::text_plain(entered_text),
                        // TODO: support replies to specific messages, attaching mentions, rich text (html), etc.
                    });
                }
            }
        }
        // Forward the event to the inner view, and thus, the inner timeline.
        self.view.handle_event(cx, event, scope)
    }
}
impl RoomScreenRef {
    /// Sets this `RoomScreen` widget to display the timeline for the given room.
    pub fn set_displayed_room(&self, room_id: OwnedRoomId) {
        let Some(mut room_screen) = self.borrow_mut() else { return };
        room_screen.room_id = Some(room_id.clone());
        room_screen.timeline(id!(timeline)).set_room(room_id);
    }
}


/// A message that is sent from a background async task to a room's timeline view
/// for the purpose of update the Timeline UI contents or metadata.
pub enum TimelineUpdate {
    /// The content of a room's timeline was updated in the background.
    NewItems {
        /// The entire list of timeline items (events) for a room.
        items: Vector<Arc<TimelineItem>>,
        /// The index of the first item in the `items` list that has changed.
        /// Any items before this index are assumed to be unchanged and need not be redrawn,
        /// while any items after this index are assumed to be changed and must be redrawn.
        index_of_first_change: usize,
    },
    /// A notice that the start of the timeline has been reached, meaning that
    /// there is no need to send further backwards pagination requests.
    TimelineStartReached,
    /// A notice that the background task doing pagination for this room has become idle,
    /// meaning that it has completed its recent pagination request(s) and is now waiting
    /// for more requests, but that the start of the timeline has not yet been reached.
    PaginationIdle,
    /// A notice that the room's members have been fetched from the server,
    /// though the success or failure of the request is not yet known until the client
    /// requests the member info via a timeline event's `sender_profile()` method.
    RoomMembersFetched,
    /// A notice that one or more requested media items (images, videos, etc.)
    /// that should be displayed in this timeline have now been fetched and are available.
    MediaFetched,
}


/// A Timeline widget displays the list of events (timeline "items") for a room.
#[derive(Live, LiveHook, Widget)]
pub struct Timeline {
    #[deref] view: View,
    
    /// The UI-relevant states for the room that this widget is currently displaying.
    #[rust] tl_state: Option<TimelineUiState>,
}

/// The global set of all timeline states, one entry per room.
static TIMELINE_STATES: Mutex<BTreeMap<OwnedRoomId, TimelineUiState>> = Mutex::new(BTreeMap::new());

/// The UI-side state of a single room's timeline, which is only accessed/updated by the UI thread.
struct TimelineUiState {
    /// The ID of the room that this timeline is for.
    room_id: OwnedRoomId,

    /// Whether this room's timeline has been fully paginated, which means
    /// that the oldest (first) event in the timeline is locally synced and available.
    /// When `true`, further backwards pagination requests will not be sent.
    fully_paginated: bool,

    /// The list of items (events) in this room's timeline that our client currently knows about.
    items: Vector<Arc<TimelineItem>>,

    /// The range of items (indices in the above `items` list) whose event **contents** have been drawn
    /// since the last update and thus do not need to be re-populated on future draw events.
    ///
    /// This range is partially cleared on each background update (see below) to ensure that
    /// items modified during the update are properly redrawn. Thus, it is a conservative
    /// "cache tracker" that may not include all items that have already been drawn,
    /// but that's okay because big updates that clear out large parts of the rangeset
    /// only occur during back pagination, which is both rare and slow in and of itself.
    /// During typical usage, new events are appended to the end of the timeline,
    /// meaning that the range of already-drawn items doesn't need to be cleared.
    ///
    /// Upon a background update, only item indices greater than or equal to the
    /// `index_of_first_change` are removed from this set. 
    content_drawn_since_last_update: RangeSet<usize>,

    /// Same as `content_drawn_since_last_update`, but for the event **profiles** (avatar, username).
    profile_drawn_since_last_update: RangeSet<usize>,

    force_redraw_media: bool,
    force_redraw_profiles: bool,

    /// The channel receiver for timeline updates for this room.
    ///
    /// Here we use a synchronous (non-async) channel because the receiver runs
    /// in a sync context and the sender runs in an async context,
    /// which is okay because a sender on an unbounded channel never needs to block.
    update_receiver: crossbeam_channel::Receiver<TimelineUpdate>,

    /// The cache of media items (images, videos, etc.) that appear in this timeline.
    ///
    /// Currently this excludes avatars, as those are shared across multiple rooms.
    media_cache: MediaCache,
    
    /// The states relevant to the UI display of this timeline that are saved upon
    /// a `Hide` action and restored upon a `Show` action.
    saved_state: SavedState,
}

/// States that are necessary to save in order to maintain a consistent UI display for a timeline.
///
/// These are saved when navigating away from a timeline (upon `Hide`)
/// and restored when navigating back to a timeline (upon `Show`).
#[derive(Default, Debug)]
struct SavedState {
    /// The ID of the first item in the timeline's PortalList that is currently visible.
    ///
    /// TODO: expose scroll position from PortalList and use that instead, which is more accurate.
    first_id: usize,
}

impl Timeline {
    /// Removes this Timeline's current visual UI state from this Timeline widget
    /// and saves it to the map of `TIMELINE_STATES` such that it can be restored later.
    ///
    /// Note: after calling this function, the timeline's `tl_state` will be `None`.
    fn save_state(&mut self) {
        let first_id = self.portal_list(id!(list)).first_id();
        let Some(mut tl) = self.tl_state.take() else { return };
        tl.saved_state.first_id = first_id;
        // Store this Timeline's `TimelineUiState` in the global map of states.
        TIMELINE_STATES.lock().unwrap().insert(tl.room_id.clone(), tl);
    }

    /// Restores the previously-saved visual UI state of this timeline.
    fn restore_state(&mut self) {
        let Some(tl) = self.tl_state.as_ref() else { return };
        let first_id = tl.saved_state.first_id;
        self.portal_list(id!(list)).set_first_id(first_id);
    }
}

impl TimelineRef {
    /// Sets this timeline widget to display the timeline for the given room.
    fn set_room(&self, room_id: OwnedRoomId) {
        let Some(mut timeline) = self.borrow_mut() else { return };
        debug_assert!( // just an optional sanity check
            timeline.tl_state.is_none(),
            "BUG: tried to set_room() on a timeline with existing state. \
            Did you forget to restore the timeline state to the global map of states?",
        );

        let (tl_state, first_time_showing_room) = if let Some(existing) = TIMELINE_STATES.lock().unwrap().remove(&room_id) {
            (existing, false)
        } else {
            let (update_sender, update_receiver) = take_timeline_update_receiver(&room_id)
                .expect("BUG: couldn't get timeline state for first-viewed room.");
            let new_tl_state = TimelineUiState {
                room_id: room_id.clone(),
                // We assume timelines being viewed for the first time haven't been fully paginated.
                fully_paginated: false,
                items: Vector::new(),
                content_drawn_since_last_update: RangeSet::new(),
                profile_drawn_since_last_update: RangeSet::new(),
                force_redraw_profiles: false,
                force_redraw_media: false,
                update_receiver,
                media_cache: MediaCache::new(MediaFormatConst::File, Some(update_sender)),
                saved_state: SavedState::default(),
            };
            (new_tl_state, true)
        };

        log!("Timeline::set_room(): opening room {room_id}
            content_drawn_since_last_update: {:#?}
            profile_drawn_since_last_update: {:#?}",
            tl_state.content_drawn_since_last_update,
            tl_state.profile_drawn_since_last_update,
        );

        // kick off a back pagination request for this room
        if !tl_state.fully_paginated {
            submit_async_request(MatrixRequest::PaginateRoomTimeline {
                room_id: room_id.clone(),
                batch_size: 50,
                max_events: 50,
            })
        } else {
            // log!("Note: skipping pagination request for room {} because it is already fully paginated.", room_id);
        }

        // Even though we specify that room member profiles should be lazy-loaded,
        // the matrix server still doesn't consistently send them to our client properly.
        // So we kick off a request to fetch the room members here upon first viewing the room.
        if first_time_showing_room {
            submit_async_request(MatrixRequest::FetchRoomMembers { room_id });
            // TODO: in the future, move the back pagination request to here,
            //       once back pagination is done dynamically based on timeline scroll position.
        }

        // Finally, store the tl_state for this room into the Timeline widget,
        // such that it can be accessed in future event/draw handlers.
        timeline.tl_state = Some(tl_state);
    }
}

impl Widget for Timeline {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        // Currently, a Signal event is only used to tell this widget
        // that its timeline events have been updated in the background.
        if let Event::Signal = event {
            let portal_list = self.portal_list(id!(list));
            let orig_first_id = portal_list.first_id();
            let Some(tl) = self.tl_state.as_mut() else { return };

            let mut done_loading = false;
            let mut force_redraw_media = false;
            let mut force_redraw_profiles = true; // always do this for now, any time we get a `Signal`.

            while let Ok(update) = tl.update_receiver.try_recv() {
                match update {
                    TimelineUpdate::NewItems { items, index_of_first_change } => {
                        // Determine which item is currently visible the top of the screen
                        // so that we can jump back to that position instantly after applying this update.
                        if let Some(top_event_id) = tl.items.get(orig_first_id).map(|item| item.unique_id()) {
                            for (idx, item) in items.iter().enumerate() {
                                if item.unique_id() == top_event_id {
                                    log!("Timeline::handle_event(): jumping from top event index {orig_first_id} to index {idx}");
                                    portal_list.set_first_id(idx);
                                    break;
                                }
                            }
                        }
                        tl.content_drawn_since_last_update.remove(index_of_first_change .. items.len());
                        tl.profile_drawn_since_last_update.remove(index_of_first_change .. items.len());
                        log!("Timeline::handle_event(): index_of_first_change: {index_of_first_change}, items len: {}\ncontent drawn: {:#?}\nprofile drawn: {:#?}", items.len(), tl.content_drawn_since_last_update, tl.profile_drawn_since_last_update);
                        tl.items = items;
                    }
                    TimelineUpdate::TimelineStartReached => {
                        log!("Timeline::handle_event(): timeline start reached for room {}", tl.room_id);
                        tl.fully_paginated = true;
                        done_loading = true;
                    }
                    TimelineUpdate::PaginationIdle => {
                        done_loading = true;
                    }
                    TimelineUpdate::RoomMembersFetched => {
                        log!("Timeline::handle_event(): room members fetched for room {}", tl.room_id);
                        // Here, to be most efficient, we could redraw only the user avatars and names in the timeline,
                        // but for now we just fall through and let the final `redraw()` call re-draw the whole timeline view.
                        force_redraw_profiles = true;
                    }
                    TimelineUpdate::MediaFetched => {
                        log!("Timeline::handle_event(): media fetched for room {}", tl.room_id);
                        // Here, to be most efficient, we could redraw only the media items in the timeline,
                        // but for now we just fall through and let the final `redraw()` call re-draw the whole timeline view.
                        force_redraw_media = true;
                    }
                }
            }

            if done_loading {
                log!("TODO: hide topspace loading animation for room {}", tl.room_id);
                // TODO FIXME: hide TopSpace loading animation, set it to invisible.
            }
            
            tl.force_redraw_profiles = force_redraw_profiles;
            tl.force_redraw_media = force_redraw_media;
            self.redraw(cx);
        }

        // Handle actions on this widget, e.g., it being hidden or shown.
        if let Event::Actions(actions) = event {
            for action in actions {
                let stack_view_subwidget_action = action.as_widget_action().cast();
                match stack_view_subwidget_action {
                    StackNavigationTransitionAction::HideEnd => {
                        self.save_state();
                        continue;
                    }
                    StackNavigationTransitionAction::ShowBegin => {
                        self.restore_state();
                        self.redraw(cx);
                        continue;
                    }
                    StackNavigationTransitionAction::HideBegin
                    | StackNavigationTransitionAction::ShowDone
                    | StackNavigationTransitionAction::None => { }
                }

                // Handle other actions here
                // TODO: handle actions upon an item being clicked.
                // for (item_id, item) in self.list.items_with_actions(&actions) {
                //     if item.button(id!(likes)).clicked(&actions) {
                //         log!("hello {}", item_id);
                //     }
                // }
            }
        }

        // Forward events to this Timeline's inner child view.
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        let Some(tl_state) = self.tl_state.as_mut() else {
            return DrawStep::done()
        };
        let tl_items = &tl_state.items;

        // Determine length of the portal list based on the number of timeline items.
        let last_item_id = tl_items.len();
        let last_item_id = last_item_id + 1; // Add 1 for the TopSpace.

        // Start the actual drawing procedure.
        while let Some(list_item) = self.view.draw_walk(cx, scope, walk).step() {
            // We only care about drawing the portal list.
            let portal_list_ref = list_item.as_portal_list();
            let Some(mut list_ref) = portal_list_ref.borrow_mut() else { continue };
            let list = list_ref.deref_mut();
        
            list.set_item_range(cx, 0, last_item_id);

            while let Some(item_id) = list.next_visible_item(cx) {
                // log!("Drawing item {}", item_id);
                let item = if item_id == 0 {
                    list.item(cx, item_id, live_id!(TopSpace)).unwrap()
                } else {
                    let tl_idx = (item_id - 1) as usize;
                    let Some(timeline_item) = tl_items.get(tl_idx) else {
                        // This shouldn't happen (unless the timeline gets corrupted or some other weird error),
                        // but we can always safely fill the item with an empty widget that takes up no space.
                        list.item(cx, item_id, live_id!(Empty)).unwrap();
                        continue;
                    };

                    // Determine whether this item's content and profile have been drawn since the last update.
                    // Pass this state to each of the `populate_*` functions so they can attempt to re-use
                    // an item in the timeline's portallist that was previously populated, if one exists.
                    let item_drawn_status = ItemDrawnStatus {
                        content_drawn: tl_state.content_drawn_since_last_update.contains(&tl_idx),
                        profile_drawn: tl_state.profile_drawn_since_last_update.contains(&tl_idx),
                    };

                    let (item, item_new_draw_status) = match timeline_item.kind() {
                        TimelineItemKind::Event(event_tl_item) => match event_tl_item.content() {
                            TimelineItemContent::Message(message) => {
                                let prev_event = tl_items.get(tl_idx.saturating_sub(1));
                                populate_message_view(
                                    cx,
                                    list,
                                    item_id,
                                    event_tl_item,
                                    message,
                                    prev_event,
                                    &mut tl_state.media_cache,
                                    item_drawn_status,
                                )
                            }
                            TimelineItemContent::RedactedMessage => populate_redacted_message_view(
                                cx,
                                list,
                                item_id,
                                event_tl_item,
                                &tl_state.room_id,
                                item_drawn_status,
                            ),
                            TimelineItemContent::MembershipChange(membership_change) => populate_membership_change_view(
                                cx,
                                list,
                                item_id,
                                event_tl_item,
                                membership_change,
                                item_drawn_status,
                            ),
                            TimelineItemContent::ProfileChange(profile_change) => (
                                populate_profile_change_view(
                                    cx,
                                    list,
                                    item_id,
                                    event_tl_item,
                                    profile_change,
                                    // item_drawn_status,
                                ),
                                ItemDrawnStatus::new(),
                            ),
                            TimelineItemContent::OtherState(other) => (
                                populate_other_state_view(
                                    cx,
                                    list,
                                    item_id,
                                    event_tl_item,
                                    other,
                                    // item_drawn_status,
                                ),
                                ItemDrawnStatus::new(),
                            ),
                            unhandled => {
                                let item = list.item(cx, item_id, live_id!(SmallStateEvent)).unwrap();
                                item.label(id!(content)).set_text(&format!("[TODO] {:?}", unhandled));
                                (item, ItemDrawnStatus::both_drawn())
                            }
                        }
                        TimelineItemKind::Virtual(VirtualTimelineItem::DayDivider(millis)) => {
                            let item = list.item(cx, item_id, live_id!(DayDivider)).unwrap();
                            let text = unix_time_millis_to_datetime(millis)
                                // format the time as a shortened date (Sat, Sept 5, 2021)
                                .map(|dt| format!("{}", dt.date().format("%a %b %-d, %Y")))
                                .unwrap_or_else(|| format!("{:?}", millis));
                            item.label(id!(date)).set_text(&text);
                            (item, ItemDrawnStatus::both_drawn())
                        }
                        TimelineItemKind::Virtual(VirtualTimelineItem::ReadMarker) => {
                            let item = list.item(cx, item_id, live_id!(ReadMarker)).unwrap();
                            (item, ItemDrawnStatus::both_drawn())
                        }
                    };

                    // Now that we've drawn the item, add its index to the set of drawn items.
                    if item_new_draw_status.content_drawn {
                        tl_state.content_drawn_since_last_update.insert(tl_idx .. tl_idx+1);
                    }
                    if item_new_draw_status.profile_drawn {
                        tl_state.profile_drawn_since_last_update.insert(tl_idx .. tl_idx+1);
                    }
                    item
                };
                item.draw_all(cx, &mut Scope::empty());
            }
        }
        DrawStep::done()
    }
}


#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct ItemDrawnStatus {
    /// Whether the profile info (avatar and displayable username) were drawn for this item.
    profile_drawn: bool,
    /// Whether the content of the item was drawn (e.g., the message text, image, video, sticker, etc).
    content_drawn: bool,
}
impl ItemDrawnStatus {
    /// Returns a new `ItemDrawnStatus` with both `profile_drawn` and `content_drawn` set to `false`.
    const fn new() -> Self {
        Self { profile_drawn: false, content_drawn: false }
    }
    /// Returns a new `ItemDrawnStatus` with both `profile_drawn` and `content_drawn` set to `true`.
    const fn both_drawn() -> Self {
        Self { profile_drawn: true, content_drawn: true }
    }
}

// TODO: return this `ItemDrawnStatus` from the populate_*_view functions and use it to determine
//       if that item ID can be added to the `drawn_since_last_update` range set (only if both are true).
//       For now, we should only add items that are fully drawn to the range set,
//       as we don't want to accidentally miss redrawing updated items that were only partially drawn.
//       In this way, we won't consider an item fully drawn until both its profile and content are fully drawn.
//       ****
//       Note: we'll also need to differentiate between:
//             an avatar not existing at all (considered fully drawn)
//             vs an avatar not being "ready" or not being fetched yet (considered not fully drawn)
      
//       ****
//       Also, we should split `drawn_since_last_update` into two separate `RangeSet`s:
//          -- one for items whose CONTENT has been drawn fully, and
//          -- one for items whose PROFILE has been drawn fully.
//         This way, we can redraw the profile of an item without redrawing its content, and vice versa --> efficient!
//       ****
//       We should also use a range to specify `index_of_first_change` AND index of last change,
//       such that we can support diff operations like set (editing/updating a single event).
//       To do so, we'll have to send interim message updates to the UI thread rather than always sending the entire batch of diffs,
//       but that's no problem because sending those updates is already very cheap.
//       Plus, we already have plans to split up the batches across multiple update messages in the future,
//       in order to support conveying more detailed info about which items were actually changed and at which indices
//       (e.g., we'll eventually send one update per contiguous set of changed items, rather than one update per entire batch of items).

      


/// Creates, populates, and adds a Message liveview widget to the given `PortalList`
/// with the given `item_id`.
///
/// The content of the returned `Message` widget is populated with data from the given `message`
/// and its parent `EventTimelineItem`.
fn populate_message_view(
    cx: &mut Cx,
    list: &mut PortalList,
    item_id: usize,
    event_tl_item: &EventTimelineItem,
    message: &timeline::Message,
    prev_event: Option<&Arc<TimelineItem>>,
    media_cache: &mut MediaCache,
    item_drawn_status: ItemDrawnStatus,
) -> (WidgetRef, ItemDrawnStatus) {

    let mut new_drawn_status = item_drawn_status;

    let ts_millis = event_tl_item.timestamp();

    // Determine whether we can use a more compact UI view that hides the user's profile info
    // if the previous message was sent by the same user within 10 minutes.
    let use_compact_view = match prev_event.map(|p| p.kind()) {
        Some(TimelineItemKind::Event(prev_event_tl_item)) => match prev_event_tl_item.content() {
            TimelineItemContent::Message(_prev_msg) => {
                let prev_msg_sender = prev_event_tl_item.sender();
                prev_msg_sender == event_tl_item.sender() &&
                    ts_millis.0.checked_sub(prev_event_tl_item.timestamp().0)
                        .map_or(false, |d| d < uint!(600000)) // 10 mins in millis
            }
            _ => false,
        }
        _ => false,
    };

    let (item, used_cached_item) = match message.msgtype() {
        MessageType::Text(text) => {
            let template = if use_compact_view {
                live_id!(CondensedMessage)
            } else {
                live_id!(Message)
            };
            let (item, existed) = list.item_with_existed(cx, item_id, template).unwrap();
            if existed && item_drawn_status.content_drawn {
                (item, true)
            } else {
                item.label(id!(content.message)).set_text(&text.body);
                new_drawn_status.content_drawn = true;
                (item, false)
            }
        }
        MessageType::Image(image) => {
            let template = if use_compact_view {
                live_id!(CondensedImageMessage)
            } else {
                live_id!(ImageMessage)
            };
            let (item, existed) = list.item_with_existed(cx, item_id, template).unwrap();
            if existed && item_drawn_status.content_drawn {
                (item, true)
            } else {
                // We don't use thumbnails, as their resolution is too low to be visually useful.
                let (mimetype, _width, _height) = if let Some(info) = image.info.as_ref() {
                    (
                        info.mimetype.as_deref().and_then(utils::ImageFormat::from_mimetype),
                        info.width,
                        info.height,
                    )
                } else {
                    (None, None, None)
                };
                let text_or_image_ref = item.text_or_image(id!(content.message));
                match &image.source {
                    MediaSource::Plain(mxc_uri) => {
                        // now that we've obtained the image URI and its mimetype, try to fetch the image.
                        match media_cache.try_get_media_or_fetch(mxc_uri.clone(), None) {
                            MediaCacheEntry::Loaded(data) => {
                                let set_image_result = text_or_image_ref.set_image(|img|
                                    match mimetype {
                                        Some(utils::ImageFormat::Png) => img.load_png_from_data(cx, &data),
                                        Some(utils::ImageFormat::Jpeg) => img.load_jpg_from_data(cx, &data),
                                        _unknown => utils::load_png_or_jpg(&img, cx, &data),
                                    }
                                );
                                if let Err(e) = set_image_result {
                                    let err_str = format!("Failed to display image: {e:?}");
                                    error!("{err_str}");
                                    text_or_image_ref.set_text(&err_str);
                                }
                                // The image content is completely drawn here, ready to be marked as cached/drawn.
                                new_drawn_status.content_drawn = true;
                            }
                            MediaCacheEntry::Requested => {
                                text_or_image_ref.set_text(&format!("Fetching image from {:?}", mxc_uri));
                            }
                            MediaCacheEntry::Failed => {
                                text_or_image_ref.set_text(&format!("Failed to fetch image from {:?}", mxc_uri));
                                // For now, we consider this as being "complete". In the future, we could support
                                // retrying to fetch the image on a user click/tap.
                                new_drawn_status.content_drawn = true;
                            }
                        }
                    }
                    MediaSource::Encrypted(encrypted) => {
                        text_or_image_ref.set_text(&format!("[TODO] fetch encrypted image at {:?}", encrypted.url));
                        new_drawn_status.content_drawn = true; // considered complete, since we don't yet support this.
                    }
                };
                (item, false)
            }
        }
        other => {
            let (item, existed) = list.item_with_existed(cx, item_id, live_id!(Message)).unwrap();
            if existed && item_drawn_status.content_drawn {
                (item, true)
            } else {
                item.label(id!(content.message)).set_text(&format!("[TODO] {}", other.body()));
                new_drawn_status.content_drawn = true;
                (item, false)
            }
        }
    };

    // If `used_cached_item` is false, we should always redraw the profile, even if profile_drawn is true.
    let skip_draw_profile = use_compact_view || (used_cached_item && item_drawn_status.profile_drawn);
    log!("populate_message_view(): item_id: {item_id}, skip_redraw?: {skip_draw_profile}, use_compact_view: {use_compact_view}, used_cached_item: {used_cached_item}, item_drawn_status: {item_drawn_status:?}, new_drawn_status: {new_drawn_status:?}", );
    if skip_draw_profile {
        log!("\t --> populate_message_view(): SKIPPING profile draw for item_id: {item_id}");
        new_drawn_status.profile_drawn = true;
    } else {
        log!("\t --> populate_message_view(): DRAWING  profile draw for item_id: {item_id}");
        let (username, profile_drawn) = set_avatar_and_get_username(
            cx,
            item.avatar(id!(profile.avatar)),
            event_tl_item,
        );
        item.label(id!(content.username)).set_text(&username);
        new_drawn_status.profile_drawn = profile_drawn;
    }

    // If we've previously drawn the item content, skip redrawing the timestamp and annotations.
    if used_cached_item && item_drawn_status.content_drawn && item_drawn_status.profile_drawn {
        return (item, new_drawn_status);
    }

    // Set the timestamp.
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

    // Temp filler: set the likes and comments count to the timeline idx (item_id - 1), just for now.
    // In the future, we'll draw annotations (reactions) here.
    item.button(id!(likes)).set_text(&format!("{}", item_id - 1));
    item.button(id!(comments)).set_text(&format!("{}", item_id - 1));

    (item, new_drawn_status)
} 




/// Creates, populates, and adds a `SmallStateEvent` liveview widget to the given `PortalList`
/// with the given `item_id`.
///
/// The content of the returned widget is populated with metadata about the redacted message
/// the corresponds to the given `EventTimelineItem`.
fn populate_redacted_message_view(
    cx: &mut Cx,
    list: &mut PortalList,
    item_id: usize,
    event_tl_item: &EventTimelineItem,
    _room_id: &OwnedRoomId,
    item_drawn_status: ItemDrawnStatus,
) -> (WidgetRef, ItemDrawnStatus) {
    let mut new_drawn_status = item_drawn_status;
    let (item, existed) = list.item_with_existed(cx, item_id, live_id!(SmallStateEvent)).unwrap();

    // The content of a redacted message view depends on the profile,
    // so we can only cache the content after the profile has been drawn and cached.
    let skip_redrawing_profile = existed && item_drawn_status.profile_drawn;
    let skip_redrawing_content = skip_redrawing_profile && item_drawn_status.content_drawn;

    if skip_redrawing_content {
        return (item, new_drawn_status);
    }

    // If the profile has been drawn, we can just quickly grab the original sender's display name
    // instead of having to call `set_avatar_and_get_username()` again.
    let original_sender_opt = if skip_redrawing_profile {
        get_profile_display_name(event_tl_item)
    } else {
        None
    };
    
    let original_sender = original_sender_opt.unwrap_or_else(|| {
        // As a fallback, call `set_avatar_and_get_username()` to get the display name
        // (or user ID) of the original sender of the now-redacted message.
        let (original_sender, profile_drawn) = set_avatar_and_get_username(
            cx,
            item.avatar(id!(avatar)),
            event_tl_item,
        );
        // Draw the timestamp as part of the profile.
        set_timestamp(&item, id!(left_container.timestamp), event_tl_item.timestamp());
        new_drawn_status.profile_drawn = profile_drawn;
        original_sender
    });


    // Proceed to draw the content, now that we have the original sender's display name. 
    let redactor_and_reason = {
        let mut rr = None;
        if let Some(redacted_msg) = event_tl_item.latest_json() {
            if let Ok(old) = redacted_msg.deserialize() {
                if let AnySyncTimelineEvent::MessageLike(AnySyncMessageLikeEvent::RoomMessage(SyncMessageLikeEvent::Redacted(redaction))) = old {
                    rr = Some((
                        redaction.unsigned.redacted_because.sender,
                        redaction.unsigned.redacted_because.content.reason,
                    ));
                }
            }
        }
        rr
    };

    let text = match redactor_and_reason {
        Some((redactor, Some(reason))) => {
            // TODO: get the redactor's display name if possible
            format!("{} deleted {}'s message: {:?}.", redactor, original_sender, reason)
        }
        Some((redactor, None)) => {
            if redactor == event_tl_item.sender() {
                format!("{} deleted their own message.", original_sender)
            } else {
                format!("{} deleted {}'s message.", redactor, original_sender)
            }
        }
        None => {
            format!("{}'s message was deleted.", original_sender)
        }
    };

    item.label(id!(content)).set_text(&text);
    new_drawn_status.content_drawn = true;
    (item, new_drawn_status)
} 


/// Creates, populates, and adds a SmallStateEvent liveview widget to the given `PortalList`
/// with the given `item_id`.
///
/// The content of the returned widget is populated with data from the
/// given room membership change and its parent `EventTimelineItem`.
fn populate_membership_change_view(
    cx: &mut Cx,
    list: &mut PortalList,
    item_id: usize,
    event_tl_item: &EventTimelineItem,
    change: &RoomMembershipChange,
    item_drawn_status: ItemDrawnStatus,
) -> (WidgetRef, ItemDrawnStatus) {
    let mut new_drawn_status = item_drawn_status;
    let (item, existed) = list.item_with_existed(cx, item_id, live_id!(SmallStateEvent)).unwrap();

    // The content of a membership change view depends on the profile,
    // so we can only cache the content after the profile has been drawn and cached.
    let skip_redrawing_profile = existed && item_drawn_status.profile_drawn;
    let skip_redrawing_content = skip_redrawing_profile && item_drawn_status.content_drawn;

    if skip_redrawing_content {
        return (item, new_drawn_status);
    }

    // If the profile has been drawn, we can just quickly grab the user's display name
    // instead of having to call `set_avatar_and_get_username()` again.
    let username_opt = if skip_redrawing_profile {
        get_profile_display_name(event_tl_item)
    } else {
        None
    };
    
    let username = username_opt.unwrap_or_else(|| {
        // As a fallback, call `set_avatar_and_get_username()` to get the user's display name.
        let (username, profile_drawn) = set_avatar_and_get_username(
            cx,
            item.avatar(id!(avatar)),
            event_tl_item,
        );
        // Draw the timestamp as part of the profile.
        set_timestamp(&item, id!(left_container.timestamp), event_tl_item.timestamp());
        new_drawn_status.profile_drawn = profile_drawn;
        username
    });

    // Proceed to draw the content, now that we have the user's display name. 
    let change_user_id = change.user_id();
    let text = match change.change() {
        None
        | Some(MembershipChange::NotImplemented)
        | Some(MembershipChange::None) => {
            // Don't actually display anything for nonexistent/unimportant membership changes.
            return (
                list.item(cx, item_id, live_id!(Empty)).unwrap(),
                ItemDrawnStatus::both_drawn(),
            );
        }
        Some(MembershipChange::Error) =>
            format!("had a membership change error."),
        Some(MembershipChange::Joined) =>
            format!("joined this room."),
        Some(MembershipChange::Left) =>
            format!("left this room."),
        Some(MembershipChange::Banned) =>
            format!("banned {} from this room.", change_user_id),
        Some(MembershipChange::Unbanned) =>
            format!("unbanned {} from this room.", change_user_id),
        Some(MembershipChange::Kicked) =>
            format!("kicked {} from this room.", change_user_id),
        Some(MembershipChange::Invited) =>
            format!("invited {} to this room.", change_user_id),
        Some(MembershipChange::KickedAndBanned) =>
            format!("kicked and banned {} from this room.", change_user_id),
        Some(MembershipChange::InvitationAccepted) =>
            format!("accepted an invitation to this room."),
        Some(MembershipChange::InvitationRejected) =>
            format!("rejected an invitation to this room."),
        Some(MembershipChange::InvitationRevoked) =>
            format!("revoked {}'s invitation to this room.", change_user_id),
        Some(MembershipChange::Knocked) =>
            format!("requested to join this room."),
        Some(MembershipChange::KnockAccepted) =>
            format!("accepted {}'s request to join this room.", change_user_id),
        Some(MembershipChange::KnockRetracted) =>
            format!("retracted their request to join this room."),
        Some(MembershipChange::KnockDenied) =>
            format!("denied {}'s request to join this room.", change_user_id),
    };

    item.label(id!(content)).set_text(&format!("{username} {text}"));
    new_drawn_status.content_drawn = true;
    (item, new_drawn_status)
}



/// Creates, populates, and adds a SmallStateEvent liveview widget to the given `PortalList`
/// with the given `item_id`.
///
/// The content of the returned `SmallStateEvent` widget is populated with data from the
/// given member profile change and its parent `EventTimelineItem`.
fn populate_profile_change_view(
    cx: &mut Cx,
    list: &mut PortalList,
    item_id: usize,
    event_tl_item: &EventTimelineItem,
    change: &MemberProfileChange,
) -> WidgetRef {
    let (item, _existed) = list.item_with_existed(cx, item_id, live_id!(SmallStateEvent)).unwrap();
    let (username, profile_drawn) = set_avatar_and_get_username(
        cx,
        item.avatar(id!(avatar)),
        event_tl_item,
    );

    let name_text = if let Some(name_change) = change.displayname_change() {
        let old = name_change.old.as_deref().unwrap_or(&username);
        if let Some(new) = name_change.new.as_ref() {
            format!("{old} changed their display name to {new:?}")
        } else {
            format!("{old} removed their display name")
        }
    } else {
        String::new()
    };

    let avatar_text = if let Some(_avatar_change) = change.avatar_url_change() {
        if name_text.is_empty() {
            format!("{} changed their profile picture", username)
        } else {
            format!(" and changed their profile picture")
        }
    } else {
        String::new()
    };

    item.label(id!(content)).set_text(&format!("{}{}.", name_text, avatar_text));
    set_timestamp(&item, id!(left_container.timestamp), event_tl_item.timestamp());
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
    item_id: usize,
    event_tl_item: &EventTimelineItem,
    other_state: &timeline::OtherState,
) -> WidgetRef {
    let text = match other_state.content() {
        AnyOtherFullStateEventContent::RoomAliases(FullStateEventContent::Original { content, .. }) => {
            let mut s = format!("set this room's aliases to ");
            let last_alias = content.aliases.len() - 1;
            for (i, alias) in content.aliases.iter().enumerate() {
                s.push_str(alias.as_str());
                if i != last_alias {
                    s.push_str(", ");
                }
            }
            s.push_str(".");
            Some(s)
        }
        AnyOtherFullStateEventContent::RoomAvatar(_) => {
            Some(format!("set this room's avatar picture."))
        }
        AnyOtherFullStateEventContent::RoomCanonicalAlias(FullStateEventContent::Original { content, .. }) => {
            Some(format!("set the main address of this room to {}.",
                content.alias.as_ref().map(|a| a.as_str()).unwrap_or("none")
            ))
        }
        AnyOtherFullStateEventContent::RoomCreate(FullStateEventContent::Original { content, .. }) => {
            Some(format!("created this room (v{}).", content.room_version.as_str()))
        }
        AnyOtherFullStateEventContent::RoomGuestAccess(FullStateEventContent::Original { content, .. }) => {
            Some(match content.guest_access {
                GuestAccess::CanJoin => format!("has allowed guests to join this room."),
                GuestAccess::Forbidden | _ => format!("has forbidden guests from joining this room."),
            })
        }
        AnyOtherFullStateEventContent::RoomHistoryVisibility(FullStateEventContent::Original { content, .. }) => {
            let visibility = match content.history_visibility {
                HistoryVisibility::Invited => "invited users, since they were invited.",
                HistoryVisibility::Joined => "joined users, since they joined.",
                HistoryVisibility::Shared => "joined users, for all of time.",
                HistoryVisibility::WorldReadable | _ => "anyone for all time.",
            };
            Some(format!("set this room's history to be visible by {}.", visibility))
        }
        AnyOtherFullStateEventContent::RoomJoinRules(FullStateEventContent::Original { content, .. }) => {
            Some(match content.join_rule {
                JoinRule::Public => format!("set this room to be joinable by anyone."),
                JoinRule::Knock => format!("set this room to be joinable by invite only or by request."),
                JoinRule::Private => format!("set this room to be private."),
                JoinRule::Restricted(_) => format!("set this room to be joinable by invite only or with restrictions."),
                JoinRule::KnockRestricted(_) => format!("set this room to be joinable by invite only or requestable with restrictions."),
                JoinRule::Invite | _ => format!("set this room to be joinable by invite only."),
            })
        }
        AnyOtherFullStateEventContent::RoomName(FullStateEventContent::Original { content, .. }) => {
            Some(format!("changed this room's name to {:?}.", content.name))
        }
        AnyOtherFullStateEventContent::RoomPowerLevels(_) => {
            None
        }
        AnyOtherFullStateEventContent::RoomTopic(FullStateEventContent::Original { content, .. }) => {
            Some(format!("changed this room's topic to {:?}.", content.topic))
        }
        AnyOtherFullStateEventContent::SpaceParent(_)
        | AnyOtherFullStateEventContent::SpaceChild(_) => None,
        _other => {
            // log!("*** Unhandled: {:?}.", _other);
            None
        }
    };

    if let Some(text) = text {
        let item = list.item(cx, item_id, live_id!(SmallStateEvent)).unwrap();
        let (username, profile_drawn) = set_avatar_and_get_username(
            cx,
            item.avatar(id!(avatar)),
            event_tl_item,
        );
        item.label(id!(content)).set_text(&format!("{username} {text}"));
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


/// Sets the given avatar and returns a displayable username based on the given timeline event.
///
/// This function will always choose a nice, displayable username and avatar.
///
/// The specific behavior is as follows:
/// * If the timeline event's sender profile *is* ready, then the `username` and `avatar`
///   will be the user's display name and avatar image, if available.
/// * If no avatar image is available, then the `avatar` will be set to the first character
///   of the user's display name, if available.
/// * If the user's display name is not available or has not been set, the user ID
///   will be used for the `username`, and the first character of the user ID for the `avatar`.
/// * If the timeline event's sender profile is not yet ready, then the `username` and `avatar`
///   will be the user ID and the first character of that user ID, respectively.
///
/// ## Return
/// Returns a tuple of:
/// 1. The displayable username that should be used to populate the username field.
/// 2. A boolean indicating whether the user's profile info has been completely drawn
///    (for purposes of caching it to avoid future redraws).
fn set_avatar_and_get_username(
    cx: &mut Cx,
    avatar: AvatarRef,
    event_tl_item: &EventTimelineItem,
) -> (String, bool) {
    let username: String;
    let mut profile_drawn = false;

    // A closure to set the item's avatar to text data,
    // skipping the first `skip` characters of the given `name`.
    let set_avatar_text = |name: &str, skip: usize| {
        avatar.set_text(
            name.graphemes(true)
                .skip(skip)
                .next()
                .map(ToString::to_string)
                .unwrap_or_default()
        );
    };

    // Set sender to the display name if available, otherwise the user id.
    match event_tl_item.sender_profile() {
        TimelineDetails::Ready(profile) => {
            // Set the sender's avatar image, or use a text character if no image is available.
            let avatar_img = match profile.avatar_url.as_ref() {
                Some(uri) => match AVATAR_CACHE.lock().unwrap().try_get_media_or_fetch(uri.clone(), None) {
                    MediaCacheEntry::Loaded(data) => {
                        profile_drawn = true;
                        Some(data)
                    }
                    MediaCacheEntry::Failed => {
                        profile_drawn = true;
                        None
                    }
                    MediaCacheEntry::Requested => None,
                }
                None => {
                    profile_drawn = true;
                    None
                }
            };
            
            // Set the username to the display name if available, otherwise the user ID after the '@'.
            let (skip, un) = if let Some(dn) = profile.display_name.as_ref() {
                (0, dn.to_owned())
            } else {
                (1, event_tl_item.sender().as_str().to_owned())
            };
            username = un;

            // Draw the avatar image if available, otherwise set the avatar to text.
            let drew_avatar_img = avatar_img.map(|data|
                avatar.set_image(|img|
                    utils::load_png_or_jpg(&img, cx, &data)
                ).is_ok()
            ).unwrap_or(false);
            
            if !drew_avatar_img {
                set_avatar_text(&username, skip);
            }
        }
        other => {
            // log!("populate_message_view(): sender profile not ready yet for event {_other:?}");
            username = event_tl_item.sender().as_str().to_owned();
            set_avatar_text(&username, 1);
            // If there was an error fetching the profile, treat that condition as fully drawn,
            // since we don't yet have a good way to re-request profile information.
            profile_drawn = matches!(other, TimelineDetails::Error(_));
        }
    }

    (username, profile_drawn)
}

/// Returns the display name of the sender of the given `event_tl_item`, if available.
fn get_profile_display_name(event_tl_item: &EventTimelineItem) -> Option<String> {
    if let TimelineDetails::Ready(profile) = event_tl_item.sender_profile() {
        profile.display_name.clone()
    } else {
        None
    }
}
