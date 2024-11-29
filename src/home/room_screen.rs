//! A room screen is the UI page that displays a single Room's timeline of events/messages
//! along with a message input bar at the bottom.

use std::{borrow::Cow, collections::{BTreeMap, HashMap}, ops::{DerefMut, Range}, sync::{Arc, Mutex}, time::{Instant, SystemTime}};

use bytesize::ByteSize;
use imbl::Vector;
use makepad_widgets::*;
use matrix_sdk::{
    ruma::{
        events::{room::{
            message::{
                AudioMessageEventContent, CustomEventContent, EmoteMessageEventContent, FileMessageEventContent, FormattedBody, ImageMessageEventContent, KeyVerificationRequestEventContent, LocationMessageEventContent, MessageFormat, MessageType, NoticeMessageEventContent, RoomMessageEventContent, ServerNoticeMessageEventContent, TextMessageEventContent, VideoMessageEventContent
            }, ImageInfo, MediaSource
        }, sticker::StickerEventContent}, matrix_uri::MatrixId, uint, EventId, MatrixToUri, MatrixUri, MilliSecondsSinceUnixEpoch, OwnedEventId, OwnedRoomId, UserId
    },
    OwnedServerName,
};
use matrix_sdk_ui::timeline::{
    self, EventTimelineItem, InReplyToDetails, MemberProfileChange, Profile, ReactionsByKeyBySender, RepliedToInfo, RoomMembershipChange, TimelineDetails, TimelineItem, TimelineItemContent, TimelineItemKind, VirtualTimelineItem
};
use robius_location::Coordinates;

use crate::{
    avatar_cache::{self, AvatarCacheEntry}, event_preview::{text_preview_of_member_profile_change, text_preview_of_other_state, text_preview_of_redacted_message, text_preview_of_room_membership_change, text_preview_of_timeline_item}, home::loading_modal::LoadingModalWidgetExt, location::{get_latest_location, init_location_subscriber, request_location_update, LocationAction, LocationRequest, LocationUpdate}, media_cache::{MediaCache, MediaCacheEntry}, profile::{
        user_profile::{AvatarState, ShowUserProfileAction, UserProfile, UserProfileAndRoomId, UserProfilePaneInfo, UserProfileSlidingPaneRef, UserProfileSlidingPaneWidgetExt},
        user_profile_cache,
    }, shared::{
        avatar::{AvatarRef, AvatarWidgetRefExt}, html_or_plaintext::{HtmlOrPlaintextRef, HtmlOrPlaintextWidgetRefExt}, jump_to_bottom_button::JumpToBottomButtonWidgetExt, text_or_image::{TextOrImageRef, TextOrImageWidgetRefExt}, typing_animation::TypingAnimationWidgetExt
    }, sliding_sync::{get_client, submit_async_request, take_timeline_endpoints, BackwardsPaginateUntilEventRequest, MatrixRequest, PaginationDirection, TimelineRequestSender}, utils::{self, unix_time_millis_to_datetime, ImageFormat, MediaFormatConst}
};
use rangemap::RangeSet;

use super::loading_modal::{LoadingModalAction, LoadingModalState};

const GEO_URI_SCHEME: &str = "geo:";

const MESSAGE_NOTICE_TEXT_COLOR: Vec3 = Vec3 { x: 0.5, y: 0.5, z: 0.5 };
const COLOR_DANGER_RED: Vec3 = Vec3 { x: 0.862, y: 0.0, z: 0.02 };


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
    import crate::shared::typing_animation::TypingAnimation;
    import crate::shared::icon_button::*;
    import crate::shared::jump_to_bottom_button::*;
    import crate::home::loading_modal::*;

    IMG_DEFAULT_AVATAR = dep("crate://self/resources/img/default_avatar.png")
    ICO_FAV = dep("crate://self/resources/icon_favorite.svg")
    ICO_COMMENT = dep("crate://self/resources/icon_comment.svg")
    ICO_REPLY = dep("crate://self/resources/icons/reply.svg")
    ICO_SEND = dep("crate://self/resources/icon_send.svg")
    ICO_LIKES = dep("crate://self/resources/icon_likes.svg")
    ICO_USER = dep("crate://self/resources/icon_user.svg")
    ICO_ADD = dep("crate://self/resources/icon_add.svg")
    ICO_CLOSE = dep("crate://self/resources/icons/close.svg")

    ICO_LOCATION_PERSON = dep("crate://self/resources/icons/location-person.svg")

    COLOR_BG = #xfff8ee
    COLOR_OVERLAY_BG = #x000000d8
    COLOR_READ_MARKER = #xeb2733
    COLOR_PROFILE_CIRCLE = #xfff8ee
    TYPING_NOTICE_ANIMATION_DURATION = 0.3

    NO_POST_PERMISSION_NOTICE = "You don't have permission to post to this room."

    FillerY = <View> {width: Fill}

    FillerX = <View> {height: Fill}

    Timestamp = <Label> {
        width: Fit, height: Fit
        padding: { bottom: 0.0, left: 0.0, right: 0.0 }
        draw_text: {
            text_style: <TIMESTAMP_TEXT_STYLE> {},
            color: (TIMESTAMP_TEXT_COLOR)
        }
        text: " "
    }

    REACTION_TEXT_COLOR = #4c00b0

    // The content of a reply preview, which shows a small preview
    // of a message that was replied to.
    //
    // This is used in both the `RepliedToMessage` and `ReplyPreview` views.
    ReplyPreviewContent = <View> {
        width: Fill
        height: Fit
        flow: Down
        padding: {left: 10.0, bottom: 5.0, top: 5.0}

        <View> {
            width: Fill
            height: Fit
            flow: Right
            margin: { bottom: 10.0, top: 0.0, right: 5.0 }
            align: {y: 0.5}

            reply_preview_avatar = <Avatar> {
                width: 19.,
                height: 19.,
                text_view = { text = { draw_text: {
                    text_style: { font_size: 6.0 }
                }}}
            }

            reply_preview_username = <Label> {
                width: Fill,
                margin: { left: 5.0 }
                draw_text: {
                    text_style: <USERNAME_TEXT_STYLE> { font_size: 10 },
                    color: (USERNAME_TEXT_COLOR)
                    wrap: Ellipsis,
                }
                text: "<Username not available>"
            }
        }

        reply_preview_body = <HtmlOrPlaintext> {
            html_view = { html = {
                font_size: (MESSAGE_REPLY_PREVIEW_FONT_SIZE)
                    draw_normal:      { text_style: { font_size: (MESSAGE_REPLY_PREVIEW_FONT_SIZE) } },
                    draw_italic:      { text_style: { font_size: (MESSAGE_REPLY_PREVIEW_FONT_SIZE) } },
                    draw_bold:        { text_style: { font_size: (MESSAGE_REPLY_PREVIEW_FONT_SIZE) } },
                    draw_bold_italic: { text_style: { font_size: (MESSAGE_REPLY_PREVIEW_FONT_SIZE) } },
                    draw_fixed:       { text_style: { font_size: (MESSAGE_REPLY_PREVIEW_FONT_SIZE) } },
                    // a = { draw_text:  { text_style: { font_size: (MESSAGE_REPLY_PREVIEW_FONT_SIZE) } } },
            } }
            plaintext_view = { pt_label = {
                draw_text: {
                    text_style: <MESSAGE_TEXT_STYLE> { font_size: (MESSAGE_REPLY_PREVIEW_FONT_SIZE) },
                }
            } }
        }
    }

    // A small inline preview of a message that was replied to by another message
    // within the room timeline.
    // That is, this view contains a preview of the earlier message
    // that is shown above the "in-reply-to" message.
    RepliedToMessage = <View> {
        visible: false
        width: Fill
        height: Fit
        flow: Down

        padding: {top: 0.0, right: 12.0, bottom: 0.0, left: 12.0}

        // A reply preview with a vertical bar drawn in the background.
        replied_to_message_content = <ReplyPreviewContent> {
            cursor: Hand
            show_bg: true
            draw_bg: {
                instance vertical_bar_color: (USERNAME_TEXT_COLOR)
                instance vertical_bar_width: 2.0
                instance radius: 0.0

                fn get_color(self) -> vec4 {
                    return self.color;
                }

                fn pixel(self) -> vec4 {
                    let sdf = Sdf2d::viewport(self.pos * self.rect_size);

                    sdf.box(
                        0.0,
                        0.0,
                        self.rect_size.x,
                        self.rect_size.y,
                        max(1.0, self.radius)
                    );
                    sdf.fill(self.get_color());

                    sdf.rect(
                        0.0,
                        0.0,
                        self.vertical_bar_width,
                        self.rect_size.y
                    );
                    sdf.fill(self.vertical_bar_color);

                    return sdf.result;
                }
            }
        }
    }

    // A view that shows action buttons for a message,
    // with buttons for sending a reply (and in the future, reactions).
    MessageMenu = <RoundedView> {
        visible: true,
        width: Fit,
        height: Fit,
        align: {x: 1, y: 0}

        draw_bg: {
            border_width: 0.0,
            border_color: #000,
            radius: 2.0
        }

        reply_button = <IconButton> {
            visible: false
            width: Fit,
            height: Fit,

            draw_icon: {
                svg_file: (ICO_REPLY),
            }
            icon_walk: {width: 15, height: 15, margin: {top: 4.0}}
        }
    }

    // An optional view used to show reactions beneath a message.
    MessageAnnotations = <View> {
        visible: false,
        width: Fill,
        height: Fit,
        padding: {top: 5.0}

        html_content = <MessageHtml> {
            width: Fill,
            height: Fit,
            padding: { bottom: 5.0, top: 0.0 },
            font_size: 10.5,
            font_color: (REACTION_TEXT_COLOR),
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
    Message = {{Message}} {
        width: Fill,
        height: Fit,
        margin: 0.0
        flow: Down,
        padding: 0.0,
        spacing: 0.0

        show_bg: true
        draw_bg: {
            instance highlight: 0.0
            instance hover: 0.0
            color: #ffffff  // default color

            instance mentions_bar_color: #ffffff
            instance mentions_bar_width: 4.0

            fn pixel(self) -> vec4 {
                let base_color = mix(
                    self.color,
                    #fafafa,
                    self.hover
                );

                let with_highlight = mix(
                    base_color,
                    #c5d6fa,
                    self.highlight
                );

                let sdf = Sdf2d::viewport(self.pos * self.rect_size);

                // draw bg
                sdf.rect(0., 0., self.rect_size.x, self.rect_size.y);
                sdf.fill(with_highlight);

                // draw the left vertical line
                sdf.rect(0., 0., self.mentions_bar_width, self.rect_size.y);
                sdf.fill(self.mentions_bar_color);

                return sdf.result;
            }
        }

        animator: {
            highlight = {
                default: off
                off = {
                    redraw: true,
                    from: { all: Forward {duration: 2.0} }
                    ease: ExpDecay {d1: 0.80, d2: 0.97}
                    apply: { draw_bg: {highlight: 0.0} }
                }
                on = {
                    redraw: true,
                    from: { all: Forward {duration: 0.5} }
                    ease: ExpDecay {d1: 0.80, d2: 0.97}
                    apply: { draw_bg: {highlight: 1.0} }
                }
            }
            hover = {
                default: off
                off = {
                    redraw: true,
                    from: { all: Snap }
                    apply: { draw_bg: {hover: 0.0} }
                }
                on = {
                    redraw: true,
                    from: { all: Snap }
                    apply: { draw_bg: {hover: 1.0} }
                }
            }
        }

        // A preview of the earlier message that this message was in reply to.
        replied_to_message = <RepliedToMessage> {
            flow: Right
            margin: { bottom: 5.0, top: 10.0 }
            replied_to_message_content = {
                margin: { left: 29 }
            }
        }

        body = <View> {
            width: Fill,
            height: Fit
            flow: Right,
            padding: 10.0,

            profile = <View> {
                align: {x: 0.5, y: 0.0} // centered horizontally, top aligned
                width: 65.0,
                height: Fit,
                margin: {top: 4.5, right: 10}
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
                timestamp = <Timestamp> {
                    padding: { top: 3.0 }
                }
                datestamp = <Timestamp> {
                    padding: { top: 3.0 }
                }
            }
            content = <View> {
                width: Fill,
                height: Fit
                flow: Down,
                padding: 0.0

                username = <Label> {
                    width: Fill,
                    margin: {bottom: 9.0, top: 11.0, right: 10.0,}
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

                message_annotations = <MessageAnnotations> {}
            }

            message_menu = <MessageMenu> {}
            // leave space for reply button (simulate a min width).
            // once the message menu is done with overlays this wont be necessary.
            <View> {
                width: 1,
                height: 1
            }
        }
    }

    // The view used for a condensed message that came right after another message
    // from the same sender, and thus doesn't need to display the sender's profile again.
    CondensedMessage = <Message> {
        padding: { top: 2.0, bottom: 2.0 }
        replied_to_message = <RepliedToMessage> {
            replied_to_message_content = {
                margin: { left: 74, bottom: 5.0 }
            }
        }
        body = {
            padding: { top: 0, bottom: 2.5, left: 10.0, right: 10.0 },
            profile = <View> {
                align: {x: 0.5, y: 0.0} // centered horizontally, top aligned
                width: 65.0,
                height: Fit,
                flow: Down,
                timestamp = <Timestamp> {
                    margin: {top: 1.5}
                }
            }
            content = <View> {
                width: Fill,
                height: Fit,
                flow: Down,
                padding: { left: 10.0 }

                message = <HtmlOrPlaintext> { }
                message_annotations = <MessageAnnotations> {}
            }
        }
    }

    // The view used for each static image-based message event in a room's timeline.
    // This excludes stickers and other animated GIFs, video clips, audio clips, etc.
    ImageMessage = <Message> {
        body = {
            content = {
                padding: { left: 10.0 }
                message = <TextOrImage> { }
                message_annotations = <MessageAnnotations> {}
            }
        }
    }

    // The view used for a condensed image message that came right after another message
    // from the same sender, and thus doesn't need to display the sender's profile again.
    // This excludes stickers and other animated GIFs, video clips, audio clips, etc.
    CondensedImageMessage = <CondensedMessage> {
        body = {
            content = {
                message = <TextOrImage> { }
                message_annotations = <MessageAnnotations> {}
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
        margin: { left: 2.5, top: 4.0, bottom: 4.0}

        body = <View> {
            width: Fill,
            height: Fit
            flow: Right,
            padding: { left: 7.0, top: 2.0, bottom: 2.0 }
            spacing: 5.0
            align: {y: 0.5}

            left_container = <View> {
                align: {x: 0.5, y: 0.5}
                width: 70.0,
                height: Fit

                timestamp = <Timestamp> {
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
                    text_style: <TITLE_TEXT>{ font_size: 7.0 }
                }}}
            }

            content = <Label> {
                width: Fill,
                height: Fit
                padding: { top: 0.0, bottom: 0.0, left: 0.0, right: 0.0 }
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
        margin: {top: 7.0, bottom: 7.0}
        flow: Right,
        padding: {left: 7.0, right: 7.0},
        spacing: 0.0,
        align: {x: 0.5, y: 0.5} // center horizontally and vertically

        left_line = <LineH> {
            draw_bg: {color: (COLOR_DIVIDER_DARK)}
        }

        date = <Label> {
            padding: {left: 7.0, right: 7.0}
            draw_text: {
                text_style: <TEXT_SUB> {},
                color: (COLOR_DIVIDER_DARK)
            }
            text: "<date>"
        }

        right_line = <LineH> {
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
            text: "Loading earlier messages..."
        }
    }

    Timeline = <View> {
        width: Fill,
        height: Fill,
        align: {x: 0.5, y: 0.0} // center horizontally, align to top vertically
        flow: Overlay,

        list = <PortalList> {
            height: Fill,
            width: Fill
            flow: Down

            auto_tail: true, // set to `true` to lock the view to the last item.

            // Below, we must place all of the possible templates (views) that can be used in the portal list.
            Message = <Message> {}
            CondensedMessage = <CondensedMessage> {}
            ImageMessage = <ImageMessage> {}
            CondensedImageMessage = <CondensedImageMessage> {}
            SmallStateEvent = <SmallStateEvent> {}
            Empty = <Empty> {}
            DayDivider = <DayDivider> {}
            ReadMarker = <ReadMarker> {}
        }

        // A jump to bottom button (with an unread message badge) that is shown
        // when the timeline is not at the bottom.
        jump_to_bottom = <JumpToBottomButton> { }
    }

    LocationPreview = {{LocationPreview}} {
        visible: false
        width: Fill
        height: Fit
        flow: Down
        padding: {left: 12.0, top: 12.0, bottom: 12.0, right: 10.0}
        spacing: 15

        show_bg: true,
        draw_bg: {
            color: #xF0F5FF,
        }

        <Label> {
            width: Fill,
            height: Fit,
            draw_text: {
                wrap: Word,
                color: (MESSAGE_TEXT_COLOR),
                text_style: <MESSAGE_TEXT_STYLE>{ font_size: 10.0 },
            }
            text: "Send your location to this room?"
        }

        location_label = <Label> {
            width: Fill,
            height: Fit,
            align: {x: 0.0, y: 0.5},
            padding: {left: 5.0}
            draw_text: {
                wrap: Word,
                color: (MESSAGE_TEXT_COLOR),
                text_style: <MESSAGE_TEXT_STYLE>{},
            }
            text: "Fetching current location..."
        }

        <View> {
            width: Fill, height: Fit
            flow: Right,
            align: {x: 0.0, y: 0.5}
            spacing: 15

            cancel_location_button = <RobrixIconButton> {
                padding: {left: 15, right: 15}
                draw_icon: {
                    svg_file: (ICON_BLOCK_USER)
                    color: (COLOR_DANGER_RED),
                }
                icon_walk: {width: 16, height: 16, margin: {left: -2, right: -1, top: -1} }

                draw_bg: {
                    border_color: (COLOR_DANGER_RED),
                    color: #fff0f0 // light red
                }
                text: "Cancel"
                draw_text:{
                    color: (COLOR_DANGER_RED),
                }
            }

            send_location_button = <RobrixIconButton> {
                // disabled by default; will be enabled upon receiving valid location update.
                enabled: false,
                padding: {left: 15, right: 15}
                draw_icon: {
                    svg_file: (ICO_SEND)
                    color: (COLOR_ACCEPT_GREEN),
                }
                icon_walk: {width: 16, height: 16, margin: {left: -2, right: -1} }

                draw_bg: {
                    border_color: (COLOR_ACCEPT_GREEN),
                    color: #f0fff0 // light green
                }
                text: "Yes"
                draw_text:{
                    color: (COLOR_ACCEPT_GREEN),
                }
            }
        }
    }


    IMG_SMILEY_FACE_BW = dep("crate://self/resources/img/smiley_face_bw.png")
    IMG_PLUS = dep("crate://self/resources/img/plus.png")
    IMG_KEYBOARD_ICON = dep("crate://self/resources/img/keyboard_icon.png")

    RoomScreen = {{RoomScreen}} {
        width: Fill, height: Fill,
        show_bg: true,
        draw_bg: {
            color: (COLOR_SECONDARY)
        }
        flow: Down, spacing: 0.0

        room_screen_wrapper = <View> {
            width: Fill, height: Fill,
            flow: Overlay,
            show_bg: true
            draw_bg: {
                color: (COLOR_PRIMARY_DARKER)
            }

            keyboard_view = <KeyboardView> {
                width: Fill, height: Fill,
                flow: Down,

                // First, display the timeline of all messages/events.
                timeline = <Timeline> {}

                // Below that, display an optional preview of the message that the user
                // is currently drafting a replied to.
                replying_preview = <View> {
                    visible: false
                    width: Fill
                    height: Fit
                    flow: Down
                    padding: 0.0

                    // Displays a "Replying to" label and a cancel button
                    // above the preview of the message being replied to.
                    <View> {
                        padding: {right: 12.0, left: 12.0}
                        width: Fill
                        height: Fit
                        flow: Right
                        align: {y: 0.5}

                        <Label> {
                            draw_text: {
                                text_style: <TEXT_SUB> {},
                                color: (COLOR_META)
                            }
                            text: "Replying to:"
                        }

                        filler = <View> {width: Fill, height: Fill}

                        // TODO: Fix style
                        cancel_reply_button = <IconButton> {
                            width: Fit,
                            height: Fit,

                            draw_icon: {
                                svg_file: (ICO_CLOSE),
                                fn get_color(self) -> vec4 {
                                   return (COLOR_META)
                                }
                            }
                            icon_walk: {width: 12, height: 12}
                        }
                    }

                    reply_preview_content = <ReplyPreviewContent> { }
                }

                // Below that, display a typing notice when other users in the room are typing.
                typing_notice = <View> {
                    visible: false
                    width: Fill
                    height: 30
                    flow: Right
                    padding: {left: 12.0, top: 8.0, bottom: 8.0, right: 10.0}
                    show_bg: true,
                    draw_bg: {
                        color: #e8f4ff,
                    }

                    typing_label = <Label> {
                        align: {x: 0.0, y: 0.5},
                        padding: {left: 5.0, right: 0.0}
                        draw_text: {
                            color: (TYPING_NOTICE_TEXT_COLOR),
                            text_style: <REGULAR_TEXT>{font_size: 9}
                        }
                        text: "Someone is typing"
                    }

                    typing_animation = <TypingAnimation> {
                        margin: {top: 1.1, left: -4 }
                        padding: 0.0,
                        draw_bg: {
                            color: (TYPING_NOTICE_TEXT_COLOR),
                        }
                    }
                }

                // Below that, display a preview of the current location that a user is about to send.
                location_preview = <LocationPreview> { }

                // Below that, display a view that holds the message input bar and send button.
                bottom_input = <View> {
                    width: Fill, height: Fit
                    flow: Right,
                    align: {y: 0.5},
                    padding: 10.
                    show_bg: true,
                    draw_bg: {
                        color: (COLOR_PRIMARY)
                    }

                    location_button = <IconButton> {
                        draw_icon: {svg_file: (ICO_LOCATION_PERSON)},
                        icon_walk: {width: 22.0, height: Fit, margin: {left: 0, right: 5}},
                        text: "",
                    }

                    message_input = <TextInput> {
                        width: Fill, height: Fit, margin: 0
                        align: {y: 0.5}
                        empty_message: "Write a message (in Markdown) ..."
                        draw_bg: {
                            color: (COLOR_PRIMARY)
                            instance radius: 2.0
                            instance border_width: 0.8
                            instance border_color: #D0D5DD
                            instance inset: vec4(0.0, 0.0, 0.0, 0.0)

                            fn get_color(self) -> vec4 {
                                return self.color
                            }

                            fn get_border_color(self) -> vec4 {
                                return self.border_color
                            }

                            fn pixel(self) -> vec4 {
                                let sdf = Sdf2d::viewport(self.pos * self.rect_size)
                                sdf.box(
                                    self.inset.x + self.border_width,
                                    self.inset.y + self.border_width,
                                    self.rect_size.x - (self.inset.x + self.inset.z + self.border_width * 2.0),
                                    self.rect_size.y - (self.inset.y + self.inset.w + self.border_width * 2.0),
                                    max(1.0, self.radius)
                                )
                                sdf.fill_keep(self.get_color())
                                if self.border_width > 0.0 {
                                    sdf.stroke(self.get_border_color(), self.border_width)
                                }
                                return sdf.result;
                            }
                        }
                        draw_text: {
                            color: (MESSAGE_TEXT_COLOR),
                            text_style: <MESSAGE_TEXT_STYLE>{},

                            fn get_color(self) -> vec4 {
                                return mix(
                                    self.color,
                                    #B,
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
                        draw_selection: {
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
                                sdf.fill(mix(#dfffd6, #bfffb0, self.focus));
                                return sdf.result
                            }
                        }
                    }

                    send_message_button = <IconButton> {
                        draw_icon: {svg_file: (ICO_SEND)},
                        icon_walk: {width: 18.0, height: Fit},
                    }
                }
                no_send_permission_notice = <View> {
                    visible: false
                    show_bg: true
                    draw_bg: {
                        color: (COLOR_SECONDARY)
                    }
                    padding: {left: 75}
                    align: {y: 0.3}
                    width: Fill, height: 37.5

                    text = <Label> {
                        draw_text: {
                            color: (COLOR_TEXT)
                            text_style: <THEME_FONT_ITALIC>{font_size: 12.2}
                        }
                        text: (NO_POST_PERMISSION_NOTICE)
                    }
                }
            }

            // The top space should be displayed on top of the timeline
            top_space = <TopSpace> { }

            // The user profile sliding pane should be displayed on top of other "static" subviews
            // (on top of all other views that are always visible).
            <View> {
                width: Fill,
                height: Fill,
                align: { x: 1.0 },
                flow: Right,

                user_profile_sliding_pane = <UserProfileSlidingPane> { }
            }

            // A pop-up modal that appears while the user must wait for something
            // in the room to finish loading, e.g., when loading an older replied-to message.
            loading_modal = <Modal> {
                content: {
                    loading_modal_inner = <LoadingModal> {}
                }
            }
        }

        animator: {
            typing_notice_animator = {
                default: show,
                show = {
                    redraw: true,
                    from: { all: Forward { duration: (TYPING_NOTICE_ANIMATION_DURATION) } }
                    apply: { room_screen_wrapper = { keyboard_view = { typing_notice = { height: 30 } } } }
                }
                hide = {
                    redraw: true,
                    from: { all: Forward { duration: (TYPING_NOTICE_ANIMATION_DURATION) } }
                    apply: { room_screen_wrapper = { keyboard_view = { typing_notice = { height: 0 } } } }
                }
            }
        }
    }
}

/// The main widget that displays a single Matrix room.
#[derive(Live, LiveHook, Widget)]
pub struct RoomScreen {
    #[deref] view: View,
    #[animator] animator: Animator,

    /// The room ID of the currently-shown room.
    #[rust] room_id: Option<OwnedRoomId>,
    /// The display name of the currently-shown room.
    #[rust] room_name: String,
    /// The persistent UI-relevant states for the room that this widget is currently displaying.
    #[rust] tl_state: Option<TimelineUiState>,
    /// 5 secs timer when scroll ends
    #[rust] fully_read_timer: Timer,
}
impl Drop for RoomScreen {
    fn drop(&mut self) {
        // This ensures that the `TimelineUiState` instance owned by this room is *always* returned
        // back to to `TIMELINE_STATES`, which ensures that its UI state(s) are not lost
        // and that other RoomScreen instances can show this room in the future.
        // RoomScreen will be dropped whenever its widget instance is destroyed, e.g.,
        // when a Tab is closed or the app is resized to a different AdaptiveView layout.
        self.hide_timeline();
    }
}

impl Widget for RoomScreen {
    // Handle events and actions for the RoomScreen widget and its inner Timeline view.
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        let widget_uid = self.widget_uid();
        let portal_list = self.portal_list(id!(timeline.list));
        let pane = self.user_profile_sliding_pane(id!(user_profile_sliding_pane));

        // Currently, a Signal event is only used to tell this widget
        // that its timeline events have been updated in the background.
        if let Event::Signal = event {
            self.process_timeline_updates(cx, &portal_list);
        }

        if let Event::Actions(actions) = event {
            for action in actions {
                // Handle actions on a message, e.g., clicking the reply button or clicking the reply preview.
                match action.as_widget_action().widget_uid_eq(widget_uid).cast() {
                    MessageAction::MessageReplyButtonClicked(item_id) => {
                        let Some(tl) = self.tl_state.as_mut() else {
                            continue;
                        };

                        if let Some(event_tl_item) = tl.items
                            .get(item_id)
                            .and_then(|tl_item| tl_item.as_event().cloned())
                        {
                            if let Ok(replied_to_info) = event_tl_item.replied_to_info() {
                                self.show_replying_to(cx, (event_tl_item, replied_to_info));
                            }
                        }
                    }
                    MessageAction::ReplyPreviewClicked { reply_message_item_id, replied_to_event } => {
                        let loading_modal_inner = self.loading_modal(id!(loading_modal_inner));
                        let loading_modal = self.modal(id!(loading_modal));
                        let Some(tl) = self.tl_state.as_mut() else {
                            continue;
                        };
                        let tl_idx = reply_message_item_id;

                        /// The maximum number of items to search through when looking for the replied-to message.
                        /// This is a safety measure to prevent the main UI thread from getting stuck in a
                        /// long-running loop if the replied-to message is not found quickly.
                        const MAX_ITEMS_TO_SEARCH_THROUGH: usize = 50;

                        // Attempt to find the index of replied-to message in the timeline.
                        // Start from the current item's index (`tl_idx`)and search backwards,
                        // since we know the replied-to message must come before the current item.
                        let mut num_items_searched = 0;
                        let replied_to_msg_tl_index = tl.items
                            .focus()
                            .narrow(..tl_idx)
                            .into_iter()
                            .rev()
                            .take(MAX_ITEMS_TO_SEARCH_THROUGH)
                            .position(|i| {
                                num_items_searched += 1;
                                i.as_event()
                                    .and_then(|e| e.event_id())
                                    .is_some_and(|ev_id| ev_id == replied_to_event)
                            })
                            .map(|position| tl_idx.saturating_sub(position).saturating_sub(1));

                        if let Some(index) = replied_to_msg_tl_index {
                            // log!("The replied-to message {replied_to_event} was immediately found in room {}, scrolling to from index {reply_message_item_id} --> {index} (first ID {}).", tl.room_id, portal_list.first_id());
                            let speed = 50.0;
                            // Scroll to the message right *before* the replied-to message.
                            // FIXME: `smooth_scroll_to` should accept a "scroll offset" (first scroll) parameter too,
                            //       so that we can scroll to the replied-to message and have it
                            //       appear beneath the top of the viewport.
                            portal_list.smooth_scroll_to(cx, index.saturating_sub(1), speed, None);
                            // start highlight animation.
                            tl.message_highlight_animation_state = MessageHighlightAnimationState::Pending {
                                item_id: index
                            };
                        } else {
                            // log!("The replied-to message {replied_to_event} wasn't immediately available in room {}, searching for it in the background...", tl.room_id);
                            // Here, we set the state of the loading modal and display it to the user.
                            // The main logic will be handled in `process_timeline_updates()`, which is the only
                            // place where we can receive updates to the timeline from the background tasks.
                            loading_modal_inner.set_state(
                                cx,
                                LoadingModalState::BackwardsPaginateUntilEvent {
                                    target_event_id: replied_to_event.clone(),
                                    events_paginated: 0,
                                    request_sender: tl.request_sender.clone(),
                                },
                            );
                            loading_modal.open(cx);

                            tl.request_sender.send_if_modified(|requests| {
                                if let Some(existing) = requests.iter_mut().find(|r| r.room_id == tl.room_id) {
                                    warning!("Unexpected: room {} already had an existing timeline request in progress, event: {:?}", tl.room_id, existing.target_event_id);
                                    // We might as well re-use this existing request...
                                    existing.target_event_id = replied_to_event;
                                } else {
                                    requests.push(BackwardsPaginateUntilEventRequest {
                                        room_id: tl.room_id.clone(),
                                        target_event_id: replied_to_event,
                                        // avoid re-searching through items we already searched through.
                                        starting_index: tl_idx.saturating_sub(num_items_searched),
                                        current_tl_len: tl.items.len(),
                                    });
                                }
                                true
                            });

                            // Don't unconditionally start backwards pagination here, because we want to give the
                            // background `timeline_subscriber_handler` task a chance to process the request first
                            // and search our locally-known timeline history for the replied-to message.
                        }
                        self.redraw(cx);
                    }
                    _ => {}
                }

                // Handle the highlight animation.
                let Some(tl) = self.tl_state.as_mut() else { return };
                if let MessageHighlightAnimationState::Pending { item_id } = tl.message_highlight_animation_state {
                    if portal_list.smooth_scroll_reached(actions) {
                        cx.widget_action(
                            widget_uid,
                            &scope.path,
                            MessageAction::MessageHighlight(item_id),
                        );
                        tl.message_highlight_animation_state = MessageHighlightAnimationState::Off;
                        // Adjust the scrolled-to item's position to be slightly beneath the top of the viewport.
                        // portal_list.set_first_id_and_scroll(portal_list.first_id(), 15.0);
                    }
                }

                // Handle the action that requests to show the user profile sliding pane.
                if let ShowUserProfileAction::ShowUserProfile(profile_and_room_id) = action.as_widget_action().cast() {
                    // Only show the user profile in room that this avatar belongs to
                    if self.room_id.as_ref().is_some_and(|r| r == &profile_and_room_id.room_id) {
                        self.show_user_profile(
                            cx,
                            &pane,
                            UserProfilePaneInfo {
                                profile_and_room_id,
                                room_name: self.room_name.clone(),
                                room_member: None,
                            },
                        );
                    }
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
                                self.show_user_profile(
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
                        log!("Opening URL \"{}\"", url);
                        if let Err(e) = robius_open::Uri::new(&url).open() {
                            error!("Failed to open URL {:?}. Error: {:?}", url, e);
                        }
                    }
                }

                if let LoadingModalAction::Close = action.as_widget_action().cast() {
                    self.modal(id!(loading_modal)).close(cx);
                }
            }

            // Set visibility of loading message banner based of pagination logic
            self.send_pagination_request_based_on_scroll_pos(cx, actions, &portal_list);
            // Handle sending any read receipts for the current logged-in user.
            self.send_user_read_receipts_based_on_scroll_pos(cx, actions, &portal_list);

            // Handle the cancel reply button being clicked.
            if self.button(id!(cancel_reply_button)).clicked(actions) {
                self.clear_replying_to();
                self.redraw(cx);
            }

            // Handle the add location button being clicked.
            if self.button(id!(location_button)).clicked(actions) {
                log!("Add location button clicked; requesting current location...");
                if let Err(_e) = init_location_subscriber(cx) {
                    error!("Failed to initialize location subscriber");
                }
                self.show_location_preview(cx);
            }

            // Handle the send location button being clicked.
            if self.button(id!(location_preview.send_location_button)).clicked(actions) {
                let location_preview = self.location_preview(id!(location_preview));
                if let Some((coords, _system_time_opt)) = location_preview.get_current_data() {
                    let geo_uri = format!("{}{},{}", GEO_URI_SCHEME, coords.latitude, coords.longitude);
                    let message = RoomMessageEventContent::new(
                        MessageType::Location(
                            LocationMessageEventContent::new(geo_uri.clone(), geo_uri)
                        )
                    );
                    submit_async_request(MatrixRequest::SendMessage {
                        room_id: self.room_id.clone().unwrap(),
                        message,
                        replied_to: self.tl_state.as_mut().and_then(
                            |tl| tl.replying_to.take().map(|(_, rep)| rep)
                        ),
                        // TODO: support attaching mentions, etc.
                    });

                    self.clear_replying_to();
                    location_preview.clear();
                    location_preview.redraw(cx);
                }
            }

            // Handle the send message button being clicked and enter key being pressed.
            let msg_input_widget = self.text_input(id!(message_input));
            let send_message_shortcut_pressed = msg_input_widget
                .key_down_unhandled(actions)
                .is_some_and(|ke| ke.key_code == KeyCode::ReturnKey && ke.modifiers.is_primary());
            if send_message_shortcut_pressed
                || self.button(id!(send_message_button)).clicked(actions)
            {
                let entered_text = msg_input_widget.text().trim().to_string();
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
                        replied_to: self.tl_state.as_mut().and_then(
                            |tl| tl.replying_to.take().map(|(_, rep)| rep)
                        ),
                        // TODO: support attaching mentions, etc.
                    });

                    self.clear_replying_to();
                    msg_input_widget.set_text_and_redraw(cx, "");
                }
            }

            // Handle the jump to bottom button: update its visibility, and handle clicks.
            self.jump_to_bottom_button(id!(jump_to_bottom)).update_from_actions(
                cx,
                &portal_list,
                actions,
            );

            // Handle a typing action on the message input box.
            if let Some(new_text) = self.text_input(id!(message_input)).changed(actions) {
                submit_async_request(MatrixRequest::SendTypingNotice {
                    room_id: self.room_id.clone().unwrap(),
                    typing: !new_text.is_empty(),
                });
            }
        }

        // Mark events as fully read after they have been displayed on screen for 5 seconds.
        if self.fully_read_timer.is_event(event).is_some() {
            if let (Some(ref mut tl_state), Some(ref _room_id)) = (&mut self.tl_state, &self.room_id) {
                for (k, (room, event, start, ref mut moved_to_queue)) in &mut tl_state.read_event_hashmap {
                    if start.elapsed() > std::time::Duration::new(5, 0) && !*moved_to_queue{
                        tl_state.marked_fully_read_queue.insert(k.clone(), (room.clone(), event.clone()));
                        *moved_to_queue = true;
                    }
                }
            }
            cx.stop_timer(self.fully_read_timer);
        }

        if self.animator_handle_event(cx, event).must_redraw() {
            self.redraw(cx);
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

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        let room_screen_widget_uid = self.widget_uid();
        while let Some(subview) = self.view.draw_walk(cx, scope, walk).step() {
            // We only care about drawing the portal list.
            let portal_list_ref = subview.as_portal_list();
            let Some(mut list_ref) = portal_list_ref.borrow_mut() else {
                error!("!!! RoomScreen::draw_walk(): BUG: expected a PortalList widget, but got something else");
                continue;
            };
            let Some(tl_state) = self.tl_state.as_mut() else {
                return DrawStep::done();
            };
            let room_id = &tl_state.room_id;
            let tl_items = &tl_state.items;

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
                                let prev_event = tl_idx.checked_sub(1).and_then(|i| tl_items.get(i));
                                populate_message_view(
                                    cx,
                                    list,
                                    item_id,
                                    room_id,
                                    event_tl_item,
                                    MessageOrSticker::Message(message),
                                    prev_event,
                                    &mut tl_state.media_cache,
                                    item_drawn_status,
                                    room_screen_widget_uid,
                                )
                            }
                            TimelineItemContent::Sticker(sticker) => {
                                let prev_event = tl_idx.checked_sub(1).and_then(|i| tl_items.get(i));
                                populate_message_view(
                                    cx,
                                    list,
                                    item_id,
                                    room_id,
                                    event_tl_item,
                                    MessageOrSticker::Sticker(sticker.content()),
                                    prev_event,
                                    &mut tl_state.media_cache,
                                    item_drawn_status,
                                    room_screen_widget_uid,
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
                                let item = list.item(cx, item_id, live_id!(SmallStateEvent));
                                item.label(id!(content)).set_text(&format!("[Unsupported] {:?}", unhandled));
                                (item, ItemDrawnStatus::both_drawn())
                            }
                        }
                        TimelineItemKind::Virtual(VirtualTimelineItem::DayDivider(millis)) => {
                            let item = list.item(cx, item_id, live_id!(DayDivider));
                            let text = unix_time_millis_to_datetime(millis)
                                // format the time as a shortened date (Sat, Sept 5, 2021)
                                .map(|dt| format!("{}", dt.date_naive().format("%a %b %-d, %Y")))
                                .unwrap_or_else(|| format!("{:?}", millis));
                            item.label(id!(date)).set_text(&text);
                            (item, ItemDrawnStatus::both_drawn())
                        }
                        TimelineItemKind::Virtual(VirtualTimelineItem::ReadMarker) => {
                            let item = list.item(cx, item_id, live_id!(ReadMarker));
                            (item, ItemDrawnStatus::both_drawn())
                        }
                    };

                    // Now that we've drawn the item, add its index to the set of drawn items.
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
}

impl RoomScreen {
    /// Processes all pending background updates to the currently-shown timeline.
    ///
    /// Redraws this RoomScreen view if any updates were applied.
    fn process_timeline_updates(&mut self, cx: &mut Cx, portal_list: &PortalListRef) {
        let bottom_input = self.view(id!(bottom_input));
        let no_send_permission_notice = self.view(id!(no_send_permission_notice));

        let top_space = self.view(id!(top_space));
        let jump_to_bottom = self.jump_to_bottom_button(id!(jump_to_bottom));
        let curr_first_id = portal_list.first_id();
        let Some(tl) = self.tl_state.as_mut() else { return };

        let mut done_loading = false;
        let mut should_continue_backwards_pagination = false;
        let mut num_updates = 0;
        let mut typing_users = Vec::new();

        while let Ok(update) = tl.update_receiver.try_recv() {
            num_updates += 1;
            match update {
                TimelineUpdate::FirstUpdate { initial_items } => {
                    tl.content_drawn_since_last_update.clear();
                    tl.profile_drawn_since_last_update.clear();
                    tl.fully_paginated = false;
                    // Set the portal list to the very bottom of the timeline.
                    portal_list.set_first_id_and_scroll(initial_items.len().saturating_sub(1), 0.0);
                    portal_list.set_tail_range(true);
                    jump_to_bottom.update_visibility(true);

                    tl.items = initial_items;
                    done_loading = true;
                }
                TimelineUpdate::NewItems { new_items, changed_indices, is_append, clear_cache } => {
                    if new_items.is_empty() {
                        if !tl.items.is_empty() {
                            log!("Timeline::handle_event(): timeline (had {} items) was cleared for room {}", tl.items.len(), tl.room_id);
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

                    if new_items.len() == tl.items.len() {
                        // log!("Timeline::handle_event(): no jump necessary for updated timeline of same length: {}", items.len());
                    }
                    else if curr_first_id > new_items.len() {
                        log!("Timeline::handle_event(): jumping to bottom: curr_first_id {} is out of bounds for {} new items", curr_first_id, new_items.len());
                        portal_list.set_first_id_and_scroll(new_items.len().saturating_sub(1), 0.0);
                        portal_list.set_tail_range(true);
                        jump_to_bottom.update_visibility(true);
                    }
                    else if let Some((curr_item_idx, new_item_idx, new_item_scroll, _event_id)) =
                        find_new_item_matching_current_item(cx, portal_list, curr_first_id, &tl.items, &new_items)
                    {
                        if curr_item_idx != new_item_idx {
                            log!("Timeline::handle_event(): jumping view from event index {curr_item_idx} to new index {new_item_idx}, scroll {new_item_scroll}, event ID {_event_id}");
                            portal_list.set_first_id_and_scroll(new_item_idx, new_item_scroll);
                            tl.prev_first_index = Some(new_item_idx);
                            cx.stop_timer(self.fully_read_timer);
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
                        // log!("is_append was true, showing unread message badge on the jump to bottom button visible");
                        jump_to_bottom.show_unread_message_badge(1);
                    }

                    if clear_cache {
                        tl.content_drawn_since_last_update.clear();
                        tl.profile_drawn_since_last_update.clear();
                        tl.fully_paginated = false;

                        // If this RoomScreen is showing the loading modal and has an ongoing backwards pagination request,
                        // then we should update the status message in that loading modal
                        // and then continue paginating backwards until we find the target event.
                        // Note that we do this here because `clear_cache` will always be true if backwards pagination occurred.
                        let loading_modal_inner = self.view.loading_modal(id!(loading_modal_inner));
                        let mut loading_modal_state = loading_modal_inner.take_state();
                        if let LoadingModalState::BackwardsPaginateUntilEvent {
                            ref mut events_paginated, target_event_id, ..
                        } = &mut loading_modal_state {
                            *events_paginated += new_items.len().saturating_sub(tl.items.len());
                            log!("While finding target event {target_event_id}, loaded {events_paginated} messages...");
                            // Here, we assume that we have not yet found the target event,
                            // so we need to continue paginating backwards.
                            // If the target event has already been found, it will be handled
                            // in the `TargetEventFound` match arm below, which will set
                            // `should_continue_backwards_pagination` to `false`.
                            // So either way, it's okay to set this to `true` here.
                            should_continue_backwards_pagination = true;
                        }
                        loading_modal_inner.set_state(cx, loading_modal_state);
                    } else {
                        tl.content_drawn_since_last_update.remove(changed_indices.clone());
                        tl.profile_drawn_since_last_update.remove(changed_indices.clone());
                        // log!("Timeline::handle_event(): changed_indices: {changed_indices:?}, items len: {}\ncontent drawn: {:#?}\nprofile drawn: {:#?}", items.len(), tl.content_drawn_since_last_update, tl.profile_drawn_since_last_update);
                    }
                    tl.items = new_items;
                    done_loading = true;
                }
                TimelineUpdate::TargetEventFound { target_event_id, index } => {
                    // log!("Target event found in room {}: {target_event_id}, index: {index}", tl.room_id);
                    tl.request_sender.send_if_modified(|requests| {
                        requests.retain(|r| r.room_id != tl.room_id);
                        // no need to notify/wake-up all receivers for a completed request
                        false
                    });

                    // sanity check: ensure the target event is in the timeline at the given `index`.
                    let item = tl.items.get(index);
                    let is_valid = item.is_some_and(|item|
                        item.as_event()
                            .is_some_and(|ev| ev.event_id() == Some(&target_event_id))
                    );
                    let loading_modal_inner = self.view.loading_modal(id!(loading_modal_inner));

                    // log!("TargetEventFound: is_valid? {is_valid}. room {}, event {target_event_id}, index {index} of {}\n  --> item: {item:?}", tl.room_id, tl.items.len());
                    if is_valid {
                        // We successfully found the target event, so we can close the loading modal,
                        // reset the loading modal state to `None`, and stop issuing backwards pagination requests.
                        loading_modal_inner.set_status(cx, "Successfully found replied-to message!");
                        loading_modal_inner.set_state(cx, LoadingModalState::None);
                        self.view.modal(id!(loading_modal)).close(cx);

                        // NOTE: this code was copied from the `ReplyPreviewClicked` action handler;
                        //       we should deduplicate them at some point.
                        let speed = 50.0;
                        // Scroll to the message right above the replied-to message.
                        // FIXME: `smooth_scroll_to` should accept a scroll offset parameter too,
                        //       so that we can scroll to the replied-to message and have it
                        //       appear beneath the top of the viewport.
                        portal_list.smooth_scroll_to(cx, index.saturating_sub(1), speed, None);
                        // start highlight animation.
                        tl.message_highlight_animation_state = MessageHighlightAnimationState::Pending {
                            item_id: index
                        };
                    }
                    else {
                        // Here, the target event was not found in the current timeline,
                        // or we found it previously but it is no longer in the timeline (or has moved),
                        // which means we encountered an error and are unable to jump to the target event.
                        error!("Target event index {index} of {} is out of bounds for room {}", tl.items.len(), tl.room_id);
                        // Show this error in the loading modal, which should already be open.
                        loading_modal_inner.set_state(cx, LoadingModalState::Error(
                            String::from("Unable to find replied-to message; it may have been deleted.")
                        ));
                    }

                    should_continue_backwards_pagination = false;

                    // redraw now before any other items get added to the timeline list.
                    self.view.redraw(cx);
                }
                TimelineUpdate::PaginationRunning(direction) => {
                    if direction == PaginationDirection::Backwards {
                        top_space.set_visible(true);
                        done_loading = false;
                    } else {
                        error!("Unexpected PaginationRunning update in the Forwards direction");
                    }
                }
                TimelineUpdate::PaginationError { error, direction } => {
                    error!("Pagination error ({direction}) in room {}: {error:?}", tl.room_id);
                    done_loading = true;
                }
                TimelineUpdate::PaginationIdle { fully_paginated, direction } => {
                    if direction == PaginationDirection::Backwards {
                        // Don't set `done_loading` to `true`` here, because we want to keep the top space visible
                        // (with the "loading" message) until the corresponding `NewItems` update is received.
                        tl.fully_paginated = fully_paginated;
                        if fully_paginated {
                            done_loading = true;
                        }
                    } else {
                        error!("Unexpected PaginationIdle update in the Forwards direction");
                    }
                }
                TimelineUpdate::EventDetailsFetched {event_id, result } => {
                    if let Err(_e) = result {
                        error!("Failed to fetch details fetched for event {event_id} in room {}. Error: {_e:?}", tl.room_id);
                    }
                    // Here, to be most efficient, we could redraw only the updated event,
                    // but for now we just fall through and let the final `redraw()` call re-draw the whole timeline view.
                }
                TimelineUpdate::RoomMembersFetched => {
                    // log!("Timeline::handle_event(): room members fetched for room {}", tl.room_id);
                    // Here, to be most efficient, we could redraw only the user avatars and names in the timeline,
                    // but for now we just fall through and let the final `redraw()` call re-draw the whole timeline view.
                }
                TimelineUpdate::MediaFetched => {
                    log!("Timeline::handle_event(): media fetched for room {}", tl.room_id);
                    // Here, to be most efficient, we could redraw only the media items in the timeline,
                    // but for now we just fall through and let the final `redraw()` call re-draw the whole timeline view.
                }

                TimelineUpdate::TypingUsers { users } => {
                    // This update loop should be kept tight & fast, so all we do here is
                    // save the list of typing users for future use after the loop exits.
                    // Then, we "process" it later (by turning it into a string) after the
                    // update loop has completed, which avoids unnecessary expensive work
                    // if the list of typing users gets updated many times in a row.
                    typing_users = users;
                }

                TimelineUpdate::CanUserPost(can_user_post) => {
                    let (bottom_input_visible, no_send_permission_notice_visible) = if can_user_post { (true, false) } else { (false, true) };

                    bottom_input.set_visible(bottom_input_visible);
                    no_send_permission_notice.set_visible(no_send_permission_notice_visible)
                }
            }
        }

        if should_continue_backwards_pagination {
            submit_async_request(MatrixRequest::PaginateRoomTimeline {
                room_id: tl.room_id.clone(),
                num_events: 50,
                direction: PaginationDirection::Backwards,
            });
        }

        if done_loading {
            top_space.set_visible(false);
        }

        if !typing_users.is_empty() {
            let typing_notice_text = match typing_users.as_slice() {
                [] => String::new(),
                [user] => format!("{user} is typing "),
                [user1, user2] => format!("{user1} and {user2} are typing "),
                [user1, user2, others @ ..] => {
                    if others.len() > 1 {
                        format!("{user1}, {user2}, and {} are typing ", &others[0])
                    } else {
                        format!(
                            "{user1}, {user2}, and {} others are typing ",
                            others.len()
                        )
                    }
                }
            };
            // Set the typing notice text and make its view visible.
            self.view.label(id!(typing_label)).set_text(&typing_notice_text);
            self.view.view(id!(typing_notice)).set_visible(true);
            // Animate in the typing notice view (sliding it up from the bottom).
            self.animator_play(cx, id!(typing_notice_animator.show));
            // Start the typing notice text animation of bouncing dots.
            let typing_animation = self.view.typing_animation(id!(typing_animation));
            typing_animation.animate(cx);
        } else {
            // Animate out the typing notice view (sliding it out towards the bottom).
            self.animator_play(cx, id!(typing_notice_animator.hide));
            let typing_animation = self.view.typing_animation(id!(typing_animation));
            typing_animation.stop_animation();
        }

        if num_updates > 0 {
            // log!("Applied {} timeline updates for room {}, redrawing with {} items...", num_updates, tl.room_id, tl.items.len());
            self.redraw(cx);
        }
    }

    /// Shows the user profile sliding pane with the given avatar info.
    fn show_user_profile(
        &mut self,
        cx: &mut Cx,
        pane: &UserProfileSlidingPaneRef,
        info: UserProfilePaneInfo,
    ) {
        pane.set_info(cx, info);
        pane.show(cx);
        // Not sure if this redraw is necessary
        self.redraw(cx);
    }

    /// Shows a preview of the given event that the user is currently replying to
    /// above the message input bar.
    fn show_replying_to(
        &mut self,
        cx: &mut Cx,
        replying_to: (EventTimelineItem, RepliedToInfo),
    ) {
        let replying_preview_view = self.view(id!(replying_preview));
        let (replying_preview_username, _) = set_avatar_and_get_username(
            cx,
            replying_preview_view.avatar(id!(reply_preview_content.reply_preview_avatar)),
            self.room_id.as_ref().unwrap(),
            replying_to.0.sender(),
            replying_to.0.sender_profile(),
            replying_to.0.event_id(),
        );

        replying_preview_view
            .label(id!(reply_preview_content.reply_preview_username))
            .set_text(replying_preview_username.as_str());

        populate_preview_of_timeline_item(
            &replying_preview_view.html_or_plaintext(id!(reply_preview_content.reply_preview_body)),
            replying_to.0.content(),
            &replying_preview_username,
        );

        self.view(id!(replying_preview)).set_visible(true);
        if let Some(tl) = self.tl_state.as_mut() {
            tl.replying_to = Some(replying_to);
        }

        // After the user clicks the reply button next to a message,
        // and we get to this point where the replying-to preview is shown,
        // we should automatically focus the keyboard on the message input box
        // so that the user can immediately start typing their reply
        // without having to manually click on the message input box.
        self.text_input(id!(message_input)).set_key_focus(cx);
        self.redraw(cx);
    }

    /// Clears (and makes invisible) the preview of the message
    /// that the user is currently replying to.
    fn clear_replying_to(&mut self) {
        self.view(id!(replying_preview)).set_visible(false);
        if let Some(tl) = self.tl_state.as_mut() {
            tl.replying_to = None;
        }
    }

    fn show_location_preview(&mut self, cx: &mut Cx) {
        self.location_preview(id!(location_preview)).show();
        self.redraw(cx);
    }

    /// Invoke this when this timeline is being shown,
    /// e.g., when the user navigates to this timeline.
    fn show_timeline(&mut self, cx: &mut Cx) {
        self.check_user_post_permission();

        let room_id = self.room_id.clone()
            .expect("BUG: Timeline::show_timeline(): no room_id was set.");
        // just an optional sanity check
        assert!(self.tl_state.is_none(),
            "BUG: tried to show_timeline() into a timeline with existing state. \
            Did you forget to save the timeline state back to the global map of states?",
        );

        let (mut tl_state, first_time_showing_room) = if let Some(existing) = TIMELINE_STATES.lock().unwrap().remove(&room_id) {
            (existing, false)
        } else {
            let (update_sender, update_receiver, request_sender) = take_timeline_endpoints(&room_id)
                .expect("BUG: couldn't get timeline state for first-viewed room.");
            let new_tl_state = TimelineUiState {
                room_id: room_id.clone(),
                // We assume timelines being viewed for the first time haven't been fully paginated.
                fully_paginated: false,
                items: Vector::new(),
                content_drawn_since_last_update: RangeSet::new(),
                profile_drawn_since_last_update: RangeSet::new(),
                update_receiver,
                request_sender,
                media_cache: MediaCache::new(MediaFormatConst::File, Some(update_sender)),
                replying_to: None,
                saved_state: SavedState::default(),
                message_highlight_animation_state: MessageHighlightAnimationState::default(),
                last_scrolled_index: usize::MAX,
                prev_first_index: None,
                read_event_hashmap: HashMap::new(),
                marked_fully_read_queue: HashMap::new(),
            };
            (new_tl_state, true)
        };

        // Subscribe to typing notices, but hide the typing notice view initially.
        self.view(id!(typing_notice)).set_visible(false);
        submit_async_request(
            MatrixRequest::SubscribeToTypingNotices {
                room_id: room_id.clone(),
                subscribe: true,
            }
        );

        // Kick off a back pagination request for this room. This is "urgent",
        // because we want to show the user some messages as soon as possible
        // when they first open the room, and there might not be any messages yet.
        if first_time_showing_room && !tl_state.fully_paginated {
            log!("Sending a first-time backwards pagination request for room {}", room_id);
            submit_async_request(MatrixRequest::PaginateRoomTimeline {
                room_id: room_id.clone(),
                num_events: 50,
                direction: PaginationDirection::Backwards,
            });

            // Even though we specify that room member profiles should be lazy-loaded,
            // the matrix server still doesn't consistently send them to our client properly.
            // So we kick off a request to fetch the room members here upon first viewing the room.
            submit_async_request(MatrixRequest::FetchRoomMembers { room_id });
        }

        // Now, restore the visual state of this timeline from its previously-saved state.
        self.restore_state(cx, &mut tl_state);

        // As the final step, store the tl_state for this room into this RoomScreen widget,
        // such that it can be accessed in future event/draw handlers.
        self.tl_state = Some(tl_state);

        // Now that we have restored the TimelineUiState into this RoomScreen widget,
        // we can proceed to processing pending background updates, and if any were processed,
        // the timeline will also be redrawn.
        if first_time_showing_room {
            let portal_list = self.portal_list(id!(list));
            self.process_timeline_updates(cx, &portal_list);
        }

        self.redraw(cx);
    }

    /// Invoke this when this RoomScreen/timeline is being hidden or no longer being shown.
    fn hide_timeline(&mut self) {
        let Some(room_id) = self.room_id.clone() else { return };

        self.save_state();

        // When closing a room view, we do the following with non-persistent states:
        // * Unsubscribe from typing notices, since we don't care about them
        //   when a given room isn't visible.
        // * Clear the location preview. We don't save this to the TimelineUiState
        //   because the location might change by the next time the user opens this same room.
        self.location_preview(id!(location_preview)).clear();
        submit_async_request(MatrixRequest::SubscribeToTypingNotices {
            room_id,
            subscribe: false,
        });
    }

    /// Removes the current room's visual UI state from this widget
    /// and saves it to the map of `TIMELINE_STATES` such that it can be restored later.
    ///
    /// Note: after calling this function, the widget's `tl_state` will be `None`.
    fn save_state(&mut self) {
        let Some(mut tl) = self.tl_state.take() else {
            error!("Timeline::save_state(): skipping due to missing state, room {:?}", self.room_id);
            return;
        };

        let portal_list = self.portal_list(id!(list));
        let first_index = portal_list.first_id();
        let message_input_box = self.text_input(id!(message_input));
        let state = SavedState {
            first_index_and_scroll: Some((first_index, portal_list.scroll_position())),
            message_input_state: message_input_box.save_state(),
            replying_to: tl.replying_to.clone(),
        };
        tl.saved_state = state;
        // Store this Timeline's `TimelineUiState` in the global map of states.
        TIMELINE_STATES.lock().unwrap().insert(tl.room_id.clone(), tl);
    }

    /// Restores the previously-saved visual UI state of this room.
    ///
    /// Note: this accepts a direct reference to the timeline's UI state,
    /// so this function must not try to re-obtain it by accessing `self.tl_state`.
    fn restore_state(&mut self, cx: &mut Cx, tl_state: &mut TimelineUiState) {
        let SavedState {
            first_index_and_scroll,
            message_input_state,
            replying_to,
        } = &mut tl_state.saved_state;
        if let Some((first_index, scroll_from_first_id)) = first_index_and_scroll {
            self.portal_list(id!(timeline.list))
                .set_first_id_and_scroll(*first_index, *scroll_from_first_id);
        } else {
            // If the first index is not set, then the timeline has not yet been scrolled by the user,
            // so we set the portal list to "tail" (track) the bottom of the list.
            self.portal_list(id!(timeline.list)).set_tail_range(true);
        }

        let saved_message_input_state = std::mem::take(message_input_state);
        self.text_input(id!(message_input))
            .restore_state(saved_message_input_state);
        if let Some(replying_to_event) = replying_to.take() {
            self.show_replying_to(cx, replying_to_event);
        } else {
            self.clear_replying_to();
        }
    }

    /// Sets this `RoomScreen` widget to display the timeline for the given room.
    pub fn set_displayed_room(
        &mut self,
        cx: &mut Cx,
        room_id: OwnedRoomId,
        room_name: String,
    ) {
        // If the room is already being displayed, then do nothing.
        if let Some(current_room_id) = &self.room_id {
            if current_room_id.eq(&room_id) {
                return;
            }
        }

        self.hide_timeline();
        let loading_modal = self.modal(id!(loading_modal));
        if loading_modal.is_open() {
            // this will also reset the state of the inner loading modal
            loading_modal.close(cx);
        }
        self.room_name = room_name;
        self.room_id = Some(room_id);
        self.show_timeline(cx);
    }

    /// Sends read receipts based on the current scroll position of the timeline.
    fn send_user_read_receipts_based_on_scroll_pos(
        &mut self,
        cx: &mut Cx,
        actions: &ActionsBuf,
        portal_list: &PortalListRef,
    ) {
        //stopped scrolling
        if portal_list.scrolled(actions) {
            return;
        }
        let first_index = portal_list.first_id();

        let Some(tl_state) = self.tl_state.as_mut() else { return };
        let Some(room_id) = self.room_id.as_ref() else { return };
        if let Some(ref mut index) = tl_state.prev_first_index {
            // to detect change of scroll when scroll ends
            if *index != first_index {
                // scroll changed
                self.fully_read_timer = cx.start_interval(5.0);
                let time_now = std::time::Instant::now();
                if first_index > *index {
                    // Store visible event messages with current time into a hashmap
                    let mut read_receipt_event = None;
                    for r in first_index .. (first_index + portal_list.visible_items() + 1) {
                        if let Some(v) = tl_state.items.get(r) {
                            if let Some(e) = v.as_event().and_then(|f| f.event_id()) {
                                read_receipt_event = Some(e.to_owned());
                                tl_state.read_event_hashmap
                                    .entry(e.to_string())
                                    .or_insert_with(|| (room_id.clone(), e.to_owned(), time_now, false));
                            }
                        }
                    }
                    if let Some(event_id) = read_receipt_event {
                        submit_async_request(MatrixRequest::ReadReceipt { room_id: room_id.clone(), event_id });
                    }
                    let mut fully_read_receipt_event = None;
                    // Implements sending fully read receipts when message is scrolled out of first row
                    for r in *index..first_index {
                        if let Some(v) = tl_state.items.get(r) {
                            if let Some(e) = v.as_event().and_then(|f| f.event_id()) {
                                let mut to_remove = vec![];
                                for (event_id_string, (_, event_id)) in &tl_state.marked_fully_read_queue {
                                    if e == event_id {
                                        fully_read_receipt_event = Some(event_id.clone());
                                        to_remove.push(event_id_string.clone());
                                    }
                                }
                                for r in to_remove {
                                    tl_state.marked_fully_read_queue.remove(&r);
                                }
                            }
                        }
                    }
                    if let Some(event_id) = fully_read_receipt_event {
                        submit_async_request(MatrixRequest::FullyReadReceipt { room_id: room_id.clone(), event_id: event_id.clone()});
                    }
                }
                *index = first_index;
            }
        } else {
            tl_state.prev_first_index = Some(first_index);
        }
    }

    /// Sends a backwards pagination request if the user is scrolling up
    /// and is approaching the top of the timeline.
    fn send_pagination_request_based_on_scroll_pos(
        &mut self,
        _cx: &mut Cx,
        actions: &ActionsBuf,
        portal_list: &PortalListRef,
    ) {
        let Some(tl) = self.tl_state.as_mut() else { return };
        if tl.fully_paginated { return };
        if !portal_list.scrolled(actions) { return };

        let first_index = portal_list.first_id();
        if first_index == 0 && tl.last_scrolled_index > 0 {
            log!("Scrolled up from item {} --> 0, sending back pagination request for room {}",
                tl.last_scrolled_index, tl.room_id,
            );
            submit_async_request(MatrixRequest::PaginateRoomTimeline {
                room_id: tl.room_id.clone(),
                num_events: 50,
                direction: PaginationDirection::Backwards,
            });
        }
        tl.last_scrolled_index = first_index;
    }

    /// Send request as `MatrixRequest` to check post permission.
    fn check_user_post_permission(&self) {
        if let Some(room_id) = self.room_id.clone() {
            submit_async_request(MatrixRequest::CheckUserPostPermission { room_id })
        }
    }
}

impl RoomScreenRef {
    /// See [`RoomScreen::set_displayed_room()`].
    pub fn set_displayed_room(
        &self,
        cx: &mut Cx,
        room_id: OwnedRoomId,
        room_name: String,
    ) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.set_displayed_room(cx, room_id, room_name);
    }
}

/// A message that is sent from a background async task to a room's timeline view
/// for the purpose of update the Timeline UI contents or metadata.
pub enum TimelineUpdate {
    /// The very first update a given room's timeline receives.
    FirstUpdate {
        /// The initial list of timeline items (events) for a room.
        initial_items: Vector<Arc<TimelineItem>>,
    },
    /// The content of a room's timeline was updated in the background.
    NewItems {
        /// The entire list of timeline items (events) for a room.
        new_items: Vector<Arc<TimelineItem>>,
        /// The range of indices in the `items` list that have been changed in this update
        /// and thus must be removed from any caches of drawn items in the timeline.
        /// Any items outside of this range are assumed to be unchanged and need not be redrawn.
        changed_indices: Range<usize>,
        /// An optimization that informs the UI whether the changes to the timeline
        /// resulted in new items being *appended to the end* of the timeline.
        is_append: bool,
        /// Whether to clear the entire cache of drawn items in the timeline.
        /// This supersedes `index_of_first_change` and is used when the entire timeline is being redrawn.
        clear_cache: bool,
    },
    /// The target event ID was found at the given `index` in the timeline items vector.
    ///
    /// This means that the RoomScreen widget can scroll the timeline up to this event,
    /// and the background `timeline_subscriber_handler` async task can stop looking for this event.
    TargetEventFound {
        target_event_id: OwnedEventId,
        index: usize,
    },
    /// A notice that the background task doing pagination for this room is currently running
    /// a pagination request in the given direction, and is waiting for that request to complete.
    PaginationRunning(PaginationDirection),
    /// An error occurred while paginating the timeline for this room.
    PaginationError {
        error: timeline::Error,
        direction: PaginationDirection,
    },
    /// A notice that the background task doing pagination for this room has become idle,
    /// meaning that it has completed its recent pagination request(s).
    PaginationIdle {
        /// If `true`, the start of the timeline has been reached, meaning that
        /// there is no need to send further pagination requests.
        fully_paginated: bool,
        direction: PaginationDirection,
    },
    /// A notice that event details have been fetched from the server,
    /// including a `result` that indicates whether the request was successful.
    EventDetailsFetched {
        event_id: OwnedEventId,
        result: Result<(), matrix_sdk_ui::timeline::Error>,
    },
    /// A notice that the room's members have been fetched from the server,
    /// though the success or failure of the request is not yet known until the client
    /// requests the member info via a timeline event's `sender_profile()` method.
    RoomMembersFetched,
    /// A notice that one or more requested media items (images, videos, etc.)
    /// that should be displayed in this timeline have now been fetched and are available.
    MediaFetched,
    /// A notice that one or more members of a this room are currently typing.
    TypingUsers {
        /// The list of users (their displayable name) who are currently typing in this room.
        users: Vec<String>,
    },
    CanUserPost (bool)
}

/// The global set of all timeline states, one entry per room.
static TIMELINE_STATES: Mutex<BTreeMap<OwnedRoomId, TimelineUiState>> = Mutex::new(BTreeMap::new());

/// The UI-side state of a single room's timeline, which is only accessed/updated by the UI thread.
///
/// This struct should only include states that need to be persisted for a given room
/// across multiple `Hide`/`Show` cycles of that room's timeline within a RoomScreen.
/// If a state is more temporary and shouldn't be persisted when the timeline is hidden,
/// then it should be stored in the RoomScreen widget itself, not in this struct.
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

    /// The sender for timeline requests from a RoomScreen showing this room
    /// to the background async task that handles this room's timeline updates.
    request_sender: TimelineRequestSender,

    /// The cache of media items (images, videos, etc.) that appear in this timeline.
    ///
    /// Currently this excludes avatars, as those are shared across multiple rooms.
    media_cache: MediaCache,

    /// Info about the event currently being replied to, if any.
    replying_to: Option<(EventTimelineItem, RepliedToInfo)>,

    /// The states relevant to the UI display of this timeline that are saved upon
    /// a `Hide` action and restored upon a `Show` action.
    saved_state: SavedState,

    /// The state of the message highlight animation.
    ///
    /// We need to run the animation once the scrolling, triggered by the click of of a
    /// a reply preview, ends. so we keep a small state for it.
    /// By default, it starts in Off.
    /// Once the scrolling is started, the state becomes Pending.
    /// If the animation was triggered, the state goes back to Off.
    message_highlight_animation_state: MessageHighlightAnimationState,

    /// The index of the timeline item that was most recently scrolled up past it.
    /// This is used to detect when the user has scrolled up past the second visible item (index 1)
    /// upwards to the first visible item (index 0), which is the top of the timeline,
    /// at which point we submit a backwards pagination request to fetch more events.
    last_scrolled_index: usize,

    prev_first_index: Option<usize>,
    read_event_hashmap: HashMap<String, (OwnedRoomId, OwnedEventId, Instant, bool)>,
    marked_fully_read_queue: HashMap<String, (OwnedRoomId, OwnedEventId)>,
}

#[derive(Default, Debug)]
enum MessageHighlightAnimationState {
    Pending { item_id: usize },
    #[default]
    Off,
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

    /// The content of the message input box.
    message_input_state: TextInputState,
    /// The event that the user is currently replying to, if any.
    replying_to: Option<(EventTimelineItem, RepliedToInfo)>,
}

/// Returns info about the item in the list of `new_items` that matches the event ID
/// of a visible item in the given `curr_items` list.
///
/// This info includes a tuple of:
/// 1. the index of the item in the current items list,
/// 2. the index of the item in the new items list,
/// 3. the positional "scroll" offset of the corresponding current item in the portal list,
/// 4. the unique event ID of the item.
fn find_new_item_matching_current_item(
    cx: &mut Cx,
    portal_list: &PortalListRef,
    starting_at_curr_idx: usize,
    curr_items: &Vector<Arc<TimelineItem>>,
    new_items: &Vector<Arc<TimelineItem>>,
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
            if let Some(event_id) = curr_item.as_event().and_then(|ev| ev.event_id()) {
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
        let Some(event_id) = new_item.as_event().and_then(|ev| ev.event_id()) else {
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
        Self {
            profile_drawn: false,
            content_drawn: false,
        }
    }
    /// Returns a new `ItemDrawnStatus` with both `profile_drawn` and `content_drawn` set to `true`.
    const fn both_drawn() -> Self {
        Self {
            profile_drawn: true,
            content_drawn: true,
        }
    }
}

/// Abstracts over a message or sticker that can be displayed in a timeline.
pub enum MessageOrSticker<'e> {
    Message(&'e timeline::Message),
    Sticker(&'e StickerEventContent),
}
impl MessageOrSticker<'_> {
    /// Returns the type of this message or sticker.
    pub fn get_type(&self) -> MessageOrStickerType {
        match self {
            Self::Message(msg) => match msg.msgtype() {
                MessageType::Audio(audio) => MessageOrStickerType::Audio(audio),
                MessageType::Emote(emote) => MessageOrStickerType::Emote(emote),
                MessageType::File(file) => MessageOrStickerType::File(file),
                MessageType::Image(image) => MessageOrStickerType::Image(image),
                MessageType::Location(location) => MessageOrStickerType::Location(location),
                MessageType::Notice(notice) => MessageOrStickerType::Notice(notice),
                MessageType::ServerNotice(server_notice) => MessageOrStickerType::ServerNotice(server_notice),
                MessageType::Text(text) => MessageOrStickerType::Text(text),
                MessageType::Video(video) => MessageOrStickerType::Video(video),
                MessageType::VerificationRequest(verification_request) => MessageOrStickerType::VerificationRequest(verification_request),
                MessageType::_Custom(custom) => MessageOrStickerType::_Custom(custom),
                _ => MessageOrStickerType::Unknown,
            },
            Self::Sticker(sticker) => MessageOrStickerType::Sticker(sticker),
        }
    }

    /// Returns the body of this message or sticker, which is a text representation of its content.
    pub fn body(&self) -> &str {
        match self {
            Self::Message(msg) => msg.body(),
            Self::Sticker(sticker) => sticker.body.as_str(),
        }
    }
    /// Returns the event that this message is replying to, if any.
    ///
    /// Returns `None` for stickers.
    pub fn in_reply_to(&self) -> Option<&InReplyToDetails> {
        match self {
            Self::Message(msg) => msg.in_reply_to(),
            _ => None,
        }
    }
}

/// Abstracts over the different types of messages or stickers that can be displayed in a timeline.
pub enum MessageOrStickerType<'e> {
    /// An audio message.
    Audio(&'e AudioMessageEventContent),
    /// An emote message.
    Emote(&'e EmoteMessageEventContent),
    /// A file message.
    File(&'e FileMessageEventContent),
    /// An image message.
    Image(&'e ImageMessageEventContent),
    /// A location message.
    Location(&'e LocationMessageEventContent),
    /// A notice message.
    Notice(&'e NoticeMessageEventContent),
    /// A server notice message.
    ServerNotice(&'e ServerNoticeMessageEventContent),
    /// A text message.
    Text(&'e TextMessageEventContent),
    /// A video message.
    Video(&'e VideoMessageEventContent),
    /// A request to initiate a key verification.
    VerificationRequest(&'e KeyVerificationRequestEventContent),
    /// A custom message.
    _Custom(&'e CustomEventContent),
    /// A sticker message.
    Sticker(&'e StickerEventContent),
    Unknown,
}
impl MessageOrStickerType<'_> {
    /// Returns details of the image for this message or sticker, if it contains one.
    pub fn get_image_info(&self) -> Option<(Option<ImageInfo>, MediaSource)> {
        match self {
            Self::Image(image) => Some((
                image.info.clone().map(|info| *info),
                image.source.clone(),
            )),
            Self::Sticker(sticker) => Some((
                Some(sticker.info.clone()),
                sticker.source.clone().into(),
            )),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Audio(_) => "Audio",
            Self::Emote(_) => "Emote",
            Self::File(_) => "File",
            Self::Image(_) => "Image",
            Self::Location(_) => "Location",
            Self::Notice(_) => "Notice",
            Self::ServerNotice(_) => "ServerNotice",
            Self::Text(_) => "Text",
            Self::Video(_) => "Video",
            Self::VerificationRequest(_) => "VerificationRequest",
            Self::_Custom(_) => "Custom",
            Self::Sticker(_) => "Sticker",
            Self::Unknown => "Unknown",
        }
    }
}


/// Creates, populates, and adds a Message liveview widget to the given `PortalList`
/// with the given `item_id`.
///
/// The content of the returned `Message` widget is populated with data from a message
/// or sticker and its containing `EventTimelineItem`.
fn populate_message_view(
    cx: &mut Cx2d,
    list: &mut PortalList,
    item_id: usize,
    room_id: &OwnedRoomId,
    event_tl_item: &EventTimelineItem,
    message: MessageOrSticker,
    prev_event: Option<&Arc<TimelineItem>>,
    media_cache: &mut MediaCache,
    item_drawn_status: ItemDrawnStatus,
    room_screen_widget_uid: WidgetUid
) -> (WidgetRef, ItemDrawnStatus) {
    let mut new_drawn_status = item_drawn_status;

    let ts_millis = event_tl_item.timestamp();

    let mut is_notice = false; // whether this message is a Notice
    let mut is_server_notice = false; // whether this message is a Server Notice

    // Determine whether we can use a more compact UI view that hides the user's profile info
    // if the previous message (including stickers) was sent by the same user within 10 minutes.
    let use_compact_view = match prev_event.map(|p| p.kind()) {
        Some(TimelineItemKind::Event(prev_event_tl_item)) => match prev_event_tl_item.content() {
            TimelineItemContent::Message(_) | TimelineItemContent::Sticker(_) => {
                let prev_msg_sender = prev_event_tl_item.sender();
                prev_msg_sender == event_tl_item.sender()
                    && ts_millis.0
                        .checked_sub(prev_event_tl_item.timestamp().0)
                        .map_or(false, |d| d < uint!(600000)) // 10 mins in millis
            }
            _ => false,
        },
        _ => false,
    };

    // Sometimes we need to call this up-front, so we save the result in this variable
    // to avoid having to call it twice.
    let mut set_username_and_get_avatar_retval = None;

    let (item, used_cached_item) = match message.get_type() {
        MessageOrStickerType::Text(TextMessageEventContent { body, formatted, .. }) => {
            let template = if use_compact_view {
                live_id!(CondensedMessage)
            } else {
                live_id!(Message)
            };
            let (item, existed) = list.item_with_existed(cx, item_id, template);
            if existed && item_drawn_status.content_drawn {
                (item, true)
            } else {
                populate_text_message_content(
                    &item.html_or_plaintext(id!(content.message)),
                    body,
                    formatted.as_ref(),
                );
                new_drawn_status.content_drawn = true;
                (item, false)
            }
        }
        // A notice message is just a message sent by an automated bot,
        // so we treat it just like a message but use a different font color.
        MessageOrStickerType::Notice(NoticeMessageEventContent { body, formatted, .. }) => {
            is_notice = true;
            let template = if use_compact_view {
                live_id!(CondensedMessage)
            } else {
                live_id!(Message)
            };
            let (item, existed) = list.item_with_existed(cx, item_id, template);
            if existed && item_drawn_status.content_drawn {
                (item, true)
            } else {
                let html_or_plaintext_ref = item.html_or_plaintext(id!(content.message));
                html_or_plaintext_ref.apply_over(cx, live!(
                    html_view = {
                        html = {
                            font_color: (MESSAGE_NOTICE_TEXT_COLOR),
                            draw_normal:      { color: (MESSAGE_NOTICE_TEXT_COLOR), }
                            draw_italic:      { color: (MESSAGE_NOTICE_TEXT_COLOR), }
                            draw_bold:        { color: (MESSAGE_NOTICE_TEXT_COLOR), }
                            draw_bold_italic: { color: (MESSAGE_NOTICE_TEXT_COLOR), }
                        }
                    }
                ));
                populate_text_message_content(
                    &html_or_plaintext_ref,
                    body,
                    formatted.as_ref(),
                );
                new_drawn_status.content_drawn = true;
                (item, false)
            }
        }
        MessageOrStickerType::ServerNotice(sn) => {
            is_server_notice = true;
            let (item, existed) = list.item_with_existed(cx, item_id, live_id!(Message));
            if existed && item_drawn_status.content_drawn {
                (item, true)
            } else {
                let html_or_plaintext_ref = item.html_or_plaintext(id!(content.message));
                html_or_plaintext_ref.apply_over(cx, live!(
                    html_view = {
                        html = {
                            font_color: (COLOR_DANGER_RED),
                            draw_normal:      { color: (COLOR_DANGER_RED), }
                            draw_italic:      { color: (COLOR_DANGER_RED), }
                            draw_bold:        { color: (COLOR_DANGER_RED), }
                            draw_bold_italic: { color: (COLOR_DANGER_RED), }
                        }
                    }
                ));
                let formatted = format!(
                    "<b>Server notice:</b> {}\n\n<i>Notice type:</i>: {}{}{}",
                    sn.body,
                    sn.server_notice_type.as_str(),
                    sn.limit_type.as_ref()
                        .map(|l| format!("\n<i>Limit type:</i> {}", l.as_str()))
                        .unwrap_or_default(),
                    sn.admin_contact.as_ref()
                        .map(|c| format!("\n<i>Admin contact:</i> {}", c))
                        .unwrap_or_default(),
                );
                populate_text_message_content(
                    &html_or_plaintext_ref,
                    &sn.body,
                    Some(&FormattedBody {
                        format: MessageFormat::Html,
                        body: formatted,
                    }),
                );
                new_drawn_status.content_drawn = true;
                (item, false)
            }
        }
        // An emote is just like a message but is prepended with the user's name
        // to indicate that it's an "action" that the user is performing.
        MessageOrStickerType::Emote(EmoteMessageEventContent { body, formatted, .. }) => {
            let template = if use_compact_view {
                live_id!(CondensedMessage)
            } else {
                live_id!(Message)
            };
            let (item, existed) = list.item_with_existed(cx, item_id, template);
            if existed && item_drawn_status.content_drawn {
                (item, true)
            } else {
                // Draw the profile up front here because we need the username for the emote body.
                let (username, profile_drawn) = set_avatar_and_get_username(
                    cx,
                    item.avatar(id!(profile.avatar)),
                    room_id,
                    event_tl_item.sender(),
                    event_tl_item.sender_profile(),
                    event_tl_item.event_id(),
                );

                // Prepend a "* <username> " to the emote body, as suggested by the Matrix spec.
                let (body, formatted) = if let Some(fb) = formatted.as_ref() {
                    (
                        Cow::from(&fb.body),
                        Some(FormattedBody {
                            format: fb.format.clone(),
                            body: format!("* {} {}", &username, &fb.body),
                        })
                    )
                } else {
                    (Cow::from(format!("* {} {}", &username, body)), None)
                };
                populate_text_message_content(
                    &item.html_or_plaintext(id!(content.message)),
                    &body,
                    formatted.as_ref(),
                );
                set_username_and_get_avatar_retval = Some((username, profile_drawn));
                new_drawn_status.content_drawn = true;
                (item, false)
            }
        }
        // Handle images and sticker messages that are static images.
        mtype @ MessageOrStickerType::Image(_) | mtype @ MessageOrStickerType::Sticker(_) => {
            let template = if use_compact_view {
                live_id!(CondensedImageMessage)
            } else {
                live_id!(ImageMessage)
            };
            let (item, existed) = list.item_with_existed(cx, item_id, template);
            if existed && item_drawn_status.content_drawn {
                (item, true)
            } else {
                let image_info = mtype.get_image_info();
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
        MessageOrStickerType::Location(location) => {
            let template = if use_compact_view {
                live_id!(CondensedMessage)
            } else {
                live_id!(Message)
            };
            let (item, existed) = list.item_with_existed(cx, item_id, template);
            if existed && item_drawn_status.content_drawn {
                (item, true)
            } else {
                let is_location_fully_drawn = populate_location_message_content(
                    &item.html_or_plaintext(id!(content.message)),
                    location,
                );
                new_drawn_status.content_drawn = is_location_fully_drawn;
                (item, false)
            }
        }
        MessageOrStickerType::File(file_content) => {
            let template = if use_compact_view {
                live_id!(CondensedMessage)
            } else {
                live_id!(Message)
            };
            let (item, existed) = list.item_with_existed(cx, item_id, template);
            if existed && item_drawn_status.content_drawn {
                (item, true)
            } else {
                new_drawn_status.content_drawn = populate_file_message_content(
                    &item.html_or_plaintext(id!(content.message)),
                    file_content,
                );
                (item, false)
            }
        }
        MessageOrStickerType::Audio(audio) => {
            let template = if use_compact_view {
                live_id!(CondensedMessage)
            } else {
                live_id!(Message)
            };
            let (item, existed) = list.item_with_existed(cx, item_id, template);
            if existed && item_drawn_status.content_drawn {
                (item, true)
            } else {
                new_drawn_status.content_drawn = populate_audio_message_content(
                    &item.html_or_plaintext(id!(content.message)),
                    audio,
                );
                (item, false)
            }
        }
        MessageOrStickerType::Video(video) => {
            let template = if use_compact_view {
                live_id!(CondensedMessage)
            } else {
                live_id!(Message)
            };
            let (item, existed) = list.item_with_existed(cx, item_id, template);
            if existed && item_drawn_status.content_drawn {
                (item, true)
            } else {
                new_drawn_status.content_drawn = populate_video_message_content(
                    &item.html_or_plaintext(id!(content.message)),
                    video,
                );
                (item, false)
            }
        }
        MessageOrStickerType::VerificationRequest(verification) => {
            let template = live_id!(Message);
            let (item, existed) = list.item_with_existed(cx, item_id, template);
            if existed && item_drawn_status.content_drawn {
                (item, true)
            } else {
                // Use `FormattedBody` to hold our custom summary of this verification request.
                let formatted = FormattedBody {
                    format: MessageFormat::Html,
                    body: format!(
                        "<i>Sent a <b>verification request</b> to {}.<br>(Supported methods: {})</i>",
                        verification.to,
                        verification.methods
                            .iter()
                            .map(|m| m.as_str())
                            .collect::<Vec<_>>()
                            .join(", "),
                    ),
                };

                populate_text_message_content(
                    &item.html_or_plaintext(id!(content.message)),
                    &verification.body,
                    Some(&formatted),
                );
                new_drawn_status.content_drawn = true;
                (item, false)
            }
        }
        other => {
            let (item, existed) = list.item_with_existed(cx, item_id, live_id!(Message));
            if existed && item_drawn_status.content_drawn {
                (item, true)
            } else {
                let kind = other.as_str();
                item.label(id!(content.message))
                    .set_text(&format!("[Unsupported ({kind})] {}", message.body()));
                new_drawn_status.content_drawn = true;
                (item, false)
            }
        }
    };

    let mut replied_to_event_id = None;

    // If we didn't use a cached item, we need to draw all other message content: the reply preview and reactions.
    if !used_cached_item {
        draw_reactions(cx, &item, event_tl_item.reactions(), item_id);
        let (is_reply_fully_drawn, replied_to_ev_id) = draw_replied_to_message(
            cx,
            &item.view(id!(replied_to_message)),
            room_id,
            message.in_reply_to(),
            event_tl_item.event_id(),
        );
        replied_to_event_id = replied_to_ev_id;
        // The content is only considered to be fully drawn if the logic above marked it as such
        // *and* if the reply preview was also fully drawn.
        new_drawn_status.content_drawn &= is_reply_fully_drawn;
    }

    // If `used_cached_item` is false, we should always redraw the profile, even if profile_drawn is true.
    let skip_draw_profile =
        use_compact_view || (used_cached_item && item_drawn_status.profile_drawn);
    if skip_draw_profile {
        // log!("\t --> populate_message_view(): SKIPPING profile draw for item_id: {item_id}");
        new_drawn_status.profile_drawn = true;
    } else {
        // log!("\t --> populate_message_view(): DRAWING  profile draw for item_id: {item_id}");
        let username_label = item.label(id!(content.username));

        if !is_server_notice { // the normal case
            let (username, profile_drawn) = set_username_and_get_avatar_retval.unwrap_or_else(||
                set_avatar_and_get_username(
                    cx,
                    item.avatar(id!(profile.avatar)),
                    room_id,
                    event_tl_item.sender(),
                    event_tl_item.sender_profile(),
                    event_tl_item.event_id(),
                )
            );
            if is_notice {
                username_label.apply_over(cx, live!(
                    draw_text: {
                        color: (MESSAGE_NOTICE_TEXT_COLOR),
                    }
                ));
            }
            username_label.set_text(&username);
            new_drawn_status.profile_drawn = profile_drawn;
        }
        else {
            // Server notices are drawn with a red color avatar background and username.
            let avatar = item.avatar(id!(profile.avatar));
            avatar.show_text(None, "⚠");
            avatar.apply_over(cx, live!(
                text_view = {
                    draw_bg: { background_color: (COLOR_DANGER_RED), }
                }
            ));
            username_label.set_text("Server notice");
            username_label.apply_over(cx, live!(
                draw_text: {
                    color: (COLOR_DANGER_RED),
                }
            ));
            new_drawn_status.profile_drawn = true;
        }
    }

    // If we've previously drawn the item content, skip all other steps.
    if used_cached_item && item_drawn_status.content_drawn && item_drawn_status.profile_drawn {
        return (item, new_drawn_status);
    }

    // Set the Message widget's metadata for reply-handling purposes.
    item.as_message().set_data(
        event_tl_item.can_be_replied_to(),
        item_id,
        replied_to_event_id,
        room_screen_widget_uid,
        match message {
            MessageOrSticker::Message(msg) => does_message_mention_current_user(msg),
            MessageOrSticker::Sticker(_) => false, // Stickers can't mention users.
        }
    );

    // Set the timestamp.
    if let Some(dt) = unix_time_millis_to_datetime(&ts_millis) {
        // format as AM/PM 12-hour time
        item.label(id!(profile.timestamp))
            .set_text(&format!("{}", dt.time().format("%l:%M %P")));
        if !use_compact_view {
            item.label(id!(profile.datestamp))
                .set_text(&format!("{}", dt.date_naive()));
        }
    } else {
        item.label(id!(profile.timestamp))
            .set_text(&format!("{}", ts_millis.get()));
    }

    (item, new_drawn_status)
}


/// Returns `true` if the given message mentions the current user.
fn does_message_mention_current_user(
    message: &timeline::Message,
) -> bool {
    let Some(client) = get_client() else {
        return false;
    };
    let Some(current_user_id) = client.user_id() else {
        return false;
    };

    // This covers both direct mentions ("@user") and a replied-to message.
    message.mentions().is_some_and(
        |mentions| mentions.user_ids.contains(current_user_id)
    )
}

/// Draws the Html or plaintext body of the given Text or Notice message into the `message_content_widget`.
fn populate_text_message_content(
    message_content_widget: &HtmlOrPlaintextRef,
    body: &str,
    formatted_body: Option<&FormattedBody>,
) {
    if let Some(formatted_body) = formatted_body.as_ref()
        .and_then(|fb| (fb.format == MessageFormat::Html).then(|| fb.body.clone()))
    {
        message_content_widget.show_html(utils::linkify(formatted_body.as_ref()));
    } else {
        match utils::linkify(body) {
            Cow::Owned(linkified_html) => message_content_widget.show_html(&linkified_html),
            Cow::Borrowed(plaintext) => message_content_widget.show_plaintext(plaintext),
        }
    }
}

/// Draws the given image message's content into the `message_content_widget`.
///
/// Returns whether the image message content was fully drawn.
fn populate_image_message_content(
    cx: &mut Cx2d,
    text_or_image_ref: &TextOrImageRef,
    image_info_source: Option<(Option<ImageInfo>, MediaSource)>,
    body: &str,
    media_cache: &mut MediaCache,
) -> bool {
    // We don't use thumbnails, as their resolution is too low to be visually useful.
    // We also don't trust the provided mimetype, as it can be incorrect.
    let (mimetype, _width, _height) = image_info_source.as_ref()
        .and_then(|(info, _)| info.as_ref()
            .map(|info| (info.mimetype.as_deref(), info.width, info.height))
        )
        .unwrap_or_default();

    // If we have a known mimetype and it's not an image (e.g., an animated)
    // then show a message about it being unsupported.
    if let Some(mime) = mimetype.as_ref() {
        if ImageFormat::from_mimetype(mime).is_none() {
            text_or_image_ref.show_text(format!(
                "{body}\n\nImages/Stickers of type {mime:?} are not yet supported.",
            ));
            return true;
        }
    }

    match image_info_source.map(|(_, source)| source) {
        Some(MediaSource::Plain(mxc_uri)) => {
            // now that we've obtained the image URI and its metadata, try to fetch the image.
            match media_cache.try_get_media_or_fetch(mxc_uri.clone(), None) {
                MediaCacheEntry::Loaded(data) => {
                    let show_image_result = text_or_image_ref.show_image(|img| {
                        utils::load_png_or_jpg(&img, cx, &data)
                            .map(|()| img.size_in_pixels(cx).unwrap())
                    });
                    if let Err(e) = show_image_result {
                        let err_str = format!("{body}\n\nFailed to display image: {e:?}");
                        error!("{err_str}");
                        text_or_image_ref.show_text(&err_str);
                    }

                    // We're done drawing the image message content, so mark it as fully drawn.
                    true
                }
                MediaCacheEntry::Requested => {
                    text_or_image_ref.show_text(format!("{body}\n\nFetching image from {:?}", mxc_uri));
                    // Do not consider this image as being fully drawn, as we're still fetching it.
                    false
                }
                MediaCacheEntry::Failed => {
                    text_or_image_ref
                        .show_text(format!("{body}\n\nFailed to fetch image from {:?}", mxc_uri));
                    // For now, we consider this as being "complete". In the future, we could support
                    // retrying to fetch the image on a user click/tap.
                    true
                }
            }
        }
        Some(MediaSource::Encrypted(encrypted)) => {
            text_or_image_ref.show_text(format!(
                "{body}\n\n[TODO] fetch encrypted image at {:?}",
                encrypted.url
            ));
            // We consider this as "fully drawn" since we don't yet support encryption.
            true
        }
        None => {
            text_or_image_ref.show_text("{body}\n\nImage message had no source URL.");
            true
        }

    }
}


/// Draws a file message's content into the given `message_content_widget`.
///
/// Returns whether the file message content was fully drawn.
fn populate_file_message_content(
    message_content_widget: &HtmlOrPlaintextRef,
    file_content: &FileMessageEventContent,
) -> bool {
    // Display the file name, human-readable size, caption, and a button to download it.
    let filename = file_content.filename();
    let size = file_content
        .info
        .as_ref()
        .and_then(|info| info.size)
        .map(|bytes| format!("  ({})", ByteSize::b(bytes.into())))
        .unwrap_or_default();
    let caption = file_content.formatted_caption()
        .map(|fb| format!("<br><i>{}</i>", fb.body))
        .or_else(|| file_content.caption().map(|c| format!("<br><i>{c}</i>")))
        .unwrap_or_default();

    // TODO: add a button to download the file

    message_content_widget.show_html(format!(
        "<b>{filename}</b>{size}{caption}<br> → <i>File download not yet supported.</i>"
    ));
    true
}

/// Draws an audio message's content into the given `message_content_widget`.
///
/// Returns whether the audio message content was fully drawn.
fn populate_audio_message_content(
    message_content_widget: &HtmlOrPlaintextRef,
    audio: &AudioMessageEventContent,
) -> bool {
    // Display the file name, human-readable size, caption, and a button to download it.
    let filename = audio.filename();
    let (duration, mime, size) = audio
        .info
        .as_ref()
        .map(|info| (
            info.duration
                .map(|d| format!("  {:.2} sec,", d.as_secs_f64()))
                .unwrap_or_default(),
            info.mimetype
                .as_ref()
                .map(|m| format!("  {m},"))
                .unwrap_or_default(),
            info.size
                .map(|bytes| format!("  ({}),", ByteSize::b(bytes.into())))
                .unwrap_or_default(),
        ))
        .unwrap_or_default();
    let caption = audio.formatted_caption()
        .map(|fb| format!("<br><i>{}</i>", fb.body))
        .or_else(|| audio.caption().map(|c| format!("<br><i>{c}</i>")))
        .unwrap_or_default();

    // TODO: add an audio to play the audio file

    message_content_widget.show_html(format!(
        "Audio: <b>{filename}</b>{mime}{duration}{size}{caption}<br> → <i>Audio playback not yet supported.</i>"
    ));
    true
}


/// Draws a video message's content into the given `message_content_widget`.
///
/// Returns whether the video message content was fully drawn.
fn populate_video_message_content(
    message_content_widget: &HtmlOrPlaintextRef,
    video: &VideoMessageEventContent,
) -> bool {
    // Display the file name, human-readable size, caption, and a button to download it.
    let filename = video.filename();
    let (duration, mime, size, dimensions) = video
        .info
        .as_ref()
        .map(|info| (
            info.duration
                .map(|d| format!("  {:.2} sec,", d.as_secs_f64()))
                .unwrap_or_default(),
            info.mimetype
                .as_ref()
                .map(|m| format!("  {m},"))
                .unwrap_or_default(),
            info.size
                .map(|bytes| format!("  ({}),", ByteSize::b(bytes.into())))
                .unwrap_or_default(),
            info.width.and_then(|width|
                info.height.map(|height| format!("  {width}x{height},"))
            ).unwrap_or_default(),
        ))
        .unwrap_or_default();
    let caption = video.formatted_caption()
        .map(|fb| format!("<br><i>{}</i>", fb.body))
        .or_else(|| video.caption().map(|c| format!("<br><i>{c}</i>")))
        .unwrap_or_default();

    // TODO: add an video to play the video file

    message_content_widget.show_html(format!(
        "Video: <b>{filename}</b>{mime}{duration}{size}{dimensions}{caption}<br> → <i>Video playback not yet supported.</i>"
    ));
    true
}



/// Draws the given location message's content into the `message_content_widget`.
///
/// Returns whether the location message content was fully drawn.
fn populate_location_message_content(
    message_content_widget: &HtmlOrPlaintextRef,
    location: &LocationMessageEventContent,
) -> bool {
    let coords = location.geo_uri
        .get(GEO_URI_SCHEME.len() ..)
        .and_then(|s| {
            let mut iter = s.split(',');
            if let (Some(lat), Some(long)) = (iter.next(), iter.next()) {
                Some((lat, long))
            } else {
                None
            }
        });
    if let Some((lat, long)) = coords {
        let short_lat = lat.find('.').and_then(|dot| lat.get(..dot + 7)).unwrap_or(lat);
        let short_long = long.find('.').and_then(|dot| long.get(..dot + 7)).unwrap_or(long);
        let html_body = format!(
            "Location: {short_lat},{short_long}\
            <p><a href=\"https://www.openstreetmap.org/?mlat={lat}&amp;mlon={long}#map=15/{lat}/{long}\">Open in OpenStreetMap</a></p>\
            <p><a href=\"https://www.google.com/maps/search/?api=1&amp;query={lat},{long}\">Open in Google Maps</a></p>\
            <p><a href=\"https://maps.apple.com/?ll={lat},{long}&amp;q={lat},{long}\">Open in Apple Maps</a></p>",
        );
        message_content_widget.show_html(html_body);
    } else {
        message_content_widget.show_html(
            format!("<i>[Location invalid]</i> {}", location.body)
        );
    }

    // Currently we do not fetch location thumbnail previews, so we consider this as fully drawn.
    // In the future, when we do support this, we'll return false until the thumbnail is fetched,
    // at which point we can return true.
    true
}

/// Draws a ReplyPreview above a message if it was in-reply to another message.
///
/// If the given `in_reply_to` details are `None`,
/// this function will mark the ReplyPreview as non-visible and consider it fully drawn.
///
/// Returns whether the in-reply-to information was available and fully drawn,
/// i.e., whether it can be considered as cached and not needing to be redrawn later.
fn draw_replied_to_message(
    cx: &mut Cx2d,
    replied_to_message_view: &ViewRef,
    room_id: &OwnedRoomId,
    in_reply_to: Option<&InReplyToDetails>,
    message_event_id: Option<&EventId>,
) -> (bool, Option<OwnedEventId>) {
    let fully_drawn: bool;
    let show_reply: bool;
    let mut replied_to_event_id = None;

    if let Some(in_reply_to_details) = in_reply_to {
        replied_to_event_id = Some(in_reply_to_details.event_id.to_owned());
        show_reply = true;

        match &in_reply_to_details.event {
            TimelineDetails::Ready(replied_to_event) => {
                let (in_reply_to_username, is_avatar_fully_drawn) = set_avatar_and_get_username(
                    cx,
                    replied_to_message_view
                        .avatar(id!(replied_to_message_content.reply_preview_avatar)),
                    room_id,
                    replied_to_event.sender(),
                    replied_to_event.sender_profile(),
                    Some(in_reply_to_details.event_id.as_ref()),
                );

                fully_drawn = is_avatar_fully_drawn;

                replied_to_message_view
                    .label(id!(replied_to_message_content.reply_preview_username))
                    .set_text(in_reply_to_username.as_str());
                let msg_body = replied_to_message_view.html_or_plaintext(id!(reply_preview_body));
                populate_preview_of_timeline_item(
                    &msg_body,
                    replied_to_event.content(),
                    &in_reply_to_username,
                );
            }
            TimelineDetails::Error(_e) => {
                fully_drawn = true;
                replied_to_message_view
                    .label(id!(replied_to_message_content.reply_preview_username))
                    .set_text("[Error fetching username]");
                replied_to_message_view
                    .avatar(id!(replied_to_message_content.reply_preview_avatar))
                    .show_text(None, "?");
                replied_to_message_view
                    .html_or_plaintext(id!(replied_to_message_content.reply_preview_body))
                    .show_plaintext("[Error fetching replied-to event]");
            }
            status @ TimelineDetails::Pending | status @ TimelineDetails::Unavailable => {
                // We don't have the replied-to message yet, so we can't fully draw the preview.
                fully_drawn = false;
                replied_to_message_view
                    .label(id!(replied_to_message_content.reply_preview_username))
                    .set_text("[Loading username...]");
                replied_to_message_view
                    .avatar(id!(replied_to_message_content.reply_preview_avatar))
                    .show_text(None, "?");
                replied_to_message_view
                    .html_or_plaintext(id!(replied_to_message_content.reply_preview_body))
                    .show_plaintext("[Loading replied-to message...]");

                // Confusingly, we need to fetch the details of the `message` (the event that is the reply),
                // not the details of the original event that this `message` is replying to.
                if matches!(status, TimelineDetails::Unavailable) {
                    if let Some(event_id) = message_event_id {
                        submit_async_request(MatrixRequest::FetchDetailsForEvent {
                            room_id: room_id.to_owned(),
                            event_id: event_id.to_owned(),
                        });
                    }
                }
            }
        }
    } else {
        // This message was not in reply to another message, so we don't need to show a reply.
        show_reply = false;
        fully_drawn = true;
    }

    replied_to_message_view.set_visible(show_reply);
    (fully_drawn, replied_to_event_id)
}

fn populate_preview_of_timeline_item(
    widget_out: &HtmlOrPlaintextRef,
    timeline_item_content: &TimelineItemContent,
    sender_username: &str,
) {
    if let TimelineItemContent::Message(m) = timeline_item_content {
        match m.msgtype() {
            MessageType::Text(TextMessageEventContent { body, formatted, .. })
            | MessageType::Notice(NoticeMessageEventContent { body, formatted, .. }) => {
                return populate_text_message_content(widget_out, body, formatted.as_ref());
            }
            _ => { } // fall through to the general case for all timeline items below.
        }
    }
    let html = text_preview_of_timeline_item(timeline_item_content, sender_username)
        .format_with(sender_username);
    widget_out.show_html(html);
}

/// Draws the reactions beneath the given `message_item`.
fn draw_reactions(
    _cx: &mut Cx2d,
    message_item: &WidgetRef,
    reactions: &ReactionsByKeyBySender,
    id: usize,
) {
    const DRAW_ITEM_ID_REACTION: bool = false;
    if reactions.is_empty() && !DRAW_ITEM_ID_REACTION {
        return;
    }

    // The message annotations view is invisible by default, so we must set it to visible
    // now that we know there are reactions to show.
    message_item
        .view(id!(content.message_annotations))
        .set_visible(true);

    let mut label_text = String::new();
    for (reaction_raw, reaction_senders) in reactions.iter() {
        // Just take the first char of the emoji, which ignores any variant selectors.
        let reaction_first_char = reaction_raw.chars().next().map(|c| c.to_string());
        let reaction_str = reaction_first_char.as_deref().unwrap_or(reaction_raw);
        let text_to_display = emojis::get(reaction_str)
            .and_then(|e| e.shortcode())
            .unwrap_or(reaction_raw);
        let count = reaction_senders.len();
        // log!("Found reaction {:?} with count {}", text_to_display, count);
        label_text = format!("{label_text}<i>:{}:</i> <b>{}</b> ", text_to_display, count);
    }

    // Debugging: draw the item ID as a reaction
    if DRAW_ITEM_ID_REACTION {
        label_text = format!("{label_text}<i>ID: {}</i>", id);
    }

    let html_reaction_view = message_item.html(id!(message_annotations.html_content));
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
        username: &str,
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
        original_sender: &str,
        _item_drawn_status: ItemDrawnStatus,
        mut new_drawn_status: ItemDrawnStatus,
    ) -> (WidgetRef, ItemDrawnStatus) {
        item.label(id!(content)).set_text(
            &text_preview_of_redacted_message(event_tl_item, original_sender)
                .format_with(original_sender),
        );
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
        username: &str,
        _item_drawn_status: ItemDrawnStatus,
        mut new_drawn_status: ItemDrawnStatus,
    ) -> (WidgetRef, ItemDrawnStatus) {
        let item = if let Some(text_preview) = text_preview_of_other_state(self) {
            item.label(id!(content))
                .set_text(&text_preview.format_with(username));
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

impl SmallStateEventContent for MemberProfileChange {
    fn populate_item_content(
        &self,
        _cx: &mut Cx,
        _list: &mut PortalList,
        _item_id: usize,
        item: WidgetRef,
        _event_tl_item: &EventTimelineItem,
        username: &str,
        _item_drawn_status: ItemDrawnStatus,
        mut new_drawn_status: ItemDrawnStatus,
    ) -> (WidgetRef, ItemDrawnStatus) {
        item.label(id!(content))
            .set_text(&text_preview_of_member_profile_change(self, username).format_with(username));
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
        username: &str,
        _item_drawn_status: ItemDrawnStatus,
        mut new_drawn_status: ItemDrawnStatus,
    ) -> (WidgetRef, ItemDrawnStatus) {
        let Some(preview) = text_preview_of_room_membership_change(self) else {
            // Don't actually display anything for nonexistent/unimportant membership changes.
            return (
                list.item(cx, item_id, live_id!(Empty)),
                ItemDrawnStatus::new(),
            );
        };

        item.label(id!(content))
            .set_text(&preview.format_with(username));
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
    room_id: &OwnedRoomId,
    event_tl_item: &EventTimelineItem,
    event_content: &impl SmallStateEventContent,
    item_drawn_status: ItemDrawnStatus,
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
        .then(|| get_profile_display_name(event_tl_item))
        .flatten();

    let username = username_opt.unwrap_or_else(|| {
        // As a fallback, call `set_avatar_and_get_username` to get the user's display name.
        let (username, profile_drawn) = set_avatar_and_get_username(
            cx,
            item.avatar(id!(avatar)),
            room_id,
            event_tl_item.sender(),
            event_tl_item.sender_profile(),
            event_tl_item.event_id(),
        );
        // Draw the timestamp as part of the profile.
        set_timestamp(
            &item,
            id!(left_container.timestamp),
            event_tl_item.timestamp(),
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
    )
}

/// Sets the text of the `Label` at the given `item`'s live ID path
/// to a typical 12-hour AM/PM timestamp format.
fn set_timestamp(item: &WidgetRef, live_id_path: &[LiveId], timestamp: MilliSecondsSinceUnixEpoch) {
    if let Some(dt) = unix_time_millis_to_datetime(&timestamp) {
        // format as AM/PM 12-hour time
        item.label(live_id_path)
            .set_text(&format!("{}", dt.time().format("%l:%M %P")));
    } else {
        item.label(live_id_path)
            .set_text(&format!("{}", timestamp.get()));
    }
}

/// Sets the given avatar and returns a displayable username based on the
/// given profile and user ID of the sender of the event with the given event ID.
///
/// If the sender profile is not ready, this function will submit an async request
/// to fetch the sender profile from the server, but only if the event ID is `Some`.
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
    room_id: &OwnedRoomId,
    sender_user_id: &UserId,
    sender_profile: &TimelineDetails<Profile>,
    event_id: Option<&EventId>,
) -> (String, bool) {
    // Get the display name and avatar URL from the sender's profile, if available,
    // or if the profile isn't ready, fall back to qeurying our user profile cache.
    let (username_opt, avatar_state) = match sender_profile {
        TimelineDetails::Ready(profile) => (
            profile.display_name.clone(),
            AvatarState::Known(profile.avatar_url.clone()),
        ),
        not_ready => {
            if matches!(not_ready, TimelineDetails::Unavailable) {
                if let Some(event_id) = event_id {
                    submit_async_request(MatrixRequest::FetchDetailsForEvent {
                        room_id: room_id.to_owned(),
                        event_id: event_id.to_owned(),
                    });
                }
            }
            // log!("populate_message_view(): sender profile not ready yet for event {not_ready:?}");
            user_profile_cache::with_user_profile(cx, sender_user_id, |profile, room_members| {
                room_members
                    .get(room_id)
                    .map(|rm| {
                        (
                            rm.display_name().map(|n| n.to_owned()),
                            AvatarState::Known(rm.avatar_url().map(|u| u.to_owned())),
                        )
                    })
                    .unwrap_or_else(|| (profile.username.clone(), profile.avatar_state.clone()))
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
        },
        AvatarState::Known(None) | AvatarState::Failed => (None, true),
        AvatarState::Unknown => (None, false),
    };

    // Set sender to the display name if available, otherwise the user id.
    let username = username_opt
        .clone()
        .unwrap_or_else(|| sender_user_id.to_string());

    // Set the sender's avatar image, or use the username if no image is available.
    avatar_img_data_opt.and_then(|data|
        avatar.show_image(
            Some((sender_user_id.to_owned(), username_opt.clone(), room_id.to_owned(), data.clone()).into()),
            |img| utils::load_png_or_jpg(&img, cx, &data)
        )
        .ok()
    )
    .unwrap_or_else(|| avatar.show_text(
        Some((sender_user_id.to_owned(), username_opt, room_id.to_owned()).into()),
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

/// A simple deref wrapper around the `LocationPreview` widget that enables us to handle actions on it.
#[derive(Live, LiveHook, Widget)]
struct LocationPreview {
    #[deref] view: View,
    #[rust] coords: Option<Result<Coordinates, robius_location::Error>>,
    #[rust] timestamp: Option<SystemTime>,
}
impl Widget for LocationPreview {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        let mut needs_redraw = false;
        if let Event::Actions(actions) = event {
            for action in actions {
                match action.downcast_ref() {
                    Some(LocationAction::Update(LocationUpdate { coordinates, time })) => {
                        self.coords = Some(Ok(*coordinates));
                        self.timestamp = *time;
                        self.button(id!(send_location_button)).set_enabled(true);
                        needs_redraw = true;
                    }
                    Some(LocationAction::Error(e)) => {
                        self.coords = Some(Err(*e));
                        self.timestamp = None;
                        self.button(id!(send_location_button)).set_enabled(false);
                        needs_redraw = true;
                    }
                    _ => { }
                }
            }

            // NOTE: the send location button click event is handled
            //       in the RoomScreen handle_event function.

            // Handle the cancel location button being clicked.
            if self.button(id!(cancel_location_button)).clicked(actions) {
                self.clear();
                needs_redraw = true;
            }
        }

        if needs_redraw {
            self.redraw(cx);
        }

        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        let text = match self.coords {
            Some(Ok(c)) => {
                // if let Some(st) = self.timestamp {
                //     format!("Current location: {:.6},{:.6}\n   Timestamp: {:?}", c.latitude, c.longitude, st)
                // } else {
                    format!("Current location: {:.6},{:.6}", c.latitude, c.longitude)
                // }
            }
            Some(Err(e)) => format!("Error getting location: {e:?}"),
            None => String::from("Current location is not yet available."),
        };
        self.label(id!(location_label)).set_text(&text);
        self.view.draw_walk(cx, scope, walk)
    }
}


impl LocationPreview {
    fn show(&mut self) {
        request_location_update(LocationRequest::UpdateOnce);
        if let Some(loc) = get_latest_location() {
            self.coords = Some(Ok(loc.coordinates));
            self.timestamp = loc.time;
        }
        self.visible = true;
    }

    fn clear(&mut self) {
        self.coords = None;
        self.timestamp = None;
        self.visible = false;
    }

    pub fn get_current_data(&self) -> Option<(Coordinates, Option<SystemTime>)> {
        self.coords
            .as_ref()
            .and_then(|res| res.ok())
            .map(|c| (c, self.timestamp))
    }
}

impl LocationPreviewRef {
    pub fn show(&self) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.show();
        }
    }

    pub fn clear(&self) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.clear();
        }
    }

    pub fn get_current_data(&self) -> Option<(Coordinates, Option<SystemTime>)> {
        self.borrow().and_then(|inner| inner.get_current_data())
    }
}


/// Actions related to a specific message within a room timeline.
#[derive(Clone, DefaultNone, Debug)]
pub enum MessageAction {
    /// The user clicked a message's reply button, indicating that they want to
    /// reply to this message (the message at the given timeline item index).
    MessageReplyButtonClicked(usize),
    /// The user clicked the inline reply preview above a message
    /// indicating that they want to jump upwards to the replied-to message shown in the preview.
    ReplyPreviewClicked {
        /// The item ID (in the timeline PortalList) of the reply message
        /// that the user clicked the reply preview above.
        reply_message_item_id: usize,
        /// The event ID of the replied-to message (the target of the reply).
        replied_to_event: OwnedEventId,
    },
    /// The message at the given item index in the timeline should be highlighted.
    MessageHighlight(usize),
    None,
}

#[derive(Live, LiveHook, Widget)]
pub struct Message {
    #[deref] view: View,
    #[animator] animator: Animator,
    #[rust(false)] hovered: bool,
    #[rust(false)] mentions_user: bool,

    #[rust] can_be_replied_to: bool,
    #[rust] item_id: usize,
    /// The event ID of the message that this message is replying to, if any.
    #[rust] replied_to_event_id: Option<OwnedEventId>,
    #[rust] room_screen_widget_uid: Option<WidgetUid>
}

impl Widget for Message {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if self.animator_handle_event(cx, event).must_redraw() {
            self.redraw(cx);
        }

        if !self.animator.is_track_animating(cx, id!(highlight))
            && self.animator_in_state(cx, id!(highlight.on))
        {
            self.animator_play(cx, id!(highlight.off));
        }

        let Some(widget_uid) = self.room_screen_widget_uid else { return };

        if let Event::Actions(actions) = event {
            if self.view.button(id!(reply_button)).clicked(actions) {
                cx.widget_action(
                    widget_uid,
                    &scope.path,
                    MessageAction::MessageReplyButtonClicked(self.item_id),
                );
            }
        }

        if let Hit::FingerUp(fe) = event.hits(cx, self.view(id!(replied_to_message)).area()) {
            if fe.was_tap() {
                if let Some(ref replied_to_event) = self.replied_to_event_id {
                    cx.widget_action(
                        widget_uid,
                        &scope.path,
                        MessageAction::ReplyPreviewClicked {
                            reply_message_item_id: self.item_id,
                            replied_to_event: replied_to_event.to_owned(),
                        },
                    );
                } else {
                    error!("BUG: reply preview clicked for message {} with no replied-to event!", self.item_id);
                }
            }
        }

        if let Event::Actions(actions) = event {
            for action in actions {
                match action.as_widget_action().cast() {
                    MessageAction::MessageHighlight(id) if id == self.item_id => {
                        self.animator_play(cx, id!(highlight.on));
                        self.redraw(cx);
                    }
                    _ => {}
                }
            }
        }

        if let Event::MouseMove(e) = event {
            let hovered = self.view.area().rect(cx).contains(e.abs);
            if (self.hovered != hovered) || (!hovered && self.animator_in_state(cx, id!(hover.on)))
            {
                self.hovered = hovered;

                // TODO: Once we have a context menu, the messageMenu can be displayed on hover or push only
                // self.view.view(id!(message_menu)).set_visible(hovered);
                let hover_animator = if self.hovered {
                    id!(hover.on)
                } else {
                    id!(hover.off)
                };
                self.animator_play(cx, hover_animator);
            }
        }

        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        if self.mentions_user {
            self.view.apply_over(
                cx, live!(
                    draw_bg: {
                        color: (vec4(1.0, 1.0, 0.82, 1.0))
                        mentions_bar_color: #ffd54f
                    }
                )
            )
        }

        self.view
            .button(id!(reply_button))
            .set_visible(self.can_be_replied_to);

        self.view.draw_walk(cx, scope, walk)
    }
}

impl Message {
    fn set_data(
        &mut self,
        can_be_replied_to: bool,
        item_id: usize,
        replied_to_event_id: Option<OwnedEventId>,
        room_screen_widget_uid: WidgetUid,
        mentions_user: bool
    ) {
        self.can_be_replied_to = can_be_replied_to;
        self.item_id = item_id;
        self.replied_to_event_id = replied_to_event_id;
        self.room_screen_widget_uid = Some(room_screen_widget_uid);
        self.mentions_user = mentions_user;
    }
}

impl MessageRef {
    fn set_data(
        &self,
        can_be_replied_to: bool,
        item_id: usize,
        replied_to_event_id: Option<OwnedEventId>,
        room_screen_widget_uid: WidgetUid,
        mentions_user: bool
    ) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_data(can_be_replied_to, item_id, replied_to_event_id, room_screen_widget_uid, mentions_user);
        };
    }
}
