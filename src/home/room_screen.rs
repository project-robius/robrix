//! A room screen is the UI page that displays a single Room's timeline of events/messages
//! along with a message input bar at the bottom.

use std::{borrow::Cow, collections::BTreeMap, ops::{Deref, DerefMut, Range}, sync::{Arc, Mutex}};

use imbl::Vector;
use makepad_widgets::*;
use matrix_sdk::{ruma::{
    events::{
        room::{
            guest_access::GuestAccess, history_visibility::HistoryVisibility, join_rules::JoinRule, message::{MessageFormat, MessageType, RoomMessageEventContent}, MediaSource
        },
        AnySyncMessageLikeEvent, AnySyncTimelineEvent, FullStateEventContent, SyncMessageLikeEvent,
    }, matrix_uri::MatrixId, uint, MatrixToUri, MatrixUri, MilliSecondsSinceUnixEpoch, OwnedEventId, OwnedRoomId, RoomId
}, OwnedServerName};
use matrix_sdk_ui::timeline::{
    self, AnyOtherFullStateEventContent, EventTimelineItem, MemberProfileChange, MembershipChange, ReactionsByKeyBySender, RoomMembershipChange, TimelineDetails, TimelineItem, TimelineItemContent, TimelineItemKind, VirtualTimelineItem
};

use rangemap::RangeSet;
use crate::{
    avatar_cache::{self, AvatarCacheEntry}, media_cache::{MediaCache, MediaCacheEntry}, profile::{user_profile::{AvatarState, ShowUserProfileAction, UserProfile, UserProfileAndRoomId, UserProfilePaneInfo, UserProfileSlidingPaneRef, UserProfileSlidingPaneWidgetExt}, user_profile_cache}, shared::{avatar::{AvatarRef, AvatarWidgetRefExt}, html_or_plaintext::HtmlOrPlaintextWidgetRefExt, text_or_image::TextOrImageWidgetRefExt}, sliding_sync::{get_client, submit_async_request, take_timeline_update_receiver, MatrixRequest}, utils::{self, unix_time_millis_to_datetime, MediaFormatConst}
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
    import crate::shared::html_or_plaintext::*;
    import crate::profile::user_profile::UserProfileSlidingPane;

    IMG_DEFAULT_AVATAR = dep("crate://self/resources/img/default_avatar.png")
    ICO_FAV = dep("crate://self/resources/icon_favorite.svg")
    ICO_COMMENT = dep("crate://self/resources/icon_comment.svg")
    ICO_REPLY = dep("crate://self/resources/icon_reply.svg")
    ICO_SEND = dep("crate://self/resources/icon_send.svg")
    ICO_LIKES = dep("crate://self/resources/icon_likes.svg")
    ICO_USER = dep("crate://self/resources/icon_user.svg")
    ICO_ADD = dep("crate://self/resources/icon_add.svg")

    TEXT_SUB = {
        font_size: (10),
        font: {path: dep("crate://makepad-widgets/resources/GoNotoKurrent-Regular.ttf")}
    }
    
    TEXT_P = {
        font_size: (12),
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
    COLOR_READ_MARKER = #xeb2733
    COLOR_PROFILE_CIRCLE = #xfff8ee
    
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

    Timestamp = <Label> {
        padding: { top: 10.0, bottom: 0.0, left: 0.0, right: 0.0 }
        draw_text: {
            text_style: <TIMESTAMP_TEXT_STYLE> {},
            color: (TIMESTAMP_TEXT_COLOR)
        }
        text: " "
    }
    
    REACTION_TEXT_COLOR = #4c00b0

    MessageMenu = <View> {
        visible: false,
        width: Fill,
        height: Fit,

        // TODO: use a set of Buttons later, so a user can click to add their own reaction.
        annotations = <RobrixHtml> {
            width: Fill,
            height: Fit,
            padding: {top: 7.5, bottom: 5.0 },
            font_size: 10.5,
            draw_normal:      { color: (REACTION_TEXT_COLOR) },
            draw_italic:      { color: (REACTION_TEXT_COLOR) },
            draw_bold:        { color: (REACTION_TEXT_COLOR) },
            draw_bold_italic: { color: (REACTION_TEXT_COLOR) },
            draw_fixed:       { color: (REACTION_TEXT_COLOR) },
            body: ""
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
                    width: Fill,
                    margin: {bottom: 10.0, top: 10.0, right: 10.0,}
                    draw_text: {
                        text_style: <USERNAME_TEXT_STYLE> {},
                        color: (USERNAME_TEXT_COLOR)
                        wrap: Ellipsis,
                    }
                    text: "<Username not available>"
                }
                message = <HtmlOrPlaintext> { }
                
                // <LineH> {
                //     margin: {top: 13.0, bottom: 5.0}
                // }
                
                message_menu = <MessageMenu> {}
            }
        }
    }

    // The view used for a condensed message that came right after another message
    // from the same sender, and thus doesn't need to display the sender's profile again.
    CondensedMessage = <Message> {
        padding: { top: 2.0, bottom: 2.0 }
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
                
                message = <HtmlOrPlaintext> { }
                message_menu = <MessageMenu> {}
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
                    image_view = { image = { fit: Horizontal } }
                }
                message_menu = <MessageMenu> {}
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
                    image_view = { image = { fit: Horizontal } }
                }
                message_menu = <MessageMenu> {}
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
        padding: { top: 1.0, bottom: 1.0 }
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
                        text_style: <TIMESTAMP_TEXT_STYLE> {},
                        color: (TIMESTAMP_TEXT_COLOR)
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
                    text_style: <SMALL_STATE_TEXT_STYLE> {},
                    color: (SMALL_STATE_TEXT_COLOR)
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

        <View> {
            width: Fill, height: Fill,
            flow: Overlay,
            
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
                        empty_message: "Write a message (in Markdown) ..."
                        draw_bg: {
                            color: #F9F9F9
                        }
                        draw_text: {
                            color: (MESSAGE_TEXT_COLOR),
                            text_style: <MESSAGE_TEXT_STYLE>{},

                            fn get_color(self) -> vec4 {
                                return mix(
                                    mix(
                                        mix(
                                            #xFFFFFF55,
                                            #xFFFFFF88,
                                            self.hover
                                        ),
                                        self.color,
                                        self.focus
                                    ),
                                    #BBBBBB,
                                    self.is_empty
                                )
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

            <View> {
                width: Fill,
                height: Fill,
                align: { x: 1.0 },
                flow: Right,

                user_profile_sliding_pane = <UserProfileSlidingPane> { }
            }
        }
    }
}

/// A simple deref wrapper around the `RoomScreen` widget that enables us to handle its events.
#[derive(Live, LiveHook, Widget)]
struct RoomScreen {
    #[deref] view: View,
    #[rust] room_id: Option<OwnedRoomId>,
    #[rust] room_name: String,
}
impl Widget for RoomScreen {
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }

    // Handle events and actions at the RoomScreen level.
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope){
        // A UI Signal indicates that something was updated in the background,
        // so we first check to see if it was a user profile update.
        if let Event::Signal = event {
            user_profile_cache::process_user_profile_updates(cx);
            avatar_cache::process_avatar_updates(cx);
            
        }

        let pane = self.user_profile_sliding_pane(id!(user_profile_sliding_pane));
        let timeline = self.timeline(id!(timeline));

        if let Event::Actions(actions) = event {
            // Handle the send message button being clicked.
            if self.button(id!(send_message_button)).clicked(&actions) {
                let msg_input_widget = self.text_input(id!(message_input));
                let entered_text = msg_input_widget.text();
                msg_input_widget.set_text_and_redraw(cx, "");
                if !entered_text.is_empty() {
                    let room_id = self.room_id.clone().unwrap();
                    log!("Sending message to room {}: {:?}", room_id, entered_text);
                    let message = if let Some(html_text) = entered_text.strip_prefix("/html") {
                        RoomMessageEventContent::text_html(html_text, html_text)
                    } else if let Some(plain_text) = entered_text.strip_prefix("/plain") {
                        RoomMessageEventContent::text_plain(plain_text)
                    } else {
                        RoomMessageEventContent::text_markdown(entered_text)
                    };
                    submit_async_request(MatrixRequest::SendMessage {
                        room_id,
                        message,
                        // TODO: support replies to specific messages, attaching mentions, rich text (html), etc.
                    });
                }
            }

            // Handle a typing action on the message input box.
            if let Some(new_text) = self.text_input(id!(message_input)).changed(actions) {
                submit_async_request(MatrixRequest::SendTypingNotice {
                    room_id: self.room_id.clone().unwrap(),
                    typing: !new_text.is_empty(),
                });
            }

            for action in actions {
                // Handle the action that requests to show the user profile sliding pane.
                if let ShowUserProfileAction::ShowUserProfile(profile_and_room_id) = action.as_widget_action().cast() {
                    timeline.show_user_profile(
                        cx,
                        &pane,
                        UserProfilePaneInfo {
                            profile_and_room_id,
                            room_name: self.room_name.clone(),
                            room_member: None,
                        },
                    );
                }

                // Handle a link being clicked.
                if let HtmlLinkAction::Clicked { url, .. } = action.as_widget_action().cast() {
                    // A closure that handles both MatrixToUri and MatrixUri links.
                    let mut handle_uri = |id: &MatrixId, _via: &[OwnedServerName]| -> bool {
                        match id {
                            MatrixId::Room(room_id) => {
                                if self.room_id.as_ref() == Some(room_id) {
                                    return true;
                                }
                                if let Some(_known_room) = get_client().and_then(|c| c.get_room(room_id)) {
                                    log!("TODO: jump to known room {}", room_id);
                                } else {
                                    log!("TODO: fetch and display room preview for room {}", room_id);
                                }

                                true
                            }
                            MatrixId::RoomAlias(room_alias) => {
                                log!("TODO: open room alias {}", room_alias);
                                // TODO: open a room loading screen that shows a spinner
                                //       while our background async task calls Client::resolve_room_alias()
                                //       and then either jumps to the room if known, or fetches and displays
                                //       a room preview for that room.
                                true
                            }
                            MatrixId::User(user_id) => {
                                log!("Opening matrix.to user link for {}", user_id);

                                // There is no synchronous way to get the user's full profile info
                                // including the details of their room membership,
                                // so we fill in with the details we *do* know currently,
                                // show the UserProfileSlidingPane, and then after that,
                                // the UserProfileSlidingPane itself will fire off
                                // an async request to get the rest of the details.
                                timeline.show_user_profile(
                                    cx,
                                    &pane,
                                    UserProfilePaneInfo {
                                        profile_and_room_id: UserProfileAndRoomId {
                                            user_profile: UserProfile {
                                                user_id: user_id.to_owned(),
                                                username: None,
                                                avatar_state: AvatarState::Unknown,
                                            },
                                            room_id: self.room_id.clone().unwrap(),
                                        },
                                        room_name: self.room_name.clone(),
                                        // TODO: use the extra `via` parameters
                                        room_member: None,
                                    },
                                );
                                true
                            }
                            MatrixId::Event(room_id, event_id) => {
                                log!("TODO: open event {} in room {}", event_id, room_id);
                                // TODO: this requires the same first step as the `MatrixId::Room` case above,
                                //       but then we need to call Room::event_with_context() to get the event
                                //       and its context (surrounding events ?).
                                true
                            }
                            _ => false,
                        }
                    };

                    let mut link_was_handled = false;
                    if let Ok(matrix_to_uri) = MatrixToUri::parse(&url) {
                        link_was_handled |= handle_uri(matrix_to_uri.id(), matrix_to_uri.via());
                    }
                    if let Ok(matrix_uri) = MatrixUri::parse(&url) {
                        link_was_handled |= handle_uri(matrix_uri.id(), matrix_uri.via());
                    }

                    if !link_was_handled {
                        if let Err(e) = robius_open::Uri::new(&url).open() {
                            error!("Failed to open URL {:?}. Error: {:?}", url, e);
                        }
                    }
                }
            }
        }

        // Only forward visibility-related events (touch/tap/scroll) to the inner timeline view
        // if the user profile sliding pane is not visible.
        if event.requires_visibility() && pane.is_currently_shown(cx) {
            // Forward the event to the user profile sliding pane,
            // preventing the underlying timeline view from receiving it.
            pane.handle_event(cx, event, scope);
        } else {
            // Forward the event to the inner timeline view.
            self.view.handle_event(cx, event, scope);
        }
    }
}

impl RoomScreenRef {
    /// Sets this `RoomScreen` widget to display the timeline for the given room.
    pub fn set_displayed_room(&self, room_name: String, room_id: OwnedRoomId) {
        let Some(mut room_screen) = self.borrow_mut() else { return };
        room_screen.room_name = room_name;
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
        /// The range of indices in the `items` list that have been changed in this update
        /// and thus must be removed from any caches of drawn items in the timeline.
        /// Any items outside of this range are assumed to be unchanged and need not be redrawn.
        changed_indices: Range<usize>,
        /// Whether to clear the entire cache of drawn items in the timeline.
        /// This supercedes `index_of_first_change` and is used when the entire timeline is being redrawn.
        clear_cache: bool,
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
    
    /// The room ID that this timeline is currently displaying.
    #[rust] room_id: Option<OwnedRoomId>,
    /// The UI-relevant states for the room that this widget is currently displaying.
    #[rust] tl_state: Option<TimelineUiState>,
    /// The timestamp when App becomes in the background
    #[rust] last_read_event:Option<(OwnedEventId,MilliSecondsSinceUnixEpoch)>,
    /// The timestamp for the last 
    #[rust] last_scroll_to_end_time:Option<std::time::Instant>,
    /// Last EventId sent to backend for read receipt
    #[rust] last_read_event_id: Option<OwnedEventId>,
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
    ///
    /// This must be reset to `false` whenever the timeline is fully cleared.
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
    
    /// The index and scroll position of the first three events that have been drawn
    /// in the most recent draw pass of this timeline's PortalList.
    ///
    /// We save three events because one of 3 adjacent timeline items is (practically)
    /// guaranteed to be a standard real event that has a true unique ID.
    /// (For example, not day dividers, not read markers, etc.)
    ///
    /// If any of the `event_ids` are `Some`, this indicates that the timeline was
    /// fully cleared and is in the process of being restored via pagination,
    /// but it has not yet been paginated enough to the point where one of events
    /// in this list are visible.
    /// Once the timeline has been sufficiently paginated to display
    /// one of the events in this list, all `event_ids` should be set to `None`.`
    first_three_events: FirstDrawnEvents<3>,

    /// The states relevant to the UI display of this timeline that are saved upon
    /// a `Hide` action and restored upon a `Show` action.
    saved_state: SavedState,
}

/// The item index, scroll position, and optional unique IDs of the first `N` events
/// that have been drawn in the most recent draw pass of a timeline's PortalList.
#[derive(Debug)]
struct FirstDrawnEvents<const N: usize> {
    index_and_scroll: [ItemIndexScroll; N],
    event_ids: [Option<OwnedEventId>; N],
}
impl<const N: usize> Default for FirstDrawnEvents<N> {
    fn default() -> Self {
        Self {
            index_and_scroll: std::array::from_fn(|_| ItemIndexScroll::default()),
            event_ids: std::array::from_fn(|_| None),
        }
    }
}

/// 
#[derive(Clone, Copy, Debug, Default)]
struct ItemIndexScroll {
    index: usize,
    scroll: f64,
}

/// States that are necessary to save in order to maintain a consistent UI display for a timeline.
///
/// These are saved when navigating away from a timeline (upon `Hide`)
/// and restored when navigating back to a timeline (upon `Show`).
#[derive(Default, Debug)]
struct SavedState {
    /// The index of the first item in the timeline's PortalList that is currently visible,
    /// and the scroll offset from the top of the list's viewport to the beginning of that item.
    /// If this is `None`, then the timeline has not yet been scrolled by the user
    /// and the portal list will be set to "tail" (track) the bottom of the list.
    first_index_and_scroll: Option<(usize, f64)>,
    /// The unique ID of the event that corresponds to the first item visible in the timeline.
    first_event_id: Option<OwnedEventId>,

    /// The content of the message input box.
    draft: Option<String>,
    /// The position of the cursor head and tail in the message input box.
    cursor: (usize, usize),
}

impl Timeline {
    /// Invoke this when this timeline is being shown,
    /// e.g., when the user navigates to this timeline.
    fn show_timeline(&mut self) {
        let room_id = self.room_id.clone()
            .expect("BUG: Timeline::show_timeline(): no room_id was set.");
        assert!( // just an optional sanity check
            self.tl_state.is_none(),
            "BUG: tried to show_timeline() into a timeline with existing state. \
            Did you forget to save the timeline state back to the global map of states?",
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
                update_receiver,
                first_three_events: Default::default(),
                media_cache: MediaCache::new(MediaFormatConst::File, Some(update_sender)),
                saved_state: SavedState::default(),
            };
            (new_tl_state, true)
        };

        // log!("Timeline::set_room(): opening room {room_id}
        //     content_drawn_since_last_update: {:#?}
        //     profile_drawn_since_last_update: {:#?}",
        //     tl_state.content_drawn_since_last_update,
        //     tl_state.profile_drawn_since_last_update,
        // );

        // kick off a back pagination request for this room
        if !tl_state.fully_paginated {
            submit_async_request(MatrixRequest::PaginateRoomTimeline {
                room_id: room_id.clone(),
                num_events: 50,
                forwards: false,
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

        // Now, restore the visual state of this timeline from its previously-saved state.
        self.restore_state(&tl_state);

        // As the final step , store the tl_state for this room into the Timeline widget,
        // such that it can be accessed in future event/draw handlers.
        self.tl_state = Some(tl_state);
    }


    /// Invoke this when this timeline is being hidden or no longer being shown,
    /// e.g., when the user navigates away from this timeline.
    fn hide_timeline(&mut self) {
        self.save_state();
    }

    /// Removes this Timeline's current visual UI state from this Timeline widget
    /// and saves it to the map of `TIMELINE_STATES` such that it can be restored later.
    ///
    /// Note: after calling this function, the timeline's `tl_state` will be `None`.
    fn save_state(&mut self) {
        let Some(mut tl) = self.tl_state.take() else {
            error!("Timeline::save_state(): skipping due to missing state, room {:?}", self.room_id);
            return;
        };
        let portal_list = self.portal_list(id!(list));
        let first_index = portal_list.first_id();
        tl.saved_state.first_index_and_scroll = Some((
            first_index,
            portal_list.scroll_position(),
        ));
        tl.saved_state.first_event_id = tl.items
            .get(first_index)
            .and_then(|item| item
                .as_event()
                .and_then(|ev| ev.event_id().map(|i| i.to_owned()))
            );


        // Store this Timeline's `TimelineUiState` in the global map of states.
        TIMELINE_STATES.lock().unwrap().insert(tl.room_id.clone(), tl);
    }

    /// Restores the previously-saved visual UI state of this timeline.
    ///
    /// Note: this accepts a direct reference to the timeline's UI state,
    /// so this function must not try to re-obtain it by accessing `self.tl_state`.
    fn restore_state(&mut self, tl_state: &TimelineUiState) {
        if let Some((first_index, scroll_from_first_id)) = tl_state.saved_state.first_index_and_scroll {
            self.portal_list(id!(list))
                .set_first_id_and_scroll(first_index, scroll_from_first_id);
        } else {
            // If the first index is not set, then the timeline has not yet been scrolled by the user,
            // so we set the portal list to "tail" (track) the bottom of the list.
            self.portal_list(id!(list)).set_tail_range(true);
        }

        // TODO: restore the message input box's draft text and cursor head/tail positions.
    }
}

impl TimelineRef {
    /// Sets this timeline widget to display the timeline for the given room.
    fn set_room(&self, room_id: OwnedRoomId) {
        let Some(mut timeline) = self.borrow_mut() else { return };
        timeline.room_id = Some(room_id);
    }

    /// Shows the user profile sliding pane with the given avatar info.
    fn show_user_profile(
        &self,
        cx: &mut Cx,
        pane: &UserProfileSlidingPaneRef,
        info: UserProfilePaneInfo,
    ) {
        let Some(mut inner) = self.borrow_mut() else { return };
        pane.set_info(cx, info);
        pane.show(cx);
        // Not sure if this redraw is necessary
        inner.redraw(cx);
    }
}

impl Widget for Timeline {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        // Implements bottom-half screen click area to register fully read receipt
        if let (Event::MouseDown(mouse),Some(room_id)) = (event,self.room_id.clone()){
            let height = self.view.area().rect(cx).size.y;
            if self.view.area().rect(cx).translate(DVec2{
                x:0.0,
                y:height/2.0
            }).contains(mouse.abs){
                if let Some(tl_state) = &self.tl_state{      
                    let len = tl_state.items.len();
                    if let Some(last_drawn) = tl_state.content_drawn_since_last_update.last().and_then(|f| Some(f.end)){
                        if last_drawn <=len {
                            if let Some(last_drawn_event_id) = tl_state.items.get(last_drawn-1).and_then(|f|f.as_event().and_then(|f|f.event_id())){
                                submit_async_request(MatrixRequest::FullyReadReceipt { room_id: room_id.clone(), event_id: last_drawn_event_id.to_owned() });
                            }
                        }
                    }
                   
                }
            }
        }
        
        if let Event::Actions(actions) = event {
            for action in actions {
                // Handle the timeline being hidden or shown.
                match action.as_widget_action().cast() {
                    StackNavigationTransitionAction::HideBegin => {
                        self.hide_timeline();
                        continue;
                    }
                    StackNavigationTransitionAction::ShowBegin => {
                        self.show_timeline();
                        self.redraw(cx);
                        continue;
                    }
                    StackNavigationTransitionAction::HideEnd
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

        // Currently, a Signal event is only used to tell this widget
        // that its timeline events have been updated in the background.
        if let Event::Signal = event {
            let portal_list = self.portal_list(id!(list));
            let orig_first_id = portal_list.first_id();
            let scroll_from_first_id = portal_list.scroll_position();
            let Some(tl) = self.tl_state.as_mut() else { return };

            let mut done_loading = false;
            while let Ok(update) = tl.update_receiver.try_recv() {
                match update {
                    TimelineUpdate::NewItems { items, changed_indices, clear_cache } => {
                        // Determine which item is currently visible the top of the screen (the first event)
                        // so that we can jump back to that position instantly after applying this update.
                        let current_first_event_id_opt = tl.items
                            .get(orig_first_id)
                            .and_then(|item| item.as_event()
                                .and_then(|ev| ev.event_id().map(|i| i.to_owned()))
                            );
                        
                        log!("current_first_event_id_opt: {current_first_event_id_opt:?}, orig_first_id: {orig_first_id}, old items: {}, new items: {}",
                            tl.items.len(), items.len(),
                        );

                        if items.is_empty() {
                            log!("Timeline::handle_event(): timeline was cleared for room {}", tl.room_id);

                            // If the bottom of the timeline (the last event) is visible, then we should
                            // set the timeline to live mode.
                            // If the bottom of the timelien is *not* visible, then we should
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
                        }else{
                            // Save Last Event id and it's timestamp, which will be used to send read receipt
                            let current_last_event_id_opt = tl.items
                            .get(items.len())
                            .and_then(|item| item.as_event()
                                .and_then(|ev|{
                                    if let Some(event_id) = ev.event_id(){
                                        Some((event_id,ev.timestamp()))
                                    }else{
                                        None
                                    }
                                } )
                            );
                            if let Some((item_event_id,timestamp)) = current_last_event_id_opt{
                                if let Some((ref mut last_event_id,ref mut last_timestamp)) = self.last_read_event{
                                    if timestamp.as_secs() > last_timestamp.as_secs(){
                                        *last_event_id = item_event_id.to_owned();
                                        *last_timestamp = timestamp;
                                    }
                                }else {
                                    self.last_read_event = Some((item_event_id.to_owned(),timestamp));
                                }
                            }
                        }

                        // Maybe todo?: we can often avoid the following loops that iterate over the `items` list
                        //       by only doing that if `clear_cache` is true, or if `changed_indices` range includes
                        //       any index that comes before (is less than) the above `orig_first_id`.



                        if let Some(top_event_id) = current_first_event_id_opt.as_ref() {
                            for (idx, item) in items.iter().enumerate() {
                                let Some(item_event_id) = item.as_event().and_then(|ev| ev.event_id()) else {
                                    continue
                                };
                                if top_event_id.deref() == item_event_id {
                                    if orig_first_id != idx {
                                        log!("Timeline::handle_event(): jumping view from top event index {orig_first_id} to new index {idx}");
                                        portal_list.set_first_id_and_scroll(idx, scroll_from_first_id);
                                    }
                                    break;
                                } else if tl.saved_state.first_event_id.as_deref() == Some(item_event_id) {
                                    // TODO: should we only do this if `clear_cache` is true? (e.g., after an (un)ignore event)
                                    log!("!!!!!!!!!!!!!!!!!!!!!!! Timeline::handle_event(): jumping view from saved first event ID to index {idx}");
                                    portal_list.set_first_id_and_scroll(idx, scroll_from_first_id);
                                    break;
                                }
                            }
                        } else {
                            warning!("Couldn't get unique event ID for event at the top of room {:?}", tl.room_id);
                        }

                        if clear_cache {
                            tl.content_drawn_since_last_update.clear();
                            tl.profile_drawn_since_last_update.clear();
                            tl.fully_paginated = false;
                        } else {
                            tl.content_drawn_since_last_update.remove(changed_indices.clone());
                            tl.profile_drawn_since_last_update.remove(changed_indices.clone());
                            // log!("Timeline::handle_event(): changed_indices: {changed_indices:?}, items len: {}\ncontent drawn: {:#?}\nprofile drawn: {:#?}", items.len(), tl.content_drawn_since_last_update, tl.profile_drawn_since_last_update);
                        }
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
                    }
                    TimelineUpdate::MediaFetched => {
                        log!("Timeline::handle_event(): media fetched for room {}", tl.room_id);
                        // Here, to be most efficient, we could redraw only the media items in the timeline,
                        // but for now we just fall through and let the final `redraw()` call re-draw the whole timeline view.
                    }
                }
            }

            if done_loading {
                log!("TODO: hide topspace loading animation for room {}", tl.room_id);
                // TODO FIXME: hide TopSpace loading animation, set it to invisible.
            }
            
            self.redraw(cx);
        }

        // Forward events to this Timeline's inner child view.
        self.view.handle_event(cx, event, scope);
    }


    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        let Some(tl_state) = self.tl_state.as_mut() else {
            return DrawStep::done()
        };
        let room_id = &tl_state.room_id;
        let tl_items = &tl_state.items;

        // Determine length of the portal list based on the number of timeline items.
        let last_item_id = tl_items.len();
        let last_item_id = last_item_id + 1; // Add 1 for the TopSpace.
        
        // Start the actual drawing procedure.
        while let Some(subview) = self.view.draw_walk(cx, scope, walk).step() {
            // We only care about drawing the portal list.
            let portal_list_ref = subview.as_portal_list();
            let Some(mut list_ref) = portal_list_ref.borrow_mut() else { continue };
            let list = list_ref.deref_mut();
            // When scrolled to the bottom, send a read receipt with last event_id. Although Matrix SDK prevents sending read receipt for older event,
            // timestamp of event for read receipt is also checked here.
            if list.is_at_end(){
                if let  Some(ref mut last_scroll_to_end_time) = self.last_scroll_to_end_time{
                    if last_scroll_to_end_time.elapsed() >std::time::Duration::new(2,0){
                        if let Some((ref event_id,_)) = self.last_read_event{
                            if let Some(ref mut last_read_event_id) = self.last_read_event_id{
                                if event_id!=last_read_event_id{
                                    *last_read_event_id = event_id.clone();
                                    submit_async_request(MatrixRequest::ReadReceipt { room_id: room_id.clone(),event_id:event_id.clone() });
                                    *last_scroll_to_end_time = std::time::Instant::now();
                                }
                            }else{
                                self.last_read_event_id = Some(event_id.clone());
                                submit_async_request(MatrixRequest::ReadReceipt { room_id: room_id.clone(),event_id:event_id.clone() });
                                *last_scroll_to_end_time = std::time::Instant::now();
                            }
                        }
                    }
                }else{
                    if let Some((ref event_id,_)) = self.last_read_event{
                        if let Some(ref mut last_read_event_id) = self.last_read_event_id{
                            if event_id!=last_read_event_id{
                                submit_async_request(MatrixRequest::ReadReceipt { room_id: room_id.clone(),event_id:event_id.clone() });
                                self.last_scroll_to_end_time = Some(std::time::Instant::now());
                                *last_read_event_id = event_id.clone();
                            }
                        }else{
                            self.last_read_event_id = Some(event_id.clone());
                            submit_async_request(MatrixRequest::ReadReceipt { room_id: room_id.clone(),event_id:event_id.clone() });
                            self.last_scroll_to_end_time = Some(std::time::Instant::now());
                        }
                       
                    }
                }
                    
            }
            list.set_item_range(cx, 0, last_item_id);

            let mut item_index_and_scroll_iter = tl_state.first_three_events.index_and_scroll.iter_mut();

            while let Some((item_id, scroll)) = list.next_visible_item_with_scroll(cx) {
                // log!("Drawing item {} at scroll: {}", item_id, scroll_offset);
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

                    if let Some(index_and_scroll) = item_index_and_scroll_iter.next() {
                        // log!("########### Saving item ID {} and scroll {} for room {}, at_end? {}",
                        //     item_id, scroll, room_id,
                        //     if list.is_at_end() { "Y" } else { "N" },
                        // );
                        *index_and_scroll = ItemIndexScroll { index: item_id, scroll };
                    }

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
                                    room_id,
                                    event_tl_item,
                                    message,
                                    prev_event,
                                    &mut tl_state.media_cache,
                                    item_drawn_status,
                                )
                            }
                            TimelineItemContent::RedactedMessage => populate_small_state_event(
                                cx,
                                list,
                                item_id,
                                room_id,
                                event_tl_item,
                                &RedactedMessageEventMarker,
                                item_drawn_status,
                            ),
                            TimelineItemContent::MembershipChange(membership_change) => populate_small_state_event(
                                cx,
                                list,
                                item_id,
                                room_id,
                                event_tl_item,
                                membership_change,
                                item_drawn_status,
                            ),
                            TimelineItemContent::ProfileChange(profile_change) => populate_small_state_event(
                                cx,
                                list,
                                item_id,
                                room_id,
                                event_tl_item,
                                profile_change,
                                item_drawn_status,
                            ),
                            TimelineItemContent::OtherState(other) => populate_small_state_event(
                                cx,
                                list,
                                item_id,
                                room_id,
                                event_tl_item,
                                other,
                                item_drawn_status,
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
                                .map(|dt| format!("{}", dt.date_naive().format("%a %b %-d, %Y")))
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


        // Note: we shouldn't need to save any states here, as the `TimelineUpdate::NewItems` event handler
        //       will be able to query the event ID of the first/top item in the timeline 
        //       **BEFORE** it actually applies the new items to the timeline's TimelineUiState.

        /*
        let first_index = portal_list.first_id();
        let scroll_from_first_id = portal_list.scroll_position();

        // TODO: the PortalList doesn't support this yet, but we should get the scroll positions
        //       of other nearby item IDs as well, in case the first item ID corresponds to
        //       a virtual event or an event that doesn't have a valid `event_id()`,
        //       such that we can jump back to the same relative position in the timeline after an update.
        let first_event_id = tl_items
            .get(first_index)
            .and_then(|item| item.as_event()
                .and_then(|ev| ev.event_id().map(|i| i.to_owned()))
            );
        tl_state.saved_state.first_event_id = first_event_id;
        */

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


/// Creates, populates, and adds a Message liveview widget to the given `PortalList`
/// with the given `item_id`.
///
/// The content of the returned `Message` widget is populated with data from the given `message`
/// and its parent `EventTimelineItem`.
fn populate_message_view(
    cx: &mut Cx2d,
    list: &mut PortalList,
    item_id: usize,
    room_id: &RoomId,
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
                let msg_body_field = item.html_or_plaintext(id!(content.message));
                // Draw the message body, either as rich HTML or as plaintext.
                if let Some(formatted_body) = text.formatted.as_ref()
                    .and_then(|fb| (fb.format == MessageFormat::Html).then(|| fb.body.clone()))
                {
                    msg_body_field.show_html(utils::linkify(formatted_body.as_ref()));
                } else {
                    match utils::linkify(&text.body) {
                        Cow::Owned(linkified_html) => msg_body_field.show_html(&linkified_html),
                        Cow::Borrowed(plaintext)   => msg_body_field.show_plaintext(plaintext),
                    }
                }
                // Draw any reactions to the message.
                draw_reactions(cx, &item, event_tl_item.reactions(), item_id - 1);
                // We're done drawing the message content, so mark it as fully drawn.
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
                // We also don't trust the provided mimetype, as it can be incorrect.
                let (_mimetype, _width, _height) = if let Some(info) = image.info.as_ref() {
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
                        // now that we've obtained the image URI and its metadata, try to fetch the image.
                        match media_cache.try_get_media_or_fetch(mxc_uri.clone(), None) {
                            MediaCacheEntry::Loaded(data) => {
                                let show_image_result = text_or_image_ref.show_image(|img|
                                    utils::load_png_or_jpg(&img, cx, &data)
                                        .map(|()| img.size_in_pixels(cx).unwrap())
                                );
                                if let Err(e) = show_image_result {
                                    let err_str = format!("Failed to display image: {e:?}");
                                    error!("{err_str}");
                                    text_or_image_ref.set_text(&err_str);
                                }
                                // Draw any reactions to the message.
                                draw_reactions(cx, &item, event_tl_item.reactions(), item_id - 1);
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
                let kind = other.msgtype();
                item.label(id!(content.message)).set_text(&format!("[TODO {kind:?}] {}", other.body()));
                new_drawn_status.content_drawn = true;
                (item, false)
            }
        }
    };

    // If `used_cached_item` is false, we should always redraw the profile, even if profile_drawn is true.
    let skip_draw_profile = use_compact_view || (used_cached_item && item_drawn_status.profile_drawn);
    if skip_draw_profile {
        // log!("\t --> populate_message_view(): SKIPPING profile draw for item_id: {item_id}");
        new_drawn_status.profile_drawn = true;
    } else {
        // log!("\t --> populate_message_view(): DRAWING  profile draw for item_id: {item_id}");
        let (username, profile_drawn) = set_avatar_and_get_username(
            cx,
            item.avatar(id!(profile.avatar)),
            room_id,
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
            &format!("{}", dt.date_naive())
        );
    } else {
        item.label(id!(profile.timestamp)).set_text(
            &format!("{}", ts_millis.get())
        );
    }

    (item, new_drawn_status)
} 



fn draw_reactions(_cx: &mut Cx2d, message_item: &WidgetRef, reactions: &ReactionsByKeyBySender, id: usize) {
    const DRAW_ITEM_ID_REACTION: bool = false;
    if reactions.is_empty() && !DRAW_ITEM_ID_REACTION {
        return;
    }
    // The message menu is invisible by default, so we must set it to visible
    // now that we know there are reactions to show.
    message_item.view(id!(content.message_menu)).set_visible(true);
    let mut label_text = String::new();
    for (reaction_raw, reaction_senders) in reactions.iter() {
        // Just take the first char of the emoji, which ignores any variant selectors.
        let reaction_first_char = reaction_raw.chars().next().map(|c| c.to_string());
        let reaction_str = reaction_first_char.as_deref().unwrap_or(reaction_raw);
        let text_to_display = emojis::get(&reaction_str)
            .and_then(|e| e.shortcode())
            .unwrap_or(&reaction_raw);
        let count = reaction_senders.len();
        // log!("Found reaction {:?} with count {}", text_to_display, count);
        label_text = format!("{label_text}<i>:{}:</i> <b>{}</b> ", text_to_display, count);
    }

    // Debugging: draw the item ID as a reaction
    if DRAW_ITEM_ID_REACTION {
        label_text = format!("{label_text}<i>ID: {}</i>", id);
    }

    let html_reaction_view = message_item.html(id!(annotations));
    html_reaction_view.set_text(&label_text);
}



/// A trait for abstracting over the different types of timeline events
/// that can be displayed in a `SmallStateEvent` widget.
trait SmallStateEventContent {
    /// Populates the *content* (not the profile) of the given `item` with data from
    /// the given `event_tl_item` and `self` (the specific type of event content).
    ///
    /// ## Arguments
    /// * `item`: a `SmallStateEvent` widget that has already been added to
    ///    the given `PortalList` at the given `item_id`.
    ///    This function may either modify that item or completely replace it
    ///    with a different widget if needed.
    /// * `item_drawn_status`: the old (prior) drawn status of the item.
    /// * `new_drawn_status`: the new drawn status of the item, which may have already
    ///    been updated to reflect the item's profile having been drawn right before this function.
    ///
    /// ## Return
    /// Returns a tuple of the drawn `item` and its `new_drawn_status`.
    fn populate_item_content(
        &self,
        cx: &mut Cx,
        list: &mut PortalList,
        item_id: usize,
        item: WidgetRef,
        event_tl_item: &EventTimelineItem,
        username: String,
        item_drawn_status: ItemDrawnStatus,
        new_drawn_status: ItemDrawnStatus,
    ) -> (WidgetRef, ItemDrawnStatus);
}


/// An empty marker struct used for populating redacted messages.
struct RedactedMessageEventMarker;

impl SmallStateEventContent for RedactedMessageEventMarker {
    fn populate_item_content(
        &self,
        _cx: &mut Cx,
        _list: &mut PortalList,
        _item_id: usize,
        item: WidgetRef,
        event_tl_item: &EventTimelineItem,
        original_sender: String,
        _item_drawn_status: ItemDrawnStatus,
        mut new_drawn_status: ItemDrawnStatus,
    ) -> (WidgetRef, ItemDrawnStatus) {
        let redactor_and_reason = {
            let mut rr = None;
            if let Some(redacted_msg) = event_tl_item.latest_json() {
                if let Ok(old) = redacted_msg.deserialize() {
                    if let AnySyncTimelineEvent::MessageLike(
                        AnySyncMessageLikeEvent::RoomMessage(
                            SyncMessageLikeEvent::Redacted(redaction)
                        )
                    ) = old {
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
}


impl SmallStateEventContent for timeline::OtherState {
    fn populate_item_content(
        &self,
        cx: &mut Cx,
        list: &mut PortalList,
        item_id: usize,
        item: WidgetRef,
        _event_tl_item: &EventTimelineItem,
        username: String,
        _item_drawn_status: ItemDrawnStatus,
        mut new_drawn_status: ItemDrawnStatus,
    ) -> (WidgetRef, ItemDrawnStatus) {
        let text = match self.content() {
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

        let item = if let Some(text) = text {
            item.label(id!(content)).set_text(&format!("{username} {text}"));
            new_drawn_status.content_drawn = true;
            item
        } else {
            let item = list.item(cx, item_id, live_id!(Empty)).unwrap();
            new_drawn_status = ItemDrawnStatus::new();
            item
        };
        (item, new_drawn_status)
    }
}

impl SmallStateEventContent for MemberProfileChange {
    fn populate_item_content(
        &self,
        _cx: &mut Cx,
        _list: &mut PortalList,
        _item_id: usize,
        item: WidgetRef,
        _event_tl_item: &EventTimelineItem,
        username: String,
        _item_drawn_status: ItemDrawnStatus,
        mut new_drawn_status: ItemDrawnStatus,
    ) -> (WidgetRef, ItemDrawnStatus) {
        let name_text = if let Some(name_change) = self.displayname_change() {
            let old = name_change.old.as_deref().unwrap_or(&username);
            if let Some(new) = name_change.new.as_ref() {
                format!("{old} changed their display name to {new:?}")
            } else {
                format!("{old} removed their display name")
            }
        } else {
            String::new()
        };

        let avatar_text = if let Some(_avatar_change) = self.avatar_url_change() {
            if name_text.is_empty() {
                format!("{} changed their profile picture", username)
            } else {
                format!(" and changed their profile picture")
            }
        } else {
            String::new()
        };

        item.label(id!(content)).set_text(&format!("{}{}.", name_text, avatar_text));
        new_drawn_status.content_drawn = true;
        (item, new_drawn_status)
    }
}

impl SmallStateEventContent for RoomMembershipChange {
    fn populate_item_content(
        &self,
        cx: &mut Cx,
        list: &mut PortalList,
        item_id: usize,
        item: WidgetRef,
        _event_tl_item: &EventTimelineItem,
        username: String,
        _item_drawn_status: ItemDrawnStatus,
        mut new_drawn_status: ItemDrawnStatus,
    ) -> (WidgetRef, ItemDrawnStatus) {
        let change_user_id = self.user_id();
        let text = match self.change() {
            None
            | Some(MembershipChange::NotImplemented)
            | Some(MembershipChange::None) => {
                // Don't actually display anything for nonexistent/unimportant membership changes.
                return (
                    list.item(cx, item_id, live_id!(Empty)).unwrap(),
                    ItemDrawnStatus::new(),
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
}


/// Creates, populates, and adds a SmallStateEvent liveview widget to the given `PortalList`
/// with the given `item_id`.
///
/// The content of the returned widget is populated with data from the
/// given room membership change and its parent `EventTimelineItem`.
fn populate_small_state_event(
    cx: &mut Cx,
    list: &mut PortalList,
    item_id: usize,
    room_id: &RoomId,
    event_tl_item: &EventTimelineItem,
    event_content: &impl SmallStateEventContent,
    item_drawn_status: ItemDrawnStatus,
) -> (WidgetRef, ItemDrawnStatus) {
    let mut new_drawn_status = item_drawn_status;
    let (item, existed) = list.item_with_existed(cx, item_id, live_id!(SmallStateEvent)).unwrap();
    
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
        .then(|| get_profile_display_name(event_tl_item))
        .flatten();
    
    let username = username_opt.unwrap_or_else(|| {
        // As a fallback, call `set_avatar_and_get_username` to get the user's display name.
        let (username, profile_drawn) = set_avatar_and_get_username(
            cx,
            item.avatar(id!(avatar)),
            room_id,
            event_tl_item,
        );
        // Draw the timestamp as part of the profile.
        set_timestamp(&item, id!(left_container.timestamp), event_tl_item.timestamp());
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
        username,
        item_drawn_status,
        new_drawn_status,
    )
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
///   * If it's not ready, we attempt to fetch the user info from the user profile cache.
/// * If no avatar image is available, then the `avatar` will be set to the first character
///   of the user's display name, if available.
/// * If the user's display name is not available or has not been set, the user ID
///   will be used for the `username`, and the first character of the user ID for the `avatar`.
/// * If the timeline event's sender profile isn't ready and the user ID isn't found in
///   our user profile cache , then the `username` and `avatar`  will be the user ID
///   and the first character of that user ID, respectively.
///
/// ## Return
/// Returns a tuple of:
/// 1. The displayable username that should be used to populate the username field.
/// 2. A boolean indicating whether the user's profile info has been completely drawn
///    (for purposes of caching it to avoid future redraws).
fn set_avatar_and_get_username(
    cx: &mut Cx,
    avatar: AvatarRef,
    room_id: &RoomId,
    event_tl_item: &EventTimelineItem,
) -> (String, bool) {
    let user_id = event_tl_item.sender();

    // Get the display name and avatar URL from the sender's profile, if available,
    // or if the profile isn't ready, fall back to qeurying our user profile cache.
    let (username_opt, avatar_state) = match event_tl_item.sender_profile() {
        TimelineDetails::Ready(profile) => {
            (profile.display_name.clone(), AvatarState::Known(profile.avatar_url.clone()))
        }
        _not_ready => {
            // log!("populate_message_view(): sender profile not ready yet for event {_not_ready:?}");
            user_profile_cache::with_user_profile(cx, user_id, |profile, room_members| {
                room_members.get(room_id)
                    .map(|rm| (
                        rm.display_name().map(|n| n.to_owned()),
                        AvatarState::Known(rm.avatar_url().map(|u| u.to_owned()))
                    ))
                    .unwrap_or_else(|| (
                        profile.username.clone(),
                        profile.avatar_state.clone(),
                    ))
                })
                .unwrap_or((None, AvatarState::Unknown))
        }
    };

    let (avatar_img_data_opt, profile_drawn) = match avatar_state {
        AvatarState::Loaded(data) => (Some(data), true),
        AvatarState::Known(Some(uri)) => match avatar_cache::get_or_fetch_avatar(cx, uri) {
            AvatarCacheEntry::Loaded(data) => (Some(data), true),
            AvatarCacheEntry::Failed => (None, true),
            AvatarCacheEntry::Requested => (None, false),
        }
        AvatarState::Known(None) | AvatarState::Failed => (None, true),
        AvatarState::Unknown => (None, false),
    };

    // Set sender to the display name if available, otherwise the user id.
    let username = username_opt.clone().unwrap_or_else(|| user_id.to_string());

    // Set the sender's avatar image, or use the username if no image is available.
    avatar_img_data_opt.and_then(|data| avatar.show_image(
        Some((user_id.to_owned(), username_opt.clone(), room_id.to_owned(), data.clone())),
        |img| utils::load_png_or_jpg(&img, cx, &data)
    ).ok())
    .unwrap_or_else(|| avatar.show_text(
        Some((user_id.to_owned(), username_opt, room_id.to_owned())),
        &username,
    ));

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
