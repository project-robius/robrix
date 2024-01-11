//! A room screen is the UI page that displays a single Room's timeline of events/messages
//! along with a message input bar at the bottom.

use std::{ops::DerefMut, sync::{Arc, Mutex}, collections::{BTreeMap, HashMap}};

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

use unicode_segmentation::UnicodeSegmentation;
use crate::{
    sliding_sync::{submit_async_request, MatrixRequest, take_timeline_update_receiver},
    utils::unix_time_millis_to_datetime, shared::avatar::AvatarWidgetRefExt,
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
                    text: "<unknown username>"
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
                width: 19.0,
                height: 19.0,
                source: (IMG_DEFAULT_AVATAR),
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

    // A view that holds the list of all timeline events for a single room.
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
            SmallStateEvent = <SmallStateEvent> {}
            Empty = <Empty> {}
            DayDivider = <DayDivider> {}
            ReadMarker = <ReadMarker> {}
        }    
    }


    // A widget that holds a list of several timelines and displays only the selected "active" one.
    TimelineList = {{TimelineList}} {
        width: Fill, height: Fill,
        flow: Overlay

        // Below, we must place all of the possible widget templates that can be used in timeline list,
        // which currently is only a Timeline.
        //
        // Note: I'm sure there's a better way to do this, but I don't know how to get a LivePtr
        //       for a Timeline widget specifically without using the template pattern (like in PortalList).
        Timeline = <Timeline> {}
    }


    IMG_SMILEY_FACE_BW = dep("crate://self/resources/img/smiley_face_bw.png")
    IMG_PLUS = dep("crate://self/resources/img/plus.png")
    IMG_KEYBOARD_ICON = dep("crate://self/resources/img/keyboard_icon.png")

    // The view that holds the entire screen (beneath the stack navigation header),
    // including the timeline of events and the message input bar at the bottom.
    RoomScreen = <KeyboardView> {
        width: Fill, height: Fill
        flow: Down
        show_bg: true,
        draw_bg: {
            color: #fff
        }

        // First, display the timeline of all messages/events.
        timeline_list = <TimelineList> {}
        
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


//////////////////////////////////////////////////////////////////////////////
//////////////////////////////////////////////////////////////////////////////
//////////////////////////////////////////////////////////////////////////////



#[derive(Live, LiveRegisterWidget, WidgetRef)]
pub struct TimelineList {
    #[deref]
    view: View,

    #[rust]
    active: Option<WidgetRef>,

    #[rust]
    templates: ComponentMap<LiveId, LivePtr>,

    #[rust]
    all_widgets: HashMap<OwnedRoomId, (LiveId, WidgetRef)>,
}

// This implementation block is based on the PortalList implementation.
impl LiveHook for TimelineList {
    fn before_apply(&mut self, _cx: &mut Cx, apply: &mut Apply, _index: usize, _nodes: &[LiveNode]) {
        if let ApplyFrom::UpdateFromDoc {..} = apply.from {
            self.templates.clear();
        }
    }
    
    // hook the apply flow to collect our templates and apply to instanced childnodes
    fn apply_value_instance(&mut self, cx: &mut Cx, apply: &mut Apply, index: usize, nodes: &[LiveNode]) -> usize {
        let id = nodes[index].id;
        match apply.from {
            ApplyFrom::NewFromDoc {file_id} | ApplyFrom::UpdateFromDoc {file_id} => {
                if nodes[index].origin.has_prop_type(LivePropType::Instance) {
                    let live_ptr = cx.live_registry.borrow().file_id_index_to_live_ptr(file_id, index);
                    self.templates.insert(id, live_ptr);
                    // Apply the new apply this thing over all our childnodes with that template
                    for (templ_id, node) in self.all_widgets.values_mut() {
                        if *templ_id == id {
                            node.apply(cx, apply, index, nodes);
                        }
                    }
                }
                else {
                    cx.apply_error_no_matching_field(live_error_origin!(), index, nodes);
                }
            }
            _ => ()
        }
        nodes.skip_node(index)
    }
}

impl Widget for TimelineList {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if let Some(active) = self.active.as_ref() {
            active.handle_event(cx, event, scope);
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep  {
        if let Some(active) = self.active.as_ref() {
            active.draw_walk(cx, scope, walk)?;
        }
        DrawStep::done()
    }
}

impl WidgetNode for TimelineList {
    fn walk(&mut self, cx:&mut Cx) -> Walk {
        self.view.walk(cx)
    }

    fn redraw(&mut self, cx: &mut Cx) {
        if let Some(active) = self.active.as_ref() {
            active.redraw(cx);
        }
    }
    
    fn find_widgets(&mut self, path: &[LiveId], cached: WidgetCache, results: &mut WidgetSet) {
        self.view.find_widgets(path, cached, results);
    }
}

impl TimelineList {
    /// Sets the active timeline being displayed to the one with the given `room_id`.
    pub fn set_active(&mut self, room_id: &OwnedRoomId, cx: &mut Cx) {
        if let Some((_, widget_ref)) = self.all_widgets.get(room_id) {
            self.active = Some(widget_ref.clone());
            self.redraw(cx);
        }
    }

    pub fn get_or_insert_timeline(&mut self, cx: &mut Cx, room_id: OwnedRoomId, template: LiveId) -> Option<WidgetRef> {
        if let Some(ptr) = self.templates.get(&template) {
            let entry = match self.all_widgets.entry(room_id) {
                std::collections::hash_map::Entry::Occupied(existing) => existing.into_mut(),
                std::collections::hash_map::Entry::Vacant(vacant) => vacant.insert(
                    //
                    // TODO: Here use new_from_ptr_with_scope() instead of new_from_ptr()
                    //
                    (template, WidgetRef::new_from_ptr(cx, Some(*ptr)))
                ),
            };
            return Some(entry.1.clone())
        }
        None
    }

    pub fn get_timeline(&self, room_id: &OwnedRoomId) -> Option<WidgetRef> {
        self.all_widgets.get(room_id).map(|(_, widget_ref)| widget_ref.clone())
    }

    pub fn remove_timeline(&mut self, room_id: &OwnedRoomId) -> Option<(LiveId, WidgetRef)> {
        self.all_widgets.remove(room_id)
    }
}


impl TimelineListRef {
    /// Sets the active timeline being displayed to the one with the given `room_id`.
    pub fn set_active(&self, room_id: &OwnedRoomId, cx: &mut Cx) {
        self.borrow_mut().map(|mut inner|
            inner.set_active(room_id, cx)
        );
    }

    pub fn get_or_insert_timeline(&self, cx: &mut Cx, room_id: OwnedRoomId, template: LiveId) -> Option<WidgetRef> {
        self.borrow_mut().and_then(|mut inner|
            inner.get_or_insert_timeline(cx, room_id, template)
        )
    }

    pub fn get_timeline(&self, room_id: &OwnedRoomId) -> Option<WidgetRef> {
        self.borrow().and_then(|inner|
            inner.get_timeline(room_id)
        )
    }

    pub fn remove_timeline(&self, room_id: &OwnedRoomId) -> Option<(LiveId, WidgetRef)> {
        self.borrow_mut().and_then(|mut inner|
            inner.remove_timeline(room_id)
        )
    }
}

//////////////////////////////////////////////////////////////////////////////
//////////////////////////////////////////////////////////////////////////////
//////////////////////////////////////////////////////////////////////////////


/// A message that is sent from a background async task to a room's timeline view
/// for the purpose of update the Timeline UI contents or metadata.
pub enum TimelineUpdate {
    /// A batch of diffs that should be applied to a timeline's `items` list.
    DiffBatch(Vec<VectorDiff<Arc<TimelineItem>>>),
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


/// The global set of all timelines and their states, one entry per room.
///
/// Note: in the future this should be stored directly in the `Timeline` widget,
/// but we cannot yet do that until Makepad supports dynamic widget creation with context
/// such that we can create new Timeline widgets dynamically with the required state
/// and then choose from them in the StackNavigation widget's ShowRoom action.
static TIMELINE_STATES: Mutex<BTreeMap<OwnedRoomId, TimelineState>> = Mutex::new(BTreeMap::new());

#[derive(Live, Widget)]
pub struct Timeline {
    #[deref] view: View,

    /// The ID of the room that this timeline is for.
    #[rust] room_id: Option<OwnedRoomId>,
}

struct TimelineState {
    /// The ID of the room that this timeline is for.
    room_id: OwnedRoomId,

    /// Whether this room's timeline has been fully paginated, which means
    /// that the oldest (first) event in the timeline is locally synced and available.
    /// When `true`, further backwards pagination requests will not be sent.
    fully_paginated: bool,

    /// The list of currently-known items in this room's timeline,
    /// which doesn't necessarily contain all timeline items as known by the server.
    ///
    /// This list is only directly accessed by the UI thread.
    /// An async background task receives updates for this timeline
    /// and then enqueues these updates in a room-specific queue,
    /// which the UI task then dequeues and applies to this list
    /// *after* the UI task has finished drawing the current frame.
    items: Vector<Arc<TimelineItem>>,

    /// The channel receiver for timeline updates for this room.
    ///
    /// Here we use a synchronous (non-async) channel because the receiver runs
    /// in a sync context and the sender runs in an async context,
    /// which is okay because a sender on an unbounded channel never needs to block.
    update_receiver: crossbeam_channel::Receiver<TimelineUpdate>,
}

// This struct is auto-generated by deriving `Widget` on `Timeline`.
impl TimelineRef {
    pub fn set_room_info(&self, room_id: OwnedRoomId) {
        if let Some(mut timeline) = self.borrow_mut() {
            // TODO: here, in the future when we move timeline state back into the timeline widget,
            //       we'll initialize all timeline state here.
            timeline.room_id = Some(room_id.clone());
        }
            
        let (first_time_showing_room, fully_paginated) = match TIMELINE_STATES.lock().unwrap().entry(room_id.clone()) {
            std::collections::btree_map::Entry::Occupied(tl_state) => {
                (false, tl_state.get().fully_paginated)
            }
            std::collections::btree_map::Entry::Vacant(entry) => {
                if let Some((items, update_receiver)) = take_timeline_update_receiver(&room_id) {
                    entry.insert(TimelineState {
                        room_id: room_id.clone(),
                        fully_paginated: false,
                        items,
                        update_receiver,
                    });
                }
                (true, false)
            }
        };

        // kick off a back pagination request for this room
        if !fully_paginated {
            submit_async_request(MatrixRequest::PaginateRoomTimeline {
                room_id: room_id.clone(),
                batch_size: 50,
                max_events: 50,
            })
        } else {
            println!("Note: skipping pagination request for room {} because it is already fully paginated.", room_id);
        }

        // Note: this isn't required any more because we now specify that room member profiles
        //       (of any users that sent messages in the room) should be lazy-loaded ("$LAZY" required state)
        //       by the initial sliding sync request.
        if false && first_time_showing_room {
            submit_async_request(MatrixRequest::FetchRoomMembers { room_id });
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
        // Currently, a Signal event is only used to tell this widget that its timeline events
        // have been updated in the background.
        if let Event::Signal = event {
            let mut timeline_states = TIMELINE_STATES.lock().unwrap();
            if let Some(tl) = timeline_states.get_mut(self.room_id.as_ref().unwrap()) {
                let mut num_updates = 0;
                let mut done_loading = false;
                while let Ok(update) = tl.update_receiver.try_recv() {
                    match update {
                        TimelineUpdate::DiffBatch(batch) => {
                            num_updates += batch.len();
                            for diff in batch {
                                diff.apply(&mut tl.items);
                            }
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
                            // Here, to be most efficient, we could redraw only the user avatars in the timeline,
                            // but for now we just fall through and let the final `redraw()` call re-draw the whole timeline view.
                        }
                    }
                }

                if num_updates > 0 {
                    println!("Timeline::handle_event(): applied {num_updates} updates for room {}", tl.room_id);
                }
                if done_loading {
                    println!("TODO: hide topspace loading animation for room {}", tl.room_id);
                    // TODO FIXME: hide TopSpace loading animation, set it to invisible.
                }
                
                self.redraw(cx);
            }
        }


        self.view.handle_event(cx, event, scope);

        // TODO: handle actions upon an item being clicked.
        // for (item_id, item) in self.list.items_with_actions(&actions) {
        //     if item.button(id!(likes)).clicked(&actions) {
        //         log!("hello {}", item_id);
        //     }
        // }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        let tl_items = TIMELINE_STATES.lock().unwrap().get(self.room_id.as_ref().unwrap()).map(|tl| tl.items.clone());

        // Determine length of the portal list based on the number of timeline items.
        let last_item_id = tl_items.as_ref().map(|i| i.len() as u64).unwrap_or(0);
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
                    let Some(timeline_item) = tl_items.as_ref().and_then(|t| t.get(tl_idx)) else {
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
) -> WidgetRef {
    let item = list.item(cx, item_id, live_id!(Message)).unwrap();
    item.label(id!(content.message)).set_text(message.body());

    let display_user_id = |user_id: &str| {
        item.label(id!(content.username)).set_text(user_id);
        item.avatar(id!(profile.avatar)).set_text(
            user_id.graphemes(true).skip(1).next().map(ToString::to_string).unwrap_or_default()
        );
    };

    // Set sender to the display name if available, otherwise the user id.
    match event_tl_item.sender_profile() {
        TimelineDetails::Ready(profile) => {
            // Set the sender's avatar image (or a text character if no image is available).
            if let Some(name) = &profile.display_name {
                item.label(id!(content.username)).set_text(&name);
                item.avatar(id!(profile.avatar)).set_text(
                    name.graphemes(true).next().map(ToString::to_string).unwrap_or_default()
                );
            } else {
                display_user_id(event_tl_item.sender().as_str());
            }

            if let Some(_url) = &profile.avatar_url {
                // TODO: fetch avatar image based on URL
            }
        }
        _other => {
            // println!("populate_message_view(): sender profile not ready yet for event {_other:?}");
            display_user_id(event_tl_item.sender().as_str());
        }
    }

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
        _other => {
            // println!("*** Unhandled: {:?}", _other);
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
