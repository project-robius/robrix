//! A room screen is the UI view that displays a single Room's timeline of events/messages
//! along with a message input bar at the bottom.

use std::{borrow::Cow, collections::BTreeMap, ops::{DerefMut, Range}, sync::{Arc, Mutex}};

use bytesize::ByteSize;
use imbl::Vector;
use makepad_widgets::{image_cache::ImageBuffer, *};
use matrix_sdk::{room::RoomMember, ruma::{
    events::{receipt::Receipt, room::{
        message::{
            AudioMessageEventContent, CustomEventContent, EmoteMessageEventContent, FileMessageEventContent, FormattedBody, ImageMessageEventContent, KeyVerificationRequestEventContent, LocationMessageEventContent, MessageFormat, MessageType, NoticeMessageEventContent, RoomMessageEventContent, ServerNoticeMessageEventContent, TextMessageEventContent, VideoMessageEventContent
        }, ImageInfo, MediaSource
    },
    sticker::StickerEventContent, Mentions}, matrix_uri::MatrixId, uint, EventId, MatrixToUri, MatrixUri, OwnedEventId, OwnedMxcUri, OwnedRoomId
}, OwnedServerName};
use matrix_sdk_ui::timeline::{
    self, EventTimelineItem, InReplyToDetails, MemberProfileChange, RepliedToInfo, RoomMembershipChange, TimelineDetails, TimelineEventItemId, TimelineItem, TimelineItemContent, TimelineItemKind, VirtualTimelineItem
};

use crate::{
    avatar_cache, event_preview::{plaintext_body_of_timeline_item, text_preview_of_member_profile_change, text_preview_of_other_state, text_preview_of_redacted_message, text_preview_of_room_membership_change, text_preview_of_timeline_item}, home::loading_pane::{LoadingPaneState, LoadingPaneWidgetExt}, location::init_location_subscriber, media_cache::{MediaCache, MediaCacheEntry}, profile::{
        user_profile::{AvatarState, ShowUserProfileAction, UserProfile, UserProfileAndRoomId, UserProfilePaneInfo, UserProfileSlidingPaneRef, UserProfileSlidingPaneWidgetExt},
        user_profile_cache,
    }, shared::{
        avatar::AvatarWidgetRefExt, callout_tooltip::TooltipAction, html_or_plaintext::{HtmlOrPlaintextRef, HtmlOrPlaintextWidgetRefExt, RobrixHtmlLinkAction}, jump_to_bottom_button::{JumpToBottomButtonWidgetExt, UnreadMessageCount}, popup_list::{enqueue_popup_notification, PopupItem}, styles::COLOR_DANGER_RED, text_or_image::{TextOrImageRef, TextOrImageWidgetRefExt}, timestamp::TimestampWidgetRefExt, typing_animation::TypingAnimationWidgetExt
    }, sliding_sync::{get_client, submit_async_request, take_timeline_endpoints, BackwardsPaginateUntilEventRequest, MatrixRequest, PaginationDirection, TimelineRequestSender, UserPowerLevels}, utils::{self, room_name_or_id, unix_time_millis_to_datetime, ImageFormat, MEDIA_THUMBNAIL_FORMAT}
};
use crate::home::event_reaction_list::ReactionListWidgetRefExt;
use crate::home::room_read_receipt::AvatarRowWidgetRefExt;
use crate::room::room_input_bar::RoomInputBarWidgetExt;
use crate::shared::mentionable_text_input::MentionableTextInputWidgetRefExt;

use rangemap::RangeSet;

use super::{editing_pane::{EditingPaneAction, EditingPaneWidgetExt}, event_reaction_list::ReactionData, loading_pane::LoadingPaneRef, location_preview::LocationPreviewWidgetExt, new_message_context_menu::{MessageAbilities, MessageDetails}, room_read_receipt::{self, populate_read_receipts, MAX_VISIBLE_AVATARS_IN_READ_RECEIPT}};

const GEO_URI_SCHEME: &str = "geo:";

const MESSAGE_NOTICE_TEXT_COLOR: Vec3 = Vec3 { x: 0.5, y: 0.5, z: 0.5 };

/// The maximum number of timeline items to search through 
/// when looking for a particular event.
///
/// This is a safety measure to prevent the main UI thread
/// from getting into a long-running loop if an event cannot be found quickly.
const MAX_ITEMS_TO_SEARCH_THROUGH: usize = 100;

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::styles::*;
    use crate::shared::helpers::*;
    use crate::shared::avatar::Avatar;
    use crate::shared::text_or_image::TextOrImage;
    use crate::shared::timestamp::*;
    use crate::shared::html_or_plaintext::*;
    use crate::shared::icon_button::*;
    use crate::shared::typing_animation::TypingAnimation;
    use crate::shared::icon_button::*;
    use crate::shared::jump_to_bottom_button::*;
    use crate::profile::user_profile::UserProfileSlidingPane;
    use crate::home::editing_pane::*;
    use crate::home::event_reaction_list::*;
    use crate::home::loading_pane::*;
    use crate::home::location_preview::*;
    use crate::room::room_input_bar::*;
    use crate::room::room_input_bar::*;
    use crate::home::room_read_receipt::*;

    IMG_DEFAULT_AVATAR = dep("crate://self/resources/img/default_avatar.png")

    ICO_LOCATION_PERSON = dep("crate://self/resources/icons/location-person.svg")

    COLOR_BG = #xfff8ee
    COLOR_OVERLAY_BG = #x000000d8
    COLOR_READ_MARKER = #xeb2733
    COLOR_PROFILE_CIRCLE = #xfff8ee
    TYPING_NOTICE_ANIMATION_DURATION = 0.3

    CAN_NOT_SEND_NOTICE = "You don't have permission to post to this room."

    FillerY = <View> {width: Fill}

    FillerX = <View> {height: Fill}

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
                flow: Right, // do not wrap
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
            margin: {left: 1.5}
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
                instance border_radius: 0.0

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
                        max(1.0, self.border_radius)
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


    // An empty view that takes up no space in the portal list.
    Empty = <View> { }

    // The view used for each text-based message event in a room's timeline.
    Message = {{Message}} {
        width: Fill,
        height: Fit,
        margin: 0.0
        flow: Down,
        cursor: Default,
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
                    margin: { top: 3.9 }
                }
            }
            content = <View> {
                width: Fill,
                height: Fit
                flow: Down,
                padding: 0.0
                <View> {
                    flow: Right,
                    width: Fill,
                    height: Fit,
                    username = <Label> {
                        width: Fill,
                        flow: Right, // do not wrap
                        padding: 0,
                        margin: {bottom: 9.0, top: 20.0, right: 10.0,}
                        draw_text: {
                            text_style: <USERNAME_TEXT_STYLE> {},
                            color: (USERNAME_TEXT_COLOR)
                            wrap: Ellipsis,
                        }
                        text: "<Username not available>"
                    }
                }

                message = <HtmlOrPlaintext> { }

                // <LineH> {
                //     margin: {top: 13.0, bottom: 5.0}
                // }
                <View> {
                    width: Fill,
                    height: Fit
                    reaction_list = <ReactionList> { }
                    avatar_row = <AvatarRow> {}
                }

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
                    margin: {top: 2.5}
                }
            }
            content = <View> {
                width: Fill,
                height: Fit,
                flow: Down,
                padding: { left: 10.0 }

                message = <HtmlOrPlaintext> { }
                <View> {
                    width: Fill,
                    height: Fit
                    reaction_list = <ReactionList> { }
                    avatar_row = <AvatarRow> {}
                }
            }
        }
    }

    // The view used for each static image-based message event in a room's timeline.
    // This excludes stickers and other animated GIFs, video clips, audio clips, etc.
    ImageMessage = <Message> {
        body = {
            content = {
                width: Fill,
                height: Fit
                padding: { left: 10.0 }
                message = <TextOrImage> { }
                v = <View> {
                    width: Fill,
                    height: Fit,
                    flow: Right,
                    reaction_list = <ReactionList> { }
                    avatar_row = <AvatarRow> {}
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
                message = <TextOrImage> { }
                <View> {
                    width: Fill,
                    height: Fit
                    reaction_list = <ReactionList> { }
                    avatar_row = <AvatarRow> {}
                }
            }

        }
    }


    // The view used for each state event (non-messages) in a room's timeline.
    // The timestamp, profile picture, and text are all very small.
    SmallStateEvent = <View> {
        width: Fill,
        height: Fit,
        flow: Right,
        margin: { top: 4.0, bottom: 4.0}
        padding: { top: 1.0, bottom: 1.0, right: 10.0 }
        spacing: 0.0
        cursor: Default

        body = <View> {
            width: Fill,
            height: Fit
            flow: Right,
            padding: { left: 7.0, top: 2.0, bottom: 2.0 }
            spacing: 5.0

            left_container = <View> {
                align: {x: 0.5, y: 0}
                width: 70.0,
                height: Fit

                timestamp = <Timestamp> {
                    margin: {top: 0.5}
                }
            }

            avatar = <Avatar> {
                width: 19.,
                height: 19.,
                margin: { top: -2} // center the avatar vertically with the text

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
            // Center the Avatar vertically with respect to the SmallStateEvent content.
            avatar_row = <AvatarRow> { margin: {top: -1.0} }
        }
    }


    // The view used for each day divider in a room's timeline.
    // The date text is centered between two horizontal lines.
    DateDivider = <View> {
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
    // This is implemented as a DateDivider with a different color and a fixed text label.
    ReadMarker = <DateDivider> {
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
        flow: Right,
        show_bg: true,
        draw_bg: {
            color: #xDAF5E5F0, // mostly opaque light green
        }

        label = <Label> {
            width: Fill,
            height: Fit,
            align: {x: 0.5, y: 0.5},
            flow: Right,
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
            max_pull_down: 0.0, // set to `0.0` to disable the pulldown bounce animation.

            // Below, we must place all of the possible templates (views) that can be used in the portal list.
            Message = <Message> {}
            CondensedMessage = <CondensedMessage> {}
            ImageMessage = <ImageMessage> {}
            CondensedImageMessage = <CondensedImageMessage> {}
            SmallStateEvent = <SmallStateEvent> {}
            Empty = <Empty> {}
            DateDivider = <DateDivider> {}
            ReadMarker = <ReadMarker> {}
        }

        // A jump to bottom button (with an unread message badge) that is shown
        // when the timeline is not at the bottom.
        jump_to_bottom = <JumpToBottomButton> { }
    }


    pub RoomScreen = {{RoomScreen}} {
        width: Fill, height: Fill,
        cursor: Default,
        flow: Down,
        spacing: 0.0

        show_bg: true,
        draw_bg: {
            color: (COLOR_SECONDARY)
        }

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
                        padding: {left: 5.0, right: 0.0, top: 0.0, bottom: 0.0}
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

                // Below that, display an optional preview of the message that the user
                // is currently drafting a replied to.
                replying_preview = <View> {
                    visible: false
                    width: Fill
                    height: Fit
                    flow: Down
                    padding: {left: 20, right: 20}

                    // Displays a "Replying to" label and a cancel button
                    // above the preview of the message being replied to.
                    <View> {
                        width: Fill
                        height: Fit
                        flow: Right
                        align: {y: 0.5}
                        padding: {left: 10, right: 5, top: 10, bottom: 10}

                        <Label> {
                            width: Fill,
                            flow: Right, // do not wrap
                            draw_text: {
                                text_style: <USERNAME_TEXT_STYLE> {},
                                color: #222,
                                wrap: Ellipsis,
                            }
                            text: "Replying to:"
                        }

                        cancel_reply_button = <RobrixIconButton> {
                            width: Fit,
                            height: Fit,
                            padding: 13,
                            spacing: 0,
                            margin: {left: 5, right: 5},

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

                    <LineH> {
                        draw_bg: {color: (COLOR_DIVIDER_DARK)}
                        margin: {bottom: 5.0}
                    }

                    reply_preview_content = <ReplyPreviewContent> { }
                }

                // Below that, display a preview of the current location that a user is about to send.
                location_preview = <LocationPreview> { }

                // Below that, display one of multiple possible views:
                // * the message input bar
                // * the slide-up editing pane
                // * a notice that the user can't send messages to this room
                <View> {
                    width: Fill, height: Fit,
                    flow: Overlay,

                    // Below that, display a view that holds the message input bar and send button.
                    input_bar = <RoomInputBar> {}

                    can_not_send_message_notice = <View> {
                        visible: false
                        show_bg: true
                        draw_bg: {
                            color: (COLOR_SECONDARY)
                        }
                        padding: {left: 50, right: 50, top: 20, bottom: 20}
                        align: {y: 0.5}
                        width: Fill, height: Fit

                        text = <Label> {
                            width: Fill,
                            draw_text: {
                                color: (COLOR_TEXT)
                                text_style: <THEME_FONT_ITALIC>{font_size: 12.2}
                                wrap: Word,
                            }
                            text: (CAN_NOT_SEND_NOTICE)
                        }
                    }

                    editing_pane = <EditingPane> { }
                }
            }

            // Note: here, we're within a View that has an Overlay flow,
            // so the order that we define the below views determines which one is on top.

            // The top space should be displayed as an overlay at the top of the timeline.
            top_space = <TopSpace> { }

            // The user profile sliding pane should be displayed on top of other "static" subviews
            // (on top of all other views that are always visible).
            user_profile_sliding_pane = <UserProfileSlidingPane> { }

            // The loading pane appears while the user is waiting for something in the room screen
            // to finish loading, e.g., when loading an older replied-to message.
            loading_pane = <LoadingPane> { }


            /*
             * TODO: add the action bar back in as a series of floating buttons.
             *
            message_action_bar_popup = <PopupNotification> {
                align: {x: 0.0, y: 0.0}
                content: {
                    height: Fit,
                    width: Fit,
                    show_bg: false,
                    align: {
                        x: 0.5,
                        y: 0.5
                    }

                    message_action_bar = <MessageActionBar> {}
                }
            }
            */
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
        let room_screen_widget_uid = self.widget_uid();
        let portal_list = self.portal_list(id!(timeline.list));
        let user_profile_sliding_pane = self.user_profile_sliding_pane(id!(user_profile_sliding_pane));
        let loading_pane = self.loading_pane(id!(loading_pane));

        // Currently, a Signal event is only used to tell this widget
        // that its timeline events have been updated in the background.
        if let Event::Signal = event {
            self.process_timeline_updates(cx, &portal_list);

            // Ideally we would do this elsewhere on the main thread, because it's not room-specific,
            // but it doesn't hurt to do it here.
            // TODO: move this up a layer to something higher in the UI tree,
            //       and wrap it in a `if let Event::Signal` conditional.
            user_profile_cache::process_user_profile_updates(cx);
            avatar_cache::process_avatar_updates(cx);
        }

        if let Event::Actions(actions) = event {
            for (_, wr) in portal_list.items_with_actions(actions) {
                let reaction_list = wr.reaction_list(id!(reaction_list));
                if let RoomScreenTooltipActions::HoverInReactionButton {
                    widget_rect,
                    bg_color,
                    reaction_data,
                } = reaction_list.hover_in(actions) {
                    let tooltip_text_arr: Vec<String> = reaction_data.reaction_senders.iter().map(|(sender, _react_info)| {
                        user_profile_cache::get_user_profile_and_room_member(cx, sender.clone(), &reaction_data.room_id, true).0
                            .map(|user_profile| user_profile.displayable_name().to_string())
                            .unwrap_or_else(|| sender.to_string())
                    }).collect();
                    let mut tooltip_text = utils::human_readable_list(&tooltip_text_arr, MAX_VISIBLE_AVATARS_IN_READ_RECEIPT);
                    tooltip_text.push_str(&format!(" reacted with: {}", reaction_data.reaction));
                    cx.widget_action(
                        self.widget_uid(),
                        &scope.path,
                        TooltipAction::HoverIn {
                            widget_rect,
                            text: tooltip_text,
                            text_color: None,
                            bg_color,
                        }
                    );
                }
                if reaction_list.hover_out(actions) {
                    cx.widget_action(
                        self.widget_uid(),
                        &scope.path,
                        TooltipAction::HoverOut
                    );
                }
                let avatar_row_ref = wr.avatar_row(id!(avatar_row));
                if let RoomScreenTooltipActions::HoverInReadReceipt {
                    widget_rect,
                    bg_color,
                    read_receipts
                } = avatar_row_ref.hover_in(actions) {
                    let Some(room_id) = &self.room_id else { return; };
                    let tooltip_text= room_read_receipt::populate_tooltip(cx, read_receipts, room_id);
                    cx.widget_action(
                        self.widget_uid(),
                        &scope.path,
                        TooltipAction::HoverIn {
                            widget_rect,
                            text: tooltip_text,
                            bg_color,
                            text_color: None,
                        }
                    );
                }
                if avatar_row_ref.hover_out(actions) {
                    cx.widget_action(
                        self.widget_uid(),
                        &scope.path,
                        TooltipAction::HoverOut
                    );
                }
            }

            self.handle_message_actions(cx, actions, &portal_list, &loading_pane);

            let message_input = self.room_input_bar(id!(input_bar)).text_input(id!(text_input));

            for action in actions {
                // Handle the highlight animation.
                let Some(tl) = self.tl_state.as_mut() else { return };
                if let MessageHighlightAnimationState::Pending { item_id } = tl.message_highlight_animation_state {
                    if portal_list.smooth_scroll_reached(actions) {
                        cx.widget_action(
                            room_screen_widget_uid,
                            &scope.path,
                            MessageAction::HighlightMessage(item_id),
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
                            &user_profile_sliding_pane,
                            UserProfilePaneInfo {
                                profile_and_room_id,
                                room_name: self.room_name.clone(),
                                room_member: None,
                            },
                        );
                    }
                }
            }

            /*
            // close message action bar if scrolled.
            if portal_list.scrolled(actions) {
                let message_action_bar_popup = self.popup_notification(id!(message_action_bar_popup));
                message_action_bar_popup.close(cx);
            }
            */

            // Set visibility of loading message banner based of pagination logic
            self.send_pagination_request_based_on_scroll_pos(cx, actions, &portal_list);
            // Handle sending any read receipts for the current logged-in user.
            self.send_user_read_receipts_based_on_scroll_pos(cx, actions, &portal_list);

            // Clear the replying-to preview pane if the "cancel reply" button was clicked
            // or if the `Escape` key was pressed within the message input box.
            if self.button(id!(cancel_reply_button)).clicked(actions)
                || message_input.escaped(actions)
            {
                self.clear_replying_to(cx);
                self.redraw(cx);
            }

            // Handle the add location button being clicked.
            if self.button(id!(location_button)).clicked(actions) {
                log!("Add location button clicked; requesting current location...");
                if let Err(_e) = init_location_subscriber(cx) {
                    error!("Failed to initialize location subscriber");
                    enqueue_popup_notification(PopupItem {
                        message: String::from("Failed to initialize location services."), 
                        auto_dismissal_duration: None
                    });
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
                    });

                    self.clear_replying_to(cx);
                    location_preview.clear();
                    location_preview.redraw(cx);
                }
            }


            // Handle the send message button being clicked or Cmd/Ctrl + Return being pressed.
            if self.button(id!(send_message_button)).clicked(actions)
                || message_input.returned(actions).is_some_and(
                    |(_text, modifiers)| modifiers.is_primary()
                )
            {
                let entered_text = message_input.text().trim().to_string();
                if !entered_text.is_empty() {
                    let room_input_bar = self.room_input_bar(id!(input_bar));
                    let room_id = self.room_id.clone().unwrap();
                    let (message, mentions) = if let Some(html_text) = entered_text.strip_prefix("/html") {
                        (
                            RoomMessageEventContent::text_html(html_text, html_text),
                            room_input_bar.mentionable_text_input(id!(message_input))
                                .get_real_mentions_in_html_text(html_text),
                        )
                    } else if let Some(plain_text) = entered_text.strip_prefix("/plain") {
                        (
                            RoomMessageEventContent::text_plain(plain_text),
                            Default::default(),
                        )
                    } else {
                        (
                            RoomMessageEventContent::text_markdown(&entered_text),
                            room_input_bar.mentionable_text_input(id!(message_input))
                                .get_real_mentions_in_markdown_text(&entered_text),
                        )
                    };
                    log!("Sending message to room {}: {:?}, mentions: {:?}", room_id, entered_text, mentions);
                    let message = message.add_mentions(Mentions::with_user_ids(mentions));
                    submit_async_request(MatrixRequest::SendMessage {
                        room_id,
                        message,
                        replied_to: self.tl_state.as_mut().and_then(
                            |tl| tl.replying_to.take().map(|(_, rep)| rep)
                        ),
                    });

                    self.clear_replying_to(cx);
                    message_input.set_text(cx, "");
                    room_input_bar.enable_send_message_button(cx, false);

                }
            }

            // Handle the user pressing the up arrow in an empty message input box
            // to edit their latest sent message.
            if message_input.text().is_empty() {
                if let Some(KeyEvent {
                    key_code: KeyCode::ArrowUp,
                    modifiers: KeyModifiers { shift: false, control: false, alt: false, logo: false },
                    ..
                }) = message_input.key_down_unhandled(actions) {
                    let Some(tl) = self.tl_state.as_mut() else { return };
                    if let Some(latest_sent_msg) = tl.items
                        .iter()
                        .rev()
                        .take(MAX_ITEMS_TO_SEARCH_THROUGH)
                        .find_map(|item| item.as_event().filter(|ev| ev.is_editable()).cloned())
                    {
                        let room_id = tl.room_id.clone();
                        self.show_editing_pane(cx, latest_sent_msg, room_id);
                    } else {
                        enqueue_popup_notification("No recent message available to edit.".to_string());
                    }
                }
            }

            // Handle the jump to bottom button: update its visibility, and handle clicks.
            self.jump_to_bottom_button(id!(jump_to_bottom)).update_from_actions(
                cx,
                &portal_list,
                actions,
            );

            // Handle a typing action on the message input box.
            if let Some(new_text) = message_input.changed(actions) {
                submit_async_request(MatrixRequest::SendTypingNotice {
                    room_id: self.room_id.clone().unwrap(),
                    typing: !new_text.is_empty(),
                });
            }
        }

        if self.animator_handle_event(cx, event).must_redraw() {
            self.redraw(cx);
        }

        // We only forward "interactive hit" events to the inner timeline view
        // if none of the various overlay views are visible.
        // We always forward "non-interactive hit" events to the inner timeline view.
        // We check which overlay views are visible in the order of those views' z-ordering,
        // such that the top-most views get a chance to handle the event first.
        //
        let is_interactive_hit = utils::is_interactive_hit_event(event);
        let is_pane_shown: bool;
        if loading_pane.is_currently_shown(cx) {
            is_pane_shown = true;
            loading_pane.handle_event(cx, event, scope);
        }
        else if user_profile_sliding_pane.is_currently_shown(cx) {
            is_pane_shown = true;
            user_profile_sliding_pane.handle_event(cx, event, scope);
        }
        else {
            is_pane_shown = false;
        }

        // TODO: once we use the `hits()` API, we can remove the above conditionals, because
        //       Makepad already delivers most events to all views regardless of visibility,
        //       so the only thing we'd need here is the conditional below.

        if !is_pane_shown || !is_interactive_hit {
            // Forward the event to the inner timeline view, but capture any actions it produces
            // such that we can handle the ones relevant to only THIS RoomScreen widget right here and now,
            // ensuring they are not mistakenly handled by other RoomScreen widget instances.
            let mut actions_generated_within_this_room_screen = cx.capture_actions(|cx|
                self.view.handle_event(cx, event, scope)
            );
            // Here, we handle and remove any general actions that are relevant to only this RoomScreen.
            // Removing the handled actions ensures they are not mistakenly handled by other RoomScreen widget instances.
            actions_generated_within_this_room_screen.retain(|action| {
                if self.handle_link_clicked(cx, action, &user_profile_sliding_pane) {
                    return false;
                }

                // When the EditingPane has been hidden, re-show the input bar.
                if let EditingPaneAction::Hide = action.as_widget_action().cast() {
                    self.on_hide_editing_pane(cx);
                    return false;
                }

                /*
                match action.as_widget_action().widget_uid_eq(room_screen_widget_uid).cast() {
                    MessageAction::ActionBarClose => {
                        let message_action_bar_popup = self.popup_notification(id!(message_action_bar_popup));
                        let message_action_bar = message_action_bar_popup.message_action_bar(id!(message_action_bar));

                        // close only if the active message is requesting it to avoid double closes.
                        if let Some(message_widget_uid) = message_action_bar.message_widget_uid() {
                            if action.as_widget_action().widget_uid_eq(message_widget_uid).is_some() {
                                message_action_bar_popup.close(cx);
                            }
                        }
                    }
                    MessageAction::ActionBarOpen { item_id, message_rect } => {
                        let message_action_bar_popup = self.popup_notification(id!(message_action_bar_popup));
                        let message_action_bar = message_action_bar_popup.message_action_bar(id!(message_action_bar));

                        let margin_x = 50.;

                        let coords = dvec2(
                            (message_rect.pos.x + message_rect.size.x) - margin_x,
                            message_rect.pos.y,
                        );

                        message_action_bar_popup.apply_over(
                            cx,
                            live! {
                                content: { margin: { left: (coords.x), top: (coords.y) } }
                            },
                        );

                        if let Some(message_widget_uid) = action.as_widget_action().map(|a| a.widget_uid) {
                            message_action_bar_popup.open(cx);
                            message_action_bar.initialize_with_data(cx, widget_uid, message_widget_uid, item_id);
                        }
                    }
                    _ => {}
                }
                */

                // Keep all unhandled actions so we can add them back to the global action list below.
                true
            });
            // Add back any unhandled actions to the global action list.
            cx.extend_actions(actions_generated_within_this_room_screen);
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        let room_screen_widget_uid = self.widget_uid();
        if self.tl_state.is_none() {
            // Tl_state may not be ready after dock loading.
            // If return DrawStep::done() inside self.view.draw_walk, turtle will misalign and panic.
            return DrawStep::done();
        }
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
                                    &tl_state.user_power,
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
                                    &tl_state.user_power,
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
                                item.label(id!(content)).set_text(cx, &format!("[Unsupported] {:?}", unhandled));
                                (item, ItemDrawnStatus::both_drawn())
                            }
                        }
                        TimelineItemKind::Virtual(VirtualTimelineItem::DateDivider(millis)) => {
                            let item = list.item(cx, item_id, live_id!(DateDivider));
                            let text = unix_time_millis_to_datetime(*millis)
                                // format the time as a shortened date (Sat, Sept 5, 2021)
                                .map(|dt| format!("{}", dt.date_naive().format("%a %b %-d, %Y")))
                                .unwrap_or_else(|| format!("{:?}", millis));
                            item.label(id!(date)).set_text(cx, &text);
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
        let top_space = self.view(id!(top_space));
        let jump_to_bottom = self.jump_to_bottom_button(id!(jump_to_bottom));
        let curr_first_id = portal_list.first_id();
        let ui = self.widget_uid();
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
                    jump_to_bottom.update_visibility(cx, true);

                    tl.items = initial_items;
                    done_loading = true;
                }
                TimelineUpdate::NewItems { new_items, changed_indices, is_append, clear_cache } => {
                    if new_items.is_empty() {
                        if !tl.items.is_empty() {
                            log!("Timeline::handle_event(): timeline (had {} items) was cleared for room {}", tl.items.len(), tl.room_id);
                            // For now, we paginate a cleared timeline in order to be able to show something at least.
                            // A proper solution would be what's described below, which would be to save a few event IDs
                            // and then either focus on them (if we're not close to the end of the timeline)
                            // or paginate backwards until we find them (only if we are close the end of the timeline).
                            should_continue_backwards_pagination = true;
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
                        jump_to_bottom.update_visibility(cx, true);
                    }
                    else if let Some((curr_item_idx, new_item_idx, new_item_scroll, _event_id)) =
                        find_new_item_matching_current_item(cx, portal_list, curr_first_id, &tl.items, &new_items)
                    {
                        if curr_item_idx != new_item_idx {
                            log!("Timeline::handle_event(): jumping view from event index {curr_item_idx} to new index {new_item_idx}, scroll {new_item_scroll}, event ID {_event_id}");
                            portal_list.set_first_id_and_scroll(new_item_idx, new_item_scroll);
                            tl.prev_first_index = Some(new_item_idx);
                            // Set scrolled_past_read_marker false when we jump to a new event
                            tl.scrolled_past_read_marker = false;
                            // When the tooltip is up, the timeline may jump. This may take away the hover out event to required to clear the tooltip
                            cx.widget_action(ui, &Scope::empty().path, RoomScreenTooltipActions::HoverOut);

                        }
                    }
                    //
                    // TODO: after an (un)ignore user event, all timelines are cleared. Handle that here.
                    //
                    else {
                        // warning!("!!! Couldn't find new event with matching ID for ANY event currently visible in the portal list");
                    }

                    // If new items were appended to the end of the timeline, show an unread messages badge on the jump to bottom button.
                    if is_append && !portal_list.is_at_end() {
                        if let Some(room_id) = &self.room_id {
                            // Immediately show the unread badge with no count while we fetch the actual count in the background.
                            jump_to_bottom.show_unread_message_badge(cx, UnreadMessageCount::Unknown);
                            submit_async_request(MatrixRequest::GetNumberUnreadMessages{ room_id: room_id.clone() });
                        }
                    }

                    if clear_cache {
                        tl.content_drawn_since_last_update.clear();
                        tl.profile_drawn_since_last_update.clear();
                        tl.fully_paginated = false;

                        // If this RoomScreen is showing the loading pane and has an ongoing backwards pagination request,
                        // then we should update the status message in that loading pane
                        // and then continue paginating backwards until we find the target event.
                        // Note that we do this here because `clear_cache` will always be true if backwards pagination occurred.
                        let loading_pane = self.view.loading_pane(id!(loading_pane));
                        let mut loading_pane_state = loading_pane.take_state();
                        if let LoadingPaneState::BackwardsPaginateUntilEvent {
                            ref mut events_paginated, target_event_id, ..
                        } = &mut loading_pane_state {
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
                        loading_pane.set_state(cx, loading_pane_state);
                    } else {
                        tl.content_drawn_since_last_update.remove(changed_indices.clone());
                        tl.profile_drawn_since_last_update.remove(changed_indices.clone());
                        // log!("Timeline::handle_event(): changed_indices: {changed_indices:?}, items len: {}\ncontent drawn: {:#?}\nprofile drawn: {:#?}", items.len(), tl.content_drawn_since_last_update, tl.profile_drawn_since_last_update);
                    }
                    tl.items = new_items;
                    done_loading = true;
                }
                TimelineUpdate::NewUnreadMessagesCount(unread_messages_count) => {
                    jump_to_bottom.show_unread_message_badge(cx, unread_messages_count);
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
                    let loading_pane = self.view.loading_pane(id!(loading_pane));

                    // log!("TargetEventFound: is_valid? {is_valid}. room {}, event {target_event_id}, index {index} of {}\n  --> item: {item:?}", tl.room_id, tl.items.len());
                    if is_valid {
                        // We successfully found the target event, so we can close the loading pane,
                        // reset the loading panestate to `None`, and stop issuing backwards pagination requests.
                        loading_pane.set_status(cx, "Successfully found replied-to message!");
                        loading_pane.set_state(cx, LoadingPaneState::None);

                        // NOTE: this code was copied from the `MessageAction::JumpToRelated` handler;
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
                        // Show this error in the loading pane, which should already be open.
                        loading_pane.set_state(cx, LoadingPaneState::Error(
                            String::from("Unable to find related message; it may have been deleted.")
                        ));
                    }

                    should_continue_backwards_pagination = false;

                    // redraw now before any other items get added to the timeline list.
                    self.view.redraw(cx);
                }
                TimelineUpdate::PaginationRunning(direction) => {
                    if direction == PaginationDirection::Backwards {
                        top_space.set_visible(cx, true);
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
                TimelineUpdate::RoomMembersSynced => {
                    // log!("Timeline::handle_event(): room members fetched for room {}", tl.room_id);
                    // Here, to be most efficient, we could redraw only the user avatars and names in the timeline,
                    // but for now we just fall through and let the final `redraw()` call re-draw the whole timeline view.
                }
                TimelineUpdate::RoomMembersListFetched { members } => {
                    // Use `pub/sub` pattern here to let multiple components share room members data
                    use crate::room::room_member_manager::room_members;
                    room_members::update(cx, tl.room_id.clone(), members);
                },
                TimelineUpdate::MediaFetched => {
                    log!("Timeline::handle_event(): media fetched for room {}", tl.room_id);
                    // Here, to be most efficient, we could redraw only the media items in the timeline,
                    // but for now we just fall through and let the final `redraw()` call re-draw the whole timeline view.
                }
                TimelineUpdate::MessageEdited { timeline_event_id, result } => {
                    self.view.editing_pane(id!(editing_pane))
                        .handle_edit_result(cx, timeline_event_id, result);
                }
                TimelineUpdate::TypingUsers { users } => {
                    // This update loop should be kept tight & fast, so all we do here is
                    // save the list of typing users for future use after the loop exits.
                    // Then, we "process" it later (by turning it into a string) after the
                    // update loop has completed, which avoids unnecessary expensive work
                    // if the list of typing users gets updated many times in a row.
                    typing_users = users;
                }

                TimelineUpdate::UserPowerLevels(user_power_level) => {
                    tl.user_power = user_power_level;

                    // Update the visibility of the message input bar based on the new power levels.
                    let can_send_message = user_power_level.can_send_message();
                    self.view.view(id!(input_bar))
                        .set_visible(cx, can_send_message);
                    self.view.view(id!(can_not_send_message_notice))
                        .set_visible(cx, !can_send_message);
                }

                TimelineUpdate::OwnUserReadReceipt(receipt) => {
                    tl.latest_own_user_receipt = Some(receipt);
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
            top_space.set_visible(cx, false);
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
            self.view.label(id!(typing_label)).set_text(cx, &typing_notice_text);
            self.view.view(id!(typing_notice)).set_visible(cx, true);
            // Animate in the typing notice view (sliding it up from the bottom).
            self.animator_play(cx, id!(typing_notice_animator.show));
            // Start the typing notice text animation of bouncing dots.
            self.view.typing_animation(id!(typing_animation)).start_animation(cx);
        } else {
            // Animate out the typing notice view (sliding it out towards the bottom).
            self.animator_play(cx, id!(typing_notice_animator.hide));
            self.view.typing_animation(id!(typing_animation)).stop_animation(cx);
        }

        if num_updates > 0 {
            // log!("Applied {} timeline updates for room {}, redrawing with {} items...", num_updates, tl.room_id, tl.items.len());
            self.redraw(cx);
        }
    }


    /// Handles a link being clicked in any child widgets of this RoomScreen.
    ///
    /// Returns `true` if the given `action` was handled as a link click.
    fn handle_link_clicked(
        &mut self,
        cx: &mut Cx,
        action: &Action,
        pane: &UserProfileSlidingPaneRef,
    ) -> bool {
        // A closure that handles both MatrixToUri and MatrixUri links,
        // and returns whether the link was handled.
        let mut handle_matrix_link = |id: &MatrixId, _via: &[OwnedServerName]| -> bool {
            match id {
                MatrixId::User(user_id) => {
                    // There is no synchronous way to get the user's full profile info
                    // including the details of their room membership,
                    // so we fill in with the details we *do* know currently,
                    // show the UserProfileSlidingPane, and then after that,
                    // the UserProfileSlidingPane itself will fire off
                    // an async request to get the rest of the details.
                    self.show_user_profile(
                        cx,
                        pane,
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
                MatrixId::Room(room_id) => {
                    if self.room_id.as_ref() == Some(room_id) {
                        enqueue_popup_notification(PopupItem { 
                            message: "You are already viewing that room.".into(), 
                            auto_dismissal_duration: None 
                        });
                        return true;
                    }
                    if let Some(_known_room) = get_client().and_then(|c| c.get_room(room_id)) {
                        log!("TODO: jump to known room {}", room_id);
                    } else {
                        log!("TODO: fetch and display room preview for room {}", room_id);
                    }
                    false
                }
                MatrixId::RoomAlias(room_alias) => {
                    log!("TODO: open room alias {}", room_alias);
                    // TODO: open a room loading screen that shows a spinner
                    //       while our background async task calls Client::resolve_room_alias()
                    //       and then either jumps to the room if known, or fetches and displays
                    //       a room preview for that room.
                    false
                }
                MatrixId::Event(room_id, event_id) => {
                    log!("TODO: open event {} in room {}", event_id, room_id);
                    // TODO: this requires the same first step as the `MatrixId::Room` case above,
                    //       but then we need to call Room::event_with_context() to get the event
                    //       and its context (surrounding events ?).
                    false
                }
                _ => false,
            }
        };

        if let HtmlLinkAction::Clicked { url, .. } = action.as_widget_action().cast() {
            let mut link_was_handled = false;
            if let Ok(matrix_to_uri) = MatrixToUri::parse(&url) {
                link_was_handled |= handle_matrix_link(matrix_to_uri.id(), matrix_to_uri.via());
            }
            else if let Ok(matrix_uri) = MatrixUri::parse(&url) {
                link_was_handled |= handle_matrix_link(matrix_uri.id(), matrix_uri.via());
            }

            if !link_was_handled {
                log!("Opening URL \"{}\"", url);
                if let Err(e) = robius_open::Uri::new(&url).open() {
                    error!("Failed to open URL {:?}. Error: {:?}", url, e);
                    enqueue_popup_notification(PopupItem {
                        message: format!("Could not open URL: {url}"), 
                        auto_dismissal_duration: None
                    });
                }
            }
            true
        }
        else if let RobrixHtmlLinkAction::ClickedMatrixLink { url, matrix_id, via, .. } = action.as_widget_action().cast() {
            let link_was_handled = handle_matrix_link(&matrix_id, &via);
            if !link_was_handled {
                log!("Opening URL \"{}\"", url);
                if let Err(e) = robius_open::Uri::new(&url).open() {
                    error!("Failed to open URL {:?}. Error: {:?}", url, e);
                    enqueue_popup_notification(PopupItem {
                        message: format!("Could not open URL: {url}"), 
                        auto_dismissal_duration: None
                    });
                }
            }
            true
        }
        else {
            false
        }
    }

    /// Handles any [`MessageAction`]s received by this RoomScreen.
    fn handle_message_actions(
        &mut self,
        cx: &mut Cx,
        actions: &ActionsBuf,
        portal_list: &PortalListRef,
        loading_pane: &LoadingPaneRef,
    ) {
        let room_screen_widget_uid = self.widget_uid();
        for action in actions {
            match action.as_widget_action().widget_uid_eq(room_screen_widget_uid).cast() {
                MessageAction::React { details, reaction } => {
                    let Some(tl) = self.tl_state.as_mut() else { return };
                    let mut success = false;
                    if let Some(timeline_item) = tl.items.get(details.item_id) {
                        if let Some(event_tl_item) = timeline_item.as_event() {
                            if event_tl_item.event_id() == details.event_id.as_deref() {
                                let timeline_event_id = event_tl_item.identifier();
                                submit_async_request(MatrixRequest::ToggleReaction {
                                    room_id: tl.room_id.clone(),
                                    timeline_event_id,
                                    reaction,
                                });
                                success = true;
                            }
                        }
                    }
                    if !success {
                        enqueue_popup_notification(PopupItem { 
                            message: "Couldn't find message in timeline to react to.".to_string(), 
                            auto_dismissal_duration: None 
                        });
                        error!("MessageAction::React: couldn't find event [{}] {:?} to react to in room {}",
                            details.item_id,
                            details.event_id.as_deref(),
                            tl.room_id,
                        );
                    }
                }
                MessageAction::Reply(details) => {
                    let mut success = false;
                    if let Some(event_tl_item) = self.tl_state.as_ref()
                        .and_then(|tl| tl.items.get(details.item_id))
                        .and_then(|tl_item| tl_item.as_event().cloned())
                        .filter(|ev| ev.event_id() == details.event_id.as_deref())
                    {
                        if let Ok(replied_to_info) = event_tl_item.replied_to_info() {
                            success = true;
                            self.show_replying_to(cx, (event_tl_item, replied_to_info));
                        }
                    }
                    if !success {
                        enqueue_popup_notification(PopupItem { message: "Could not find message in timeline to reply to.".to_string(), auto_dismissal_duration: None });
                        error!("MessageAction::Reply: couldn't find event [{}] {:?} to reply to in room {:?}",
                            details.item_id,
                            details.event_id.as_deref(),
                            self.room_id,
                        );
                    }
                }
                MessageAction::Edit(details) => {
                    let Some(tl) = self.tl_state.as_ref() else { return };
                    if let Some(event_tl_item) = tl.items.get(details.item_id)
                        .and_then(|tl_item| tl_item.as_event().cloned())
                        .filter(|ev| ev.event_id() == details.event_id.as_deref())
                    {
                        self.show_editing_pane(cx, event_tl_item, tl.room_id.clone());
                    }
                    else {
                        enqueue_popup_notification(PopupItem { message: "Could not find message in timeline to edit.".to_string(), auto_dismissal_duration: None });
                        error!("MessageAction::Edit: couldn't find event [{}] {:?} to edit in room {:?}",
                            details.item_id,
                            details.event_id.as_deref(),
                            self.room_id,
                        );
                    }
                }
                MessageAction::Pin(_details) => {
                    // TODO
                    enqueue_popup_notification(PopupItem { message: "Pinning messages is not yet implemented.".to_string(), auto_dismissal_duration: None });
                }
                MessageAction::Unpin(_details) => {
                    // TODO
                    enqueue_popup_notification(PopupItem { message: "Unpinning messages is not yet implemented.".to_string(), auto_dismissal_duration: None });
                }
                MessageAction::CopyText(details) => {
                    let Some(tl) = self.tl_state.as_mut() else { return };
                    if let Some(text) = tl.items
                        .get(details.item_id)
                        .and_then(|tl_item| tl_item.as_event().map(plaintext_body_of_timeline_item))
                    {
                        cx.copy_to_clipboard(&text);
                    }
                    else {
                        enqueue_popup_notification(PopupItem { message: "Could not find message in timeline to copy text from.".to_string(), auto_dismissal_duration: None});
                        error!("MessageAction::CopyText: couldn't find event [{}] {:?} to copy text from in room {}",
                            details.item_id,
                            details.event_id.as_deref(),
                            tl.room_id,
                        );
                    }
                }
                MessageAction::CopyHtml(details) => {
                    let Some(tl) = self.tl_state.as_ref() else { return };
                    // The logic for getting the formatted body of a message is the same
                    // as the logic used in `populate_message_view()`.
                    let mut success = false;
                    if let Some(event_tl_item) = tl.items
                        .get(details.item_id)
                        .and_then(|tl_item| tl_item.as_event())
                        .filter(|ev| ev.event_id() == details.event_id.as_deref())
                    {
                        if let TimelineItemContent::Message(message) = event_tl_item.content() {
                            match message.msgtype() {
                                MessageType::Text(TextMessageEventContent { formatted: Some(FormattedBody { body, .. }), .. })
                                | MessageType::Notice(NoticeMessageEventContent { formatted: Some(FormattedBody { body, .. }), .. })
                                | MessageType::Emote(EmoteMessageEventContent { formatted: Some(FormattedBody { body, .. }), .. })
                                | MessageType::Image(ImageMessageEventContent { formatted: Some(FormattedBody { body, .. }), .. })
                                | MessageType::File(FileMessageEventContent { formatted: Some(FormattedBody { body, .. }), .. })
                                | MessageType::Audio(AudioMessageEventContent { formatted: Some(FormattedBody { body, .. }), .. })
                                | MessageType::Video(VideoMessageEventContent { formatted: Some(FormattedBody { body, .. }), .. })
                                | MessageType::VerificationRequest(KeyVerificationRequestEventContent { formatted: Some(FormattedBody { body, .. }), .. }) =>
                                {
                                    cx.copy_to_clipboard(body);
                                    success = true;
                                }
                                _ => {}
                            }
                        }
                    }
                    if !success {
                        enqueue_popup_notification(PopupItem { message: "Could not find message in timeline to copy HTML from.".to_string(), auto_dismissal_duration: None });
                        error!("MessageAction::CopyHtml: couldn't find event [{}] {:?} to copy HTML from in room {}",
                            details.item_id,
                            details.event_id.as_deref(),
                            tl.room_id,
                        );
                    }
                }
                MessageAction::CopyLink(details) => {
                    let Some(tl) = self.tl_state.as_mut() else { return };
                    if let Some(event_id) = details.event_id {
                        let matrix_to_uri = tl.room_id.matrix_to_event_uri(event_id);
                        cx.copy_to_clipboard(&matrix_to_uri.to_string());
                    } else {
                        enqueue_popup_notification(PopupItem { message: "Couldn't create permalink to message.".to_string(), auto_dismissal_duration: None });
                        error!("MessageAction::CopyLink: no `event_id`: [{}] {:?} in room {}",
                            details.item_id,
                            details.event_id.as_deref(),
                            tl.room_id,
                        );
                    }
                }
                MessageAction::ViewSource(_details) => {
                    enqueue_popup_notification(PopupItem { message: "Viewing an event's source is not yet implemented.".to_string(), auto_dismissal_duration: None });
                    // TODO: re-use Franco's implementation below:

                    // let Some(tl) = self.tl_state.as_mut() else { continue };
                    // let Some(event_tl_item) = tl.items
                    //     .get(details.item_id)
                    //     .and_then(|tl_item| tl_item.as_event().cloned())
                    //     .filter(|ev| ev.event_id() == details.event_id.as_deref())
                    // else {
                    //     continue;
                    // };

                    // let Some(_message_event) = event_tl_item.content().as_message() else {
                    //     continue;
                    // };

                    // let original_json: Option<serde_json::Value> = event_tl_item
                    //     .original_json()
                    //     .and_then(|raw_event| serde_json::to_value(raw_event).ok());
                    // let room_id = self.room_id.to_owned();
                    // let event_id = event_tl_item.event_id().map(|e| e.to_owned());

                    // cx.widget_action(
                    //     widget_uid,
                    //     &scope.path,
                    //     MessageAction::MessageSourceModalOpen { room_id, event_id, original_json },
                    // );
                }
                MessageAction::JumpToRelated(details) => {
                    let Some(tl) = self.tl_state.as_mut() else { continue };
                    let Some(related_event_id) = details.related_event_id.as_ref() else {
                        error!("BUG: MessageAction::JumpToRelated had not related event ID.");
                        continue;
                    };
                    let tl_idx = details.item_id;

                    // Attempt to find the index of replied-to message in the timeline.
                    // Start from the current item's index (`tl_idx`)and search backwards,
                    // since we know the related message must come before the current item.
                    let mut num_items_searched = 0;
                    let related_msg_tl_index = tl.items
                        .focus()
                        .narrow(..tl_idx)
                        .into_iter()
                        .rev()
                        .take(MAX_ITEMS_TO_SEARCH_THROUGH)
                        .position(|i| {
                            num_items_searched += 1;
                            i.as_event()
                                .and_then(|e| e.event_id())
                                .is_some_and(|ev_id| ev_id == related_event_id)
                        })
                        .map(|position| tl_idx.saturating_sub(position).saturating_sub(1));

                    if let Some(index) = related_msg_tl_index {
                        // log!("The related message {replied_to_event} was immediately found in room {}, scrolling to from index {reply_message_item_id} --> {index} (first ID {}).", tl.room_id, portal_list.first_id());
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
                        // Here, we set the state of the loading pane and display it to the user.
                        // The main logic will be handled in `process_timeline_updates()`, which is the only
                        // place where we can receive updates to the timeline from the background tasks.
                        loading_pane.set_state(
                            cx,
                            LoadingPaneState::BackwardsPaginateUntilEvent {
                                target_event_id: related_event_id.clone(),
                                events_paginated: 0,
                                request_sender: tl.request_sender.clone(),
                            },
                        );
                        loading_pane.show(cx);

                        tl.request_sender.send_if_modified(|requests| {
                            if let Some(existing) = requests.iter_mut().find(|r| r.room_id == tl.room_id) {
                                warning!("Unexpected: room {} already had an existing timeline request in progress, event: {:?}", tl.room_id, existing.target_event_id);
                                // We might as well re-use this existing request...
                                existing.target_event_id = related_event_id.clone();
                            } else {
                                requests.push(BackwardsPaginateUntilEventRequest {
                                    room_id: tl.room_id.clone(),
                                    target_event_id: related_event_id.clone(),
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
                MessageAction::Redact { details, reason } => {
                    let Some(tl) = self.tl_state.as_mut() else { return };
                    let mut success = false;
                    if let Some(timeline_item) = tl.items.get(details.item_id) {
                        if let Some(event_tl_item) = timeline_item.as_event() {
                            if event_tl_item.event_id() == details.event_id.as_deref() {
                                let timeline_event_id = event_tl_item.identifier();
                                submit_async_request(MatrixRequest::RedactMessage {
                                    room_id: tl.room_id.clone(),
                                    timeline_event_id,
                                    reason,
                                });
                                success = true;
                            }
                        }
                    }
                    if !success {
                        enqueue_popup_notification(PopupItem { message: "Couldn't find message in timeline to delete.".to_string(), auto_dismissal_duration: None });
                        error!("MessageAction::Redact: couldn't find event [{}] {:?} to react to in room {}",
                            details.item_id,
                            details.event_id.as_deref(),
                            tl.room_id,
                        );
                    }
                }
                // MessageAction::Report(details) => {
                //     // TODO
                //     enqueue_popup_notification(PopupItem { message: "Reporting messages is not yet implemented.".to_string(), auto_dismissal_duration: None });
                // }

                // This is handled within the Message widget itself.
                MessageAction::HighlightMessage(..) => { }
                // This is handled by the top-level App itself.
                MessageAction::OpenMessageContextMenu { .. } => { }
                // This isn't yet handled, as we need to completely redesign it.
                MessageAction::ActionBarOpen { .. } => { }
                // This isn't yet handled, as we need to completely redesign it.
                MessageAction::ActionBarClose => { }
                MessageAction::None => { }
            }
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
        self.redraw(cx);
    }

    /// Shows the editing pane to allow the user to edit the given event.
    fn show_editing_pane(
        &mut self,
        cx: &mut Cx,
        event_tl_item: EventTimelineItem,
        room_id: OwnedRoomId,
    ) {
        // We must hide the input_bar while the editing pane is shown,
        // otherwise a very-tall input bar might show up underneath a shorter editing pane.
        self.view.room_input_bar(id!(input_bar)).set_visible(cx, false);

        self.editing_pane(id!(editing_pane)).show(
            cx,
            event_tl_item,
            room_id,
        );
        self.redraw(cx);
    }

    /// Handles the EditingPane in this RoomScreen being fully hidden.
    fn on_hide_editing_pane(&mut self, cx: &mut Cx) {
        // In `show_editing_pane()` above, we hid the input_bar while the editing pane
        // is being shown, so here we need to make it visible again.
        self.view.room_input_bar(id!(input_bar)).set_visible(cx, true);
        self.redraw(cx);
        // We don't need to do anything with the editing pane itself here,
        // because it has already been hidden by the time this function gets called.
    }

    /// Shows a preview of the given event that the user is currently replying to
    /// above the message input bar.
    fn show_replying_to(
        &mut self,
        cx: &mut Cx,
        replying_to: (EventTimelineItem, RepliedToInfo),
    ) {
        let replying_preview_view = self.view(id!(replying_preview));
        let (replying_preview_username, _) = replying_preview_view
            .avatar(id!(reply_preview_content.reply_preview_avatar))
            .set_avatar_and_get_username(
                cx,
                self.room_id.as_ref().unwrap(),
                replying_to.0.sender(),
                Some(replying_to.0.sender_profile()),
                replying_to.0.event_id(),
            );

        replying_preview_view
            .label(id!(reply_preview_content.reply_preview_username))
            .set_text(cx, replying_preview_username.as_str());

        populate_preview_of_timeline_item(
            cx,
            &replying_preview_view.html_or_plaintext(id!(reply_preview_content.reply_preview_body)),
            replying_to.0.content(),
            &replying_preview_username,
        );

        self.view(id!(replying_preview)).set_visible(cx, true);
        if let Some(tl) = self.tl_state.as_mut() {
            tl.replying_to = Some(replying_to);
        }

        // After the user clicks the reply button next to a message,
        // and we get to this point where the replying-to preview is shown,
        // we should automatically focus the keyboard on the message input box
        // so that the user can immediately start typing their reply
        // without having to manually click on the message input box.
        self.text_input(id!(input_bar.message_input.text_input)).set_key_focus(cx);
        self.redraw(cx);
    }

    /// Clears (and makes invisible) the preview of the message
    /// that the user is currently replying to.
    fn clear_replying_to(&mut self, cx: &mut Cx) {
        self.view(id!(replying_preview)).set_visible(cx, false);
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
        let room_id = self.room_id.clone()
            .expect("BUG: Timeline::show_timeline(): no room_id was set.");
        // just an optional sanity check
        assert!(self.tl_state.is_none(),
            "BUG: tried to show_timeline() into a timeline with existing state. \
            Did you forget to save the timeline state back to the global map of states?",
        );

        // Obtain the current user's power levels for this room.
        submit_async_request(MatrixRequest::GetRoomPowerLevels { room_id: room_id.clone() });

        let state_opt = TIMELINE_STATES.lock().unwrap().remove(&room_id);
        let (mut tl_state, first_time_showing_room) = if let Some(existing) = state_opt {
            (existing, false)
        } else {
            let (update_sender, update_receiver, request_sender) = take_timeline_endpoints(&room_id)
                .expect("BUG: couldn't get timeline state for first-viewed room.");
            let new_tl_state = TimelineUiState {
                room_id: room_id.clone(),
                // We assume the user has all power levels by default, just to avoid
                // unexpectedly hiding any UI elements that should be visible to the user.
                // This doesn't mean that the user can actually perform all actions.
                user_power: UserPowerLevels::all(),
                // We assume timelines being viewed for the first time haven't been fully paginated.
                fully_paginated: false,
                items: Vector::new(),
                content_drawn_since_last_update: RangeSet::new(),
                profile_drawn_since_last_update: RangeSet::new(),
                update_receiver,
                request_sender,
                media_cache: MediaCache::new(Some(update_sender)),
                replying_to: None,
                saved_state: SavedState::default(),
                message_highlight_animation_state: MessageHighlightAnimationState::default(),
                last_scrolled_index: usize::MAX,
                prev_first_index: None,
                scrolled_past_read_marker: false,
                latest_own_user_receipt: None,
            };
            (new_tl_state, true)
        };

        // Subscribe to typing notices, but hide the typing notice view initially.
        self.view(id!(typing_notice)).set_visible(cx, false);
        submit_async_request(
            MatrixRequest::SubscribeToTypingNotices {
                room_id: room_id.clone(),
                subscribe: true,
            }
        );

        submit_async_request(MatrixRequest::SubscribeToOwnUserReadReceiptsChanged { room_id: room_id.clone(), subscribe: true });
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
            submit_async_request(MatrixRequest::SyncRoomMemberList { room_id });
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
            room_id: room_id.clone(),
            subscribe: false,
        });
        submit_async_request(MatrixRequest::SubscribeToOwnUserReadReceiptsChanged {
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
        let message_input = self.text_input(id!(input_bar.message_input.text_input));
        let editing_event = self.editing_pane(id!(editing_pane)).get_event_being_edited();
        let state = SavedState {
            first_index_and_scroll: Some((portal_list.first_id(), portal_list.scroll_position())),
            message_input_state: message_input.save_state(),
            replying_to: tl.replying_to.clone(),
            editing_event,
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
            editing_event,
        } = &mut tl_state.saved_state;
        // 1. Restore the position of the timeline.
        if let Some((first_index, scroll_from_first_id)) = first_index_and_scroll {
            self.portal_list(id!(timeline.list))
                .set_first_id_and_scroll(*first_index, *scroll_from_first_id);
        } else {
            // If the first index is not set, then the timeline has not yet been scrolled by the user,
            // so we set the portal list to "tail" (track) the bottom of the list.
            self.portal_list(id!(timeline.list)).set_tail_range(true);
        }

        // 2. Restore the state of the message input box.
        let saved_message_input_state = std::mem::take(message_input_state);
        self.text_input(id!(input_bar.message_input.text_input))
            .restore_state(cx, saved_message_input_state);

        // 3. Restore the state of the replying-to preview.
        if let Some(replying_to_event) = replying_to.take() {
            self.show_replying_to(cx, replying_to_event);
        } else {
            self.clear_replying_to(cx);
        }

        // 4. Restore the state of the editing pane.
        if let Some(editing_event) = editing_event.take() {
            self.show_editing_pane(cx, editing_event, tl_state.room_id.clone());
        } else {
            self.editing_pane(id!(editing_pane)).force_hide(cx);
            self.on_hide_editing_pane(cx);
        }
    }

    /// Sets this `RoomScreen` widget to display the timeline for the given room.
    pub fn set_displayed_room<S: Into<Option<String>>>(
        &mut self,
        cx: &mut Cx,
        room_id: OwnedRoomId,
        room_name: S,
    ) {
        // If the room is already being displayed, then do nothing.
        if self.room_id.as_ref().is_some_and(|id| id == &room_id) { return; }
        

        self.hide_timeline();
        // Reset the the state of the inner loading pane.
        self.loading_pane(id!(loading_pane)).take_state();
        self.room_name = room_name_or_id(room_name.into(), &room_id);
        self.room_id = Some(room_id.clone());

        // Clear any mention input state
        let input_bar = self.view.room_input_bar(id!(input_bar));
        let message_input = input_bar.mentionable_text_input(id!(message_input));
        message_input.set_room_id(room_id);

        self.show_timeline(cx);
    }

    /// Sends read receipts based on the current scroll position of the timeline.
    fn send_user_read_receipts_based_on_scroll_pos(
        &mut self,
        _cx: &mut Cx,
        actions: &ActionsBuf,
        portal_list: &PortalListRef,
    ) {
        //stopped scrolling
        if portal_list.scrolled(actions) {
            return;
        }
        let first_index = portal_list.first_id();
        let Some(tl_state) = self.tl_state.as_mut() else { return };

        if let Some(ref mut index) = tl_state.prev_first_index {
            // to detect change of scroll when scroll ends
            if *index != first_index {
                if first_index >= *index {
                    // Get event_id and timestamp for the last visible event
                    let Some((last_event_id, last_timestamp)) = tl_state
                        .items
                        .get(std::cmp::min(
                            first_index + portal_list.visible_items(),
                            tl_state.items.len().saturating_sub(1)
                        ))
                        .and_then(|f| f.as_event())
                        .and_then(|f| f.event_id().map(|e| (e, f.timestamp())))
                    else {
                        *index = first_index;
                        return;
                    };
                    submit_async_request(MatrixRequest::ReadReceipt {
                        room_id: tl_state.room_id.clone(),
                        event_id: last_event_id.to_owned(),
                    });
                    if tl_state.scrolled_past_read_marker {
                        submit_async_request(MatrixRequest::FullyReadReceipt {
                            room_id: tl_state.room_id.clone(),
                            event_id: last_event_id.to_owned(),
                        });
                    } else {
                        if let Some(own_user_receipt_timestamp) = &tl_state.latest_own_user_receipt.clone()
                        .and_then(|receipt| receipt.ts) {
                            let Some((_first_event_id, first_timestamp)) = tl_state
                                .items
                                .get(first_index)
                                .and_then(|f| f.as_event())
                                .and_then(|f| f.event_id().map(|e| (e, f.timestamp())))
                                else {
                                    *index = first_index;
                                    return;
                                };
                            if own_user_receipt_timestamp >= &first_timestamp
                                && own_user_receipt_timestamp <= &last_timestamp
                            {
                                tl_state.scrolled_past_read_marker = true;
                                submit_async_request(MatrixRequest::FullyReadReceipt {
                                    room_id: tl_state.room_id.clone(),
                                    event_id: last_event_id.to_owned(),
                                });
                            }

                        }
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
}

impl RoomScreenRef {
    /// See [`RoomScreen::set_displayed_room()`].
    pub fn set_displayed_room<S: Into<Option<String>>>(
        &self,
        cx: &mut Cx,
        room_id: OwnedRoomId,
        room_name: S,
    ) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.set_displayed_room(cx, room_id, room_name);
    }
}

/// Actions for the room screen's tooltip.
#[derive(Clone, Debug, DefaultNone)]
pub enum RoomScreenTooltipActions {
    /// Mouse over event when the mouse is over the read receipt.
    HoverInReadReceipt {
        /// The rect of the moused over widget
        widget_rect: Rect,
        /// Color of the background, default is black
        bg_color: Option<Vec4>,
        /// Includes the list of users who have seen this event
        read_receipts: indexmap::IndexMap<matrix_sdk::ruma::OwnedUserId, Receipt>,
    },
    /// Mouse over event when the mouse is over the reaction button.
    HoverInReactionButton {
        /// The rect of the moused over widget
        widget_rect: Rect,
        /// Color of the background, default is black
        bg_color: Option<Vec4>,
        /// Includes the list of users who have reacted to the emoji
        reaction_data: ReactionData,
    },
    /// Mouse out event and clear tooltip.
    HoverOut,
    None,
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
    /// The updated number of unread messages in the room.
    NewUnreadMessagesCount(UnreadMessageCount),
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
    /// The result of a request to edit a message in this timeline.
    MessageEdited {
        timeline_event_id: TimelineEventItemId,
        result: Result<(), matrix_sdk_ui::timeline::Error>,
    },
    /// A notice that the room's members have been fetched from the server,
    /// though the success or failure of the request is not yet known until the client
    /// requests the member info via a timeline event's `sender_profile()` method.
    RoomMembersSynced,
    /// A notice that the room's full member list has been fetched from the server,
    /// includes a complete list of room members that can be shared across components.
    /// This is different from RoomMembersSynced which only indicates members were fetched
    /// but doesn't provide the actual data.
    RoomMembersListFetched {
        members: Vec<RoomMember>,
    },
    /// A notice that one or more requested media items (images, videos, etc.)
    /// that should be displayed in this timeline have now been fetched and are available.
    MediaFetched,
    /// A notice that one or more members of a this room are currently typing.
    TypingUsers {
        /// The list of users (their displayable name) who are currently typing in this room.
        users: Vec<String>,
    },
    /// An update containing the currently logged-in user's power levels for this room.
    UserPowerLevels(UserPowerLevels),
    /// An update to the currently logged-in user's own read receipt for this room.
    OwnUserReadReceipt(Receipt),
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

    /// The power levels of the currently logged-in user in this room.
    user_power: UserPowerLevels,

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

    /// The index of the first item shown in the timeline's PortalList from *before* the last "jump".
    ///
    /// This index is saved before the timeline undergoes any jumps, e.g.,
    /// receiving new items, major scroll changes, or other timeline view jumps.
    prev_first_index: Option<usize>,

    /// Whether the user has scrolled past their latest read marker.
    ///
    /// This is used to determine whether we should send a fully-read receipt
    /// after the user scrolls past their "read marker", i.e., their latest fully-read receipt.
    /// Its value is determined by comparing the fully-read event's timestamp with the
    /// first and last timestamp of displayed events in the timeline.
    /// When scrolling down, if the value is true, we send a fully-read receipt
    /// for the last visible event in the timeline.
    ///
    /// When new message come in, this value is reset to `false`.
    scrolled_past_read_marker: bool,
    latest_own_user_receipt: Option<Receipt>,
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
    /// The event that the user is currently editing, if any.
    editing_event: Option<EventTimelineItem>,
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
    user_power_levels: &UserPowerLevels,
    item_drawn_status: ItemDrawnStatus,
    room_screen_widget_uid: WidgetUid,
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
                        .is_some_and(|d| d < uint!(600000)) // 10 mins in millis
            }
            _ => false,
        },
        _ => false,
    };

    let has_html_body: bool;

    // Sometimes we need to call this up-front, so we save the result in this variable
    // to avoid having to call it twice.
    let mut set_username_and_get_avatar_retval = None;
    let (item, used_cached_item) = match message.get_type() {
        MessageOrStickerType::Text(TextMessageEventContent { body, formatted, .. }) => {
            has_html_body = formatted.as_ref().is_some_and(|f| f.format == MessageFormat::Html);
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
                    cx,
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
            has_html_body = formatted.as_ref().is_some_and(|f| f.format == MessageFormat::Html);
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
                    cx,
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
            has_html_body = false;
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
                    cx,
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
            has_html_body = formatted.as_ref().is_some_and(|f| f.format == MessageFormat::Html);
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
                let (username, profile_drawn) = item.avatar(id!(profile.avatar)).set_avatar_and_get_username(
                    cx,
                    room_id,
                    event_tl_item.sender(),
                    Some(event_tl_item.sender_profile()),
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
                    cx,
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
            has_html_body = match mtype {
                MessageOrStickerType::Image(image) => image.formatted.as_ref()
                    .is_some_and(|f| f.format == MessageFormat::Html),
                _ => false,
            };
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
            has_html_body = false;
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
                    cx,
                    &item.html_or_plaintext(id!(content.message)),
                    location,
                );
                new_drawn_status.content_drawn = is_location_fully_drawn;
                (item, false)
            }
        }
        MessageOrStickerType::File(file_content) => {
            has_html_body = file_content.formatted.as_ref().is_some_and(|f| f.format == MessageFormat::Html);
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
                    cx,
                    &item.html_or_plaintext(id!(content.message)),
                    file_content,
                );
                (item, false)
            }
        }
        MessageOrStickerType::Audio(audio) => {
            has_html_body = audio.formatted.as_ref().is_some_and(|f| f.format == MessageFormat::Html);
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
                    cx,
                    &item.html_or_plaintext(id!(content.message)),
                    audio,
                );
                (item, false)
            }
        }
        MessageOrStickerType::Video(video) => {
            has_html_body = video.formatted.as_ref().is_some_and(|f| f.format == MessageFormat::Html);
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
                    cx,
                    &item.html_or_plaintext(id!(content.message)),
                    video,
                );
                (item, false)
            }
        }
        MessageOrStickerType::VerificationRequest(verification) => {
            has_html_body = verification.formatted.as_ref().is_some_and(|f| f.format == MessageFormat::Html);
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
                    cx,
                    &item.html_or_plaintext(id!(content.message)),
                    &verification.body,
                    Some(&formatted),
                );
                new_drawn_status.content_drawn = true;
                (item, false)
            }
        }
        other => {
            has_html_body = false;
            let (item, existed) = list.item_with_existed(cx, item_id, live_id!(Message));
            if existed && item_drawn_status.content_drawn {
                (item, true)
            } else {
                let kind = other.as_str();
                item.label(id!(content.message)).set_text(
                    cx,
                    &format!("[Unsupported ({kind})] {}", message.body()),
                );
                new_drawn_status.content_drawn = true;
                (item, false)
            }
        }
    };

    let mut replied_to_event_id = None;

    // If we didn't use a cached item, we need to draw all other message content: the reply preview and reactions.
    if !used_cached_item {
        item.reaction_list(id!(content.reaction_list)).set_list(
            cx,
            &event_tl_item.content().reactions(),
            room_id.to_owned(),
            event_tl_item.identifier(),
            item_id,
        );
        populate_read_receipts(&item, cx, room_id, event_tl_item);
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
                item.avatar(id!(profile.avatar)).set_avatar_and_get_username(
                    cx,
                    room_id,
                    event_tl_item.sender(),
                    Some(event_tl_item.sender_profile()),
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
            username_label.set_text(cx, &username);
            new_drawn_status.profile_drawn = profile_drawn;
        }
        else {
            // Server notices are drawn with a red color avatar background and username.
            let avatar = item.avatar(id!(profile.avatar));
            avatar.show_text(cx, None, "⚠");
            avatar.apply_over(cx, live!(
                text_view = {
                    draw_bg: { background_color: (COLOR_DANGER_RED), }
                }
            ));
            username_label.set_text(cx, "Server notice");
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
    item.as_message().set_data(MessageDetails {
        event_id: event_tl_item.event_id().map(|id| id.to_owned()),
        item_id,
        related_event_id: replied_to_event_id,
        room_screen_widget_uid,
        abilities: MessageAbilities::from_user_power_and_event(
            user_power_levels,
            event_tl_item,
            &message,
            has_html_body,
        ),
        should_be_highlighted: event_tl_item.is_highlighted()
    });

    // Set the timestamp.
    if let Some(dt) = unix_time_millis_to_datetime(ts_millis) {
        item.timestamp(id!(profile.timestamp)).set_date_time(cx, dt);
    }

    (item, new_drawn_status)
}

/// Draws the Html or plaintext body of the given Text or Notice message into the `message_content_widget`.
fn populate_text_message_content(
    cx: &mut Cx,
    message_content_widget: &HtmlOrPlaintextRef,
    body: &str,
    formatted_body: Option<&FormattedBody>,
) {
    // The message was HTML-formatted rich text.
    if let Some(fb) = formatted_body.as_ref()
        .and_then(|fb| (fb.format == MessageFormat::Html).then_some(fb))
    {
        message_content_widget.show_html(
            cx,
            utils::linkify(
                utils::trim_start_html_whitespace(&fb.body),
                true,
            )
        );
    }
    // The message was non-HTML plaintext.
    else {
        match utils::linkify(body, false) {
            Cow::Owned(linkified_html) => message_content_widget.show_html(cx, &linkified_html),
            Cow::Borrowed(plaintext) => message_content_widget.show_plaintext(cx, plaintext),
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

    // If we have a known mimetype and it's not a static image,
    // then show a message about it being unsupported (e.g., for animated gifs).
    if let Some(mime) = mimetype.as_ref() {
        if ImageFormat::from_mimetype(mime).is_none() {
            text_or_image_ref.show_text(
                cx,
                format!("{body}\n\nImages/Stickers of type {mime:?} are not yet supported."),
            );
            return true; // consider this as fully drawn
        }
    }

    let mut fully_drawn = false;

    // A closure that fetches and shows the image from the given `mxc_uri`,
    // marking it as fully drawn if the image was available.
    let mut fetch_and_show_image_uri = |cx: &mut Cx2d, mxc_uri: OwnedMxcUri, image_info: Option<&ImageInfo>| {
        match media_cache.try_get_media_or_fetch(mxc_uri.clone(), MEDIA_THUMBNAIL_FORMAT.into()) {
            (MediaCacheEntry::Loaded(data), _media_format) => {
                let show_image_result = text_or_image_ref.show_image(cx, |cx, img| {
                    utils::load_png_or_jpg(&img, cx, &data)
                        .map(|()| img.size_in_pixels(cx).unwrap_or_default())
                });
                if let Err(e) = show_image_result {
                    let err_str = format!("{body}\n\nFailed to display image: {e:?}");
                    error!("{err_str}");
                    text_or_image_ref.show_text(cx, &err_str);
                }

                // We're done drawing the image, so mark it as fully drawn.
                fully_drawn = true;
            }
            (MediaCacheEntry::Requested, _media_format) => {
                if let Some(image_info) = image_info {
                    if let (Some(ref blurhash), Some(width), Some(height)) = (image_info.blurhash.clone(), image_info.width, image_info.height) {
                        let show_image_result = text_or_image_ref.show_image(cx, |cx, img| {
                            let (Ok(width), Ok(height)) = (width.try_into(), height.try_into()) else { return Err(image_cache::ImageError::EmptyData)};
                            if let Ok(data) = blurhash::decode(blurhash, width, height, 1.0) {
                                ImageBuffer::new(&data, width as usize, height as usize).map(|img_buff| {
                                    let texture = Some(img_buff.into_new_texture(cx));
                                    img.set_texture(cx, texture);
                                    img.size_in_pixels(cx).unwrap_or_default()
                                })
                            } else {
                                Err(image_cache::ImageError::EmptyData)
                            }
                        });
                        if let Err(e) = show_image_result {
                            let err_str = format!("{body}\n\nFailed to display image: {e:?}");
                            error!("{err_str}");
                            text_or_image_ref.show_text(cx, &err_str);
                        }
                    }
                }
                fully_drawn = false;
            }
            (MediaCacheEntry::Failed, _media_format) => {
                text_or_image_ref
                    .show_text(cx, format!("{body}\n\nFailed to fetch image from {:?}", mxc_uri));
                // For now, we consider this as being "complete". In the future, we could support
                // retrying to fetch thumbnail of the image on a user click/tap.
                fully_drawn = true;
            }
        }
    };

    let mut fetch_and_show_media_source = |cx: &mut Cx2d, media_source: MediaSource, image_info: Option<&ImageInfo>| {
        match media_source {
            MediaSource::Encrypted(encrypted) => {
                // We consider this as "fully drawn" since we don't yet support encryption.
                text_or_image_ref.show_text(
                    cx,
                    format!("{body}\n\n[TODO] fetch encrypted image at {:?}", encrypted.url)
                );
            },
            MediaSource::Plain(mxc_uri) => {
                fetch_and_show_image_uri(cx, mxc_uri, image_info)
            }
        }
    };

    match image_info_source {
        Some((image_info, original_source)) => {
            // Use the provided thumbnail URI if it exists; otherwise use the original URI.
            let media_source = image_info.clone()
                .and_then(|image_info| image_info.thumbnail_source)
                .unwrap_or(original_source);
            fetch_and_show_media_source(cx, media_source, image_info.as_ref());
        }
        None => {
            text_or_image_ref.show_text(cx, "{body}\n\nImage message had no source URL.");
            fully_drawn = true;
        }
    }

    fully_drawn
}


/// Draws a file message's content into the given `message_content_widget`.
///
/// Returns whether the file message content was fully drawn.
fn populate_file_message_content(
    cx: &mut Cx,
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

    message_content_widget.show_html(
        cx,
        format!("<b>{filename}</b>{size}{caption}<br> → <i>File download not yet supported.</i>"),
    );
    true
}

/// Draws an audio message's content into the given `message_content_widget`.
///
/// Returns whether the audio message content was fully drawn.
fn populate_audio_message_content(
    cx: &mut Cx,
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

    message_content_widget.show_html(
        cx,
        format!("Audio: <b>{filename}</b>{mime}{duration}{size}{caption}<br> → <i>Audio playback not yet supported.</i>"),
    );
    true
}


/// Draws a video message's content into the given `message_content_widget`.
///
/// Returns whether the video message content was fully drawn.
fn populate_video_message_content(
    cx: &mut Cx,
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

    message_content_widget.show_html(
        cx,
        format!("Video: <b>{filename}</b>{mime}{duration}{size}{dimensions}{caption}<br> → <i>Video playback not yet supported.</i>"),
    );
    true
}



/// Draws the given location message's content into the `message_content_widget`.
///
/// Returns whether the location message content was fully drawn.
fn populate_location_message_content(
    cx: &mut Cx,
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
            "Location: <a href=\"{}\">{short_lat},{short_long}</a><br>\
            <ul>\
            <li><a href=\"https://www.openstreetmap.org/?mlat={lat}&amp;mlon={long}#map=15/{lat}/{long}\">Open in OpenStreetMap</a></li>\
            <li><a href=\"https://www.google.com/maps/search/?api=1&amp;query={lat},{long}\">Open in Google Maps</a></li>\
            <li><a href=\"https://maps.apple.com/?ll={lat},{long}&amp;q={lat},{long}\">Open in Apple Maps</a></li>\
            </ul>",
            location.geo_uri,
        );
        message_content_widget.show_html(cx, html_body);
    } else {
        message_content_widget.show_html(
            cx,
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
                let (in_reply_to_username, is_avatar_fully_drawn) =
                    replied_to_message_view
                        .avatar(id!(replied_to_message_content.reply_preview_avatar))
                        .set_avatar_and_get_username(
                            cx,
                            room_id,
                            replied_to_event.sender(),
                            Some(replied_to_event.sender_profile()),
                            Some(in_reply_to_details.event_id.as_ref()),
                        );

                fully_drawn = is_avatar_fully_drawn;

                replied_to_message_view
                    .label(id!(replied_to_message_content.reply_preview_username))
                    .set_text(cx, in_reply_to_username.as_str());
                let msg_body = replied_to_message_view.html_or_plaintext(id!(reply_preview_body));
                populate_preview_of_timeline_item(
                    cx,
                    &msg_body,
                    replied_to_event.content(),
                    &in_reply_to_username,
                );
            }
            TimelineDetails::Error(_e) => {
                fully_drawn = true;
                replied_to_message_view
                    .label(id!(replied_to_message_content.reply_preview_username))
                    .set_text(cx, "[Error fetching username]");
                replied_to_message_view
                    .avatar(id!(replied_to_message_content.reply_preview_avatar))
                    .show_text(cx, None, "?");
                replied_to_message_view
                    .html_or_plaintext(id!(replied_to_message_content.reply_preview_body))
                    .show_plaintext(cx, "[Error fetching replied-to event]");
            }
            status @ TimelineDetails::Pending | status @ TimelineDetails::Unavailable => {
                // We don't have the replied-to message yet, so we can't fully draw the preview.
                fully_drawn = false;
                replied_to_message_view
                    .label(id!(replied_to_message_content.reply_preview_username))
                    .set_text(cx, "[Loading username...]");
                replied_to_message_view
                    .avatar(id!(replied_to_message_content.reply_preview_avatar))
                    .show_text(cx, None, "?");
                replied_to_message_view
                    .html_or_plaintext(id!(replied_to_message_content.reply_preview_body))
                    .show_plaintext(cx, "[Loading replied-to message...]");

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

    replied_to_message_view.set_visible(cx, show_reply);
    (fully_drawn, replied_to_event_id)
}

fn populate_preview_of_timeline_item(
    cx: &mut Cx,
    widget_out: &HtmlOrPlaintextRef,
    timeline_item_content: &TimelineItemContent,
    sender_username: &str,
) {
    if let TimelineItemContent::Message(m) = timeline_item_content {
        match m.msgtype() {
            MessageType::Text(TextMessageEventContent { body, formatted, .. })
            | MessageType::Notice(NoticeMessageEventContent { body, formatted, .. }) => {
                return populate_text_message_content(cx, widget_out, body, formatted.as_ref());
            }
            _ => { } // fall through to the general case for all timeline items below.
        }
    }
    let html = text_preview_of_timeline_item(timeline_item_content, sender_username)
        .format_with(sender_username, true);
    widget_out.show_html(cx, html);
}


/// A trait for abstracting over the different types of timeline events
/// that can be displayed in a `SmallStateEvent` widget.
trait SmallStateEventContent {
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
        cx: &mut Cx,
        _list: &mut PortalList,
        _item_id: usize,
        item: WidgetRef,
        event_tl_item: &EventTimelineItem,
        original_sender: &str,
        _item_drawn_status: ItemDrawnStatus,
        mut new_drawn_status: ItemDrawnStatus,
    ) -> (WidgetRef, ItemDrawnStatus) {
        item.label(id!(content)).set_text(
            cx,
            &text_preview_of_redacted_message(event_tl_item, original_sender)
                .format_with(original_sender, false),
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
        let item = if let Some(text_preview) = text_preview_of_other_state(self, false) {
            item.label(id!(content))
                .set_text(cx, &text_preview.format_with(username, false));
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
        cx: &mut Cx,
        _list: &mut PortalList,
        _item_id: usize,
        item: WidgetRef,
        _event_tl_item: &EventTimelineItem,
        username: &str,
        _item_drawn_status: ItemDrawnStatus,
        mut new_drawn_status: ItemDrawnStatus,
    ) -> (WidgetRef, ItemDrawnStatus) {
        item.label(id!(content)).set_text(
            cx,
            &text_preview_of_member_profile_change(self, username, false)
                .format_with(username, false),
        );
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
        let Some(preview) = text_preview_of_room_membership_change(self, false) else {
            // Don't actually display anything for nonexistent/unimportant membership changes.
            return (
                list.item(cx, item_id, live_id!(Empty)),
                ItemDrawnStatus::new(),
            );
        };

        item.label(id!(content))
            .set_text(cx, &preview.format_with(username, false));
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
    populate_read_receipts(&item, cx, room_id, event_tl_item);
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
        let avatar_ref = item.avatar(id!(avatar));
        let (username, profile_drawn) = avatar_ref.set_avatar_and_get_username(
            cx,
            room_id,
            event_tl_item.sender(),
            Some(event_tl_item.sender_profile()),
            event_tl_item.event_id(),
        );
        // Draw the timestamp as part of the profile.
        if let Some(dt) = unix_time_millis_to_datetime(event_tl_item.timestamp()) {
            item.timestamp(id!(left_container.timestamp)).set_date_time(cx, dt);
        }
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


/// Returns the display name of the sender of the given `event_tl_item`, if available.
fn get_profile_display_name(event_tl_item: &EventTimelineItem) -> Option<String> {
    if let TimelineDetails::Ready(profile) = event_tl_item.sender_profile() {
        profile.display_name.clone()
    } else {
        None
    }
}


/// Actions related to a specific message within a room timeline.
#[derive(Clone, DefaultNone, Debug)]
pub enum MessageAction {
    /// The user clicked the "react" button on a message
    /// and wants to send the given `reaction` to that message.
    React {
        details: MessageDetails,
        reaction: String,
    },
    /// The user clicked the "reply" button on a message.
    Reply(MessageDetails),
    /// The user clicked the "edit" button on a message.
    Edit(MessageDetails),
    /// The user clicked the "pin" button on a message.
    Pin(MessageDetails),
    /// The user clicked the "unpin" button on a message.
    Unpin(MessageDetails),
    /// The user clicked the "copy text" button on a message.
    CopyText(MessageDetails),
    /// The user clicked the "copy HTML" button on a message.
    CopyHtml(MessageDetails),
    /// The user clicked the "copy link" button on a message.
    CopyLink(MessageDetails),
    /// The user clicked the "view source" button on a message.
    ViewSource(MessageDetails),
    /// The user clicked the "jump to related" button on a message,
    /// indicating that they want to auto-scroll back to the related message,
    /// e.g., a replied-to message.
    JumpToRelated(MessageDetails),
    /// The user clicked the "delete" button on a message.
    #[doc(alias("delete"))]
    Redact {
        details: MessageDetails,
        reason: Option<String>,
    },

    // /// The user clicked the "report" button on a message.
    // Report(MessageDetails),

    /// The message at the given item index in the timeline should be highlighted.
    HighlightMessage(usize),
    /// The user requested that we show a context menu with actions
    /// that can be performed on a given message.
    OpenMessageContextMenu {
        details: MessageDetails,
        /// The absolute position where we should show the context menu,
        /// in which the (0,0) origin coordinate is the top left corner of the app window.
        abs_pos: DVec2,
    },
    /// The user requested opening the message action bar
    ActionBarOpen {
        /// At the given timeline item index
        item_id: usize,
        /// The message rect, so the action bar can be possitioned relative to it
        message_rect: Rect,
    },
    /// The user requested closing the message action bar
    ActionBarClose,
    None,
}

/// A widget representing a single message of any kind within a room timeline.
#[derive(Live, LiveHook, Widget)]
pub struct Message {
    #[deref] view: View,
    #[animator] animator: Animator,

    #[rust] details: Option<MessageDetails>,
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

        let Some(details) = self.details.clone() else { return };

        // We first handle a click on the replied-to message preview, if present,
        // because we don't want any widgets within the replied-to message to be
        // clickable or otherwise interactive.
        match event.hits(cx, self.view(id!(replied_to_message)).area()) {
            Hit::FingerDown(fe) => {
                cx.set_key_focus(self.view(id!(replied_to_message)).area());
                if fe.device.mouse_button().is_some_and(|b| b.is_secondary()) {
                    cx.widget_action(
                        details.room_screen_widget_uid,
                        &scope.path,
                        MessageAction::OpenMessageContextMenu {
                            details: details.clone(),
                            abs_pos: fe.abs,
                        }
                    );
                }
            }
            Hit::FingerLongPress(lp) => {
                cx.widget_action(
                    details.room_screen_widget_uid,
                    &scope.path,
                    MessageAction::OpenMessageContextMenu {
                        details: details.clone(),
                        abs_pos: lp.abs,
                    }
                );
            }
            // If the hit occurred on the replied-to message preview, jump to it.
            Hit::FingerUp(fe) if fe.is_over && fe.is_primary_hit() && fe.was_tap() => {
                // TODO: move this to the event handler for any reply preview content,
                //       since we also want this jump-to-reply behavior for the reply preview
                //       that appears above the message input box when you click the reply button.
                cx.widget_action(
                    details.room_screen_widget_uid,
                    &scope.path,
                    MessageAction::JumpToRelated(details.clone()),
                );
            }
            _ => { }
        }

        // Next, we forward the event to the child view such that it has the chance
        // to handle it before the Message widget handles it.
        // This ensures that events like right-clicking/long-pressing a reaction button
        // or a link within a message will be treated as an action upon that child view
        // rather than an action upon the message itself.
        self.view.handle_event(cx, event, scope);

        // Finally, handle any hits on the rest of the message body itself.
        let message_view_area = self.view.area();
        match event.hits(cx, message_view_area) {
            Hit::FingerDown(fe) => {
                cx.set_key_focus(message_view_area);
                // A right click means we should display the context menu.
                if fe.device.mouse_button().is_some_and(|b| b.is_secondary()) {
                    cx.widget_action(
                        details.room_screen_widget_uid,
                        &scope.path,
                        MessageAction::OpenMessageContextMenu {
                            details: details.clone(),
                            abs_pos: fe.abs,
                        }
                    );
                }
            }
            Hit::FingerLongPress(lp) => {
                cx.widget_action(
                    details.room_screen_widget_uid,
                    &scope.path,
                    MessageAction::OpenMessageContextMenu {
                        details: details.clone(),
                        abs_pos: lp.abs,
                    }
                );
            }
            Hit::FingerHoverIn(..) => {
                self.animator_play(cx, id!(hover.on));
                // TODO: here, show the "action bar" buttons upon hover-in
            }
            Hit::FingerHoverOut(_fho) => {
                self.animator_play(cx, id!(hover.off));
                // TODO: here, hide the "action bar" buttons upon hover-out
            }
            _ => { }
        }

        if let Event::Actions(actions) = event {
            for action in actions {
                match action.as_widget_action().cast() {
                    MessageAction::HighlightMessage(id) if id == details.item_id => {
                        self.animator_play(cx, id!(highlight.on));
                        self.redraw(cx);
                    }
                    _ => {}
                }
            }
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        if self.details.as_ref().is_some_and(|d| d.should_be_highlighted) {
            self.view.apply_over(
                cx, live!(
                    draw_bg: {
                        color: (vec4(1.0, 1.0, 0.82, 1.0))
                        mentions_bar_color: #ffd54f
                    }
                )
            )
        }

        self.view.draw_walk(cx, scope, walk)
    }
}

impl Message {
    fn set_data(&mut self, details: MessageDetails) {
        self.details = Some(details);
    }
}

impl MessageRef {
    fn set_data(&self, details: MessageDetails) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.set_data(details);
    }
}
