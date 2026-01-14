//! A room screen is the UI view that displays a single Room's timeline of events/messages
//! along with a message input bar at the bottom.

use std::{borrow::Cow, cell::RefCell, collections::BTreeMap, ops::{DerefMut, Range}, sync::Arc};

use bytesize::ByteSize;
use imbl::Vector;
use makepad_widgets::{image_cache::ImageBuffer, *};
use matrix_sdk::{
    OwnedServerName, RoomDisplayName, media::{MediaFormat, MediaRequestParameters}, room::RoomMember, ruma::{
        EventId, MatrixToUri, MatrixUri, OwnedEventId, OwnedMxcUri, OwnedRoomId, UserId, events::{
            receipt::Receipt,
            room::{
                ImageInfo, MediaSource, message::{
                    AudioMessageEventContent, EmoteMessageEventContent, FileMessageEventContent, FormattedBody, ImageMessageEventContent, KeyVerificationRequestEventContent, LocationMessageEventContent, MessageFormat, MessageType, NoticeMessageEventContent, TextMessageEventContent, VideoMessageEventContent
                }
            },
            sticker::{StickerEventContent, StickerMediaSource},
        }, matrix_uri::MatrixId, uint
    }
};
use matrix_sdk_ui::timeline::{
    self, EmbeddedEvent, EncryptedMessage, EventTimelineItem, InReplyToDetails, MemberProfileChange, MembershipChange, MsgLikeContent, MsgLikeKind, OtherMessageLike, PollState, RoomMembershipChange, TimelineDetails, TimelineEventItemId, TimelineItem, TimelineItemContent, TimelineItemKind, VirtualTimelineItem
};
use ruma::OwnedUserId;

use crate::{
    app::AppStateAction, avatar_cache, event_preview::{plaintext_body_of_timeline_item, text_preview_of_encrypted_message, text_preview_of_member_profile_change, text_preview_of_other_message_like, text_preview_of_other_state, text_preview_of_redacted_message, text_preview_of_room_membership_change, text_preview_of_timeline_item}, home::{edited_indicator::EditedIndicatorWidgetRefExt, link_preview::{LinkPreviewCache, LinkPreviewRef, LinkPreviewWidgetRefExt}, loading_pane::{LoadingPaneState, LoadingPaneWidgetExt}, room_image_viewer::{get_image_name_and_filesize, populate_matrix_image_modal}, rooms_list::RoomsListRef, tombstone_footer::SuccessorRoomDetails}, media_cache::{MediaCache, MediaCacheEntry}, profile::{
        user_profile::{AvatarState, ShowUserProfileAction, UserProfile, UserProfileAndRoomId, UserProfilePaneInfo, UserProfileSlidingPaneRef, UserProfileSlidingPaneWidgetExt},
        user_profile_cache,
    },
    room::{BasicRoomDetails, room_input_bar::RoomInputBarState, typing_notice::TypingNoticeWidgetExt},
    shared::{
        avatar::AvatarWidgetRefExt, callout_tooltip::{CalloutTooltipOptions, TooltipAction, TooltipPosition}, confirmation_modal::ConfirmationModalContent, html_or_plaintext::{HtmlOrPlaintextRef, HtmlOrPlaintextWidgetRefExt, RobrixHtmlLinkAction}, image_viewer::{ImageViewerAction, ImageViewerMetaData, LoadState}, jump_to_bottom_button::{JumpToBottomButtonWidgetExt, UnreadMessageCount}, popup_list::{PopupItem, PopupKind, enqueue_popup_notification}, restore_status_view::RestoreStatusViewWidgetExt, styles::*, text_or_image::{TextOrImageAction, TextOrImageRef, TextOrImageWidgetRefExt}, timestamp::TimestampWidgetRefExt
    },
    sliding_sync::{BackwardsPaginateUntilEventRequest, MatrixRequest, PaginationDirection, TimelineEndpoints, TimelineRequestSender, UserPowerLevels, get_client, submit_async_request, take_timeline_endpoints}, utils::{self, ImageFormat, MEDIA_THUMBNAIL_FORMAT, RoomNameId, unix_time_millis_to_datetime}
};
use crate::home::event_reaction_list::ReactionListWidgetRefExt;
use crate::home::room_read_receipt::AvatarRowWidgetRefExt;
use crate::room::room_input_bar::RoomInputBarWidgetExt;
use crate::shared::mentionable_text_input::MentionableTextInputAction;

use rangemap::RangeSet;

use super::{event_reaction_list::ReactionData, loading_pane::LoadingPaneRef, new_message_context_menu::{MessageAbilities, MessageDetails}, room_read_receipt::{self, populate_read_receipts, MAX_VISIBLE_AVATARS_IN_READ_RECEIPT}};

/// The maximum number of timeline items to search through
/// when looking for a particular event.
///
/// This is a safety measure to prevent the main UI thread
/// from getting into a long-running loop if an event cannot be found quickly.
const MAX_ITEMS_TO_SEARCH_THROUGH: usize = 100;

/// The max size (width or height) of a blurhash image to decode.
const BLURHASH_IMAGE_MAX_SIZE: u32 = 500;

static UNNAMED_ROOM: &str = "Unnamed Room";

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
    use crate::shared::jump_to_bottom_button::*;
    use crate::profile::user_profile::UserProfileSlidingPane;
    use crate::home::edited_indicator::*;
    use crate::home::event_reaction_list::*;
    use crate::home::loading_pane::*;
    use crate::room::room_input_bar::*;
    use crate::room::reply_preview::RepliedToMessage;
    use crate::room::typing_notice::*;
    use crate::home::room_read_receipt::*;
    use crate::rooms_list::*;
    use crate::shared::restore_status_view::*;
    use crate::home::link_preview::LinkPreview;
    use link::tsp_link::TspSignIndicator;

    COLOR_BG = #xfff8ee
    COLOR_OVERLAY_BG = #x000000d8
    COLOR_READ_MARKER = #xeb2733

    REACTION_TEXT_COLOR = #4c00b0


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
            margin: { bottom: 3, top: 10 }
            replied_to_message_content = {
                margin: { left: 29 }
                padding: { bottom: 10 }
            }
        }

        body = <View> {
            width: Fill,
            height: Fit
            flow: Right,
            padding: {top: 0, bottom: 10, left: 10, right: 10},

            profile = <View> {
                align: {x: 0.5, y: 0.0} // centered horizontally, top aligned
                width: 65.0,
                height: Fit,
                margin: {top: 4.5, right: 10}
                flow: Down,
                avatar = <Avatar> {
                    width: 48,
                    height: 48,
                }
                timestamp = <Timestamp> {
                    margin: { top: 5.9 }
                }
                edited_indicator = <EditedIndicator> { }
                tsp_sign_indicator = <TspSignIndicator> { }
            }
            content = <View> {
                width: Fill,
                height: Fit
                flow: Down,
                padding: 0.0
                username_view = <View> {
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
                link_preview_view = <LinkPreview> {}

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
                edited_indicator = <EditedIndicator> { }
                tsp_sign_indicator = <TspSignIndicator> { }
            }
            content = <View> {
                width: Fill,
                height: Fit,
                flow: Down,
                padding: { left: 10.0 }

                message = <HtmlOrPlaintext> { }
                link_preview_view = <LinkPreview> {}
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
                    margin: {top: 3}
                }
            }

            avatar = <Avatar> {
                width: 19.,
                height: 19.,
                margin: 0

                text_view = { text = { draw_text: {
                    text_style: <TITLE_TEXT>{ font_size: 7.0 }
                }}}
            }

            // Show an invite button only for a `Knocked` room membership change.
            // All other small state events will not show this button.
            invite_user_button = <RobrixIconButton> {
                visible: false
                margin: { top: -1.5, left: 2, right: 2}
                padding: {top: 4, bottom: 4, left: 9, right: 9}
                draw_bg: {
                    color: (COLOR_BG_ACCEPT_GREEN)
                    border_size: 0.75
                    border_color: (COLOR_FG_ACCEPT_GREEN)
                }
                draw_icon: {
                    svg_file: (ICON_ADD_USER)
                    color: (COLOR_FG_ACCEPT_GREEN)
                }
                draw_text: {
                    color: (COLOR_FG_ACCEPT_GREEN)
                    text_style: <SMALL_STATE_TEXT_STYLE> {},
                }
                icon_walk: {width: 15, height: Fit, margin: {right: -4}}
                text: "Invite to Room"
            }

            content = <Label> {
                width: Fill,
                height: Fit
                margin: {top: 2.5}
                padding: { top: 0.0, bottom: 0.0, left: 0.0, right: 0.0 }
                draw_text: {
                    wrap: Word,
                    text_style: <SMALL_STATE_TEXT_STYLE> {},
                    color: (SMALL_STATE_TEXT_COLOR)
                }
                text: ""
            }

            avatar_row = <AvatarRow> {}
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

        left_line = <LineH> { }

        date = <Label> {
            padding: {left: 7.0, right: 7.0}
            draw_text: {
                text_style: <TEXT_SUB> {},
                color: (COLOR_DIVIDER_DARK)
            }
            text: "<date>"
        }

        right_line = <LineH> { }
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

        room_screen_wrapper = <View> {
            width: Fill, height: Fill,
            flow: Overlay,
            show_bg: true
            draw_bg: {
                color: (COLOR_PRIMARY_DARKER)
            }

            restore_status_view = <RestoreStatusView> {}

            // Widgets within this view will get shifted upwards when the on-screen keyboard is shown.
            keyboard_view = <KeyboardView> {
                width: Fill, height: Fill,
                flow: Down,

                // First, display the timeline of all messages/events.
                timeline = <Timeline> {
                    // margin: {bottom: 10}
                }

                // Below that, display a typing notice when other users in the room are typing.
                typing_notice = <TypingNotice> { }

                room_input_bar = <RoomInputBar> {
                    // margin: {top: 20}
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
    }
}

/// The main widget that displays a single Matrix room.
#[derive(Live, Widget)]
pub struct RoomScreen {
    #[deref] view: View,

    /// The name and ID of the currently-shown room, if any.
    #[rust] room_name_id: Option<RoomNameId>,
    /// The persistent UI-relevant states for the room that this widget is currently displaying.
    #[rust] tl_state: Option<TimelineUiState>,
    /// The set of pinned events in this room.
    #[rust] pinned_events: Vec<OwnedEventId>,
    /// Whether this room has been successfully loaded (received from the homeserver).
    #[rust] is_loaded: bool,
    /// Whether or not all rooms have been loaded (received from the homeserver).
    #[rust] all_rooms_loaded: bool,
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
impl LiveHook for RoomScreen {
    fn after_update_from_doc(&mut self, cx: &mut Cx) {
        if let Some(tl_state) = &mut self.tl_state.as_mut() {
            // Clear the timeline's drawn items caches and redraw it.
            tl_state.content_drawn_since_last_update.clear();
            tl_state.profile_drawn_since_last_update.clear();
            self.view.redraw(cx);
        }
    }
}

impl Widget for RoomScreen {
    // Handle events and actions for the RoomScreen widget and its inner Timeline view.
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        let room_screen_widget_uid = self.widget_uid();
        let portal_list = self.portal_list(ids!(timeline.list));
        let user_profile_sliding_pane = self.user_profile_sliding_pane(ids!(user_profile_sliding_pane));
        let loading_pane = self.loading_pane(ids!(loading_pane));

        // Handle actions here before processing timeline updates.
        // Normally (in most other widgets), the order of event handling doesn't matter much.
        // However, since actions may refer to a specific timeline item's index,
        // we want to handle those before processing any updates that might change
        // the set of timeline indices (which would invalidate the index values in any actions).
        if let Event::Actions(actions) = event {
            for (index, wr) in portal_list.items_with_actions(actions) {
                // Handle a hover-in action on the reaction list: show a reaction summary.
                let reaction_list = wr.reaction_list(ids!(reaction_list));
                if let RoomScreenTooltipActions::HoverInReactionButton {
                    widget_rect,
                    reaction_data,
                } = reaction_list.hovered_in(actions) {
                    let Some(_tl_state) = self.tl_state.as_ref() else { continue };
                    let tooltip_text_arr: Vec<String> = reaction_data.reaction_senders.iter().map(|(sender, _react_info)| {
                        user_profile_cache::get_user_profile_and_room_member(cx, sender.clone(), &reaction_data.room_id, true).0
                            .map(|user_profile| user_profile.displayable_name().to_string())
                            .unwrap_or_else(|| sender.to_string())
                    }).collect();
                    let mut tooltip_text = utils::human_readable_list(&tooltip_text_arr, MAX_VISIBLE_AVATARS_IN_READ_RECEIPT);
                    tooltip_text.push_str(&format!(" reacted with: {}", reaction_data.reaction));
                    cx.widget_action(
                        room_screen_widget_uid,
                        &scope.path,
                        TooltipAction::HoverIn {
                            text: tooltip_text,
                            widget_rect,
                            options: CalloutTooltipOptions {
                                position: TooltipPosition::Bottom,
                                ..Default::default()
                            },
                        },
                    );
                }

                // Handle a hover-out action on the reaction list or avatar row.
                let avatar_row_ref = wr.avatar_row(ids!(avatar_row));
                if reaction_list.hovered_out(actions)
                    || avatar_row_ref.hover_out(actions)
                {
                    cx.widget_action(
                        room_screen_widget_uid,
                        &scope.path,
                        TooltipAction::HoverOut,
                    );
                }

                // Handle a hover-in action on the avatar row: show a read receipts summary.
                if let RoomScreenTooltipActions::HoverInReadReceipt {
                    widget_rect,
                    read_receipts
                } = avatar_row_ref.hover_in(actions) {
                    let Some(room_id) = self.room_id() else { return; };
                    let tooltip_text= room_read_receipt::populate_tooltip(cx, read_receipts, room_id);
                    cx.widget_action(
                        room_screen_widget_uid,
                        &scope.path,
                        TooltipAction::HoverIn {
                            text: tooltip_text,
                            widget_rect,
                            options: CalloutTooltipOptions {
                                position: TooltipPosition::Left,
                                ..Default::default()
                            },
                        },
                    );
                }

                // Handle an image within the message being clicked.
                let content_message = wr.text_or_image(ids!(content.message));
                if let TextOrImageAction::Clicked(mxc_uri) = actions.find_widget_action(content_message.widget_uid()).cast() {
                    let texture = content_message.get_texture(cx);
                    self.handle_image_click(
                        cx,
                        mxc_uri,
                        texture,
                        index,
                    );
                    continue;
                }

                // Handle the invite_user_button (in a SmallStateEvent) being clicked.
                if wr.button(ids!(invite_user_button)).clicked(actions) {
                    let Some(tl) = self.tl_state.as_ref() else { continue };
                    if let Some(event_tl_item) = tl.items.get(index).and_then(|item| item.as_event()) {
                        log!("invite_user_button clicked: index {index}, details: {:?}", event_tl_item);
                        let user_id = event_tl_item.sender().to_owned();
                        let username = if let TimelineDetails::Ready(profile) = event_tl_item.sender_profile() {
                            profile.display_name.as_deref().unwrap_or(user_id.as_str())
                        } else {
                            user_id.as_str()
                        };
                        let room_id = tl.room_id.clone();
                        let content = ConfirmationModalContent {
                            title_text: "Send Invitation".into(),
                            body_text: format!("Are you sure you want to invite {username} to this room?").into(),
                            accept_button_text: Some("Invite".into()),
                            on_accept_clicked: Some(Box::new(move |_cx| {
                                submit_async_request(MatrixRequest::InviteUser { room_id, user_id });
                            })),
                            ..Default::default()
                        };
                        cx.action(InviteAction::ShowConfirmationModal(RefCell::new(Some(content))));
                    }
                }
            }

            self.handle_message_actions(cx, actions, &portal_list, &loading_pane);

            for action in actions {
                // Handle actions related to restoring the previously-saved state of rooms.
                if let Some(AppStateAction::RoomLoadedSuccessfully { room_name_id, ..}) = action.downcast_ref() {
                    if self.room_name_id.as_ref().is_some_and(|rn| rn.room_id() == room_name_id.room_id()) {
                        // `set_displayed_room()` does nothing if the room_name_id is unchanged, so we clear it first.
                        self.room_name_id = None;
                        self.set_displayed_room(cx, room_name_id);
                        return;
                    }
                }

                // Handle the highlight animation for a message.
                let Some(tl) = self.tl_state.as_mut() else { continue };
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
                // TODO: move this into the `actions_generated_within_this_room_screen.retain(...)` code block,
                //       where we won't need to bother checking if the room ID is the same as this RoomScreen,
                //       because that block guarantees that it came from this RoomScreen.
                if let ShowUserProfileAction::ShowUserProfile(profile_and_room_id) = action.as_widget_action().cast() {
                    // Only show the user profile in room that this avatar belongs to
                    if self.room_name_id.as_ref().is_some_and(|rn| rn.room_id() == &profile_and_room_id.room_id) {
                        self.show_user_profile(
                            cx,
                            &user_profile_sliding_pane,
                            UserProfilePaneInfo {
                                profile_and_room_id,
                                room_name: self.room_name_id.as_ref().map_or_else(
                                    || UNNAMED_ROOM.to_string(),
                                    |r| r.to_string(),
                                ),
                                room_member: None,
                            },
                        );
                    }
                }
            }

            /*
            // close message action bar if scrolled.
            if portal_list.scrolled(actions) {
                let message_action_bar_popup = self.popup_notification(ids!(message_action_bar_popup));
                message_action_bar_popup.close(cx);
            }
            */

            // Set visibility of loading message banner based of pagination logic
            self.send_pagination_request_based_on_scroll_pos(cx, actions, &portal_list);
            // Handle sending any read receipts for the current logged-in user.
            self.send_user_read_receipts_based_on_scroll_pos(cx, actions, &portal_list);

            // Handle the jump to bottom button: update its visibility, and handle clicks.
            self.jump_to_bottom_button(ids!(jump_to_bottom)).update_from_actions(
                cx,
                &portal_list,
                actions,
            );
        }

        // Currently, a Signal event is only used to tell this widget:
        // 1. to check if the room has been loaded from the homeserver yet, or
        // 2. that its timeline events have been updated in the background.
        if let Event::Signal = event {
            if let (false, Some(room_name_id), true) = (self.is_loaded, self.room_name_id.as_ref(), cx.has_global::<RoomsListRef>()) {
                let rooms_list_ref = cx.get_global::<RoomsListRef>();
                if rooms_list_ref.is_room_loaded(room_name_id.room_id()) {
                    let room_name_clone = room_name_id.clone();
                    // This room has been loaded now, so we call `set_displayed_room()`.
                    // We first clear the `room_name_id`, otherwise that function will do nothing.
                    self.room_name_id = None;
                    self.set_displayed_room(cx, &room_name_clone);
                } else {
                    self.all_rooms_loaded = rooms_list_ref.all_rooms_loaded();
                    return;
                }
            }

            self.process_timeline_updates(cx, &portal_list);

            // Ideally we would do this elsewhere on the main thread, because it's not room-specific,
            // but it doesn't hurt to do it here.
            // TODO: move this up a layer to something higher in the UI tree,
            //       and wrap it in a `if let Event::Signal` conditional.
            user_profile_cache::process_user_profile_updates(cx);
            avatar_cache::process_avatar_updates(cx);
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
            if is_interactive_hit {
                loading_pane.handle_event(cx, event, scope);
            }
        }
        else if user_profile_sliding_pane.is_currently_shown(cx) {
            is_pane_shown = true;
            if is_interactive_hit {
                user_profile_sliding_pane.handle_event(cx, event, scope);
            }
        }
        else {
            is_pane_shown = false;
        }

        // TODO: once we use the `hits()` API, should be able to remove the above conditionals
        //       about whether the loading pane or user profile pane are shown, because
        //       Makepad already delivers most events to all views regardless of visibility,
        //       so the only thing we'd need here is the conditional below.

        if !is_pane_shown || !is_interactive_hit {
            // Create a Scope with RoomScreenProps containing the room members.
            // This scope is needed by child widgets like MentionableTextInput during event handling.
            let room_props = if let Some(tl) = self.tl_state.as_ref() {
                let room_id = tl.room_id.clone();
                let room_members = tl.room_members.clone();

                // Fetch room data once to avoid duplicate expensive lookups
                let (room_display_name, room_avatar_url) = get_client()
                    .and_then(|client| client.get_room(&room_id))
                    .map(|room| (
                        room.cached_display_name().unwrap_or(RoomDisplayName::Empty),
                        room.avatar_url()
                    ))
                    .unwrap_or((RoomDisplayName::Empty, None));

                RoomScreenProps {
                    room_screen_widget_uid,
                    room_name_id: RoomNameId::new(room_display_name, room_id),
                    room_members,
                    room_avatar_url,
                }
            } else if let Some(room_name) = &self.room_name_id {
                // Fallback case: we have a room_name but no tl_state yet
                RoomScreenProps {
                    room_screen_widget_uid,
                    room_name_id: room_name.clone(),
                    room_members: None,
                    room_avatar_url: None,
                }
            } else {
                // No room selected yet, skip event handling that requires room context
                log!("RoomScreen handling event with no room_name_id and no tl_state, skipping room-dependent event handling");
                if !is_pane_shown || !is_interactive_hit {
                    return;
                }
                // Use a dummy room props for non-room-specific events
                RoomScreenProps {
                    room_screen_widget_uid,
                    room_name_id: RoomNameId::new(
                        RoomDisplayName::Empty,
                        matrix_sdk::ruma::OwnedRoomId::try_from("!dummy:matrix.org").unwrap(),
                    ),
                    room_members: None,
                    room_avatar_url: None,
                }
            };
            let mut room_scope = Scope::with_props(&room_props);


            // Forward the event to the inner timeline view, but capture any actions it produces
            // such that we can handle the ones relevant to only THIS RoomScreen widget right here and now,
            // ensuring they are not mistakenly handled by other RoomScreen widget instances.
            let mut actions_generated_within_this_room_screen = cx.capture_actions(|cx|
                self.view.handle_event(cx, event, &mut room_scope)
            );
            // Here, we handle and remove any general actions that are relevant to only this RoomScreen.
            // Removing the handled actions ensures they are not mistakenly handled by other RoomScreen widget instances.
            actions_generated_within_this_room_screen.retain(|action| {
                if self.handle_link_clicked(cx, action, &user_profile_sliding_pane) {
                    return false;
                }

                /*
                match action.as_widget_action().widget_uid_eq(room_screen_widget_uid).cast() {
                    MessageAction::ActionBarClose => {
                        let message_action_bar_popup = self.popup_notification(ids!(message_action_bar_popup));
                        let message_action_bar = message_action_bar_popup.message_action_bar(ids!(message_action_bar));

                        // close only if the active message is requesting it to avoid double closes.
                        if let Some(message_widget_uid) = message_action_bar.message_widget_uid() {
                            if action.as_widget_action().widget_uid_eq(message_widget_uid).is_some() {
                                message_action_bar_popup.close(cx);
                            }
                        }
                    }
                    MessageAction::ActionBarOpen { item_id, message_rect } => {
                        let message_action_bar_popup = self.popup_notification(ids!(message_action_bar_popup));
                        let message_action_bar = message_action_bar_popup.message_action_bar(ids!(message_action_bar));

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
        // If the room isn't loaded yet, we show the restore status label only.
        if !self.is_loaded {
            let Some(room_name) = &self.room_name_id else {
                // No room selected yet, nothing to show.
                return DrawStep::done();
            };
            let mut restore_status_view = self.view.restore_status_view(ids!(restore_status_view));
            restore_status_view.set_content(cx, self.all_rooms_loaded, room_name);
            return restore_status_view.draw(cx, scope);
        }
        if self.tl_state.is_none() {
            // Tl_state may not be ready after dock loading.
            // If return DrawStep::done() inside self.view.draw_walk, turtle will misalign and panic.
            return DrawStep::done();
        }


        let room_screen_widget_uid = self.widget_uid();
        while let Some(subview) = self.view.draw_walk(cx, scope, walk).step() {
            // Here, we only need to handle drawing the portal list.
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
                        list.item(cx, item_id, id!(Empty));
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
                            TimelineItemContent::MsgLike(msg_like_content) => match &msg_like_content.kind {
                                MsgLikeKind::Message(_) | MsgLikeKind::Sticker(_) => {
                                    let prev_event = tl_idx.checked_sub(1).and_then(|i| tl_items.get(i));
                                    populate_message_view(
                                        cx,
                                        list,
                                        item_id,
                                        room_id,
                                        event_tl_item,
                                        msg_like_content,
                                        prev_event,
                                        &mut tl_state.media_cache,
                                        &mut tl_state.link_preview_cache,
                                        &tl_state.user_power,
                                        &self.pinned_events,
                                        item_drawn_status,
                                        room_screen_widget_uid,
                                    )
                                },
                                // TODO: properly implement `Poll` as a regular Message-like timeline item.
                                MsgLikeKind::Poll(poll_state) => populate_small_state_event(
                                    cx,
                                    list,
                                    item_id,
                                    room_id,
                                    event_tl_item,
                                    poll_state,
                                    item_drawn_status,
                                ),
                                MsgLikeKind::Redacted => populate_small_state_event(
                                    cx,
                                    list,
                                    item_id,
                                    room_id,
                                    event_tl_item,
                                    &RedactedMessageEventMarker,
                                    item_drawn_status,
                                ),
                                MsgLikeKind::UnableToDecrypt(utd) => populate_small_state_event(
                                    cx,
                                    list,
                                    item_id,
                                    room_id,
                                    event_tl_item,
                                    utd,
                                    item_drawn_status,
                                ),
                                MsgLikeKind::Other(other) => populate_small_state_event(
                                    cx,
                                    list,
                                    item_id,
                                    room_id,
                                    event_tl_item,
                                    other,
                                    item_drawn_status,
                                ),
                            },
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
                                let item = list.item(cx, item_id, id!(SmallStateEvent));
                                item.label(ids!(content)).set_text(cx, &format!("[Unsupported] {:?}", unhandled));
                                (item, ItemDrawnStatus::both_drawn())
                            }
                        }
                        TimelineItemKind::Virtual(VirtualTimelineItem::DateDivider(millis)) => {
                            let item = list.item(cx, item_id, id!(DateDivider));
                            let text = unix_time_millis_to_datetime(*millis)
                                // format the time as a shortened date (Sat, Sept 5, 2021)
                                .map(|dt| format!("{}", dt.date_naive().format("%a %b %-d, %Y")))
                                .unwrap_or_else(|| format!("{:?}", millis));
                            item.label(ids!(date)).set_text(cx, &text);
                            (item, ItemDrawnStatus::both_drawn())
                        }
                        TimelineItemKind::Virtual(VirtualTimelineItem::ReadMarker) => {
                            let item = list.item(cx, item_id, id!(ReadMarker));
                            (item, ItemDrawnStatus::both_drawn())
                        }
                        TimelineItemKind::Virtual(VirtualTimelineItem::TimelineStart) => {
                            let item = list.item(cx, item_id, id!(Empty));
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
                item.draw_all(cx, scope);
            }

            // If the list is not filling the viewport, we need to back paginate the timeline
            // until we have enough events items to fill the viewport.
            if !tl_state.fully_paginated && !list.is_filling_viewport() {
                log!("Automatically paginating timeline to fill viewport for room {:?}", self.room_name_id);
                submit_async_request(MatrixRequest::PaginateRoomTimeline {
                    room_id: room_id.clone(),
                    num_events: 50,
                    direction: PaginationDirection::Backwards,
                });
            }
        }
        DrawStep::done()
    }
}

impl RoomScreen {
    fn room_id(&self) -> Option<&OwnedRoomId> {
        self.room_name_id.as_ref().map(|r| r.room_id())
    }

    /// Processes all pending background updates to the currently-shown timeline.
    ///
    /// Redraws this RoomScreen view if any updates were applied.
    fn process_timeline_updates(&mut self, cx: &mut Cx, portal_list: &PortalListRef) {
        let top_space = self.view(ids!(top_space));
        let jump_to_bottom = self.jump_to_bottom_button(ids!(jump_to_bottom));
        let curr_first_id = portal_list.first_id();
        let ui = self.widget_uid();
        let Some(tl) = self.tl_state.as_mut() else { return };

        let mut done_loading = false;
        let mut should_continue_backwards_pagination = false;
        let mut typing_users = None;
        let mut num_updates = 0;
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
                            log!("process_timeline_updates(): timeline (had {} items) was cleared for room {}", tl.items.len(), tl.room_id);
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

                    let prior_items_changed = clear_cache || changed_indices.start <= curr_first_id;

                    if new_items.len() == tl.items.len() {
                        // log!("process_timeline_updates(): no jump necessary for updated timeline of same length: {}", items.len());
                    }
                    else if curr_first_id > new_items.len() {
                        log!("process_timeline_updates(): jumping to bottom: curr_first_id {} is out of bounds for {} new items", curr_first_id, new_items.len());
                        portal_list.set_first_id_and_scroll(new_items.len().saturating_sub(1), 0.0);
                        portal_list.set_tail_range(true);
                        jump_to_bottom.update_visibility(cx, true);
                    }
                    // If the prior items changed, we need to find the new index of an item that was visible
                    // in the timeline viewport so that we can maintain the scroll position of that item,
                    // which ensures that the timeline doesn't jump around unexpectedly and ruin the user's experience.
                    else if let Some((curr_item_idx, new_item_idx, new_item_scroll, _event_id)) =
                        prior_items_changed.then(||
                            find_new_item_matching_current_item(cx, portal_list, curr_first_id, &tl.items, &new_items)
                        )
                        .flatten()
                    {
                        if curr_item_idx != new_item_idx {
                            log!("process_timeline_updates(): jumping view from event index {curr_item_idx} to new index {new_item_idx}, scroll {new_item_scroll}, event ID {_event_id}");
                            portal_list.set_first_id_and_scroll(new_item_idx, new_item_scroll);
                            tl.prev_first_index = Some(new_item_idx);
                            // Set scrolled_past_read_marker false when we jump to a new event
                            tl.scrolled_past_read_marker = false;
                            // Hide the tooltip when the timeline jumps, as a hover-out event won't occur.
                            cx.widget_action(ui, &HeapLiveIdPath::default(), RoomScreenTooltipActions::HoverOut);
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
                        // Immediately show the unread badge with no count while we fetch the actual count in the background.
                        jump_to_bottom.show_unread_message_badge(cx, UnreadMessageCount::Unknown);
                        submit_async_request(MatrixRequest::GetNumberUnreadMessages{ room_id: tl.room_id.clone() });
                    }

                    if prior_items_changed {
                        // If this RoomScreen is showing the loading pane and has an ongoing backwards pagination request,
                        // then we should update the status message in that loading pane
                        // and then continue paginating backwards until we find the target event.
                        // Note that we do this here because `clear_cache` will always be true if backwards pagination occurred.
                        let loading_pane = self.view.loading_pane(ids!(loading_pane));
                        let mut loading_pane_state = loading_pane.take_state();
                        if let LoadingPaneState::BackwardsPaginateUntilEvent {
                            events_paginated, target_event_id, ..
                        } = &mut loading_pane_state {
                            *events_paginated += new_items.len().saturating_sub(tl.items.len());
                            log!("While finding target event {target_event_id}, we have now loaded {events_paginated} messages...");
                            // Here, we assume that we have not yet found the target event,
                            // so we need to continue paginating backwards.
                            // If the target event has already been found, it will be handled
                            // in the `TargetEventFound` match arm below, which will set
                            // `should_continue_backwards_pagination` to `false`.
                            // So either way, it's okay to set this to `true` here.
                            should_continue_backwards_pagination = true;
                        }
                        loading_pane.set_state(cx, loading_pane_state);
                    }

                    if clear_cache {
                        tl.content_drawn_since_last_update.clear();
                        tl.profile_drawn_since_last_update.clear();
                        tl.fully_paginated = false;
                    } else {
                        tl.content_drawn_since_last_update.remove(changed_indices.clone());
                        tl.profile_drawn_since_last_update.remove(changed_indices.clone());
                        // log!("process_timeline_updates(): changed_indices: {changed_indices:?}, items len: {}\ncontent drawn: {:#?}\nprofile drawn: {:#?}", items.len(), tl.content_drawn_since_last_update, tl.profile_drawn_since_last_update);
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
                    let loading_pane = self.view.loading_pane(ids!(loading_pane));

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
                    error!("Pagination error ({direction}) in {:?}: {error:?}", self.room_name_id);
                    let room_name = self.room_name_id.as_ref().map(|r| r.to_string());
                    enqueue_popup_notification(PopupItem {
                        message: utils::stringify_pagination_error(&error, room_name.as_deref().unwrap_or(UNNAMED_ROOM)),
                        auto_dismissal_duration: None,
                        kind: PopupKind::Error,
                    });
                    done_loading = true;
                }
                TimelineUpdate::PaginationIdle { fully_paginated, direction } => {
                    if direction == PaginationDirection::Backwards {
                        // Don't set `done_loading` to `true` here, because we want to keep the top space visible
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
                    // log!("process_timeline_updates(): room members fetched for room {}", tl.room_id);
                    // Here, to be most efficient, we could redraw only the user avatars and names in the timeline,
                    // but for now we just fall through and let the final `redraw()` call re-draw the whole timeline view.
                }
                TimelineUpdate::RoomMembersListFetched { members } => {
                    // Store room members directly in TimelineUiState
                    tl.room_members = Some(Arc::new(members));
                },
                TimelineUpdate::MediaFetched(request) => {
                    log!("process_timeline_updates(): media fetched for room {}", tl.room_id);
                    // Set Image to image viewer modal if the media is not a thumbnail.
                    if let (MediaFormat::File, media_source) = (request.format, request.source) {
                        populate_matrix_image_modal(cx, media_source, &mut tl.media_cache);
                    }
                    // Here, to be most efficient, we could redraw only the media items in the timeline,
                    // but for now we just fall through and let the final `redraw()` call re-draw the whole timeline view.
                }
                TimelineUpdate::MessageEdited { timeline_event_id, result } => {
                    self.view.room_input_bar(ids!(room_input_bar))
                        .handle_edit_result(cx, timeline_event_id, result);
                }
                TimelineUpdate::PinResult { result, pin, .. } => {
                    let (message, auto_dismissal_duration, kind) = match &result {
                        Ok(true) => (
                            format!("Successfully {} event.", if pin { "pinned" } else { "unpinned" }),
                            Some(4.0),
                            PopupKind::Success
                        ),
                        Ok(false) => (
                            format!("Message was already {}.", if pin { "pinned" } else { "unpinned" }),
                            Some(4.0),
                            PopupKind::Info
                        ),
                        Err(e) => (
                            format!("Failed to {} event. Error: {e}", if pin { "pin" } else { "unpin" }),
                            None,
                            PopupKind::Error
                        ),
                    };
                    enqueue_popup_notification(PopupItem { message, auto_dismissal_duration, kind, });
                }
                TimelineUpdate::TypingUsers { users } => {
                    // This update loop should be kept tight & fast, so all we do here is
                    // save the list of typing users for future use after the loop exits.
                    // Then, we "process" it later (by turning it into a string) after the
                    // update loop has completed, which avoids unnecessary expensive work
                    // if the list of typing users gets updated many times in a row.
                    typing_users = Some(users);
                }
                TimelineUpdate::PinnedEvents(pinned_events) => {
                    self.pinned_events = pinned_events;
                    // We need to redraw any events that might have been pinned or unpinned
                    // in order to have all events properly reflect their pinned state.
                    // However, it's intractable to find exactly which events in the timeline
                    // had a change in their pinned state, so we just clear all draw caches.
                    tl.content_drawn_since_last_update.clear();
                    tl.profile_drawn_since_last_update.clear();
                }
                TimelineUpdate::UserPowerLevels(user_power_levels) => {
                    tl.user_power = user_power_levels;
                    self.view.room_input_bar(ids!(room_input_bar))
                        .update_user_power_levels(cx, user_power_levels);
                    // Update the @room mention capability based on the user's power level
                    cx.action(MentionableTextInputAction::PowerLevelsUpdated {
                        room_id: tl.room_id.clone(),
                        can_notify_room: user_power_levels.can_notify_room(),
                    });
                    // We need to redraw all events in order to reflect the new power levels,
                    // e.g., for the message context menu to be correctly populated.
                    tl.content_drawn_since_last_update.clear();
                    tl.profile_drawn_since_last_update.clear();
                }
                TimelineUpdate::OwnUserReadReceipt(receipt) => {
                    tl.latest_own_user_receipt = Some(receipt);
                }
                TimelineUpdate::Tombstoned(successor_room_details) => {
                    self.view.room_input_bar(ids!(room_input_bar))
                        .update_tombstone_footer(cx, &tl.room_id, Some(&successor_room_details));
                    tl.tombstone_info = Some(successor_room_details);
                }
                TimelineUpdate::LinkPreviewFetched => {}
                TimelineUpdate::InviteSent { result, .. } => {
                    match result {
                        Ok(_) => enqueue_popup_notification(PopupItem {
                            message: "Sent invite successfully.".to_string(),
                            auto_dismissal_duration: Some(4.0),
                            kind: PopupKind::Success,
                        }),
                        Err(e) => enqueue_popup_notification(PopupItem {
                            message: format!("Failed to send invite.\n\nError: {e}"),
                            auto_dismissal_duration: None,
                            kind: PopupKind::Error,
                        }),
                    }
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

        if let Some(users) = typing_users {
            self.view
                .typing_notice(ids!(typing_notice))
                .show_or_hide(cx, &users);
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
                    let Some(room_name_id) = self.room_name_id.as_ref() else {
                        return false;
                    };
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
                                room_id: room_name_id.room_id().clone(),
                            },
                            room_name: room_name_id.to_string(),
                            // TODO: use the extra `via` parameters
                            room_member: None,
                        },
                    );
                    true
                }
                MatrixId::Room(room_id) => {
                    if self.room_name_id.as_ref().is_some_and(|r| r.room_id() == room_id) {
                        enqueue_popup_notification(PopupItem {
                            message: "You are already viewing that room.".into(),
                            kind: PopupKind::Error,
                            auto_dismissal_duration: None
                        });
                        return true;
                    }
                    if let Some(room_name_id) = cx.get_global::<RoomsListRef>().get_room_name(room_id) {
                        cx.action(AppStateAction::NavigateToRoom {
                            room_to_close: None,
                            destination_room: BasicRoomDetails::Name(room_name_id),
                        });
                        return true;
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
                        kind: PopupKind::Error,
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
                        kind: PopupKind::Error,
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

    /// Handles image clicks in message content by opening the image viewer.
    fn handle_image_click(
        &mut self,
        cx: &mut Cx,
        mxc_uri: Option<MediaSource>,
        texture: Option<Texture>,
        item_id: usize,
    ) {
        let Some(media_source) = mxc_uri else {
            return;
        };
        let Some(tl_state) = self.tl_state.as_mut() else { return };
        let Some(event_tl_item) = tl_state.items.get(item_id).and_then(|item| item.as_event()) else { return };

        let timestamp_millis = event_tl_item.timestamp();
        let (image_name, image_file_size) = get_image_name_and_filesize(event_tl_item);
        cx.action(ImageViewerAction::Show(LoadState::Loading(
            texture.clone(),
            Some(ImageViewerMetaData {
                image_name,
                image_file_size,
                timestamp: unix_time_millis_to_datetime(timestamp_millis),
                avatar_parameter: Some((
                    tl_state.room_id.clone(),
                    event_tl_item.clone(),
                )),
            }),
        )));

        populate_matrix_image_modal(cx, media_source, &mut tl_state.media_cache);
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
                    let Some(tl) = self.tl_state.as_ref() else { return };
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
                            kind: PopupKind::Error,
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
                    let Some(tl) = self.tl_state.as_ref() else { return };
                    if let Some(event_tl_item) = tl.items.get(details.item_id)
                        .and_then(|tl_item| tl_item.as_event().cloned())
                        .filter(|ev| ev.event_id() == details.event_id.as_deref())
                    {
                        let replied_to_info = EmbeddedEvent::from_timeline_item(&event_tl_item);
                        self.view.room_input_bar(ids!(room_input_bar))
                            .show_replying_to(cx, (event_tl_item, replied_to_info), &tl.room_id);
                    }
                    else {
                        enqueue_popup_notification(PopupItem { message: "Could not find message in timeline to reply to. Please try again!".to_string(), kind: PopupKind::Error, auto_dismissal_duration: None });
                        error!("MessageAction::Reply: couldn't find event [{}] {:?} to reply to in room {:?}",
                            details.item_id,
                            details.event_id.as_deref(),
                            self.room_id(),
                        );
                    }
                }
                MessageAction::Edit(details) => {
                    let Some(tl) = self.tl_state.as_ref() else { return };
                    if let Some(event_tl_item) = tl.items.get(details.item_id)
                        .and_then(|tl_item| tl_item.as_event().cloned())
                        .filter(|ev| ev.event_id() == details.event_id.as_deref())
                    {
                        self.view.room_input_bar(ids!(room_input_bar))
                            .show_editing_pane(cx, event_tl_item, tl.room_id.clone());
                    }
                    else {
                        enqueue_popup_notification(PopupItem { message: "Could not find message in timeline to edit. Please try again!".to_string(), kind: PopupKind::Error, auto_dismissal_duration: None });
                        error!("MessageAction::Edit: couldn't find event [{}] {:?} to edit in room {:?}",
                            details.item_id,
                            details.event_id.as_deref(),
                            self.room_id(),
                        );
                    }
                }
                MessageAction::EditLatest => {
                    let Some(tl) = self.tl_state.as_ref() else { return };
                    if let Some(latest_sent_msg) = tl.items
                        .iter()
                        .rev()
                        .take(MAX_ITEMS_TO_SEARCH_THROUGH)
                        .find_map(|item| item.as_event().filter(|ev| ev.is_editable()).cloned())
                    {
                        self.view.room_input_bar(ids!(room_input_bar))
                            .show_editing_pane(cx, latest_sent_msg, tl.room_id.clone());
                    }
                    else {
                        enqueue_popup_notification(PopupItem {
                            message: "No recent message available to edit.".to_string(),
                            kind: PopupKind::Warning,
                            auto_dismissal_duration: Some(3.0),
                        });
                    }
                }
                MessageAction::Pin(details) => {
                    let Some(tl) = self.tl_state.as_ref() else { return };
                    if let Some(event_id) = details.event_id {
                        submit_async_request(MatrixRequest::PinEvent {
                            event_id,
                            room_id: tl.room_id.clone(),
                            pin: true,
                        });
                    } else {
                        enqueue_popup_notification(PopupItem {
                            message: String::from("This event cannot be pinned."),
                            auto_dismissal_duration: None,
                            kind: PopupKind::Error,
                        });
                    }
                }
                MessageAction::Unpin(details) => {
                    let Some(tl) = self.tl_state.as_ref() else { return };
                    if let Some(event_id) = details.event_id {
                        submit_async_request(MatrixRequest::PinEvent {
                            event_id,
                            room_id: tl.room_id.clone(),
                            pin: false,
                        });
                    } else {
                        enqueue_popup_notification(PopupItem {
                            message: String::from("This event cannot be unpinned."),
                            auto_dismissal_duration: None,
                            kind: PopupKind::Error,
                        });
                    }
                }
                MessageAction::CopyText(details) => {
                    let Some(tl) = self.tl_state.as_ref() else { return };
                    if let Some(text) = tl.items
                        .get(details.item_id)
                        .and_then(|tl_item| tl_item.as_event().map(plaintext_body_of_timeline_item))
                    {
                        cx.copy_to_clipboard(&text);
                    }
                    else {
                        enqueue_popup_notification(PopupItem { message: "Could not find message in timeline to copy text from. Please try again!".to_string(), kind: PopupKind::Error, auto_dismissal_duration: None});
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
                        if let Some(message) = event_tl_item.content().as_message() {
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
                        enqueue_popup_notification(PopupItem { message: "Could not find message in timeline to copy HTML from. Please try again!".to_string(), kind: PopupKind::Error, auto_dismissal_duration: None });
                        error!("MessageAction::CopyHtml: couldn't find event [{}] {:?} to copy HTML from in room {}",
                            details.item_id,
                            details.event_id.as_deref(),
                            tl.room_id,
                        );
                    }
                }
                MessageAction::CopyLink(details) => {
                    let Some(tl) = self.tl_state.as_ref() else { return };
                    if let Some(event_id) = details.event_id {
                        let matrix_to_uri = tl.room_id.matrix_to_event_uri(event_id);
                        cx.copy_to_clipboard(&matrix_to_uri.to_string());
                    } else {
                        enqueue_popup_notification(PopupItem { message: "Couldn't create permalink to message.".to_string(), kind: PopupKind::Error, auto_dismissal_duration: None });
                        error!("MessageAction::CopyLink: no `event_id`: [{}] {:?} in room {}",
                            details.item_id,
                            details.event_id.as_deref(),
                            tl.room_id,
                        );
                    }
                }
                MessageAction::ViewSource(_details) => {
                    enqueue_popup_notification(PopupItem { message: "Viewing an event's source is not yet implemented.".to_string(), kind: PopupKind::Error, auto_dismissal_duration: None });
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
                    let Some(related_event_id) = details.related_event_id.as_ref() else {
                        error!("BUG: MessageAction::JumpToRelated had no related event ID.\n{details:#?}");
                        continue;
                    };
                    self.jump_to_event(
                        cx,
                        related_event_id,
                        Some(details.item_id),
                        portal_list,
                        loading_pane
                    );
                }
                MessageAction::JumpToEvent(event_id) => {
                    self.jump_to_event(
                        cx,
                        &event_id,
                        None,
                        portal_list,
                        loading_pane
                    );
                }
                MessageAction::Redact { details, reason } => {
                    let Some(tl) = self.tl_state.as_ref() else { return };
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
                        enqueue_popup_notification(PopupItem { message: "Couldn't find message in timeline to delete.".to_string(), kind: PopupKind::Error, auto_dismissal_duration: None });
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

    /// Jumps to the target event ID in this timeline by smooth scrolling to it.
    ///
    /// This function searches backwards from the given `max_tl_idx` in the timeline
    /// for the given `event_id`. If found, it smooth-scrolls the portal list to that event.
    /// If not found, it displays the loading pane and starts a background search for the event.
    fn jump_to_event(
        &mut self,
        cx: &mut Cx,
        target_event_id: &OwnedEventId,
        max_tl_idx: Option<usize>,
        portal_list: &PortalListRef,
        loading_pane: &LoadingPaneRef,
    ) {
        let Some(tl) = self.tl_state.as_mut() else { return };
        let max_tl_idx = max_tl_idx.unwrap_or_else(|| tl.items.len());

        // Attempt to find the index of replied-to message in the timeline.
        // Start from the current item's index (`tl_idx`) and search backwards,
        // since we know the related message must come before the current item.
        let mut num_items_searched = 0;
        let related_msg_tl_index = tl.items
            .focus()
            .narrow(..max_tl_idx)
            .into_iter()
            .rev()
            .take(MAX_ITEMS_TO_SEARCH_THROUGH)
            .position(|i| {
                num_items_searched += 1;
                i.as_event()
                    .and_then(|e| e.event_id())
                    .is_some_and(|ev_id| ev_id == target_event_id)
            })
            .map(|position| max_tl_idx.saturating_sub(position).saturating_sub(1));

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
            log!("The related event {target_event_id} wasn't immediately available in room {}, searching for it in the background...", tl.room_id);
            // Here, we set the state of the loading pane and display it to the user.
            // The main logic will be handled in `process_timeline_updates()`, which is the only
            // place where we can receive updates to the timeline from the background tasks.
            loading_pane.set_state(
                cx,
                LoadingPaneState::BackwardsPaginateUntilEvent {
                    target_event_id: target_event_id.clone(),
                    events_paginated: 0,
                    request_sender: tl.request_sender.clone(),
                },
            );
            loading_pane.show(cx);

            tl.request_sender.send_if_modified(|requests| {
                if let Some(existing) = requests.iter_mut().find(|r| r.room_id == tl.room_id) {
                    warning!("Unexpected: room {} already had an existing timeline request in progress, event: {:?}", tl.room_id, existing.target_event_id);
                    // We might as well re-use this existing request...
                    existing.target_event_id = target_event_id.clone();
                } else {
                    requests.push(BackwardsPaginateUntilEventRequest {
                        room_id: tl.room_id.clone(),
                        target_event_id: target_event_id.clone(),
                        // avoid re-searching through items we already searched through.
                        starting_index: max_tl_idx.saturating_sub(num_items_searched),
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

    /// Invoke this when this timeline is being shown,
    /// e.g., when the user navigates to this timeline.
    fn show_timeline(&mut self, cx: &mut Cx) {
        let room_id = self
            .room_id()
            .expect("BUG: Timeline::show_timeline(): no room_name was set.")
            .clone();

        let state_opt = TIMELINE_STATES.with_borrow_mut(|ts| ts.remove(&room_id));
        let (mut tl_state, mut is_first_time_being_loaded) = if let Some(existing) = state_opt {
            (existing, false)
        } else {
            let Some(timeline_endpoints) = take_timeline_endpoints(&room_id) else {
                if !self.is_loaded && self.all_rooms_loaded {
                    panic!("BUG: timeline is not loaded, but room_id {:?} \
                    was not waiting for its timeline to be loaded.", room_id);
                }
                return;
            };
            let TimelineEndpoints {
                update_receiver,
                update_sender,
                request_sender,
                successor_room,
            } = timeline_endpoints;

            // Start with the basic tombstone info, and fetch the full details
            // if the room has been tombstoned.
            let tombstone_info = if let Some(sr) = successor_room {
                submit_async_request(MatrixRequest::GetSuccessorRoomDetails {
                    tombstoned_room_id: room_id.clone(),
                });
                Some(SuccessorRoomDetails::Basic(sr))
            } else {
                None
            };

            let tl_state = TimelineUiState {
                room_id: room_id.clone(),
                // Initially, we assume the user has all power levels by default.
                // This avoids unexpectedly hiding any UI elements that should be visible to the user.
                // This doesn't mean that the user can actually perform all actions;
                // the power levels will be updated from the homeserver once the room is opened.
                user_power: UserPowerLevels::all(),
                // Room members start as None and get populated when fetched from the server
                room_members: None,
                // We assume timelines being viewed for the first time haven't been fully paginated.
                fully_paginated: false,
                items: Vector::new(),
                content_drawn_since_last_update: RangeSet::new(),
                profile_drawn_since_last_update: RangeSet::new(),
                update_receiver,
                request_sender,
                media_cache: MediaCache::new(Some(update_sender.clone())),
                link_preview_cache: LinkPreviewCache::new(Some(update_sender)),
                saved_state: SavedState::default(),
                message_highlight_animation_state: MessageHighlightAnimationState::default(),
                last_scrolled_index: usize::MAX,
                prev_first_index: None,
                scrolled_past_read_marker: false,
                latest_own_user_receipt: None,
                tombstone_info,
            };
            (tl_state, true)
        };

        // It is possible that this room has already been loaded (received from the server)
        // but that the RoomsList doesn't yet know about it.
        // In that case, `is_first_time_being_loaded` will already be `true` here,
        // so we can bypass checking the RoomsList to determine if a room is loaded.
        //
        // Note that we *do* still need to check the RoomsList to see whether this room is loaded
        // in order to handle the case when we're switching between rooms within
        // the same RoomScreen widget, as one room may be loaded while another is not.
        if is_first_time_being_loaded {
            self.is_loaded = true;
        } else if cx.has_global::<RoomsListRef>() {
            let rooms_list_ref = cx.get_global::<RoomsListRef>();
            let is_loaded_now = rooms_list_ref.is_room_loaded(&room_id);
            if is_loaded_now && !self.is_loaded {
                // log!("Detected that room {:?} is now loaded for the first time",
                //     self.room_name_id
                // );
                is_first_time_being_loaded = true;
            }
            self.is_loaded = is_loaded_now;
        }

        self.view.restore_status_view(ids!(restore_status_view)).set_visible(cx, !self.is_loaded);

        // Kick off a back pagination request if it's the first time loading this room,
        // because we want to show the user some messages as soon as possible
        // when they first open the room, and there might not be any messages yet.
        if is_first_time_being_loaded {
            if !tl_state.fully_paginated {
                log!("Sending a first-time backwards pagination request for room {:?}", self.room_name_id);
                submit_async_request(MatrixRequest::PaginateRoomTimeline {
                    room_id: room_id.clone(),
                    num_events: 50,
                    direction: PaginationDirection::Backwards,
                });
            }

            // Even though we specify that room member profiles should be lazy-loaded,
            // the matrix server still doesn't consistently send them to our client properly.
            // So we kick off a request to fetch the room members here upon first viewing the room.
            submit_async_request(MatrixRequest::SyncRoomMemberList { room_id: room_id.clone() });
        }

        // Hide the typing notice view initially.
        self.view(ids!(typing_notice)).set_visible(cx, false);
        // If the room is loaded, we need to get a few key states:
        // 1. Get the current user's power levels for this room so that we can
        //    show/hide UI elements based on the user's permissions.
        // 2. Get the list of members in this room (from the SDK's local cache).
        // 3. Subscribe to our own user's read receipts so that we can update the
        //    read marker and properly send read receipts while scrolling through the timeline.
        // 4. Subscribe to typing notices again, now that the room is being shown.
        if self.is_loaded {
            submit_async_request(MatrixRequest::GetRoomPowerLevels {
                room_id: room_id.clone(),
            });
            submit_async_request(MatrixRequest::GetRoomMembers {
                room_id: room_id.clone(),
                memberships: matrix_sdk::RoomMemberships::JOIN,
                // Fetch from the local cache, as we already requested to sync
                // the room members from the homeserver above.
                local_only: true,
            });
            submit_async_request(MatrixRequest::SubscribeToTypingNotices {
                room_id: room_id.clone(),
                subscribe: true,
            });
            submit_async_request(MatrixRequest::SubscribeToOwnUserReadReceiptsChanged {
                room_id: room_id.clone(),
                subscribe: true,
            });
            submit_async_request(MatrixRequest::SubscribeToPinnedEvents {
                room_id: room_id.clone(),
                subscribe: true,
            });
        }

        // Now, restore the visual state of this timeline from its previously-saved state.
        self.restore_state(cx, &mut tl_state);

        // Store the tl_state for this room into this RoomScreen widget,
        // such that it can be accessed in future functions like event/draw handlers.
        self.tl_state = Some(tl_state);

        // Now that we have restored the TimelineUiState into this RoomScreen widget,
        // we can proceed to processing pending background updates.
        self.process_timeline_updates(cx, &self.portal_list(ids!(list)));

        self.redraw(cx);
    }

    /// Invoke this when this RoomScreen/timeline is being hidden or no longer being shown.
    fn hide_timeline(&mut self) {
        let Some(room_id) = self.room_id().cloned() else { return };

        self.save_state();

        // When closing a room view, we do the following with non-persistent states:
        // * Unsubscribe from typing notices, since we don't care about them
        //   when a given room isn't visible.
        // * Unsubscribe from updates to our own user's read receipts, for the same reason.
        // * Unsubscribe from updates to this room's pinned events, for the same reason.
        submit_async_request(MatrixRequest::SubscribeToTypingNotices {
            room_id: room_id.clone(),
            subscribe: false,
        });
        submit_async_request(MatrixRequest::SubscribeToOwnUserReadReceiptsChanged {
            room_id: room_id.clone(),
            subscribe: false,
        });
        submit_async_request(MatrixRequest::SubscribeToPinnedEvents {
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
            error!("Timeline::save_state(): skipping due to missing state, room {:?}", self.room_name_id);
            return;
        };

        let portal_list = self.portal_list(ids!(list));
        let state = SavedState {
            first_index_and_scroll: Some((portal_list.first_id(), portal_list.scroll_position())),
            room_input_bar_state: self.room_input_bar(ids!(room_input_bar)).save_state(),
        };
        tl.saved_state = state;
        // Clear room_members to avoid wasting memory (in case this room is never re-opened).
        tl.room_members = None;
        // Store this Timeline's `TimelineUiState` in the global map of states.
        TIMELINE_STATES.with_borrow_mut(|ts| ts.insert(tl.room_id.clone(), tl));
    }

    /// Restores the previously-saved visual UI state of this room.
    ///
    /// Note: this accepts a direct reference to the timeline's UI state,
    /// so this function must not try to re-obtain it by accessing `self.tl_state`.
    fn restore_state(&mut self, cx: &mut Cx, tl_state: &mut TimelineUiState) {
        let SavedState {
            first_index_and_scroll,
            room_input_bar_state,
        } = &mut tl_state.saved_state;
        // 1. Restore the position of the timeline.
        if let Some((first_index, scroll_from_first_id)) = first_index_and_scroll {
            self.portal_list(ids!(timeline.list))
                .set_first_id_and_scroll(*first_index, *scroll_from_first_id);
        } else {
            // If the first index is not set, then the timeline has not yet been scrolled by the user,
            // so we set the portal list to "tail" (track) the bottom of the list.
            self.portal_list(ids!(timeline.list)).set_tail_range(true);
        }

        // 2. Restore the state of the room input bar.
        let room_input_bar = self.view.room_input_bar(ids!(room_input_bar));
        let saved_room_input_bar_state = std::mem::take(room_input_bar_state);
        room_input_bar.restore_state(
            cx,
            &tl_state.room_id,
            saved_room_input_bar_state,
            tl_state.user_power,
            tl_state.tombstone_info.as_ref(),
        );
    }

    /// Sets this `RoomScreen` widget to display the timeline for the given room.
    pub fn set_displayed_room(
        &mut self,
        cx: &mut Cx,
        room_name_id: &RoomNameId,
    ) {
        // If the room is already being displayed, then do nothing.
        if self.room_name_id.as_ref().is_some_and(|rn| rn.room_id() == room_name_id.room_id()) { return; }

        self.hide_timeline();
        // Reset the the state of the inner loading pane.
        self.loading_pane(ids!(loading_pane)).take_state();

        let room_id = room_name_id.room_id().clone();
        self.room_name_id = Some(room_name_id.clone());

        // We initially tell every MentionableTextInput widget that the current user
        // *does not* have privileges to notify the entire room;
        // this gets properly updated when room PowerLevels get fetched.
        cx.action(MentionableTextInputAction::PowerLevelsUpdated {
            room_id: room_id.clone(),
            can_notify_room: false,
        });

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
    pub fn set_displayed_room(
        &self,
        cx: &mut Cx,
        room_name_id: &RoomNameId,
    ) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.set_displayed_room(cx, room_name_id);
    }
}

/// Immutable RoomScreen states passed via Scope props
/// from a RoomScreen widget to its child widgets for event/draw handlers.
pub struct RoomScreenProps {
    pub room_screen_widget_uid: WidgetUid,
    pub room_name_id: RoomNameId,
    pub room_members: Option<Arc<Vec<RoomMember>>>,
    pub room_avatar_url: Option<OwnedMxcUri>,
}


/// Actions for the room screen's tooltip.
#[derive(Clone, Debug, DefaultNone)]
pub enum RoomScreenTooltipActions {
    /// Mouse over event when the mouse is over the read receipt.
    HoverInReadReceipt {
        /// The rect of the moused over widget
        widget_rect: Rect,
        /// Includes the list of users who have seen this event
        read_receipts: indexmap::IndexMap<matrix_sdk::ruma::OwnedUserId, Receipt>,
    },
    /// Mouse over event when the mouse is over the reaction button.
    HoverInReactionButton {
        /// The rectangle (bounds) of the hovered-over widget.
        widget_rect: Rect,
        /// Includes the list of users who have reacted to the emoji.
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
    /// A notice with an option of Media Request Parameters that one or more requested media items (images, videos, etc.)
    /// that should be displayed in this timeline have now been fetched and are available.
    MediaFetched(MediaRequestParameters),
    /// A notice that one or more members of a this room are currently typing.
    TypingUsers {
        /// The list of users (their displayable name) who are currently typing in this room.
        users: Vec<String>,
    },
    /// The result of a pin/unpin request ([`MatrixRequest::PinEvent`]).
    PinResult {
        event_id: OwnedEventId,
        result: Result<bool, matrix_sdk::Error>,
        pin: bool,
    },
    /// An update containing the set of pinned events in this room.
    PinnedEvents(Vec<OwnedEventId>),
    /// An update containing the currently logged-in user's power levels for this room.
    UserPowerLevels(UserPowerLevels),
    /// An update to the currently logged-in user's own read receipt for this room.
    OwnUserReadReceipt(Receipt),
    /// A notice that the given room has been tombstoned (closed)
    /// and replaced by the given successor room.
    Tombstoned(SuccessorRoomDetails),
    /// A notice that link preview data for a URL has been fetched and is now available.
    LinkPreviewFetched,
    /// A notice that inviting the given user to this room succeeded or failed.
    InviteSent {
        user_id: OwnedUserId,
        result: matrix_sdk::Result<()>,
    },
}

thread_local! {
    /// The global set of all timeline states, one entry per room.
    ///
    /// This is only useful when accessed from the main UI thread.
    static TIMELINE_STATES: RefCell<BTreeMap<OwnedRoomId, TimelineUiState>> = const {
        RefCell::new(BTreeMap::new())
    };
}

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

    /// The list of room members for this room.
    room_members: Option<Arc<Vec<RoomMember>>>,

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

    /// Cache for link preview data indexed by URL to avoid redundant network requests.
    link_preview_cache: LinkPreviewCache,

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

    /// If `Some`, this room has been tombstoned and the details of its successor room
    /// are contained within. If `None`, the room has not been tombstoned.
    tombstone_info: Option<SuccessorRoomDetails>,
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
#[derive(Default)]
struct SavedState {
    /// The index of the first item in the timeline's PortalList that is currently visible,
    /// and the scroll offset from the top of the list's viewport to the beginning of that item.
    /// If this is `None`, then the timeline has not yet been scrolled by the user
    /// and the portal list will be set to "tail" (track) the bottom of the list.
    first_index_and_scroll: Option<(usize, f64)>,
    /// The state of all UI elements in the `RoomInputBar`.
    room_input_bar_state: RoomInputBarState,
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
    msg_like_content: &MsgLikeContent,
    prev_event: Option<&Arc<TimelineItem>>,
    media_cache: &mut MediaCache,
    link_preview_cache: &mut LinkPreviewCache,
    user_power_levels: &UserPowerLevels,
    pinned_events: &[OwnedEventId],
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
            TimelineItemContent::MsgLike(_msg_like_content) => {
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
    let (item, used_cached_item) = match &msg_like_content.kind {
        MsgLikeKind::Message(msg) => {
            match msg.msgtype() {
                MessageType::Text(TextMessageEventContent { body, formatted, .. }) => {
                     has_html_body = formatted.as_ref().is_some_and(|f| f.format == MessageFormat::Html);
                    let template = if use_compact_view {
                        id!(CondensedMessage)
                    } else {
                        id!(Message)
                    };
                    let (item, existed) = list.item_with_existed(cx, item_id, template);
                    if existed && item_drawn_status.content_drawn {
                        (item, true)
                    } else {
                        new_drawn_status.content_drawn = populate_text_message_content(
                            cx,
                            &item.html_or_plaintext(ids!(content.message)),
                            body,
                            formatted.as_ref(),
                            Some(&mut item.link_preview(ids!(content.link_preview_view))),
                            Some(media_cache),
                            Some(link_preview_cache),
                        );
                        (item, false)
                    }
                }
                // A notice message is just a message sent by an automated bot,
                // so we treat it just like a message but use a different font color.
                MessageType::Notice(NoticeMessageEventContent{body, formatted, ..}) => {
                    is_notice = true;
                    has_html_body = formatted.as_ref().is_some_and(|f| f.format == MessageFormat::Html);
                    let template = if use_compact_view {
                        id!(CondensedMessage)
                    } else {
                        id!(Message)
                    };
                    let (item, existed) = list.item_with_existed(cx, item_id, template);
                    if existed && item_drawn_status.content_drawn {
                        (item, true)
                    } else {
                        let html_or_plaintext_ref = item.html_or_plaintext(ids!(content.message));
                        html_or_plaintext_ref.apply_over(cx, live!(
                            html_view = {
                                html = {
                                    font_color: (COLOR_MESSAGE_NOTICE_TEXT),
                                    draw_normal:      { color: (COLOR_MESSAGE_NOTICE_TEXT), }
                                    draw_italic:      { color: (COLOR_MESSAGE_NOTICE_TEXT), }
                                    draw_bold:        { color: (COLOR_MESSAGE_NOTICE_TEXT), }
                                    draw_bold_italic: { color: (COLOR_MESSAGE_NOTICE_TEXT), }
                                }
                            }
                        ));
                        new_drawn_status.content_drawn = populate_text_message_content(
                            cx,
                            &html_or_plaintext_ref,
                            body,
                            formatted.as_ref(),
                            Some(&mut item.link_preview(ids!(content.link_preview_view))),
                            Some(media_cache),
                            Some(link_preview_cache),
                        );
                        (item, false)
                    }
                }
                MessageType::ServerNotice(sn) => {
                    is_server_notice = true;
                    has_html_body = false;
                    let (item, existed) = list.item_with_existed(cx, item_id, id!(Message));
                    if existed && item_drawn_status.content_drawn {
                        (item, true)
                    } else {
                        let html_or_plaintext_ref = item.html_or_plaintext(ids!(content.message));
                        html_or_plaintext_ref.apply_over(cx, live!(
                            html_view = {
                                html = {
                                    font_color: (COLOR_FG_DANGER_RED),
                                    draw_normal:      { color: (COLOR_FG_DANGER_RED), }
                                    draw_italic:      { color: (COLOR_FG_DANGER_RED), }
                                    draw_bold:        { color: (COLOR_FG_DANGER_RED), }
                                    draw_bold_italic: { color: (COLOR_FG_DANGER_RED), }
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
                        new_drawn_status.content_drawn = populate_text_message_content(
                            cx,
                            &html_or_plaintext_ref,
                            &sn.body,
                            Some(&FormattedBody {
                                format: MessageFormat::Html,
                                body: formatted,
                            }),
                            Some(&mut item.link_preview(ids!(content.link_preview_view))),
                            Some(media_cache),
                            Some(link_preview_cache),
                        );
                        (item, false)
                    }
                }
                // An emote is just like a message but is prepended with the user's name
                // to indicate that it's an "action" that the user is performing.
                MessageType::Emote(EmoteMessageEventContent { body, formatted, .. }) => {
                    has_html_body = formatted.as_ref().is_some_and(|f| f.format == MessageFormat::Html);
                    let template = if use_compact_view {
                        id!(CondensedMessage)
                    } else {
                        id!(Message)
                    };
                    let (item, existed) = list.item_with_existed(cx, item_id, template);
                    if existed && item_drawn_status.content_drawn {
                        (item, true)
                    } else {
                        // Draw the profile up front here because we need the username for the emote body.
                        let (username, profile_drawn) = item.avatar(ids!(profile.avatar)).set_avatar_and_get_username(
                            cx,
                            room_id,
                            event_tl_item.sender(),
                            Some(event_tl_item.sender_profile()),
                            event_tl_item.event_id(),
                            true,
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
                        let link_previews_drawn = populate_text_message_content(
                            cx,
                            &item.html_or_plaintext(ids!(content.message)),
                            &body,
                            formatted.as_ref(),
                            Some(&mut item.link_preview(ids!(content.link_preview_view))),
                            Some(media_cache),
                            Some(link_preview_cache),
                        );
                        set_username_and_get_avatar_retval = Some((username, profile_drawn));
                        new_drawn_status.content_drawn = link_previews_drawn;
                        (item, false)
                    }
                }
                MessageType::Image(image) => {
                    has_html_body = image.formatted.as_ref()
                        .is_some_and(|f| f.format == MessageFormat::Html);
                    let template = if use_compact_view {
                        id!(CondensedImageMessage)
                    } else {
                        id!(ImageMessage)
                    };
                    let (item, existed) = list.item_with_existed(cx, item_id, template);
                    if existed && item_drawn_status.content_drawn {
                        (item, true)
                    } else {
                        let image_info = image.info.clone();
                        let is_image_fully_drawn = populate_image_message_content(
                            cx,
                            &item.text_or_image(ids!(content.message)),
                            image_info,
                            image.source.clone(),
                            msg.body(),
                            media_cache,
                        );
                        new_drawn_status.content_drawn = is_image_fully_drawn;
                        (item, false)
                    }
                }
                MessageType::Location(location) => {
                    has_html_body = false;
                    let template = if use_compact_view {
                        id!(CondensedMessage)
                    } else {
                        id!(Message)
                    };
                    let (item, existed) = list.item_with_existed(cx, item_id, template);
                    if existed && item_drawn_status.content_drawn {
                        (item, true)
                    } else {
                        let is_location_fully_drawn = populate_location_message_content(
                            cx,
                            &item.html_or_plaintext(ids!(content.message)),
                            location,
                        );
                        new_drawn_status.content_drawn = is_location_fully_drawn;
                        (item, false)
                    }
                }
                MessageType::File(file_content) => {
                    has_html_body = file_content.formatted.as_ref().is_some_and(|f| f.format == MessageFormat::Html);
                    let template = if use_compact_view {
                        id!(CondensedMessage)
                    } else {
                        id!(Message)
                    };
                    let (item, existed) = list.item_with_existed(cx, item_id, template);
                    if existed && item_drawn_status.content_drawn {
                        (item, true)
                    } else {
                        new_drawn_status.content_drawn = populate_file_message_content(
                            cx,
                            &item.html_or_plaintext(ids!(content.message)),
                            file_content,
                        );
                        (item, false)
                    }
                }
                MessageType::Audio(audio) => {
                    has_html_body = audio.formatted.as_ref().is_some_and(|f| f.format == MessageFormat::Html);
                    let template = if use_compact_view {
                        id!(CondensedMessage)
                    } else {
                        id!(Message)
                    };
                    let (item, existed) = list.item_with_existed(cx, item_id, template);
                    if existed && item_drawn_status.content_drawn {
                        (item, true)
                    } else {
                        new_drawn_status.content_drawn = populate_audio_message_content(
                            cx,
                            &item.html_or_plaintext(ids!(content.message)),
                            audio,
                        );
                        (item, false)
                    }
                }
                MessageType::Video(video) => {
                    has_html_body = video.formatted.as_ref().is_some_and(|f| f.format == MessageFormat::Html);
                    let template = if use_compact_view {
                        id!(CondensedMessage)
                    } else {
                        id!(Message)
                    };
                    let (item, existed) = list.item_with_existed(cx, item_id, template);
                    if existed && item_drawn_status.content_drawn {
                        (item, true)
                    } else {
                        new_drawn_status.content_drawn = populate_video_message_content(
                            cx,
                            &item.html_or_plaintext(ids!(content.message)),
                            video,
                        );
                        (item, false)
                    }
                }
                MessageType::VerificationRequest(verification) => {
                    has_html_body = verification.formatted.as_ref().is_some_and(|f| f.format == MessageFormat::Html);
                    let template = id!(Message);
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

                        new_drawn_status.content_drawn = populate_text_message_content(
                            cx,
                            &item.html_or_plaintext(ids!(content.message)),
                            &verification.body,
                            Some(&formatted),
                            Some(&mut item.link_preview(ids!(content.link_preview_view))),
                            Some(media_cache),
                            Some(link_preview_cache),
                        );
                        (item, false)
                    }
                }
                _ => {
                    has_html_body = false;
                    let (item, existed) = list.item_with_existed(cx, item_id, id!(Message));
                    if existed && item_drawn_status.content_drawn {
                        (item, true)
                    } else {
                        item.label(ids!(content.message)).set_text(
                            cx,
                            &format!("[Unsupported {:?}]", msg_like_content.kind),
                        );
                        new_drawn_status.content_drawn = true;
                        (item, false)
                    }
                }
            }
        }
        // Handle sticker messages that are static images.
        MsgLikeKind::Sticker(sticker) => {
            has_html_body = false;
            let StickerEventContent { body, info, source, .. } = sticker.content();

            let template = if use_compact_view {
                id!(CondensedImageMessage)
            } else {
                id!(ImageMessage)
            };
            let (item, existed) = list.item_with_existed(cx, item_id, template);

            if existed && item_drawn_status.content_drawn {
                (item, true)
            } else {
                if let StickerMediaSource::Plain(owned_mxc_url) = source {
                    let image_info = info;
                    let is_image_fully_drawn = populate_image_message_content(
                        cx,
                        &item.text_or_image(ids!(content.message)),
                        Some(Box::new(image_info.clone())),
                        MediaSource::Plain(owned_mxc_url.clone()),
                        body,
                        media_cache,
                    );
                    new_drawn_status.content_drawn = is_image_fully_drawn;
                    (item, false)
                } else {
                    (item, true)
                }
            }
        }
        other => {
            has_html_body = false;
            let (item, existed) = list.item_with_existed(cx, item_id, id!(Message));
            if existed && item_drawn_status.content_drawn {
                (item, true)
            } else {
                item.label(ids!(content.message)).set_text(
                    cx,
                    &format!("[Unsupported {:?}] ", other),
                );
                new_drawn_status.content_drawn = true;
                (item, false)
            }
        }
    };

    // If we didn't use a cached item, we need to draw all other message content:
    // the reactions, the read receipts avatar row, and the reply preview.
    // We also must set the message details/metadata for the `item` widget representing this message.
    if !used_cached_item {
        item.reaction_list(ids!(content.reaction_list)).set_list(
            cx,
            event_tl_item.content().reactions(),
            room_id.to_owned(),
            event_tl_item.identifier(),
            item_id,
        );
        populate_read_receipts(&item, cx, room_id, event_tl_item);
        let replied_to_message_view = item.view(ids!(replied_to_message));
        let (is_reply_fully_drawn, replied_to_event_id) = draw_replied_to_message(
            cx,
            &replied_to_message_view,
            room_id,
            msg_like_content.in_reply_to.as_ref(),
            event_tl_item.event_id(),
        );

        // Set the message details/metadata for the Message widget so that it can handle events.
        let message_details = MessageDetails {
            event_id: event_tl_item.event_id().map(|id| id.to_owned()),
            item_id,
            related_event_id: replied_to_event_id,
            room_screen_widget_uid,
            abilities: MessageAbilities::from_user_power_and_event(
                user_power_levels,
                event_tl_item,
                msg_like_content,
                pinned_events,
                has_html_body,
            ),
            should_be_highlighted: event_tl_item.is_highlighted(),
        };
        item.as_message().set_data(message_details);

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
        let username_label = item.label(ids!(content.username));

        if !is_server_notice { // the normal case
            let (username, profile_drawn) = set_username_and_get_avatar_retval.unwrap_or_else(||
                item.avatar(ids!(profile.avatar)).set_avatar_and_get_username(
                    cx,
                    room_id,
                    event_tl_item.sender(),
                    Some(event_tl_item.sender_profile()),
                    event_tl_item.event_id(),
                    true,
                )
            );
            if is_notice {
                username_label.apply_over(cx, live!(
                    draw_text: {
                        color: (COLOR_MESSAGE_NOTICE_TEXT),
                    }
                ));
            }
            username_label.set_text(cx, &username);
            new_drawn_status.profile_drawn = profile_drawn;
        }
        else {
            // Server notices are drawn with a red color avatar background and username.
            let avatar = item.avatar(ids!(profile.avatar));
            avatar.show_text(cx, Some(COLOR_FG_DANGER_RED), None, "⚠");
            username_label.set_text(cx, "Server notice");
            username_label.apply_over(cx, live!(
                draw_text: {
                    color: (COLOR_FG_DANGER_RED),
                }
            ));
            new_drawn_status.profile_drawn = true;
        }
    }

    // If we've previously drawn the item content, skip all other steps.
    if used_cached_item && item_drawn_status.content_drawn && item_drawn_status.profile_drawn {
        return (item, new_drawn_status);
    }

    // Set the timestamp.
    if let Some(dt) = unix_time_millis_to_datetime(ts_millis) {
        item.timestamp(ids!(profile.timestamp)).set_date_time(cx, dt);
    }

    // Set the "edited" indicator if this message was edited.
    if msg_like_content.as_message().is_some_and(|m| m.is_edited()) {
        item.edited_indicator(ids!(profile.edited_indicator)).set_latest_edit(
            cx,
            event_tl_item,
        );
    }

    #[cfg(feature = "tsp")] {
        use matrix_sdk::ruma::serde::Base64;
        use crate::tsp::{self, tsp_sign_indicator::{TspSignState, TspSignIndicatorWidgetRefExt}};

        if let Some(mut tsp_sig) = event_tl_item.latest_json()
            .and_then(|raw| raw.get_field::<serde_json::Value>("content").ok())
            .flatten()
            .and_then(|content_obj| content_obj.get("org.robius.tsp_signature").cloned())
            .and_then(|tsp_sig_value| serde_json::from_value::<Base64>(tsp_sig_value).ok())
            .map(|b64| b64.into_inner())
        {
            log!("Found event {:?} with TSP signature.", event_tl_item.event_id());
            let tsp_sign_state = if let Some(sender_vid) = tsp::tsp_state_ref().lock().unwrap()
                .get_verified_vid_for(event_tl_item.sender())
            {
                log!("Found verified VID for sender {}: \"{}\"", event_tl_item.sender(), sender_vid.identifier());
                tsp_sdk::crypto::verify(&*sender_vid, &mut tsp_sig).map_or(
                    TspSignState::WrongSignature,
                    |(msg, msg_type)| {
                        log!("TSP signature verified successfully!\n    Msg type: {msg_type:?}\n    Message: {:?} ({msg:X?})", std::str::from_utf8(msg));
                        TspSignState::Verified
                    }
                )
            } else {
                TspSignState::Unknown
            };

            log!("TSP signature state for event {:?} is {:?}", event_tl_item.event_id(), tsp_sign_state);
            item.tsp_sign_indicator(ids!(profile.tsp_sign_indicator))
                .show_with_state(cx, tsp_sign_state);
        }
    }

    (item, new_drawn_status)
}

/// Draws the Html or plaintext body of the given Text or Notice message into the `message_content_widget`.
/// Also populates link previews if a link_preview_ref is provided.
/// Returns whether the text items were fully drawn.
fn populate_text_message_content(
    cx: &mut Cx,
    message_content_widget: &HtmlOrPlaintextRef,
    body: &str,
    formatted_body: Option<&FormattedBody>,
    link_preview_ref: Option<&mut LinkPreviewRef>,
    media_cache: Option<&mut MediaCache>,
    link_preview_cache: Option<&mut LinkPreviewCache>,
) -> bool {
    // The message was HTML-formatted rich text.
    let mut links = Vec::new();
    if let Some(fb) = formatted_body.as_ref()
        .and_then(|fb| (fb.format == MessageFormat::Html).then_some(fb))
    {
        let linkified_html = utils::linkify_get_urls(
            utils::trim_start_html_whitespace(&fb.body),
            true,
            Some(&mut links),
        );
        message_content_widget.show_html(cx, linkified_html);
    }
    // The message was non-HTML plaintext.
    else {
        let linkified_html = utils::linkify_get_urls(body, false, Some(&mut links));
        match linkified_html {
            Cow::Owned(linkified_html) => message_content_widget.show_html(cx, &linkified_html),
            Cow::Borrowed(plaintext) => message_content_widget.show_plaintext(cx, plaintext),
        }
    };

    // Populate link previews if all required parameters are provided
    if let (Some(link_preview_ref), Some(media_cache), Some(link_preview_cache)) = 
        (link_preview_ref, media_cache, link_preview_cache)
    {
        link_preview_ref.populate_below_message(
            cx,
            &links,
            media_cache,
            link_preview_cache,
            &populate_image_message_content,
        )
    } else {
        true
    }
}

/// Draws the given image message's content into the `message_content_widget`.
///
/// Returns whether the image message content was fully drawn.
fn populate_image_message_content(
    cx: &mut Cx,
    text_or_image_ref: &TextOrImageRef,
    image_info_source: Option<Box<ImageInfo>>,
    original_source: MediaSource,
    body: &str,
    media_cache: &mut MediaCache,
) -> bool {
    // We don't use thumbnails, as their resolution is too low to be visually useful.
    // We also don't trust the provided mimetype, as it can be incorrect.
    let (mimetype, _width, _height) = image_info_source.as_ref()
        .map(|info| (info.mimetype.as_deref(), info.width, info.height))
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
    let mut fetch_and_show_image_uri = |cx: &mut Cx, mxc_uri: OwnedMxcUri, image_info: Box<ImageInfo>| {
        match media_cache.try_get_media_or_fetch(mxc_uri.clone(), MEDIA_THUMBNAIL_FORMAT.into()) {
            (MediaCacheEntry::Loaded(data), _media_format) => {
                let show_image_result = text_or_image_ref.show_image(cx, Some(MediaSource::Plain(mxc_uri)),|cx, img| {
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
                // If the image is being fetched, we try to show its blurhash.
                if let (Some(ref blurhash), Some(width), Some(height)) = (image_info.blurhash.clone(), image_info.width, image_info.height) {
                    let show_image_result = text_or_image_ref.show_image(cx, Some(MediaSource::Plain(mxc_uri)), |cx, img| {
                        let (Ok(width), Ok(height)) = (width.try_into(), height.try_into()) else {
                            return Err(image_cache::ImageError::EmptyData)
                        };
                        let (width, height): (u32, u32) = (width, height);
                        if width == 0 || height == 0 {
                            warning!("Image had an invalid aspect ratio (width or height of 0).");
                            return Err(image_cache::ImageError::EmptyData);
                        }
                        let aspect_ratio: f32 = width as f32 / height as f32;
                        // Cap the blurhash to a max size of 500 pixels in each dimension
                        // because the `blurhash::decode()` function can be rather expensive.
                        let (mut capped_width, mut capped_height) = (width, height);
                        if capped_height > BLURHASH_IMAGE_MAX_SIZE {
                            capped_height = BLURHASH_IMAGE_MAX_SIZE;
                            capped_width = (capped_height as f32 * aspect_ratio).floor() as u32;
                        }
                        if capped_width > BLURHASH_IMAGE_MAX_SIZE {
                            capped_width = BLURHASH_IMAGE_MAX_SIZE;
                            capped_height = (capped_width as f32 / aspect_ratio).floor() as u32;
                        }

                        match blurhash::decode(blurhash, capped_width, capped_height, 1.0) {
                            Ok(data) => {
                                ImageBuffer::new(&data, capped_width as usize, capped_height as usize).map(|img_buff| {
                                    let texture = Some(img_buff.into_new_texture(cx));
                                    img.set_texture(cx, texture);
                                    img.size_in_pixels(cx).unwrap_or_default()
                                })
                            }
                            Err(e) => {
                                error!("Failed to decode blurhash {e:?}");
                                Err(image_cache::ImageError::EmptyData)
                            }   
                        }
                    });
                    if let Err(e) = show_image_result {
                        let err_str = format!("{body}\n\nFailed to display image: {e:?}");
                        error!("{err_str}");
                        text_or_image_ref.show_text(cx, &err_str);
                    }
                }
                fully_drawn = false;
            }
            (MediaCacheEntry::Failed(_status_code), _media_format) => {
                if text_or_image_ref.view(ids!(default_image_view)).visible() {
                    fully_drawn = true;
                    return;
                }
                text_or_image_ref
                    .show_text(cx, format!("{body}\n\nFailed to fetch image from {:?}", mxc_uri));
                // For now, we consider this as being "complete". In the future, we could support
                // retrying to fetch thumbnail of the image on a user click/tap.
                fully_drawn = true;
            }
        }
    };

    let mut fetch_and_show_media_source = |cx: &mut Cx, media_source: MediaSource, image_info: Box<ImageInfo>| {
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
        Some(image_info) => {
            // Use the provided thumbnail URI if it exists; otherwise use the original URI.
            let media_source = image_info.thumbnail_source.clone()
                .unwrap_or(original_source);
            fetch_and_show_media_source(cx, media_source, image_info);
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
        .get(utils::GEO_URI_SCHEME.len() ..)
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
/// ## Arguments
/// * `replied_to_message_view`: the destination `RepliedToMessage` view that will be populated.
/// * `in_reply_to`: if `Some`, the details that will be used to populate the `replied_to_message_view`.
///   If `None`, this function will mark it as non-visible and consider it fully drawn.
/// * `message_event_id`: the [`EventId`] of the message that is the reply itself (the response).
///   This is needed to fetch the details of the replied-to message (if not yet available).
///
/// Returns whether the in-reply-to information was available and fully drawn,
/// i.e., whether it can be considered cached and not needing to be redrawn later.
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
                        .avatar(ids!(replied_to_message_content.reply_preview_avatar))
                        .set_avatar_and_get_username(
                            cx,
                            room_id,
                            &replied_to_event.sender,
                            Some(&replied_to_event.sender_profile),
                            Some(in_reply_to_details.event_id.as_ref()),
                            true,
                        );

                fully_drawn = is_avatar_fully_drawn;

                replied_to_message_view
                    .label(ids!(replied_to_message_content.reply_preview_username))
                    .set_text(cx, in_reply_to_username.as_str());
                let msg_body = replied_to_message_view.html_or_plaintext(ids!(reply_preview_body));
                populate_preview_of_timeline_item(
                    cx,
                    &msg_body,
                    &replied_to_event.content,
                    &replied_to_event.sender,
                    &in_reply_to_username,
                );
            }
            TimelineDetails::Error(_e) => {
                fully_drawn = true;
                replied_to_message_view
                    .label(ids!(replied_to_message_content.reply_preview_username))
                    .set_text(cx, "[Error fetching username]");
                replied_to_message_view
                    .avatar(ids!(replied_to_message_content.reply_preview_avatar))
                    .show_text(cx, None, None, "?");
                replied_to_message_view
                    .html_or_plaintext(ids!(replied_to_message_content.reply_preview_body))
                    .show_plaintext(cx, "[Error fetching replied-to event]");
            }
            status @ TimelineDetails::Pending | status @ TimelineDetails::Unavailable => {
                // We don't have the replied-to message yet, so we can't fully draw the preview.
                fully_drawn = false;
                replied_to_message_view
                    .label(ids!(replied_to_message_content.reply_preview_username))
                    .set_text(cx, "[Loading username...]");
                replied_to_message_view
                    .avatar(ids!(replied_to_message_content.reply_preview_avatar))
                    .show_text(cx, None, None, "?");
                replied_to_message_view
                    .html_or_plaintext(ids!(replied_to_message_content.reply_preview_body))
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

/// Generates a rich HTML text preview of the given `timeline_item_content`
/// and populates the given `widget_out` with that content.
pub fn populate_preview_of_timeline_item(
    cx: &mut Cx,
    widget_out: &HtmlOrPlaintextRef,
    timeline_item_content: &TimelineItemContent,
    sender_user_id: &UserId,
    sender_username: &str,
) {
    if let Some(m) = timeline_item_content.as_message() {
        match m.msgtype() {
            MessageType::Text(TextMessageEventContent { body, formatted, .. })
            | MessageType::Notice(NoticeMessageEventContent { body, formatted, .. }) => {
                let _ = populate_text_message_content(cx, widget_out, body, formatted.as_ref(), None, None, None);
                return;
            }
            _ => { } // fall through to the general case for all timeline items below.
        }
    }
    let html = text_preview_of_timeline_item(
        timeline_item_content,
        sender_user_id,
        sender_username,
    ).format_with(sender_username, true);
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
        item.label(ids!(content)).set_text(
            cx,
            &text_preview_of_redacted_message(
                event_tl_item.latest_json(),
                event_tl_item.sender(),
                original_sender,
            ).format_with(original_sender, false),
        );
        new_drawn_status.content_drawn = true;
        (item, new_drawn_status)
    }
}

// For unable to decrypt messages.
impl SmallStateEventContent for EncryptedMessage {
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
        item.label(ids!(content)).set_text(
            cx,
            &text_preview_of_encrypted_message(self).format_with(username, false),
        );
        new_drawn_status.content_drawn = true;
        (item, new_drawn_status)
    }
}

// For other message-like content (custom message-like events).
impl SmallStateEventContent for OtherMessageLike {
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
        item.label(ids!(content)).set_text(
            cx,
            &text_preview_of_other_message_like(self).format_with(username, false),
        );
        new_drawn_status.content_drawn = true;
        (item, new_drawn_status)
    }
}

// TODO: once we properly display polls, we should remove this,
//       because Polls shouldn't be displayed using the SmallStateEvent widget.
impl SmallStateEventContent for PollState {
    fn populate_item_content(
        &self,
        cx: &mut Cx,
        _list: &mut PortalList,
        _item_id: usize,
        item: WidgetRef,
        _event_tl_item: &EventTimelineItem,
        _username: &str,
        _item_drawn_status: ItemDrawnStatus,
        mut new_drawn_status: ItemDrawnStatus,
    ) -> (WidgetRef, ItemDrawnStatus) {
        item.label(ids!(content)).set_text(
            cx,
            self.fallback_text().unwrap_or_else(|| self.results().question).as_str(),
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
            item.label(ids!(content))
                .set_text(cx, &text_preview.format_with(username, false));
            new_drawn_status.content_drawn = true;
            item
        } else {
            let item = list.item(cx, item_id, id!(Empty));
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
        item.label(ids!(content)).set_text(
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
                list.item(cx, item_id, id!(Empty)),
                ItemDrawnStatus::new(),
            );
        };

        item.label(ids!(content))
            .set_text(cx, &preview.format_with(username, false));

        // The invite_user_button is only used for "Knocked" membership change events.
        item.button(ids!(invite_user_button)).set_visible(
            cx,
            matches!(self.change(), Some(MembershipChange::Knocked)),
        );

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
    let (item, existed) = list.item_with_existed(cx, item_id, id!(SmallStateEvent));
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
        let avatar_ref = item.avatar(ids!(avatar));

        let (username, profile_drawn) = avatar_ref.set_avatar_and_get_username(
            cx,
            room_id,
            event_tl_item.sender(),
            Some(event_tl_item.sender_profile()),
            event_tl_item.event_id(),
            true,
        );
        // Draw the timestamp as part of the profile.
        if let Some(dt) = unix_time_millis_to_datetime(event_tl_item.timestamp()) {
            item.timestamp(ids!(left_container.timestamp)).set_date_time(cx, dt);
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


/// Actions related to invites within a room.
///
/// These are NOT widget actions, just regular actions.
#[derive(Debug)]
pub enum InviteAction {
    /// Show a confirmation modal for sending an invite.
    ///
    /// The content is wrapped in a `RefCell` to ensure that only one entity handles it
    /// and that that one entity can take ownership of the content object,
    /// which avoids having to clone it.
    ShowConfirmationModal(RefCell<Option<ConfirmationModalContent>>),
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
    /// The user requested to edit their latest message in this room.
    EditLatest,
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
    /// The user requested to jump to a specific event in this room.
    JumpToEvent(OwnedEventId),
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
        /// The message rect, so the action bar can be positioned relative to it
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

        if !self.animator.is_track_animating(cx, ids!(highlight))
            && self.animator_in_state(cx, ids!(highlight.on))
        {
            self.animator_play(cx, ids!(highlight.off));
        }

        let Some(details) = self.details.clone() else { return };

        // We first handle a click on the replied-to message preview, if present,
        // because we don't want any widgets within the replied-to message to be
        // clickable or otherwise interactive.
        match event.hits(cx, self.view(ids!(replied_to_message)).area()) {
            Hit::FingerDown(fe) => {
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
                self.animator_play(cx, ids!(hover.on));
                // TODO: here, show the "action bar" buttons upon hover-in
            }
            Hit::FingerHoverOut(_fho) => {
                self.animator_play(cx, ids!(hover.off));
                // TODO: here, hide the "action bar" buttons upon hover-out
            }
            _ => { }
        }

        if let Event::Actions(actions) = event {
            for action in actions {
                match action.as_widget_action().cast() {
                    MessageAction::HighlightMessage(id) if id == details.item_id => {
                        self.animator_play(cx, ids!(highlight.on));
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

/// Clears all UI-related timeline states for all known rooms.
///
/// This function requires passing in a reference to `Cx`,
/// which isn't used, but acts as a guarantee that this function
/// must only be called by the main UI thread. 
pub fn clear_timeline_states(_cx: &mut Cx) {
    // Clear timeline states cache
    TIMELINE_STATES.with_borrow_mut(|states| {
        states.clear();
    });
}
