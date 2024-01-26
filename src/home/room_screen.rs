//! A room screen is the UI page that displays a single Room's timeline of events/messages
//! along with a message input bar at the bottom.

use std::{ops::DerefMut, sync::{Arc, Mutex}, collections::BTreeMap};

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
            join_rules::JoinRule, message::{MessageType, RoomMessageEventContent}, MediaSource,
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

use unicode_segmentation::UnicodeSegmentation;
use crate::{
    media_cache::{MediaCache, AVATAR_CACHE},
    shared::avatar::{AvatarWidgetRefExt, AvatarRef},
    sliding_sync::{submit_async_request, MatrixRequest, take_timeline_update_receiver},
    utils::{unix_time_millis_to_datetime, self, MediaFormatConst},
};

live_design! {
    import makepad_draw::shader::std::*;
    import makepad_widgets::base::*;
    import makepad_widgets::theme_desktop_dark::*;

    import crate::shared::styles::*;
    import crate::shared::helpers::*;
    import crate::shared::search_bar::SearchBar;
    import crate::shared::avatar::Avatar;

    IMG_DEFAULT_AVATAR = dep("crate://self/resources/img/default_avatar.png")
    IMG_LOADING = dep("crate://self/resources/img/loading.png")
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
                
                <LineH> {
                    margin: {top: 13.0, bottom: 5.0}
                }
                
                <MessageMenu> {}
            }
        }
    }

    // The view used for each static image-based message event in a room's timeline.
    // This excludes stickers and other animated GIFs, video clips, audio clips, etc.
    ImageMessage = <View> {
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
                    }
                    text: "<Username not available>"
                }
                img = <Image> {
                    width: Fill, height: 200,
                    min_width: 10., min_height: 10.,
                    fit: Horizontal,
                    source: (IMG_LOADING),
                }
                
                // message = <RoundedView> {
                //     width: Fill,
                //     height: Fit,
                //     align: { x: 0.5, y: 0.5 }
                //     draw_bg: {
                //         instance radius: 4.0,
                //         instance border_width: 1.0,
                //         // instance border_color: #ddd,
                //         color: #dfd
                //     }
                //     img = <Image> {
                //         width: 200., height: 200.0, // TODO FIXME: use actual image dimensions
                //         source: (IMG_LOADING),
                //     }
                // }
                
                <LineH> {
                    margin: {top: 13.0, bottom: 5.0}
                }
                
                <MessageMenu> {}
            }
        }

        // body = {
        //     content = {
        //         message = <RoundedView> {
        //             align: { x: 0.5, y: 0.5 }
        //             draw_bg: {
        //                 instance radius: 4.0,
        //                 instance border_width: 1.0,
        //                 // instance border_color: #ddd,
        //                 color: #dfd
        //             }
        //             img = <Image> {
        //                 width: Fill, height: 100.0, // TODO FIXME: use actual image dimensions
        //                 source: (IMG_LOADING),
        //             }
        //         }
        //     }
        // }
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
    ReadMarker = <View> {
        width: Fill,
        height: Fit,
        margin: 0.0,
        flow: Right,
        padding: 0.0,
        spacing: 0.0,
        align: {x: 0.5, y: 0.5} // center horizontally and vertically

        left_line = <LineH> {
            margin: {top: 10.0, bottom: 10.0}
            draw_bg: {color: (COLOR_READ_MARKER)}
        }

        date = <Label> {
            padding: {left: 7.0, right: 7.0}
            margin: {bottom: 10.0, top: 10.0}
            draw_text: {
                text_style: <TEXT_SUB> {},
                color: (COLOR_READ_MARKER)
            }
            text: "New Messages"
        }

        right_line = <LineH> {
            margin: {top: 10.0, bottom: 10.0}
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
            auto_tail: false, // set to `true` to lock the view to the last item.
            height: Fill,
            width: Fill
            flow: Down
    
            // Below, we must place all of the possible templates (views) that can be used in the portal list.
            TopSpace = <TopSpace> {}
            Message = <Message> {}
            ImageMessage = <ImageMessage> {}
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
            <Image> {
                source: (IMG_SMILEY_FACE_BW),
                width: 36., height: 36.
            }
            <Image> {
                source: (IMG_PLUS),
                width: 36., height: 36.
            }
            send_message_button = <IconButton> {
                draw_icon: {svg_file: (ICO_REPLY)},
                icon_walk: {width: 15.0, height: Fit},
                text: "Send",
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
                let entered_text = self.text_input(id!(message_input)).text();
                if !entered_text.is_empty() {
                    let room_id = self.room_id.clone().unwrap();
                    println!("Sending message to room {}: {:?}", room_id, entered_text);
                    submit_async_request(MatrixRequest::SendMessage {
                        room_id,
                        message: RoomMessageEventContent::text_plain(entered_text),
                        // TODO: support replies to specific messages, attaching mentions, etc.
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
        self.timeline(id!(timeline)).set_room(room_id);
    }
}


/// A message that is sent from a background async task to a room's timeline view
/// for the purpose of update the Timeline UI contents or metadata.
pub enum TimelineUpdate {
    /// A update containing the entire list of timeline items for a room,
    /// indicating that it has been updated in the background.
    NewItems(Vector<Arc<TimelineItem>>),
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
    first_id: u64,
}

impl Timeline {
    /// Removes this Timeline's current visual UI state from this Timeline widget
    /// and saves it to the map of `TIMELINE_STATES` such that it can be restored later.
    ///
    /// Note: after calling this function, the timeline's `tl_state` will be `None`.
    fn save_state(&mut self) {
        println!("Saving state for room {}", self.tl_state.as_ref().unwrap().room_id);
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
            let update_receiver = take_timeline_update_receiver(&room_id)
                .expect("BUG: couldn't get timeline state for first-viewed room.");
            let new_tl_state = TimelineUiState {
                room_id: room_id.clone(),
                // We assume timelines being viewed for the first time haven't been fully paginated.
                fully_paginated: false,
                items: Vector::new(),
                update_receiver,
                media_cache: MediaCache::new(MediaFormatConst::File),
                saved_state: SavedState::default(),
            };
            (new_tl_state, true)
        };

        // kick off a back pagination request for this room
        if !tl_state.fully_paginated {
            submit_async_request(MatrixRequest::PaginateRoomTimeline {
                room_id: room_id.clone(),
                batch_size: 50,
                max_events: 50,
            })
        } else {
            // println!("Note: skipping pagination request for room {} because it is already fully paginated.", room_id);
        }

        // Even though we specify that room member profiles should be lazy-loaded,
        // the matrix server still doesn't consistently send them to our client properly.
        // So we kick off a request to fetch the room members here upon first viewing the room.
        if first_time_showing_room {
            submit_async_request(MatrixRequest::FetchRoomMembers { room_id });
        }

        // Finally, store the tl_state for this room into the Timeline widget,
        // such that it can be accessed in future event/draw handlers.
        timeline.tl_state = Some(tl_state);
    }
}

impl Widget for Timeline {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        // Currently, a Signal event is only used to tell this widget that its timeline events
        // have been updated in the background.
        if let Event::Signal = event {
            let Some(tl) = self.tl_state.as_mut() else { return };
            let mut done_loading = false;
            while let Ok(update) = tl.update_receiver.try_recv() {
                match update {
                    TimelineUpdate::NewItems(items) => {
                        tl.items = items;
                    }
                    TimelineUpdate::TimelineStartReached => {
                        println!("Timeline::handle_event(): timeline start reached for room {}", tl.room_id);
                        tl.fully_paginated = true;
                        done_loading = true;
                    }
                    TimelineUpdate::PaginationIdle => {
                        done_loading = true;
                    }
                    TimelineUpdate::RoomMembersFetched => {
                        println!("Timeline::handle_event(): room members fetched for room {}", tl.room_id);
                        // Here, to be most efficient, we could redraw only the user avatars and names in the timeline,
                        // but for now we just fall through and let the final `redraw()` call re-draw the whole timeline view.
                    }
                }
            }

            if done_loading {
                println!("TODO: hide topspace loading animation for room {}", tl.room_id);
                // TODO FIXME: hide TopSpace loading animation, set it to invisible.
            }
            
            self.redraw(cx);
        }

        // Handle actions on this widget, e.g., it being hidden or shown.
        if let Event::Actions(actions) = event {
            for action in actions {
                let stack_view_subwidget_action = action.as_widget_action().cast();
                match stack_view_subwidget_action {
                    // TODO: this should be `HideEnd`, but we don't currently receive any `HideEnd` events
                    //       at all due to a presumed bug with the Stack Navigation widget.
                    StackNavigationTransitionAction::HideBegin => {
                        self.save_state();
                        continue;
                    }
                    StackNavigationTransitionAction::Show => {
                        self.restore_state();
                        self.redraw(cx);
                        continue;
                    }
                    StackNavigationTransitionAction::HideEnd
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
        let last_item_id = tl_items.len() as u64;
        let last_item_id = last_item_id + 1; // Add 1 for the TopSpace.

        // Start the actual drawing procedure.
        while let Some(list_item) = self.view.draw_walk(cx, scope, walk).step() {
            // We only care about drawing the portal list.
            let portal_list_ref = list_item.as_portal_list();
            let Some(mut list_ref) = portal_list_ref.borrow_mut() else { continue };
            let list = list_ref.deref_mut();
        
            list.set_item_range(cx, 0, last_item_id);

            while let Some(item_id) = list.next_visible_item(cx) {
                // println!("Drawing item {}", item_id);
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
                                    &mut tl_state.media_cache,
                                ),
                                TimelineItemContent::RedactedMessage => populate_redacted_message_view(
                                    cx,
                                    list,
                                    item_id,
                                    event_tl_item,
                                    &tl_state.room_id,
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
                            list.item(cx, item_id, live_id!(ReadMarker)).unwrap()
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
    media_cache: &mut MediaCache,
) -> WidgetRef {
    let item = match message.msgtype() {
        MessageType::Text(text) => {
            let item = list.item(cx, item_id, live_id!(Message)).unwrap();
            item.label(id!(content.message)).set_text(&text.body);
            item
        }
        MessageType::Image(image) => {
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
            let uri = match &image.source {
                MediaSource::Plain(mxc_uri) => Some(mxc_uri.clone()),
                MediaSource::Encrypted(_) => None,
            };
            // now that we've obtained the image URI and its mimetype, try to fetch the image.
            let item_result = if let Some(mxc_uri) = uri {
                let item = list.item(cx, item_id, live_id!(ImageMessage)).unwrap();

                let img_ref = item.image(id!(body.content.img));
                if let Some(data) = media_cache.try_get_media_or_fetch(mxc_uri, None) {
                    match mimetype {
                        Some(utils::ImageFormat::Png) => img_ref.load_png_from_data(cx, &data),
                        Some(utils::ImageFormat::Jpeg) => img_ref.load_jpg_from_data(cx, &data),
                        _unknown => utils::load_png_or_jpg(&img_ref, cx, &data),
                    }.map(|_| item)
                } else {
                    // waiting for the image to be fetched
                    Ok(item)
                }
            } else {
                Err(ImageError::EmptyData)
            };

            match item_result {
                Ok(item) => item,
                Err(e) => {
                    let item = list.item(cx, item_id, live_id!(Message)).unwrap();
                    if let MediaSource::Encrypted(encrypted) = &image.source {
                        item.label(id!(content.message)).set_text(&format!("[TODO] Display encrypted image at {:?}", encrypted.url));
                    } else {
                        item.label(id!(content.message)).set_text(&format!("Failed to get image: {e:?}:\n {:#?}", image));
                    }
                    item
                }
            }
        }
        other => {
            let item = list.item(cx, item_id, live_id!(Message)).unwrap();
            item.label(id!(content.message)).set_text(&format!("[TODO] {}", other.body()));
            item
        }

    };

    let username = set_avatar_and_get_username(
        cx,
        item.avatar(id!(profile.avatar)),
        event_tl_item,
    );
    item.label(id!(content.username)).set_text(&username);

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
    let original_sender = set_avatar_and_get_username(
        cx,
        item.avatar(id!(avatar)),
        event_tl_item,
    );
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

    let change_user_id = change.user_id();

    let text = match change.change() {
        None
        | Some(MembershipChange::NotImplemented)
        | Some(MembershipChange::None) => {
            // Don't actually display anything for nonexistent/unimportant membership changes.
            return list.item(cx, item_id, live_id!(Empty)).unwrap();
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

    let item = list.item(cx, item_id, live_id!(SmallStateEvent)).unwrap();
    set_timestamp(&item, id!(left_container.timestamp), event_tl_item.timestamp());
    let username = set_avatar_and_get_username(
        cx,
        item.avatar(id!(avatar)),
        event_tl_item,
    );
    
    item.label(id!(content)).set_text(&format!("{username} {text}"));
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
    let username = set_avatar_and_get_username(
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
    item_id: u64,
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
            // println!("*** Unhandled: {:?}.", _other);
            None
        }
    };

    if let Some(text) = text {
        let item = list.item(cx, item_id, live_id!(SmallStateEvent)).unwrap();
        let username = set_avatar_and_get_username(
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


/// Sets the given avatar returns a displayable username, based on the info from the given timeline event.
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
fn set_avatar_and_get_username(
    cx: &mut Cx,
    mut avatar: AvatarRef,
    event_tl_item: &EventTimelineItem,
) -> String {
    let mut username = String::new();

    // A closure to set the item's avatar and username to text data,
    // skipping the first `skip` characters of the given `name` for the avatar text.
    let mut set_avatar_text_and_name = |name: &str, skip: usize| {
        username = name.to_owned();
        avatar.set_text(
            name.graphemes(true).skip(skip).next()
                .map(ToString::to_string)
                .unwrap_or_default()
        );
    };

    // Set sender to the display name if available, otherwise the user id.
    match event_tl_item.sender_profile() {
        TimelineDetails::Ready(profile) => {
            // Set the sender's avatar image, or use a text character if no image is available.
            let avatar_img = profile.avatar_url.as_ref()
                .and_then(|uri| AVATAR_CACHE.lock().unwrap().try_get_media_or_fetch(uri.clone(), None));
            match (avatar_img, &profile.display_name) {
                // Both the avatar image and display name are available.
                (Some(avatar_img), Some(name)) => {
                    let _ = avatar.set_image(|img| utils::load_png_or_jpg(&img, cx, &avatar_img));
                    username = name.to_owned();
                }
                // The avatar image is available, but the display name is not.
                (Some(avatar_img), None) => {
                    let _ = avatar.set_image(|img| utils::load_png_or_jpg(&img, cx, &avatar_img));
                    username = event_tl_item.sender().as_str().to_owned();
                }
                // The avatar image is not available, but the display name is.
                (None, Some(name)) => {
                    set_avatar_text_and_name(name, 0);
                }
                // Neither the avatar image nor the display name are available.
                (None, None) => {
                    set_avatar_text_and_name(event_tl_item.sender().as_str(), 1);
                }
            }
        }
        _other => {
            // println!("populate_message_view(): sender profile not ready yet for event {_other:?}");
            set_avatar_text_and_name(event_tl_item.sender().as_str(), 1);
        }
    }

    username
}
