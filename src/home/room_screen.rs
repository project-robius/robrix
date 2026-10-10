//! The `RoomScreen` widget is the UI view that displays a single room or thread's timeline
//! of events (messages，state changes, etc.), along with an input bar at the bottom.

use std::{borrow::Cow, cell::RefCell, ops::{DerefMut, Range}, path::PathBuf, sync::Arc, time::{Duration, Instant}};

use hashbrown::{HashMap, HashSet};
use imbl::Vector;
use makepad_widgets::{image_cache::ImageBuffer, makepad_platform::event::finger::TouchState, *};
use matrix_sdk::reqwest::StatusCode;
use matrix_sdk::{
    RoomState, media::{MediaFormat, MediaRequestParameters}, room::{RoomMember, reply::{EnforceThread, Reply}}, serde_helpers::extract_bundled_thread, ruma::{
        EventId, OwnedEventId, OwnedMxcUri, OwnedRoomId, OwnedRoomOrAliasId, OwnedTransactionId, RoomId, UserId, events::{
            receipt::Receipt,
            room::{
                ImageInfo, MediaSource, message::{
                    AudioMessageEventContent, EmoteMessageEventContent, FileMessageEventContent, FormattedBody, ImageMessageEventContent, KeyVerificationRequestEventContent, LocationMessageEventContent, MessageFormat, MessageType, NoticeMessageEventContent, ReplyWithinThread, RoomMessageEventContent, TextMessageEventContent, VideoMessageEventContent
                }
            },
            sticker::StickerEventContent,
        }, matrix_uri::MatrixId
    }
};
use matrix_sdk_ui::timeline::{
    self, EmbeddedEvent, EventSendState, EventTimelineItem, InReplyToDetails, MsgLikeContent, MsgLikeKind, TimelineDetails, TimelineEventItemId, TimelineItem, TimelineItemContent, TimelineItemKind, VirtualTimelineItem
};
use ruma::{OwnedUserId, api::client::receipt::create_receipt::v3::ReceiptType, events::{AnyRedactionEvent, AnySyncMessageLikeEvent, AnySyncTimelineEvent, SyncMessageLikeEvent}};

use matrix_sdk_ui::sync_service::State;
use crate::{
    app::{AppStateAction, ConfirmDeleteAction, SelectedRoom}, event_preview::{plaintext_body_of_timeline_item, text_preview_of_thread_reply, text_preview_of_timeline_item}, home::{edited_indicator::EditedIndicatorWidgetRefExt, invite_modal::InviteModalAction, link_preview::{LinkPreviewCache, LinkPreviewRef, LinkPreviewWidgetRefExt}, loading_pane::LoadingPaneWidgetExt, navigation_tab_bar::NavigationBarAction, room_image_viewer::{fetch_full_image_for_viewer, get_image_file_details}, rooms_list::{RoomsListAction, RoomsListRef, RoomsListUpdate, enqueue_rooms_list_update}, rooms_list_header::RoomsListHeaderAction, tombstone_footer::SuccessorRoomDetails}, media_cache::{get_image_cache_key, MediaCache, MediaCacheEntry}, profile::{
        user_profile::{ShowUserProfileAction, UserProfile, UserProfileAndRoomId, UserProfilePaneAction, UserProfilePaneInfo, UserProfileSlidingPaneRef, UserProfileSlidingPaneWidgetExt},
        user_profile_cache,
    },
    room::{reply_preview::{CollapsiblePreviewRef, CollapsiblePreviewWidgetRefExt}, room_input_bar::{RoomInputBarState, RoomInputBarWidgetRefExt}, typing_notice::TypingNoticeWidgetExt},
    shared::{
        attachment_download::{enqueue_already_downloading_notification, DownloadDisplayState, DownloadKind, DownloadableAttachment, PendingDownload, PendingDownloadState, TimelineUpdateSenderOption, TransferKind, media_source_mxc, start_attachment_download, start_attachment_share}, avatar::{AvatarState, AvatarWidgetRefExt}, confirmation_modal::ConfirmationModalContent, context_menu::ContextMenuClosed, file_upload_modal::FileUploadAttemptId, hover_highlight::handle_hover_hit, html_or_plaintext::{HtmlOrPlaintextRef, HtmlOrPlaintextWidgetRefExt, RobrixHtmlLinkAction}, image_viewer::{ImageViewerAction, ImageViewerMetaData, LoadState}, jump_to_bottom_button::{JumpToBottomButtonWidgetExt, UnreadMessageCount, SCROLL_TO_BOTTOM_SPEED}, popup_list::{PopupKind, enqueue_popup_notification}, restore_status_view::RestoreStatusViewWidgetExt, room_input_popup_menu::{RoomInputPopupMenuAction, RoomInputPopupMenuRef, RoomInputPopupMenuWidgetExt}, styles::*, text_or_image::{TextOrImageAction, TextOrImageRef, TextOrImageWidgetRefExt}, timestamp::TimestampWidgetRefExt
    },
    sliding_sync::{BackwardsPaginateUntilEventRequest, MatrixRequest, PaginationDirection, TimelineEndpoints, TimelineKind, TimelineRequestSender, UserPowerLevels, submit_async_request, take_timeline_endpoints, TimelineEndpointsRecreated}, utils::{self, ANIMATED_MEDIA_THUMBNAIL_FORMAT, MEDIA_THUMBNAIL_FORMAT, RoomNameId, unix_time_millis_to_datetime}
};
use crate::home::event_reaction_list::ReactionListWidgetRefExt;
use crate::home::backwards_pagination::BackwardsPaginationState;
use crate::home::scroll_anchors::ScrollAnchors;
use crate::home::state_event_group::{self, StateEventGroups};
use crate::home::small_state_event::{populate_small_state_event, populate_group_summary_item};
use crate::home::timeline_items::{ChangedItems, ItemDraw, PendingKnocks, TimelineInfo, date_divider_text, divider_span_end, index_of_event, item_draw, uses_compact_view};
use crate::room::{
    pane_dock::{RoomPaneDockAction, RoomPaneDockWidgetExt, RoomPaneDockWidgetRefExt, SavedRoomPane},
    pinned_messages_list::{PinnedMessagesListAction, confirm_unpin_message},
    room_action_bar::{RoomActionBarAction, RoomActionBarWidgetExt},
    room_members_list::{RoomMembersChanged, RoomMembersListAction, show_member_profile},
    room_pane::{self, RoomPaneKind},
};
use crate::home::failed_send_banner::{BlockedSend, FailedSendBannerWidgetExt};
use crate::home::send_status_indicator::{SendStatusIndicatorAction, SendStatusIndicatorRef, SendStatusIndicatorWidgetExt};
use crate::room::room_input_bar::RoomInputBarWidgetExt;
use crate::settings::app_preferences::{AppPreferencesAction, AppPreferencesGlobal, MarkAsReadBehavior, preferred_receipt_type};

use rangemap::RangeSet;

use super::{event_reaction_list::ReactionData, loading_pane::LoadingPaneRef, new_message_context_menu::{MessageAbilities, MessageDetails}, room_read_receipt::{self, populate_read_receipts}};

/// The maximum number of timeline items to search through
/// when looking for a particular event.
///
/// This is a safety measure to prevent the main UI thread
/// from getting into a long-running loop if an event cannot be found quickly.
const MAX_ITEMS_TO_SEARCH_THROUGH: usize = 100;

/// The max size (width or height) of a blurhash image to decode.
/// Blurhash is a blurred placeholder — it is designed to be decoded at a small
/// size and then stretched by the GPU. Decoding at large sizes is extremely
/// expensive (CPU-bound, O(width*height)) and completely unnecessary since the
/// result is inherently blurry. A 32×32 decode is ~240x faster than 500×500
/// while being visually indistinguishable when scaled up.
const BLURHASH_IMAGE_MAX_SIZE: u32 = 32;

/// How long after scrolling/interaction stops before we send read receipts.
const READ_RECEIPT_SEND_DELAY: f64 = 0.5;

/// How long after a backwards pagination fails before we try again, automatically or when the user scrolls up.
///
/// When offline, every try fails right away and pops up an error, so we shouldn't retry on every draw or scroll.
const RETRY_PAGINATION_AFTER_ERROR_DELAY: Duration = Duration::from_secs(3);


static UNNAMED_ROOM: &str = "Unnamed Room";

/// #FFF4E5
const COLOR_THREAD_SUMMARY_BG: Vec4 = vec4(1.0, 0.957, 0.898, 1.0);
/// #FFEACC
const COLOR_THREAD_SUMMARY_BG_HOVER: Vec4 = vec4(1.0, 0.918, 0.8, 1.0);


script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*


    mod.widgets.COLOR_BG = #xfff8ee
    mod.widgets.COLOR_OVERLAY_BG = #x000000d8
    mod.widgets.COLOR_READ_MARKER = #xeb2733

    mod.widgets.REACTION_TEXT_COLOR = #4c00b0

    // An empty item that takes up no space in the portal list.
    mod.widgets.ZeroHeightItem = #(ZeroHeightItem::register_widget(vm)) {
        width: Fill, height: 0
    }

    // A download button or loading spinner shown beneath a message.
    mod.widgets.MessageDownloadSection = View {
        visible: false,
        width: Fill, height: Fit,
        flow: Flow.Right{wrap: true},
        spacing: 8,
        wrap_spacing: 8
        margin: Inset{top: 8, bottom: 2}

        download_button := RobrixIconButton {
            height: mod.widgets.SETTINGS_BUTTON_HEIGHT,
            padding: Inset{left: 12, right: 12}
            margin: 0
            draw_icon.svg: (ICON_DOWNLOAD)
            icon_walk: Walk{width: 16, height: 16}
            text: "Download"
        }

        share_button := RobrixIconButton {
            height: mod.widgets.SETTINGS_BUTTON_HEIGHT,
            padding: Inset{left: 12, right: 12}
            margin: 0
            draw_icon.svg: (ICON_SHARE)
            icon_walk: Walk{width: 16, height: 16}
            text: "Share"
        }

        downloading_view := View {
            visible: false,
            width: Fit, height: mod.widgets.SETTINGS_BUTTON_HEIGHT
            flow: Right,
            align: Align{y: 0.5}
            spacing: 8,
            padding: Inset{left: 12, right: 6}

            spinner := LoadingSpinner {
                width: 16, height: 16
                draw_bg.color: (COLOR_ACTIVE_PRIMARY)
            }
            status_label := Label {
                width: Fit, height: Fit,
                padding: 0
                margin: 0
                draw_text +: {
                    text_style: REGULAR_TEXT { font_size: 11 },
                    color: (COLOR_ACTIVE_PRIMARY)
                }
                text: "Downloading…"
            }
            cancel_button := RobrixNegativeIconButton {
                height: mod.widgets.SETTINGS_BUTTON_HEIGHT,
                padding: Inset{left: 12, right: 12}
                margin: 0
                draw_icon.svg: (ICON_CLOSE)
                icon_walk: Walk{width: 16, height: 16}
                text: "Cancel"
            }
        }

        success_button := RobrixPositiveIconButton {
            visible: false,
            height: mod.widgets.SETTINGS_BUTTON_HEIGHT,
            padding: Inset{left: 12, right: 12}
            margin: 0
            draw_icon.svg: (ICON_CHECKMARK)
            icon_walk: Walk{width: 16, height: 16}
            text: "Downloaded"
        }

        failure_button := RobrixNegativeIconButton {
            visible: false,
            height: mod.widgets.SETTINGS_BUTTON_HEIGHT,
            padding: Inset{left: 12, right: 12}
            margin: 0
            draw_icon.svg: (ICON_CLOSE)
            icon_walk: Walk{width: 16, height: 16}
            text: "Download Failed"
        }
    }

    // A summary at the bottom of a message that is the root of a thread.
    mod.widgets.ThreadRootSummary = RoundedView {
        visible: false
        width: Fill,
        height: Fit
        flow: Right,
        align: Align{x: 0.0, y: 0.5}
        spacing: 5.0
        margin: Inset{ top: 5.0 }
        padding: 12,
        cursor: MouseCursor.Hand

        show_bg: true
        draw_bg +: {
            color: (mod.widgets.COLOR_THREAD_SUMMARY_BG)
            border_radius: 4.0
            border_size: 1.5
            border_color: (mod.widgets.COLOR_THREAD_SUMMARY_BORDER)
        }

        thread_summary_count := Label {
            width: Fit,
            draw_text +: {
                text_style: USERNAME_TEXT_STYLE { font_size: 11 }
                color: (mod.widgets.COLOR_THREAD_SUMMARY_REPLY_COUNT)
            }
            text: ""
        }

        Icon {
            width: Fit, height: Fit,
            align: Align{x: 0.5, y: 0.5}
            draw_icon +: {
                svg: crate_resource("self://resources/icons/double_chat.svg")
                color: (mod.widgets.COLOR_THREAD_SUMMARY_REPLY_COUNT)
            }
            icon_walk: Walk{ width: 25, height: 25, margin: Inset{top: 3, right: 7} }
        }

        thread_summary_latest := MessageHtml {
            max_lines: 2
            text_overflow: Ellipsis
            // A two-line preview keeps its paragraphs close together.
            paragraph_margin: Inset{ top: 0.33, bottom: 0.33 }
        }
    }

    // The view used for each text-based message event in a room's timeline.
    mod.widgets.Message = set_type_default() do #(Message::register_widget(vm)) {

        width: Fill,
        height: Fit,
        margin: 0.0
        flow: Down,
        cursor: MouseCursor.Default,
        padding: 0.0,
        spacing: 0.0

        show_bg: true
        draw_bg +: {
            highlight: instance(0.0)
            hover: instance(0.0)
            color: instance((COLOR_PRIMARY)) // default color)
            color_hover: instance(COLOR_LIST_ITEM_BG_HOVER)

            mentions_bar_color: instance(#0000)
            mentions_bar_width: instance(4.0)
            border_radius: uniform(4.0)
            border_inset: uniform(vec4(4.0, 0.0, 4.0, 0.0))

            pixel: fn() {
                let base_color = mix(
                    self.color,
                    self.color_hover,
                    self.hover
                );

                let with_highlight = mix(
                    base_color,
                    #c5d6fa,
                    self.highlight
                );

                let sdf = Sdf2d.viewport(self.pos * self.rect_size);

                // A mention's highlight covers the full width, while other highlights are inset.
                let not_mention = 1.0 - step(0.001, self.mentions_bar_color.w);
                let inset = self.border_inset * not_mention;

                // draw bg
                sdf.box(
                    inset.x,
                    inset.y,
                    self.rect_size.x - (inset.x + inset.z),
                    self.rect_size.y - (inset.y + inset.w),
                    self.border_radius
                );
                sdf.fill_keep(with_highlight);

                // draw the left vertical line
                sdf.rect(inset.x, 0., self.mentions_bar_width, self.rect_size.y);
                sdf.intersect(); // clip it to the bg's rounded corners
                sdf.fill(self.mentions_bar_color);

                return sdf.result;
            }
        }

        animator: Animator{
            highlight: {
                default: @off
                off: AnimatorState{
                    redraw: true,
                    from: { all: Forward {duration: 4.5} }
                    ease: InQuart
                    apply: { draw_bg: {highlight: 0.0} }
                }
                on: AnimatorState{
                    redraw: true,
                    from: { all: Forward {duration: 0.5} }
                    ease: ExpDecay {d1: 0.80, d2: 0.97}
                    apply: { draw_bg: {highlight: 1.0} }
                }
            }
            bg_hover: {
                default: @off
                off: AnimatorState{
                    redraw: true,
                    from: { all: Snap }
                    apply: { draw_bg: {hover: 0.0} }
                }
                on: AnimatorState{
                    redraw: true,
                    from: { all: Snap }
                    apply: { draw_bg: {hover: 1.0} }
                }
            }
        }

        // A preview of the earlier message that this message was in reply to.
        replied_to_message := mod.widgets.RepliedToMessage {
            flow: Down
            margin: Inset{ bottom: 3, top: 10 }
            preview_content +: {
                margin +: { left: 20 }
                padding +: { bottom: 10 }
            }
        }

        // The sender's avatar and username, which a condensed message hides.
        header := View {
            width: Fill,
            height: Fit
            flow: Right,
            padding: Inset{left: 8, right: 10},

            avatar := Avatar {
                width: 42,
                height: 42,
                // Centered over the timestamp column below it.
                margin: Inset{top: 7.5, bottom: 6.1, left: 4, right: 12}
            }
            username := Label {
                width: Fill,
                flow: Flow.Right { wrap: false },
                padding: 0,
                margin: Inset{top: 20.0, right: 10.0} // centers it on the avatar
                max_lines: 1
                text_overflow: Ellipsis
                draw_text +: {
                    text_style: USERNAME_TEXT_STYLE {},
                    color: (USERNAME_TEXT_COLOR)
                }
                text: "<Username not available>"
            }
        }

        body := View {
            width: Fill,
            height: Fit
            // Aligns the timestamp with the baseline of the content's first line
            flow: Flow.Right{row_align: RowAlign.Baseline},
            padding: Inset{top: 0, bottom: 7.5, left: 8, right: 10},

            profile := View {
                align: Align{x: 0.5, y: 0.0} // centered horizontally, top aligned
                width: 50.0,
                height: Fit,
                margin: Inset{right: 8}
                flow: Down,
                timestamp := Timestamp { }
                edited_indicator := EditedIndicator { }
                tsp_sign_indicator := TspSignIndicator { }
            }

            content := View {
                width: Fill,
                height: Fit
                flow: Down,
                padding: 0.0

                message := HtmlOrPlaintext { }
                link_preview_view := mod.widgets.LinkPreview {}
                download_section := mod.widgets.MessageDownloadSection {}
                View {
                    width: Fill,
                    height: Fit
                    flow: Right,
                    reaction_list := mod.widgets.ReactionList { }
                    avatar_row := mod.widgets.AvatarRow {}
                    send_status_indicator := mod.widgets.SendStatusIndicator {}
                }
                thread_root_summary := mod.widgets.ThreadRootSummary {}
            }
        }
    }

    // The view used for a condensed message that came right after another message
    // from the same sender, and thus doesn't need to display the sender's profile again.
    mod.widgets.CondensedMessage = mod.widgets.Message {
        padding: Inset{ top: 2.0, bottom: 2.0 }
        replied_to_message +: {
            preview_content +: {
                margin: Inset{ left: 55, bottom: 5.0 }
            }
        }
        header +: { visible: false }
        body +: {
            padding: Inset{ top: 2.5, bottom: 2.5, left: 8.0, right: 10.0 },
        }
    }

    // A single shared object on the script heap of type `Size::Fit{max: ...}`,
    // which is used for the max image thumbnail height for every `Image` widget
    // within a message widget.
    // Also see: `AppPreferences::on_thumbnail_max_height_changed`).
    mod.widgets.IMG_MSG_FIT = Fit{max: FitBound.Abs(300.0)}

    // The view used for each static image-based message event in a room's timeline.
    // This excludes stickers and other animated GIFs, video clips, audio clips, etc.
    mod.widgets.ImageMessage = mod.widgets.Message {
        body +: {
            content +: {
                message := View {
                    width: Fill, height: Fit,
                    flow: Down,
                    caption_view := View {
                        visible: false,
                        width: Fill, height: Fit,
                        margin: Inset{ bottom: 5.0 }
                        caption := HtmlOrPlaintext {}
                    }
                    image := TextOrImage {
                        image_view +: {
                            // The same spacing as timestamps in other text-based messages
                            baseline: Baseline.At(13.57)
                            image +: {
                                height: (mod.widgets.IMG_MSG_FIT)
                            }
                        }
                    }
                }
                download_section := mod.widgets.MessageDownloadSection {}
                View {
                    width: Fill,
                    height: Fit,
                    flow: Right,
                    reaction_list := mod.widgets.ReactionList { }
                    avatar_row := mod.widgets.AvatarRow {}
                    send_status_indicator := mod.widgets.SendStatusIndicator {}
                }
                thread_root_summary := mod.widgets.ThreadRootSummary {}
            }

        }
    }

    // The view used for a condensed image message that came right after another message
    // from the same sender, and thus doesn't need to display the sender's profile again.
    // This excludes stickers and other animated GIFs, video clips, audio clips, etc.
    mod.widgets.CondensedImageMessage = mod.widgets.CondensedMessage {
        body +: {
            content +: {
                message := View {
                    width: Fill, height: Fit,
                    flow: Down,
                    caption_view := View {
                        visible: false,
                        width: Fill, height: Fit,
                        margin: Inset{ bottom: 5.0 }
                        caption := HtmlOrPlaintext {}
                    }
                    image := TextOrImage {
                        image_view +: {
                            // The same spacing as timestamps in other text-based messages
                            baseline: Baseline.At(13.57)
                            image +: {
                                height: (mod.widgets.IMG_MSG_FIT)
                            }
                        }
                    }
                }
                download_section := mod.widgets.MessageDownloadSection {}
                View {
                    width: Fill,
                    height: Fit,
                    flow: Right,
                    reaction_list := mod.widgets.ReactionList { }
                    avatar_row := mod.widgets.AvatarRow {}
                    send_status_indicator := mod.widgets.SendStatusIndicator {}
                }
                thread_root_summary := mod.widgets.ThreadRootSummary {}
            }
        }
    }


    // The view used for each day divider in a room's timeline.
    // The date text is centered between two horizontal lines.
    mod.widgets.DateDivider = View {
        width: Fill,
        height: Fit,
        margin: Inset{top: 7.0, bottom: 7.0}
        flow: Right,
        padding: Inset{left: 7.0, right: 7.0},
        spacing: 0.0,
        align: Align{x: 0.5, y: 0.5} // center horizontally and vertically

        left_line := LineH { }

        date := Label {
            padding: Inset{left: 7.0, right: 7.0}
            draw_text +: {
                text_style: TEXT_SUB {},
                color: (COLOR_DIVIDER_DARK)
            }
            text: "<date>"
        }

        right_line := LineH { }
    }

    // The view used for the divider indicating where the user's last-viewed message is.
    // This is implemented as a DateDivider with a different color and a fixed text label.
    mod.widgets.ReadMarker = mod.widgets.DateDivider {
        left_line := LineH {
            draw_bg.color: (mod.widgets.COLOR_READ_MARKER)
        }

        date := Label {
            draw_text.color: (mod.widgets.COLOR_READ_MARKER)
            text: "New Messages"
        }

        right_line := LineH {
            draw_bg.color: (mod.widgets.COLOR_READ_MARKER)
        }
    }


    // The top space is used to display a loading message while the room is being paginated.
    mod.widgets.TopSpace = SolidView {
        visible: false,
        width: Fill,
        height: Fit,
        align: Align{x: 0.5, y: 0}
        flow: Right,
        show_bg: true,
        draw_bg.color: #xDAF5E5F0, // mostly opaque light green

        label := Label {
            width: Fill,
            height: Fit,
            align: Align{x: 0.5, y: 0.5},
            flow: Flow.Right { wrap: true },
            padding: Inset{ top: 10.0, bottom: 7.0, left: 15.0, right: 15.0 }
            draw_text +: {
                text_style: MESSAGE_TEXT_STYLE { font_size: 10 },
                color: (TIMESTAMP_TEXT_COLOR)
            }
            text: "Loading earlier messages..."
        }
    }

    mod.widgets.Timeline = View {
        width: Fill,
        height: Fill,
        align: Align{x: 0.5, y: 0.0} // center horizontally, align to top vertically
        flow: Overlay,
        new_batch: true

        list := PortalList {
            height: Fill,
            width: Fill
            flow: Down
            scroll_bar: ListScrollBar {}

            auto_tail: true, // set to `true` to lock the view to the last item.
            // `draw_walk()` turns this on once the timeline is fully paginated;
            // before that, reaching the top back-paginates instead of bouncing.
            bounce_at_start: false,
            bounce_at_end: true,
            // Read-receipt logic listens for scroll position changes.
            emit_scroll_actions: true,
            // Prefetch older history shortly before the user actually hits the top.
            reached_start_margin: 2,
            // TODO: enable `reuse_items: true` once Makepad's Html/TextFlow widget
            //   properly resets all internal state during `script_apply(Reload)`.
            //   Currently, stale TextFlow layout state (particularly related to
            //   list items) leaks through when a widget is recycled, causing
            //   excessive whitespace in HTML messages with `<ul>`/`<ol>` lists.

            // Below, we must place all of the possible templates (views) that can be used in the portal list.
            Message := mod.widgets.Message {}
            CondensedMessage := mod.widgets.CondensedMessage {}
            ImageMessage := mod.widgets.ImageMessage {}
            CondensedImageMessage := mod.widgets.CondensedImageMessage {}
            SmallStateEvent := mod.widgets.SmallStateEvent {}
            GroupSummaryItem := mod.widgets.GroupSummaryItem {}
            ZeroHeightItem := mod.widgets.ZeroHeightItem {}
            DateDivider := mod.widgets.DateDivider {}
            ReadMarker := mod.widgets.ReadMarker {}
        }

        // The top space is displayed as an overlay at the top of the timeline.
        top_space := mod.widgets.TopSpace { }

        // A jump to bottom button (with an unread message badge) that is shown
        // when the timeline is not at the bottom.
        jump_to_bottom_button := JumpToBottomButton { }
    }


    mod.widgets.RoomScreen = #(RoomScreen::register_widget(vm)) {
        width: Fill, height: Fill,
        cursor: MouseCursor.Default,
        flow: Down,
        spacing: 0.0

        room_actions := mod.widgets.RoomActionBar {}

        room_screen_wrapper := SolidView {
            width: Fill, height: Fill,
            flow: Overlay,

            show_bg: true
            draw_bg.color: (COLOR_PRIMARY_DARKER)

            restore_status_view := RestoreStatusView {}

            // This used to be a KeyboardView wrapper, but now the on-screen keyboard shift
            // is handled by the top-level Window.
            timeline_and_input_bar := View {
                width: Fill, height: Fill,
                flow: Down,

                // First, display the timeline of all messages/events,
                // surrounded by any of this room's panes that are docked around it.
                room_pane_dock := mod.widgets.RoomPaneDock {
                    body +: { mid +: { center +: {
                        timeline := mod.widgets.Timeline { }
                    }}}
                }

                // Below that, display a typing notice when other users in the room are typing.
                typing_notice := TypingNotice { }

                // Below that, warn about a failed message that is holding up this room's send queue.
                failed_send_banner := FailedSendBanner { }

                room_input_bar := RoomInputBar { }
            }

            // Note: here, we're within a View that has an Overlay flow,
            // so the order that we define the below views determines which one is on top.

            // The user profile sliding pane should be displayed on top of other "static" subviews
            // (on top of all other views that are always visible).
            user_profile_sliding_pane := mod.widgets.UserProfileSlidingPane { }

            // The loading pane appears while the user is waiting for something in the room screen
            // to finish loading, e.g., when loading an older replied-to message.
            loading_pane := LoadingPane { }

            // The popup menu for uploading/sending other content to this room,
            // which is controlled by actions from the RoomInputBar.
            room_input_popup_menu := RoomInputPopupMenu { }


            /*
             * TODO: add the action bar back in as a series of floating buttons.
             *
            message_action_bar_popup := PopupNotification {
                align: Align{x: 0.0, y: 0.0}
                content: {
                    height: Fit,
                    width: Fit,
                    show_bg: false,
                    align: Align{
                        x: 0.5,
                        y: 0.5
                    }

                    message_action_bar := MessageActionBar {}
                }
            }
            */
        }
    }
}

/// Tracks when the user has seen a timeline event, for sending read receipts.
///
/// A short timer starts upon direct user interaction with the timeline.
/// When the timer fires, receipts are sent for the visible events,
/// unless the user was scrolling up towards earlier/older messages.
#[derive(Default)]
struct ReadReceiptState {
    timer: Timer,
    /// Whether the pending timer came from direct user input (`true`)
    /// or programmatic movement (`false`). We only send read receipts if true.
    from_user_input: bool,
    /// The timeline's `user_scroll_travel` when the timer was started.
    user_scroll_travel_at_timer_start: f64,
    /// The timeline's `user_scroll_travel` when we last sampled it.
    last_user_scroll_travel: f64,
}

impl ReadReceiptState {
    /// Starts the read receipt timer for a direct user interaction.
    ///
    /// If a timer was already running, it gets restarted because that means
    /// the scrolling action has continued.
    fn start_timer(&mut self, cx: &mut Cx, portal_list: &PortalListRef) {
        let was_already_pending = !self.timer.is_empty() && self.from_user_input;
        cx.stop_timer(self.timer);
        self.timer = cx.start_timeout(READ_RECEIPT_SEND_DELAY);
        self.from_user_input = true;
        if !was_already_pending {
            self.user_scroll_travel_at_timer_start = portal_list.user_scroll_travel();
        }
    }

    /// Cancels any pending read receipt send.
    fn cancel_timer(&mut self, cx: &mut Cx) {
        cx.stop_timer(self.timer);
        self.clear();
    }

    /// Forgets a pending read receipt send, but doesn't stop the timer.
    fn clear(&mut self) {
        self.timer = Timer::empty();
        self.from_user_input = false;
    }

    /// Reacts to the timeline having scrolled, based on whether the user
    /// caused the movement and in which direction.
    fn handle_scroll(&mut self, cx: &mut Cx, portal_list: &PortalListRef) {
        let user_travel = portal_list.user_scroll_travel();
        let did_scroll_up = user_travel > self.last_user_scroll_travel;
        let did_scroll_down = user_travel < self.last_user_scroll_travel;
        self.last_user_scroll_travel = user_travel;
        if did_scroll_up {
            // Scrolling up to earlier messages doesn't mean the user read them!
            // they might've just been scrolling up to find an old message quickly.
            self.cancel_timer(cx);
        }
        else if did_scroll_down {
            self.start_timer(cx, portal_list);
        }
        // If this movement wasn't from a user interaction, restart the timer
        // unless we're at the bottom of a room where new messages are being appended.
        else if !self.timer.is_empty() && !portal_list.is_at_end() {
            cx.stop_timer(self.timer);
            self.timer = cx.start_timeout(READ_RECEIPT_SEND_DELAY);
        }
    }

    /// If this is a timer event, clears the pending send and returns whether
    /// read receipts should actually be sent for the currently-visible events.
    fn should_send_on_timer_event(&mut self, event: &Event, portal_list: &PortalListRef) -> bool {
        if self.timer.is_event(event).is_none() {
            return false;
        }
        let from_user_input = self.from_user_input;
        let did_scroll_up = portal_list.user_scroll_travel() > self.user_scroll_travel_at_timer_start;
        self.clear();
        from_user_input && !did_scroll_up
    }

    /// Reads the last user scroll travel when a new timeline is first shown.
    fn on_timeline_shown(&mut self, portal_list: &PortalListRef) {
        self.last_user_scroll_travel = portal_list.user_scroll_travel();
    }
}


/// Returns the user's display name in the given room, falling back to their user ID.
fn display_name_or_user_id(cx: &mut Cx, room_id: &OwnedRoomId, user_id: OwnedUserId) -> String {
    user_profile_cache::get_user_display_name_for_room(cx, user_id.clone(), Some(room_id), false)
        .into_option()
        .unwrap_or_else(|| user_id.to_string())
}

/// The main widget that displays a single Matrix room.
#[derive(Script, Widget)]
pub struct RoomScreen {
    #[deref] view: View,

    /// The name and ID of the currently-shown room, if any.
    #[rust] room_name_id: Option<RoomNameId>,
    /// The timeline currently displayed by this RoomScreen, if any.
    #[rust] timeline_kind: Option<TimelineKind>,
    /// The persistent UI-relevant states for the room that this widget is currently displaying.
    #[rust] tl_state: Option<TimelineUiState>,
    /// The set of pinned events in this room.
    #[rust] pinned_events: Vec<OwnedEventId>,
    /// Whether this room has been successfully loaded (received from the homeserver).
    #[rust] is_loaded: bool,
    /// Whether or not all rooms have been loaded (received from the homeserver).
    #[rust] all_rooms_loaded: bool,
    /// A flag to set key focus for the text input after it has been drawn.
    #[rust] focus_input_bar_on_show: bool,
    /// The timeline's `user_scroll_travel()` as of the last event, to tell which way the user scrolls.
    #[rust] last_scroll_travel: f64,
    #[rust] cached_refs: Option<RoomScreenWidgetRefs>,
    /// Decides when to send read receipts; see [`ReadReceiptState`].
    #[rust] read_receipt_state: ReadReceiptState,
    /// The user whose read receipt we're currently waiting to jump to, if any.
    /// This lets us ignore a response that arrives after the user gave up on it.
    #[rust] pending_read_receipt_jump: Option<OwnedUserId>,
    /// A jump that's waiting for the timeline to be drawn with its latest items.
    #[rust] deferred_jump: Option<DeferredJump>,
}

/// Cached references to RoomScreen child widgets used in every event handler.
#[derive(Clone)]
struct RoomScreenWidgetRefs {
    portal_list: PortalListRef,
    user_profile_sliding_pane: UserProfileSlidingPaneRef,
    loading_pane: LoadingPaneRef,
    room_input_popup_menu: RoomInputPopupMenuRef,
}

impl Drop for RoomScreen {
    fn drop(&mut self) {
        // This ensures that the `TimelineUiState` instance owned by this room is *always* returned
        // back to the timeline state store, which ensures that its UI state(s) are not lost
        // and that other RoomScreen instances can show this room in the future.
        // RoomScreen will be dropped whenever its widget instance is destroyed, e.g.,
        // when a Tab is closed or the app is resized to a different AdaptiveView layout.
        self.hide_timeline();
    }
}

impl ScriptHook for RoomScreen {
    fn on_after_reload(&mut self, vm: &mut ScriptVm) {
        // A script reload changes the RoomScreen's children; invalidate the ones we cached.
        self.cached_refs = None;
        vm.with_cx_mut(|cx| {
            if let Some(tl_state) = &mut self.tl_state.as_mut() {
                // Clear the timeline's drawn items caches and redraw it.
                tl_state.content_drawn_since_last_update.clear();
                tl_state.profile_drawn_since_last_update.clear();
                // The reapply also resets the RoomInputBar, so we need to re-update its state.
                let room_input_bar = self.view.room_input_bar(cx, ids!(room_input_bar));
                room_input_bar.update_room_state(
                    cx,
                    tl_state.kind.room_id(),
                    tl_state.tombstone_info.as_ref(),
                    tl_state.user_power,
                );
                // Restore the send button's flag, colors, and encryption state too.
                room_input_bar.update_encryption_state(cx, tl_state.is_encrypted);
                self.view.redraw(cx);
            }
        });
    }
}

impl Widget for RoomScreen {
    // Handle events and actions for the RoomScreen widget and its inner Timeline view.
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, _scope: &mut Scope) {
        // Skip event handling if this RoomScreen is uninitialized (a background dock tab after dock restore).
        if self.tl_state.is_none() && self.room_name_id.is_none() {
            return;
        }

        let room_screen_widget_uid = self.widget_uid();
        let RoomScreenWidgetRefs {
            portal_list,
            user_profile_sliding_pane,
            loading_pane,
            room_input_popup_menu,
        } = self.cached_widget_refs(cx);

        let is_pane_shown = loading_pane.is_currently_shown(cx)
            || user_profile_sliding_pane.is_currently_shown(cx);
        let is_popup_menu_open = room_input_popup_menu.is_open();

        // Only direct interaction with the timeline itself can send a read receipt;
        // the direction of any scrolling gets checked later via `user_scroll_travel()`.
        let interaction_pos = match event {
            Event::MouseDown(e) => Some(e.abs),
            Event::Scroll(e) if e.scroll.y != 0.0 => Some(e.abs),
            Event::TouchUpdate(e) => e.touches.first()
                .filter(|t| matches!(t.state, TouchState::Start))
                .map(|t| t.abs),
            _ => None,
        };
        if interaction_pos.is_some_and(|pos| portal_list.area().rect(cx).contains(pos))
            // Interactions aimed at one of our own panes don't count, nor do those aimed
            // at an app-level overlay (modals and menus block scrolling while open).
            && !is_pane_shown
            && !is_popup_menu_open
            && cx.is_scrolling_allowed_within(&portal_list.area())
        {
            self.read_receipt_state.start_timer(cx, &portal_list);
        }
        // we wanna make sure to check that direct user input occurred to avoid
        // sending read receipts for auto-tailed messages that appeared on screen
        // but that the user may not have necessarily seen.
        if self.read_receipt_state.should_send_on_timer_event(event, &portal_list)
            && !loading_pane.is_currently_shown(cx)
        {
            self.send_read_receipts_for_visible_events(cx, &portal_list);
        }

        // Perform a jump that was waiting for the timeline to be drawn.
        if let Some(jump) = self.deferred_jump.as_ref() && jump.frame.is_event(event).is_some() {
            if self.tl_state.as_ref().is_some_and(|tl| tl.items.len() != jump.num_items) {
                // If the timeline changed after it was drawn, wait for it to be drawn again.
                self.redraw(cx);
            }
            else if let Some(jump) = self.deferred_jump.take() {
                match jump.kind {
                    DeferredJumpKind::Search { event_id, description } if !loading_pane.is_searching_for(&event_id) => {
                        loading_pane.hide(cx);
                        self.jump_to_event(cx, &event_id, None, description, &portal_list, &loading_pane);
                    }
                    DeferredJumpKind::ScrollTo { event_id } => {
                        if let Some(index) = self.tl_state.as_ref()
                            .and_then(|tl| index_of_event(&tl.items, &event_id, tl.items.len(), usize::MAX))
                        {
                            self.scroll_to_event(cx, &portal_list, index, event_id);
                        }
                    }
                    _ => {}
                }
            }
        }

        // Handle actions here before processing timeline updates.
        // Normally (in most other widgets), the order of event handling doesn't matter much.
        // However, since actions may refer to a specific timeline item's index,
        // we want to handle those before processing any updates that might change
        // the set of timeline indices (which would invalidate the index values in any actions).
        if let Event::Actions(actions) = event {
            if let Some(action) = room_input_popup_menu.selected(actions) {
                self.handle_room_input_popup_menu_action(cx, action);
            }

            let mut toggled_group_index = None;
            for (index, wr) in portal_list.items_with_actions(actions) {
                // Handle a hover-in action on the reaction list: show a reaction summary.
                let reaction_list = wr.reaction_list(cx, ids!(reaction_list));
                if let RoomScreenTooltipActions::HoverInReactionButton {
                    widget_rect,
                    reaction_data,
                } = reaction_list.hovered_in(actions) {
                    let Some(_tl_state) = self.tl_state.as_ref() else { continue };
                    let mut tooltip_text = room_read_receipt::tooltip_list_of_users(
                        cx,
                        reaction_data.reaction_senders.keys(),
                        &reaction_data.room_id,
                    );
                    tooltip_text.push_str(&format!(" reacted with: {}", reaction_data.reaction));
                    cx.widget_action(
                        room_screen_widget_uid, 
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

                // Handle a hover-out action on the reaction list or an avatar row.
                let avatar_rows = state_event_group::avatar_rows(cx, &wr);
                if (reaction_list.hovered_out(actions) || avatar_rows.iter().any(|row| row.hover_out(actions)))
                    // Don't hover out if any current actions are about to hover in and show any tooltip.
                    // This prevents a brief flicker when hovering out of one tooltip to hovering into another one immediately.
                    && !actions.iter().any(|a| a.as_widget_action().is_some_and(|wa|
                        matches!(
                            wa.action.downcast_ref::<TooltipAction>(),
                            Some(TooltipAction::HoverIn { .. }),
                        )
                        || matches!(
                            wa.action.downcast_ref::<RoomScreenTooltipActions>(),
                            Some(RoomScreenTooltipActions::HoverInReactionButton { .. }
                                | RoomScreenTooltipActions::HoverInReadReceipt { .. }
                            ),
                        )
                    ))
                {
                    cx.widget_action(
                        room_screen_widget_uid, 
                        TooltipAction::HoverOut,
                    );
                }

                // Handle a hover-in action on an avatar row: show a read receipts summary.
                if let Some(RoomScreenTooltipActions::HoverInReadReceipt {
                    widget_rect,
                    read_receipts
                }) = avatar_rows.iter()
                    .map(|row| row.hover_in(actions))
                    .find(|action| matches!(action, RoomScreenTooltipActions::HoverInReadReceipt { .. }))
                {
                    let Some(room_id) = self.room_id() else { return; };
                    let tooltip_text= room_read_receipt::populate_tooltip(cx, read_receipts, room_id);
                    cx.widget_action(
                        room_screen_widget_uid, 
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
                let content_message = wr.text_or_image(cx, ids!(content.message.image));
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

                // Handle a click on the header of a small state event group, and also
                // a click on the collapse button/line after the last event in an expanded group.
                if toggled_group_index != Some(index) && state_event_group::group_toggled(cx, &wr, actions) {
                    toggled_group_index = Some(index);
                    self.toggle_state_event_group(cx, index, &portal_list);
                    continue;
                }

                // Handle the invite_user_button (in a SmallStateEvent) being clicked.
                if wr.button(cx, ids!(invite_user_button)).clicked(actions) {
                    let Some(tl) = self.tl_state.as_ref() else { continue };
                    if let Some(event_tl_item) = tl.items.get(index).and_then(|item| item.as_event()) {
                        let user_id = event_tl_item.sender().to_owned();
                        let username = utils::get_or_fetch_event_sender(event_tl_item, None);
                        let room_id = tl.kind.room_id().clone();
                        let content = ConfirmationModalContent {
                            title_text: "Send Invitation".into(),
                            body_text: format!("Are you sure you want to invite {username} to this room?").into(),
                            accept_button_text: Some("Invite".into()),
                            on_accept_clicked: Some(Box::new(move |_cx| {
                                submit_async_request(MatrixRequest::InviteUser { room_id, user_id });
                            })),
                            ..Default::default()
                        };
                        cx.action(InviteAction::ShowInviteConfirmationModal(RefCell::new(Some(content))));
                    }
                }
            }

            self.handle_message_actions(cx, actions, &portal_list, &loading_pane);

            // If we're showing the room member pane, refresh this timeline's member list.
            if actions.iter().any(|a| a.downcast_ref::<RoomMembersChanged>()
                .is_some_and(|c| self.tl_state.as_ref().is_some_and(|tl| c.room_id == *tl.kind.room_id())))
            {
                self.refresh_members_pane(cx);
            }

            let room_pane_dock_uid = self.view.room_pane_dock(cx, ids!(room_pane_dock)).widget_uid();
            for action in actions {
                // Highlight the buttons in the room action bar for any panes that are currently shown.
                if let RoomPaneDockAction::ShownPanesChanged(kinds) = action.as_widget_action().widget_uid_eq(room_pane_dock_uid).cast() {
                    self.view.room_action_bar(cx, ids!(room_actions)).set_shown_panes(cx, kinds);
                    continue;
                }

                // Our timeline may show users whose profiles were just fetched, e.g., for an item whose sender wasn't known.
                if action.downcast_ref::<user_profile_cache::UserProfilesUpdated>().is_some() {
                    self.redraw(cx);
                    continue;
                }

                // If the backend sync task rebuilt this room's timeline, our timeline update receiver is dead,
                // so we need to get a new one.
                if let Some(TimelineEndpointsRecreated { room_id }) = action.downcast_ref()
                    && self.timeline_kind.as_ref().is_some_and(|k| k.room_id() == room_id)
                {
                    self.reconnect_timeline_endpoints(cx, true);
                    continue;
                }

                if let Some(AppStateAction::RoomNameUpdated(new_room_name)) = action.downcast_ref()
                    && let Some(room_name_id) = self.room_name_id.as_mut()
                    && room_name_id.room_id() == new_room_name.room_id()
                {
                    *room_name_id = new_room_name.clone();
                    continue;
                }

                // Handle actions related to restoring the previously-saved state of rooms.
                if let Some(AppStateAction::RoomLoadedSuccessfully { room_name_id, ..}) = action.downcast_ref() {
                    if self.room_name_id.as_ref().is_some_and(|rn| rn.room_id() == room_name_id.room_id()) {
                        let was_shown = self.tl_state.is_some();
                        // `set_displayed_room()` does nothing if the room_name_id is unchanged, so we clear it first.
                        self.room_name_id = None;
                        let thread_root_event_id = self.timeline_kind.as_ref()
                            .and_then(|k| k.thread_root_event_id().cloned());
                        self.set_displayed_room(cx, room_name_id, thread_root_event_id);
                        if was_shown {
                            // If the timeline was already shown, continue processing actions for it.
                            continue;
                        }
                        return;
                    }
                }

                // Once we resolve a clicked link, navigate to that destination (unless the user canceled it already).
                if let Some(RoomLinkResolved { link, result }) = action.downcast_ref()
                    && loading_pane.is_resolving_link(link)
                {
                    match result {
                        Ok(destination) => {
                            loading_pane.hide(cx);
                            self.show_link_destination(cx, link, destination, &loading_pane, &portal_list);
                        }
                        Err(error_message) => loading_pane.show_error(cx, error_message.clone()),
                    }
                    continue;
                }

                // Handle InviteResultAction to show popup notifications.
                if let Some(InviteResultAction::Sent { room_id, .. }) = action.downcast_ref() {
                    // Only handle if this is for the current room.
                    if self.room_name_id.as_ref().is_some_and(|rn| rn.room_id() == room_id) {
                        enqueue_popup_notification(
                            "Sent invite successfully.",
                            PopupKind::Success,
                            Some(4.0),
                        );
                    }
                }
                if let Some(InviteResultAction::Failed { room_id, error, .. }) = action.downcast_ref() {
                    // Only handle if this is for the current room.
                    if self.room_name_id.as_ref().is_some_and(|rn| rn.room_id() == room_id) {
                        enqueue_popup_notification(
                            format!("Failed to send invite.\n\nError: {error}"),
                            PopupKind::Error,
                            None,
                        );
                    }
                }

                // When transitioning from offline to online, abort all pending downloads
                // and clear stale `Requested`/`Failed` entries from per-room caches so they can be re-fetched.
                if let Some(RoomsListHeaderAction::StateUpdate(new_state)) = action.downcast_ref() {
                    if matches!(new_state, State::Offline) {
                        if let Some(tl) = self.tl_state.as_mut() {
                            // Tell the worker to abort every in-flight download
                            // for this room before we drop them from local state.
                            for entry in tl.pending_downloads.drain(..) {
                                submit_async_request(MatrixRequest::CancelDownload(entry.mxc));
                            }
                            self.view.portal_list(cx, ids!(timeline.list)).redraw(cx);
                        }
                    } else if let Some(tl) = self.tl_state.as_mut() {
                        tl.media_cache.clear_all_pending_and_failed_requests();
                        tl.link_preview_cache.clear_all_pending_and_failed_requests();
                        tl.content_drawn_since_last_update.clear();
                        self.view.portal_list(cx, ids!(timeline.list)).redraw(cx);
                        // Retry syncing members that failed to sync while we were offline.
                        self.refresh_members_pane(cx);
                    }
                    continue;
                }

                if let Some(AppPreferencesAction::ShowTypingNoticesChanged(show)) = action.downcast_ref() {
                    if !*show {
                        self.view.typing_notice(cx, ids!(typing_notice)).show_or_hide(cx, &[], Animate::No);
                    }
                    // Only change the typing subscription for a loaded main room that we're still in.
                    if self.is_loaded
                        && let Some(tl) = self.tl_state.as_ref()
                        && matches!(tl.kind, TimelineKind::MainRoom { .. })
                        && !timeline_state_store::is_invalidated(&tl.kind)
                    {
                        submit_async_request(MatrixRequest::SubscribeToTypingNotices {
                            room_id: tl.kind.room_id().clone(),
                            subscribe: *show,
                        });
                    }
                    continue;
                }

                // Handle the highlight animation for a message.
                let Some(tl) = self.tl_state.as_mut() else { continue };
                if let MessageHighlightAnimationState::Pending { item_id, .. } = tl.message_highlight_animation_state {
                    if portal_list.smooth_scroll_reached(actions) {
                        cx.widget_action(
                            room_screen_widget_uid, 
                            MessageAction::HighlightMessage(item_id),
                        );
                        // State events aren't yet treated as regular events, so just handle their highlight here and now.
                        // TODO: treat messages and events similarly so you can do things like:
                        //       reply to an event, jump to an event, right-click on an event, etc
                        if let Some((_, item)) = portal_list.get_item(item_id) {
                            state_event_group::highlight_small_state_event(cx, &item);
                        }
                        tl.message_highlight_animation_state = MessageHighlightAnimationState::Off;
                    }
                }
            }

            /*
            // close message action bar if scrolled.
            if portal_list.scrolled(actions) {
                let message_action_bar_popup = self.popup_notification(cx, ids!(message_action_bar_popup));
                message_action_bar_popup.close(cx);
            }
            */

            // Back paginate the timeline when the start of the timeline comes into view.
            self.send_pagination_request_on_reached_start(cx, actions, &portal_list);

            // Once scrolling stops, the read receipt timer determines which events are
            // actually visible and sends read receipts for them.
            if portal_list.scrolled(actions) {
                self.read_receipt_state.handle_scroll(cx, &portal_list);
            }

            // Handle the jump to bottom button: update its visibility, and handle clicks.
            self.jump_to_bottom_button(cx, ids!(jump_to_bottom_button)).update_from_actions(
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
                    let thread_root_event_id = self.timeline_kind.as_ref()
                        .and_then(|k| k.thread_root_event_id().cloned());
                    // This room has been loaded now, so we call `set_displayed_room()`.
                    // We first clear the `room_name_id`, otherwise that function will do nothing.
                    self.room_name_id = None;
                    self.set_displayed_room(cx, &room_name_clone, thread_root_event_id);
                } else {
                    self.all_rooms_loaded = rooms_list_ref.all_rooms_loaded();
                    return;
                }
            }

            // If this RoomScreen is waiting to show a thread timeline (not the main room timeline),
            // then we need to retry showing the timeline now (upon a Signal),
            // because the thread timeline may have been successfully created.
            if self.tl_state.is_none() && self.timeline_kind.is_some() {
                self.show_timeline(cx);
            }

            self.process_timeline_updates(cx, &portal_list);
        }

        // Forward the event to the inner timeline view, but capture any actions it produces
        // such that we can handle the ones relevant to only THIS RoomScreen widget right here and now,
        // ensuring they are not mistakenly handled by other RoomScreen widget instances.
        // When an overlay pane is shown, all "interactive" user inputs are only forwarded to it.
        // The popup menu allows events to fall through, but they do dismiss it.
        let mut actions_generated_within_this_room_screen = cx.capture_actions(|cx| {
            if is_pane_shown && utils::is_interactive_hit_event(event) {
                if loading_pane.is_currently_shown(cx) {
                    loading_pane.handle_event(cx, event, &mut Scope::empty());
                } else {
                    user_profile_sliding_pane.handle_event(cx, event, &mut Scope::empty());
                }
            } else if is_popup_menu_open && room_input_popup_menu.is_event_within_popup_menu(cx, event) {
                room_input_popup_menu.handle_event(cx, event, &mut Scope::empty());
            } else {
                self.view.handle_event(cx, event, &mut Scope::empty());
            }
        });

        let scroll_travel = portal_list.user_scroll_travel();
        let last_scroll_travel = std::mem::replace(&mut self.last_scroll_travel, scroll_travel);

        // Scrolling up while the start of the timeline is showing still kicks off back pagination,
        // even if the timeline is too short to actually move or be scrolled.
        // Also covers the case when pagination failed (see [`RETRY_PAGINATION_AFTER_ERROR_DELAY`]).
        let scrolled_up = scroll_travel > last_scroll_travel;
        if scrolled_up
            && let Some(tl) = self.tl_state.as_mut()
            && !tl.backwards_pagination.is_fully_paginated()
            && !tl.backwards_pagination.is_loading()
            && !tl.failed_recently()
            && !tl.has_older_content(portal_list.first_id())
        {
            tl.paginate_backwards();
        }

        // Here, we handle and remove any general actions that are relevant to only this RoomScreen.
        // Removing the handled actions ensures they are not mistakenly handled by other RoomScreen widget instances.
        actions_generated_within_this_room_screen.retain(|action| {
            if self.handle_link_clicked(cx, action, &user_profile_sliding_pane, &loading_pane, &portal_list) {
                return false;
            }

            // Handle actions related to the room input popup menu.
            match action.as_widget_action().cast() {
                RoomInputPopupMenuAction::None => {}
                room_popup_menu_action => {
                    self.handle_room_input_popup_menu_action(cx, room_popup_menu_action);
                    return false;
                }
            }

            // Handle the action that requests to show the user profile sliding pane.
            if let ShowUserProfileAction::ShowUserProfile(profile_and_room_id) = action.as_widget_action().cast() {
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

            // Handle a button being clicked in this room's action bar.
            match action.as_widget_action().cast() {
                RoomActionBarAction::LayoutChanged { .. } | RoomActionBarAction::None => {}
                bar_action => {
                    self.handle_room_action_bar_action(cx, bar_action);
                    return false;
                }
            }

            // Handle a member being clicked in the room member pane.
            if let RoomMembersListAction::MemberClicked { room_name_id, member } = action.as_widget_action().cast() {
                show_member_profile(cx, &user_profile_sliding_pane, &room_name_id, member);
                self.redraw(cx);
                return false;
            }

            // Handle a message being clicked in the pinned messages pane.
            if let PinnedMessagesListAction::MessageClicked { timeline_kind, event_id, description, .. } = action.as_widget_action().cast() {
                if !self.is_event_in_this_timeline(&timeline_kind, &event_id) {
                    // Our parent will show the timeline that contains this message,
                    // so jump to it in that room screen's timeline instead of here.
                    return true;
                }
                self.jump_to_event_after_draw(cx, event_id, description);
                return false;
            }

            // Handle a request to jump to a given user's latest read receipt (last-seen event).
            if let UserProfilePaneAction::JumpToReadReceipt(user_id) = action.as_widget_action().cast() {
                let Some(timeline_kind) = self.tl_state.as_ref().map(|tl| tl.kind.clone()) else {
                    error!("BUG: can't jump to {user_id}'s read receipt with no timeline kind.");
                    return false;
                };
                submit_async_request(MatrixRequest::GetUserReadReceipt {
                    timeline_kind,
                    user_id: user_id.clone(),
                });
                self.pending_read_receipt_jump = Some(user_id);
                return false;
            }

            /*
            match action.as_widget_action().widget_uid_eq(room_screen_widget_uid).cast() {
                MessageAction::ActionBarClose => {
                    let message_action_bar_popup = self.popup_notification(cx, ids!(message_action_bar_popup));
                    let message_action_bar = message_action_bar_popup.message_action_bar(cx, ids!(message_action_bar));

                    // close only if the active message is requesting it to avoid double closes.
                    if let Some(message_widget_uid) = message_action_bar.message_widget_uid() {
                        if action.as_widget_action().widget_uid_eq(message_widget_uid).is_some() {
                            message_action_bar_popup.close(cx);
                        }
                    }
                }
                MessageAction::ActionBarOpen { item_id, message_rect } => {
                    let message_action_bar_popup = self.popup_notification(cx, ids!(message_action_bar_popup));
                    let message_action_bar = message_action_bar_popup.message_action_bar(cx, ids!(message_action_bar));

                    let margin_x = 50.;

                    let coords = dvec2(
                        (message_rect.pos.x + message_rect.size.x) - margin_x,
                        message_rect.pos.y,
                    );

                    script_apply_eval!(cx, message_action_bar_popup, {
                        content +: { margin +: { left: #(coords.x), top: #(coords.y) } }
                    });

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
        self.update_top_space_visibility(cx);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        // If the room isn't loaded yet, we show the restore status label only.
        if !self.is_loaded {
            let Some(room_name) = &self.room_name_id else {
                // No room selected yet, nothing to show.
                return DrawStep::done();
            };
            let mut restore_status_view = self.view.restore_status_view(cx, ids!(restore_status_view));
            restore_status_view.set_content(cx, self.all_rooms_loaded, room_name);
            return restore_status_view.draw(cx, scope);
        }
        if self.tl_state.is_none() {
            // Tl_state may not be ready after dock loading.
            // If return DrawStep::done() inside self.view.draw_walk, turtle will misalign and panic.
            return DrawStep::done();
        }

        self.update_top_space_visibility(cx);
        let top_space = self.view.view(cx, ids!(top_space));

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

            // Set the portal list's range based on the number of timeline items.
            let tl_items = &tl_state.items;
            let timeline = TimelineInfo { items: tl_items, kind: &tl_state.kind, pending_knocks: &tl_state.pending_knocks };
            let last_item_id = tl_items.len();

            let list = list_ref.deref_mut();
            list.set_item_range(cx, 0, last_item_id);
            // Set the ranges of collapsed groups, such that the portallist will skip iterating over them
            // instead of drawing them only for them to be invisible anyway, a big perf win!
            list.set_skipped_ranges(tl_state.state_event_groups.collapsed_ranges());
            list.bounce_at_start = tl_state.backwards_pagination.is_fully_paginated();

            while let Some(item_id) = list.next_visible_item(cx) {
                let item = {
                    let tl_idx = item_id;
                    let Some(draw) = item_draw(timeline, &tl_state.state_event_groups, tl_idx) else {
                        // This shouldn't happen (unless the timeline gets corrupted or some other weird error),
                        // but we can always safely fill the item with an empty widget that takes up no space.
                        list.item(cx, item_id, id!(ZeroHeightItem));
                        continue;
                    };

                    // Determine whether this item's content and profile have been drawn since the last update.
                    // Pass this state to each of the `populate_*` functions so they can attempt to re-use
                    // an item in the timeline's portallist that was previously populated, if one exists.
                    let item_drawn_status = ItemDrawnStatus {
                        content_drawn: tl_state.content_drawn_since_last_update.contains(&tl_idx),
                        profile_drawn: tl_state.profile_drawn_since_last_update.contains(&tl_idx),
                    };
                    let (item, item_new_draw_status) = match draw {
                        ItemDraw::SummaryItem { group, event, content } => populate_group_summary_item(
                            cx,
                            list,
                            item_id,
                            timeline,
                            group,
                            event,
                            &content,
                            item_drawn_status,
                        ),
                        ItemDraw::Message(event_tl_item, msg_like_content) => {
                            let prev_event = tl_idx.checked_sub(1).and_then(|i| tl_items.get(i));
                            let is_newest_sent = tl_state.index_of_last_own_sent == Some(tl_idx);
                            let is_blocked_by_failed_send = tl_state.index_of_first_own_failed.is_some_and(|i| i < tl_idx);
                            populate_message_view(
                                cx,
                                list,
                                item_id,
                                &tl_state.kind,
                                event_tl_item,
                                msg_like_content,
                                prev_event,
                                &mut tl_state.media_cache,
                                &mut tl_state.link_preview_cache,
                                &tl_state.fetched_thread_summaries,
                                &mut tl_state.pending_thread_summary_fetches,
                                &tl_state.user_power,
                                &self.pinned_events,
                                &tl_state.pending_downloads,
                                &tl_state.expanded_reply_previews,
                                is_newest_sent,
                                is_blocked_by_failed_send,
                                tl_state.is_encrypted,
                                item_drawn_status,
                                room_screen_widget_uid,
                            )
                        }
                        ItemDraw::SmallState { event, content, shows_collapse_line, shows_invite_button } => populate_small_state_event(
                            cx,
                            list,
                            item_id,
                            &tl_state.kind,
                            event,
                            &content,
                            item_drawn_status,
                            shows_collapse_line,
                            shows_invite_button,
                        ),
                        ItemDraw::DateDivider(millis) => {
                            let (item, existed) = list.item_with_existed(cx, item_id, id!(DateDivider));
                            if !(existed && item_drawn_status.content_drawn) {
                                // A collapsed group right under this divider may span multiple days (or months/years),
                                // so we show that date range as part of this divider so that the user can easily understand the timeline.
                                let span_end = divider_span_end(timeline, &tl_state.state_event_groups, tl_idx);
                                item.label(cx, ids!(date)).set_text(cx, &date_divider_text(millis, span_end));
                            }
                            (item, ItemDrawnStatus::both_drawn())
                        }
                        ItemDraw::ReadMarker => {
                            let item = list.item(cx, item_id, id!(ReadMarker));
                            (item, ItemDrawnStatus::both_drawn())
                        }
                        ItemDraw::Empty => (list.item(cx, item_id, id!(ZeroHeightItem)), ItemDrawnStatus::both_drawn()),
                    };

                    // Now that we've drawn the item, record whether it's fully drawn (if that changed).
                    if item_new_draw_status.content_drawn != item_drawn_status.content_drawn {
                        if item_new_draw_status.content_drawn {
                            tl_state.content_drawn_since_last_update.insert(tl_idx .. tl_idx + 1);
                        } else {
                            tl_state.content_drawn_since_last_update.remove(tl_idx .. tl_idx + 1);
                        }
                    }
                    if item_new_draw_status.profile_drawn != item_drawn_status.profile_drawn {
                        if item_new_draw_status.profile_drawn {
                            tl_state.profile_drawn_since_last_update.insert(tl_idx .. tl_idx + 1);
                        } else {
                            tl_state.profile_drawn_since_last_update.remove(tl_idx .. tl_idx + 1);
                        }
                    }
                    item
                };
                item.draw_all(cx, scope);
            }

            tl_state.scroll_anchors = None;

            // If the list is not filling the viewport (and back pagination isn't already in-progress),
            // then we need to back paginate the timeline until we have enough history to fill the viewport.
            if !tl_state.backwards_pagination.is_loading()
                && !tl_state.failed_recently()
                && tl_state.backwards_pagination.needs_more_history(
                    false,
                    true,
                    list.is_filling_viewport(),
                    false,
                )
            {
                log!("Automatically paginating timeline to fill viewport for room {:?}", self.room_name_id);
                tl_state.paginate_backwards();
            }
            top_space.set_visible(cx, tl_state.backwards_pagination.is_loading());
        }

        let room_rect = self.view.area().rect(cx);
        self.view.room_action_bar(cx, ids!(room_actions)).draw_shadow(cx, room_rect);

        // If this RoomScreen was just drawn for the first time after being opened for
        // a "Reply In Thread", then then focus on the text input in the RoomInputBar.
        if self.focus_input_bar_on_show {
            self.focus_input_bar_on_show = false;
            self.view.room_input_bar(cx, ids!(room_input_bar)).set_key_focus(cx);
        }

        // Now that the timeline has been drawn with its items, a deferred jump can happen.
        if let Some(jump) = self.deferred_jump.as_mut()
            && let Some(tl) = self.tl_state.as_ref()
            && !tl.items.is_empty()
        {
            jump.num_items = tl.items.len();
            jump.frame = cx.new_next_frame();
        }

        DrawStep::done()
    }
}

impl RoomScreen {
    fn room_id(&self) -> Option<&OwnedRoomId> {
        self.room_name_id.as_ref().map(|r| r.room_id())
    }

    fn show_room_input_popup_menu(&mut self, cx: &mut Cx, button_rect: Rect) {
        let popup_menu = self.room_input_popup_menu(cx, ids!(room_input_popup_menu));
        let room_screen_rect = self.view(cx, ids!(room_screen_wrapper)).area().rect(cx);
        let margin = Inset {
            left: button_rect.pos.x - room_screen_rect.pos.x,
            top: 0.0,
            right: 0.0,
            bottom: room_screen_rect.pos.y + room_screen_rect.size.y
                - button_rect.pos.y
                + 9.0
        };

        let mut main_content = popup_menu.view(cx, ids!(main_content));
        script_apply_eval!(cx, main_content, {
            margin: #(margin)
        });
        popup_menu.show(cx);
        self.view.redraw(cx);
    }

    fn handle_room_input_popup_menu_action(
        &mut self,
        cx: &mut Cx,
        action: RoomInputPopupMenuAction,
    ) {
        let room_input_bar = self.view.room_input_bar(cx, ids!(room_input_bar));
        match action {
            RoomInputPopupMenuAction::Show { button_rect } => {
                self.show_room_input_popup_menu(cx, button_rect);
            }
            RoomInputPopupMenuAction::UploadPhotoOrVideo => {
                let Some(timeline_kind) = self.timeline_kind.clone() else { return };
                room_input_bar.open_photo_video_picker(cx, timeline_kind);
            }
            RoomInputPopupMenuAction::UploadFile => {
                let Some(timeline_kind) = self.timeline_kind.clone() else { return };
                room_input_bar.open_file_picker(cx, timeline_kind);
            }
            RoomInputPopupMenuAction::SendCurrentLocation => {
                room_input_bar.show_current_location_preview(cx);
            }
            RoomInputPopupMenuAction::None => {}
        }
    }

    /// Processes all pending background updates to the currently-shown timeline.
    ///
    /// Redraws this RoomScreen view if any updates were applied.
    fn process_timeline_updates(&mut self, cx: &mut Cx, portal_list: &PortalListRef) {
        let top_space = self.view(cx, ids!(top_space));
        let jump_to_bottom_button = self.jump_to_bottom_button(cx, ids!(jump_to_bottom_button));
        let loading_pane = self.view.loading_pane(cx, ids!(loading_pane));
        let curr_first_id = portal_list.first_id();
        let ui = self.widget_uid();
        let Some(tl) = self.tl_state.as_mut() else { return };

        let mut items_changed = false;
        let mut should_continue_backwards_pagination = false;
        let mut typing_users = None;
        let mut jump_to_read_receipt = None;
        let mut num_updates = 0;

        while let Ok(update) = tl.update_receiver.try_recv() {
            num_updates += 1;
            let update = match update {
                // When an existing timeline is reconnected (new channels/tasks in the backend),
                // it sends a `FirstUpdate` that includes the new channel endpoints.
                // We differentiate this from the initial `FirstUpdate` that occurs when the timeline
                // is first created by checking to see if we have any timeline items.
                // If we do, we just treat it as a `NewItems` update instead, which maintains the user's scroll anchor.
                TimelineUpdate::FirstUpdate { initial_items } if !tl.items.is_empty() => {
                    let len = initial_items.len();
                    TimelineUpdate::NewItems {
                        new_items: initial_items,
                        changed_indices: 0..len,
                        clear_cache: true,
                        was_timeline_reset: true,
                        is_append: false,
                        num_unchanged_at_end: 0,
                    }
                }
                update => update,
            };

            match update {
                TimelineUpdate::FirstUpdate { initial_items } => {
                    tl.content_drawn_since_last_update.clear();
                    tl.profile_drawn_since_last_update.clear();
                    tl.backwards_pagination.mark_items_updated(
                        initial_items.is_empty(),
                        initial_items.front().is_some_and(|item| item.is_timeline_start()),
                        true,
                    );
                    tl.paginate_again_when_done = false;
                    // Set the portal list to the very bottom of the timeline.
                    portal_list.set_first_id_and_scroll(initial_items.len().saturating_sub(1), 0.0);
                    portal_list.set_tail_range(true);
                    jump_to_bottom_button.update_visibility(cx, true);

                    tl.items = initial_items;
                    tl.pending_knocks = PendingKnocks::new(&tl.items);
                    let (groups, timeline) = tl.groups_and_timeline_info();
                    groups.rebuild(&timeline, 0..usize::MAX, 0);
                    // The list hasn't drawn these items yet, so until it does, keep its new first item where it is.
                    tl.scroll_anchors = Some(ScrollAnchors::at_list_position(portal_list, &tl.items, &tl.state_event_groups));
                    items_changed = true;
                }

                TimelineUpdate::NewItems { new_items, changed_indices, is_append, clear_cache, was_timeline_reset, num_unchanged_at_end } => {
                    if new_items.is_empty() {
                        if !tl.items.is_empty() {
                            log!("process_timeline_updates(): timeline (had {} items) was cleared for room {}", tl.items.len(), tl.kind.room_id());
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

                    let prior_items_changed = clear_cache || changed_indices.start <= tl.next_drawn_index(curr_first_id);

                    let first_change = if clear_cache { 0 } else { changed_indices.start };
                    let changes = ChangedItems::between(&tl.items, &new_items, first_change, num_unchanged_at_end);
                    // Knocks that were changed (answered or not) might need us to recalculate collapsed groups.
                    let changed_knocks = tl.pending_knocks.update(&tl.items, &new_items, &changes);

                    // Whether older events were added, even if they're not visible (like in a collapsed group).
                    let added_older_events = new_items.iter().find_map(|i| i.as_event()?.event_id())
                        != tl.items.iter().find_map(|i| i.as_event()?.event_id());

                    // The items that were on screen when the portallist was last drawn,
                    // which we want to keep at the same places on screen (in the viewport)
                    // to prevent the view from jumping around.
                    let mut anchors = ScrollAnchors::take(&mut tl.scroll_anchors, portal_list, &tl.items, &tl.state_event_groups);
                    let added_at_front = first_change == 0 && added_older_events;
                    let anchors_moved = anchors.refind(&tl.items, &new_items, added_at_front);

                    // If the last event in the timeline was even partially visible, we auto-tail it to the end.
                    let list_height = portal_list.area().rect(cx).size.y;
                    let bottom_was_visible = portal_list.is_at_end()
                        || tl.items.len().checked_sub(1).is_some_and(|last_id| {
                            // A last item hidden in a collapsed group takes up no space,
                            // so check the summary item for that group instead.
                            let last_shown = tl.state_event_groups.summary_item_if_collapsed(last_id).unwrap_or(last_id);
                            portal_list.position_of_item(cx, last_shown).is_some_and(|pos| pos < list_height)
                        });

                    // New items shouldn't put the list at its end while a jump is already happening.
                    let jumping = matches!(tl.message_highlight_animation_state,
                        MessageHighlightAnimationState::Pending { item_id, .. } if portal_list.is_smooth_scrolling() == Some(item_id)
                    );
                    if is_append {
                        if bottom_was_visible && !jumping {
                            portal_list.smooth_scroll_to_end(cx, SCROLL_TO_BOTTOM_SPEED, None);
                        }
                        // Otherwise the new items are off-screen (or will be once the ongoing jump is done),
                        // so flag them on the jump to bottom button.
                        else {
                            // Show the unread badge (with an unknown count at first) so that
                            // the user knows more content is available below the viewport.
                            jump_to_bottom_button.show_unread_message_badge(cx, UnreadMessageCount::Unknown);
                            // We can fetch the actual unread count for MainRoom timelines only (an SDK limitation).
                            if matches!(tl.kind, TimelineKind::MainRoom { .. }) {
                                submit_async_request(MatrixRequest::GetNumberUnreadMessages{
                                    timeline_kind: tl.kind.clone(),
                                });
                            }
                        }
                    }

                    if prior_items_changed {
                        // update the loading pane with the number of new items we back-paginated
                        loading_pane.paginated_more_events(cx, new_items.len().saturating_sub(tl.items.len()));
                    }

                    tl.backwards_pagination.mark_items_updated(
                        new_items.is_empty(),
                        new_items.front().is_some_and(|item| item.is_timeline_start()),
                        was_timeline_reset,
                    );
                    let has_more_history = clear_cache && !tl.backwards_pagination.is_fully_paginated();

                    if clear_cache {
                        tl.content_drawn_since_last_update.clear();
                        tl.profile_drawn_since_last_update.clear();
                    } else {
                        tl.forget_drawn([changed_indices.clone()]);
                        // An answer to a knock changes whether that (earlier) knock shows an invite button.
                        tl.forget_drawn(changed_knocks.iter().map(|&index| index..index + 1));
                        // log!("process_timeline_updates(): changed_indices: {changed_indices:?}, items len: {}\ncontent drawn: {:#?}\nprofile drawn: {:#?}", items.len(), tl.content_drawn_since_last_update, tl.profile_drawn_since_last_update);
                    }

                    tl.items = new_items;

                    let (groups, timeline) = tl.groups_and_timeline_info();
                    let regrouped = groups.rebuild_ranges(&timeline, changes.iter().map(|change| (change.new.clone(), change.len_change())));
                    // Regrouping can also change items outside of the given `changed_indices`,
                    // like a group's summary or the day divider above it,
                    // so we have to redraw those too just to be safe.
                    if !clear_cache {
                        tl.forget_drawn(regrouped);
                    }

                    // A knock that has been answered is now eligible to be included in a collapsed group.
                    let (groups, timeline) = tl.groups_and_timeline_info();
                    let regrouped = groups.regroup_around(&timeline, &changed_knocks);
                    tl.forget_drawn(regrouped);

                    // Find the first item that is still in the same viewport position that it was before,
                    // and anchor our scroll position on that.
                    if let Some((first_id, first_scroll)) = anchors.pin(&tl.items, &tl.state_event_groups) {
                        if anchors_moved {
                            log!("process_timeline_updates(): keeping the view in place at index {first_id}, scroll {first_scroll}");
                            // Hide the tooltip when items move, as a hover-out event won't occur.
                            cx.widget_action(ui, TooltipAction::HoverOut);
                        }
                        portal_list.set_first_id_and_scroll_in_place(first_id, first_scroll);
                    }
                    else if portal_list.first_id() >= tl.items.len() {
                        log!("process_timeline_updates(): jumping to bottom: first_id {} is out of bounds for {} new items", portal_list.first_id(), tl.items.len());
                        portal_list.set_first_id_and_scroll(tl.items.len().saturating_sub(1), 0.0);
                        portal_list.set_tail_range(true);
                        jump_to_bottom_button.update_visibility(cx, true);
                    }
                    //
                    // TODO: after a user is (un)blocked, all timelines are cleared. Handle that here.
                    //
                    anchors.remember_list_position(portal_list);
                    tl.scroll_anchors = Some(anchors);

                    // If the top of the timeline is still showing after getting older items,
                    // go ahead and paginate more so the user doesn't have to scroll up again manually.
                    if has_more_history {
                        if tl.has_older_content(portal_list.first_id()) {
                            // Something new showed up above, so the user can just keep scrolling up for more.
                            tl.paginate_again_when_done = false;
                        } else {
                            // Older events were added but they all went into the collapsed group at the top.
                            // This is still progress, but they're not obviously visible to the user.
                            // Keep going until older content becomes visible or we reach the timeline start.
                            should_continue_backwards_pagination = true;
                        }
                    }
                    items_changed = true;
                }

                TimelineUpdate::NewUnreadMessagesCount(unread_messages_count) => {
                    // Only main room timelines get a count on their unread badge,
                    // because the matrix SDK doesn't currently support querying unread message counts for threads.
                    if matches!(tl.kind, TimelineKind::MainRoom { .. }) {
                        jump_to_bottom_button.show_unread_message_badge(cx, unread_messages_count);
                    }
                }

                TimelineUpdate::TargetEventFound { target_event_id, index } => {
                    // log!("Target event found in room {}: {target_event_id}, index: {index}", tl.kind.room_id());
                    // Ignore a target-event-found result if we're no longer waiting on it.
                    if !loading_pane.is_searching_for(&target_event_id) {
                        continue;
                    }
                    // sanity check: ensure the target event is in the timeline at the given `index`.
                    let item = tl.items.get(index);
                    let is_valid = item.is_some_and(|item|
                        item.as_event()
                            .is_some_and(|ev| ev.event_id() == Some(&target_event_id))
                    );

                    // log!("TargetEventFound: is_valid? {is_valid}. room {}, event {target_event_id}, index {index} of {}\n  --> item: {item:?}", tl.kind.room_id(), tl.items.len());
                    if is_valid {
                        // We successfully found the target event, so we can close the loading pane,
                        // reset the loading panestate to `None`, and stop issuing backwards pagination requests.
                        loading_pane.hide(cx);
                        // It may have just been paginated in, so we can't scroll to it until it's been drawn.
                        self.deferred_jump = Some(DeferredJump::new(DeferredJumpKind::ScrollTo { event_id: target_event_id }));
                    }
                    else {
                        // Here, the target event was not found in the current timeline,
                        // or we found it previously but it is no longer in the timeline (or has moved),
                        // which means we encountered an error and are unable to jump to the target event.
                        error!("Target event index {index} of {} is out of bounds for room {}", tl.items.len(), tl.kind.room_id());
                        // Show this error in the loading pane, which should already be open.
                        loading_pane.search_failed(cx);
                    }

                    should_continue_backwards_pagination = false;
                    tl.paginate_again_when_done = false;

                    // redraw now before any other items get added to the timeline list.
                    self.view.redraw(cx);
                }

                TimelineUpdate::PaginationRunning(direction) => {
                    if direction == PaginationDirection::Backwards {
                        tl.backwards_pagination.mark_running();
                    } else {
                        error!("Unexpected PaginationRunning update in the Forwards direction");
                    }
                }

                TimelineUpdate::PaginationError { error, direction } => {
                    error!("Pagination error ({direction}) in {:?}: {error:?}", self.room_name_id);
                    let room_name = self.room_name_id.as_ref().map(|r| r.to_string());
                    enqueue_popup_notification(
                        utils::stringify_pagination_error(&error, room_name.as_deref().unwrap_or(UNNAMED_ROOM)),
                        PopupKind::Error,
                        Some(10.0),
                    );
                    // We could automatically retry here after a failure, but it's not
                    // really that valuable when the user can just try to scroll again.
                    tl.paginate_again_when_done = false;
                    if direction == PaginationDirection::Backwards {
                        tl.backwards_pagination.mark_error();
                        tl.last_pagination_error_at = Some(Instant::now());
                    }
                    should_continue_backwards_pagination = false;
                    if direction == PaginationDirection::Backwards && loading_pane.is_searching() {
                        loading_pane.search_failed(cx);
                    }
                }

                TimelineUpdate::PaginationCompleted { is_fully_paginated, direction } => {
                    if direction == PaginationDirection::Backwards {
                        // The backend sends the page's items before this completion notice.
                        // We've handled those items by now, so we can mark the request as finished.
                        tl.backwards_pagination.mark_completed(is_fully_paginated);
                        tl.last_pagination_error_at = None;
                    } else {
                        error!("Unexpected PaginationCompleted update in the Forwards direction");
                    }
                }
                TimelineUpdate::EventDetailsFetched {event_id, result } => {
                    if let Err(_e) = result {
                        error!("Failed to fetch details fetched for event {event_id} in room {}. Error: {_e:?}", tl.kind.room_id());
                    }
                    // Here, to be most efficient, we could redraw only the updated event,
                    // but for now we just fall through and let the final `redraw()` call re-draw the whole timeline view.
                }
                TimelineUpdate::ThreadSummaryDetailsFetched {
                    thread_root_event_id,
                    timeline_item_index,
                    num_replies,
                    latest_reply_preview_text,
                } => {
                    tl.pending_thread_summary_fetches.remove(&thread_root_event_id);
                    let event_id_matches_at_index = tl.items
                        .get(timeline_item_index)
                        .and_then(|item| item.as_event())
                        .and_then(|ev| ev.event_id())
                        .is_some_and(|id| id == thread_root_event_id);
                    let sdk_num_replies = tl.items
                        .get(timeline_item_index)
                        .filter(|_| event_id_matches_at_index)
                        .or_else(|| tl.items.iter().find(|item| item.as_event().and_then(|ev| ev.event_id()) == Some(&*thread_root_event_id)))
                        .and_then(|item| item.as_event()?.content().thread_summary())
                        .map_or(0, |summary| summary.num_replies);
                    tl.fetched_thread_summaries.insert(
                        thread_root_event_id.clone(),
                        FetchedThreadSummary {
                            num_replies,
                            latest_reply_preview_text,
                            sdk_num_replies_at_fetch: sdk_num_replies,
                        },
                    );
                    if event_id_matches_at_index {
                        tl.content_drawn_since_last_update
                            .remove(timeline_item_index .. timeline_item_index + 1);
                    } else {
                        tl.content_drawn_since_last_update.clear();
                    }
                }
                TimelineUpdate::RoomMembersSynced => {
                    // log!("process_timeline_updates(): room members fetched for room {}", tl.kind.room_id());
                    // Now that the full room members list has been synced in the background,
                    // we need to actually get the new list for use in this room screen.
                    submit_async_request(MatrixRequest::GetRoomMembers {
                        timeline_kind: tl.kind.clone(),
                        memberships: matrix_sdk::RoomMemberships::ACTIVE,
                        local_only: true,
                    });
                    // Here, to be most efficient, we could redraw only the user avatars and names in the timeline,
                    // but for now we just fall through and let the final `redraw()` call re-draw the whole timeline view.
                }
                TimelineUpdate::RoomMembersListFetched { members } => {
                    // Store room members directly in TimelineUiState
                    tl.room_members = Some(Arc::new(members));
                    self.view.room_input_bar(cx, ids!(room_input_bar))
                        .set_room_context(cx, ui, tl.kind.clone(), tl.room_members.clone());
                    self.view.room_pane_dock(cx, ids!(room_pane_dock))
                        .set_room_members(cx, tl.room_members.clone());
                },
                TimelineUpdate::RoomMembersListFetchFailed { error } => {
                    // Keep showing any members fetched earlier.
                    if tl.room_members.is_none() {
                        self.view.room_pane_dock(cx, ids!(room_pane_dock)).set_room_members_error(cx, error);
                    }
                }
                TimelineUpdate::MediaFetched(_request) => {
                    log!("process_timeline_updates(): media fetched for room {}", tl.kind.room_id());
                    // Here, to be most efficient, we could redraw only the media items in the timeline,
                    // but for now we just fall through and let the final `redraw()` call re-draw the whole timeline view.
                }
                TimelineUpdate::MessageEdited { timeline_event_item_id: timeline_event_id, result } => {
                    self.view.room_input_bar(cx, ids!(room_input_bar))
                        .handle_edit_result(cx, timeline_event_id, result);
                }
                TimelineUpdate::TypingUsers { users } => {
                    // This update loop should be kept tight & fast, so all we do here is
                    // save the list of typing users for future use after the loop exits.
                    // Then, we "process" it later (by turning it into a string) after the
                    // update loop has completed, which avoids unnecessary expensive work
                    // if the list of typing users gets updated many times in a row.
                    typing_users = Some(users);
                }
                TimelineUpdate::PinnedEventIds(pinned_events) => {
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
                    self.view.room_input_bar(cx, ids!(room_input_bar))
                        .update_room_state(cx, tl.kind.room_id(), tl.tombstone_info.as_ref(), user_power_levels);
                    // We need to redraw all events in order to reflect the new power levels,
                    // e.g., for the message context menu to be correctly populated.
                    tl.content_drawn_since_last_update.clear();
                    tl.profile_drawn_since_last_update.clear();
                }
                TimelineUpdate::Tombstoned(successor_room_details) => {
                    tl.tombstone_info = Some(successor_room_details);
                    self.view.room_input_bar(cx, ids!(room_input_bar))
                        .update_room_state(cx, tl.kind.room_id(), tl.tombstone_info.as_ref(), tl.user_power);
                }
                TimelineUpdate::RoomEncrypted => {
                    tl.is_encrypted = true;
                    self.view.room_input_bar(cx, ids!(room_input_bar))
                        .update_encryption_state(cx, true);
                }
                TimelineUpdate::ReadReceiptSendFailed { receipt_type, event_id } => {
                    let last_sent = if matches!(receipt_type, ReceiptType::FullyRead) {
                        &mut tl.last_sent_fully_read
                    } else {
                        &mut tl.last_sent_read_receipt
                    };
                    // Only clear it if a newer receipt hasn't replaced it already.
                    if last_sent.as_deref() == Some(&event_id) {
                        *last_sent = None; // allow this receipt to be re-sent later
                    }
                }
                TimelineUpdate::UserReadReceiptFetched { user_id, event_id } => {
                    if self.pending_read_receipt_jump.take_if(|u| *u == user_id).is_none() {
                        continue;
                    }
                    let name = display_name_or_user_id(cx, tl.kind.room_id(), user_id);
                    if let Some(event_id) = event_id {
                        jump_to_read_receipt = Some((event_id, format!("the latest event seen by {name}")));
                    } else {
                        enqueue_popup_notification(
                            format!("Couldn't find the last-seen event of {name} in this {}.", tl.kind.desc()),
                            PopupKind::Error,
                            Some(6.0),
                        );
                    }
                }
                TimelineUpdate::LinkPreviewFetched => {
                    // fall through to this item being redrawn
                }
                TimelineUpdate::FileUploadStarted { upload_id, file_name, in_reply_to, abort_handle } => {
                    self.view.room_input_bar(cx, ids!(room_input_bar))
                        .handle_file_upload_started(cx, upload_id, &file_name, in_reply_to.as_ref(), abort_handle, tl.kind.clone());
                }
                TimelineUpdate::FileUploadQueuing { upload_id, transaction_id } => {
                    self.view.room_input_bar(cx, ids!(room_input_bar))
                        .set_upload_as_queued(cx, upload_id, transaction_id, tl.is_encrypted);
                }
                TimelineUpdate::FileUploadProgress { upload_id, current_bytes, total_bytes } => {
                    self.view.room_input_bar(cx, ids!(room_input_bar))
                        .set_upload_progress(cx, upload_id, current_bytes, total_bytes);
                }
                TimelineUpdate::LocalEchoProgress { new_items } => {
                    // we don't have to do anything except update the timeline items and redraw.
                    tl.items = new_items;
                    portal_list.redraw(cx);
                }
                TimelineUpdate::SendFailedBeforeBeingQueued { message, replied_to } => {
                    // Every message in a separate thread also has a `replied_to` field,
                    // which is just there to indicate that its part of a threaded reply to the thread room message.
                    // Obviously those shouldn't be treated as a "reply" to restore as the replying_preview.
                    let real_reply = replied_to.as_ref().filter(
                        |reply| !matches!(reply.enforce_thread, EnforceThread::Threaded(ReplyWithinThread::No))
                    );
                    let replied_to_item = real_reply
                        .and_then(|reply| index_of_event(&tl.items, &reply.event_id, tl.items.len(), MAX_ITEMS_TO_SEARCH_THROUGH))
                        .and_then(|index| tl.items.get(index)?.as_event().cloned())
                        .map(|event_tl_item| {
                            let replied_to_info = EmbeddedEvent::from_timeline_item(&event_tl_item);
                            (event_tl_item, replied_to_info)
                        });
                    if real_reply.is_some() && replied_to_item.is_none() {
                        enqueue_popup_notification(
                            "Your unsent reply was restored, but the message you were replying to is no longer loaded. Please reply to it again.",
                            PopupKind::Warning,
                            Some(10.0),
                        );
                    }
                    self.view.room_input_bar(cx, ids!(room_input_bar))
                        .restore_unsent_message(cx, &message, replied_to_item, &tl.kind);
                }
                TimelineUpdate::FileUploadError { upload_id, error, retryable_upload } => {
                    self.view.room_input_bar(cx, ids!(room_input_bar))
                        .show_upload_error(cx, upload_id, &error, retryable_upload);
                }
                TimelineUpdate::FileUploadComplete { upload_id } => {
                    self.view.room_input_bar(cx, ids!(room_input_bar))
                        .hide_upload_progress(cx, upload_id);
                }
                TimelineUpdate::AttachmentDownloadFinished(mxc, result) => {
                    if let Some(entry) = tl.pending_downloads.iter_mut().find(|p| p.mxc == mxc) {
                        entry.state = match result {
                            Ok(()) => PendingDownloadState::JustSucceeded,
                            Err(_) => PendingDownloadState::JustFailed,
                        };
                    }
                    portal_list.redraw(cx);
                }
                TimelineUpdate::AttachmentDownloadReset(mxc) => {
                    tl.pending_downloads.retain(|p| p.mxc != mxc);
                    portal_list.redraw(cx);
                }
            }
        }

        // If older items came in during a jump, its target is now at a different index,
        // so re-start the jump procedure that targets it (unless another jump is already waiting).
        if self.deferred_jump.is_none()
            && let MessageHighlightAnimationState::Pending { item_id, event_id } = &tl.message_highlight_animation_state
            && portal_list.is_smooth_scrolling() == Some(*item_id)
            && tl.items.get(*item_id).and_then(|item| item.as_event()?.event_id()) != Some(event_id)
        {
            let event_id = event_id.clone();
            tl.message_highlight_animation_state = MessageHighlightAnimationState::Off;
            self.deferred_jump = Some(DeferredJump::new(DeferredJumpKind::ScrollTo { event_id }));
        }

        if items_changed {
            (tl.index_of_last_own_sent, tl.index_of_first_own_failed) =
                own_send_indices(&tl.items, tl.kind.thread_root_event_id().is_none());
        }

        let blocked_send = tl.index_of_first_own_failed
            .and_then(|i| tl.items.get(i))
            .and_then(|item| item.as_event())
            .and_then(|event| match event.send_state() {
                Some(EventSendState::SendingFailed { error, .. }) => Some(BlockedSend {
                    timeline_kind: tl.kind.clone(),
                    timeline_event_id: event.identifier(),
                    error: error.clone(),
                }),
                _ => None,
            });

        if let Some(is_fully_paginated) = tl.backwards_pagination.take_completed_result() {
            if is_fully_paginated {
                tl.paginate_again_when_done = false;
                should_continue_backwards_pagination = false;
                if loading_pane.is_searching() {
                    loading_pane.search_failed(cx);
                }
            } else {
                // Check the updated scroll anchors after the new page of events has actually arrived.
                should_continue_backwards_pagination |= 
                    std::mem::take(&mut tl.paginate_again_when_done)
                    || tl.backwards_pagination.needs_more_history(
                        portal_list.first_id() <= 2,
                        tl.has_older_content(portal_list.first_id()),
                        true,
                        loading_pane.is_searching(),
                    );
            }
        }

        // If we're searching for an event, we must always keep back paginating until the search ends.
        if should_continue_backwards_pagination {
            tl.paginate_backwards();
        }
        top_space.set_visible(cx, tl.backwards_pagination.is_loading());

        self.view.failed_send_banner(cx, ids!(failed_send_banner))
            .show_or_hide(cx, blocked_send);

        // We unsubscribe once typing notices are hidden, but one might've already been in flight.
        if let Some(users) = typing_users
            && cx.global::<AppPreferencesGlobal>().0.show_typing_notices
        {
            self.view
                .typing_notice(cx, ids!(typing_notice))
                .show_or_hide(cx, &users, Animate::Yes);
        }

        if let Some((event_id, searching_for)) = jump_to_read_receipt {
            // This jump supersedes any search already showing, so close that one out first.
            loading_pane.hide(cx);
            self.jump_to_event(cx, &event_id, None, searching_for, portal_list, &loading_pane);
        }

        if num_updates > 0 {
            // log!("Applied {} timeline updates for room {}, redrawing with {} items...", num_updates, tl.kind.room_id(), tl.items.len());
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
        loading_pane: &LoadingPaneRef,
        portal_list: &PortalListRef,
    ) -> bool {
        let (url, matrix_id) = if let HtmlLinkAction::Clicked { url, .. } = action.as_widget_action().cast() {
            let matrix_id = utils::parse_matrix_link(&url).map(|(matrix_id, _via)| matrix_id);
            (url, matrix_id)
        } else if let RobrixHtmlLinkAction::ClickedMatrixLink { url, matrix_id, .. } = action.as_widget_action().cast() {
            (url, Some(matrix_id))
        } else {
            return false;
        };

        let (room_or_alias_id, event_id) = match matrix_id {
            Some(MatrixId::Room(room_id)) => (room_id.into(), None),
            Some(MatrixId::RoomAlias(alias)) => (alias.into(), None),
            Some(MatrixId::Event(room_or_alias_id, event_id)) => (room_or_alias_id, Some(event_id)),
            Some(MatrixId::User(user_id)) => {
                let Some(room_name_id) = self.room_name_id.as_ref() else {
                    utils::open_url(&url);
                    return true;
                };
                // There is no synchronous way to get the user's full profile info
                // including the details of their room membership,
                // so we fill in with the details we *do* know currently
                // and then show the UserProfileSlidingPane immediately.
                // Then, the UserProfileSlidingPane itself will fire off an async request
                // to get the rest of the details.
                self.show_user_profile(
                    cx,
                    pane,
                    UserProfilePaneInfo {
                        profile_and_room_id: UserProfileAndRoomId {
                            user_profile: UserProfile {
                                user_id,
                                username: None,
                                avatar_state: AvatarState::Unknown,
                            },
                            room_id: room_name_id.room_id().clone(),
                        },
                        room_name: room_name_id.to_string(),
                        room_member: None,
                    },
                );
                return true;
            }
            _ => {
                utils::open_url(&url);
                return true;
            }
        };
        let link = RoomLink { room_or_alias_id, event_id, url };

        // Fast path: first check the rooms list to see if we already know the room.
        let rooms_list_ref = cx.get_global::<RoomsListRef>();
        let known_room = match <&RoomId>::try_from(&*link.room_or_alias_id) {
            Ok(room_id) => rooms_list_ref.get_room_name(&room_id.to_owned()),
            Err(alias) => rooms_list_ref.get_room_name_by_alias(alias),
        };
        let known_room_state = known_room.as_ref().and_then(|rn| rooms_list_ref.get_room_state(rn.room_id()));
        let destination = match (&known_room, known_room_state, &link.event_id) {
            (Some(room_name_id), Some(RoomState::Invited), _) => Some(RoomLinkDestination::Invite(room_name_id.clone())),
            (Some(room_name_id), Some(RoomState::Joined), None) => Some(RoomLinkDestination::Timeline {
                room_name_id: room_name_id.clone(),
                timeline_kind: TimelineKind::MainRoom { room_id: room_name_id.room_id().clone() },
            }),
            (Some(room_name_id), Some(RoomState::Joined), Some(event_id)) => self.tl_state.as_ref()
                .filter(|tl| tl.kind.room_id() == room_name_id.room_id()
                    && index_of_event(&tl.items, event_id, tl.items.len(), MAX_ITEMS_TO_SEARCH_THROUGH).is_some()
                )
                .map(|tl| RoomLinkDestination::Timeline { room_name_id: room_name_id.clone(), timeline_kind: tl.kind.clone() }),
            _ => None,
        };
        match destination {
            Some(destination) => self.show_link_destination(cx, &link, &destination, loading_pane, portal_list),
            None => {
                loading_pane.start_resolving_link(cx, link.clone());
                submit_async_request(MatrixRequest::ResolveRoomLink {
                    link,
                    known_room_id: known_room.map(|rn| rn.room_id().clone()),
                });
                self.redraw(cx);
            }
        }
        true
    }

    /// Returns `true` if this RoomScreen's timeline shows the given event from the given timeline.
    fn is_event_in_this_timeline(&self, timeline_kind: &TimelineKind, event_id: &OwnedEventId) -> bool {
        // A thread's timeline also includes its root message.
        self.tl_state.as_ref().is_some_and(|tl|
            &tl.kind == timeline_kind || tl.kind.thread_root_event_id() == Some(event_id)
        )
    }

    /// Shows the given destination of a clicked link to a room, space, or event.
    fn show_link_destination(
        &mut self,
        cx: &mut Cx,
        link: &RoomLink,
        destination: &RoomLinkDestination,
        loading_pane: &LoadingPaneRef,
        portal_list: &PortalListRef,
    ) {
        let (room_name_id, navigation) = match destination {
            RoomLinkDestination::Timeline { room_name_id, timeline_kind } => match &link.event_id {
                Some(event_id) => {
                    let description = String::from("the linked message");
                    if self.is_event_in_this_timeline(timeline_kind, event_id) {
                        self.jump_to_event(cx, event_id, None, description, portal_list, loading_pane);
                        return;
                    }
                    (room_name_id, NavigateToLinkAction::Event {
                        room_name_id: room_name_id.clone(),
                        timeline_kind: timeline_kind.clone(),
                        event_id: event_id.clone(),
                        description,
                    })
                }
                None if self.timeline_kind.as_ref() == Some(timeline_kind) => {
                    enqueue_popup_notification(
                        "You are already viewing that room.",
                        PopupKind::Info,
                        Some(4.0),
                    );
                    return;
                }
                None => (room_name_id, NavigateToLinkAction::Screen(room_pane::timeline_screen(room_name_id, timeline_kind))),
            },
            RoomLinkDestination::Space(space_name_id) => (
                space_name_id,
                NavigateToLinkAction::Screen(SelectedRoom::Space { space_name_id: space_name_id.clone() }),
            ),
            RoomLinkDestination::Invite(room_name_id) => (
                room_name_id,
                NavigateToLinkAction::Screen(SelectedRoom::InvitedRoom { room_name_id: room_name_id.clone() }),
            ),
            RoomLinkDestination::NotJoined => {
                cx.action(NavigationBarAction::GoToAddRoom { search_for: Some(link.url.clone()) });
                return;
            }
        };
        enqueue_rooms_list_update(RoomsListUpdate::ScrollToRoom(room_name_id.room_id().clone()));
        cx.action(navigation);
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
        let Some(tl_state) = self.tl_state.as_ref() else { return };
        let Some(event_tl_item) = tl_state.items.get(item_id).and_then(|item| item.as_event()) else { return };

        let timestamp_millis = event_tl_item.timestamp();
        let image_details = get_image_file_details(event_tl_item);
        let downloadable = Some(DownloadableAttachment {
            media_source: media_source.clone(),
            filename: image_details.name.clone(),
            size: image_details.size_in_bytes,
            kind: DownloadKind::Image,
        });
        cx.action(ImageViewerAction::Show(LoadState::Loading(
            texture.clone(),
            Some(ImageViewerMetaData {
                image_name: image_details.name,
                image_caption: image_details.caption,
                image_format: image_details.format,
                image_file_size: image_details.size_in_bytes,
                timestamp: unix_time_millis_to_datetime(timestamp_millis),
                avatar_parameter: Some((
                    tl_state.kind.clone(),
                    event_tl_item.clone(),
                )),
                downloadable,
            }),
        )));

        fetch_full_image_for_viewer(media_source);
    }

    /// Looks up the event specified by the given message details in the given timeline.
    ///
    /// This will first try an instant index-based lookup via `details.item_id`,
    /// and then fall back to searching the timeline in reverse for the `details.event_id`
    /// if the index is "stale", meaning the timeline items have changed (e.g., due to pagination)
    /// since the message context menu was opened or the `MessageAction` was received by the `RoomScreen`.
    ///
    /// We search in reverse because it is far more likely that the user is interacting
    /// with an event that is close to the end of the timeline.
    fn find_event_in_timeline<'a>(
        items: &'a Vector<Arc<TimelineItem>>,
        details: &MessageDetails,
    ) -> Option<&'a EventTimelineItem> {
        let Some(target_event_id) = details.event_id() else {
            // A local echo doesn't have an event ID yet, so we can only find it via its index.
            return items.get(details.item_id)?.as_event()
                .filter(|ev| ev.identifier() == details.timeline_event_id);
        };
        if let Some(event) = items.get(details.item_id)
            .and_then(|item| item.as_event())
            .filter(|ev| ev.event_id().is_some_and(|id| id == target_event_id))
        {
            return Some(event);
        }
        let index = index_of_event(items, target_event_id, items.len(), MAX_ITEMS_TO_SEARCH_THROUGH)?;
        items.get(index).and_then(|item| item.as_event())
    }

    /// Registers a pending download for a media transfer (e.g., download, share)
    /// and shows the loading spinner, then calls `start` to kick off the transfer.
    ///
    /// Does nothing if the transfer was already in progress.
    fn begin_media_transfer(
        &mut self,
        cx: &mut Cx,
        portal_list: &PortalListRef,
        info: &DownloadableAttachment,
        kind: TransferKind,
        start: fn(DownloadableAttachment, TimelineUpdateSenderOption),
    ) {
        let Some(tl) = self.tl_state.as_mut() else { return };
        let mxc = media_source_mxc(&info.media_source);
        if tl.pending_downloads.iter().any(|p| &p.mxc == mxc) {
            enqueue_already_downloading_notification();
            return;
        }
        tl.pending_downloads.push(PendingDownload {
            mxc: mxc.clone(),
            state: PendingDownloadState::InProgress,
            kind,
        });
        portal_list.redraw(cx);
        let update_sender = tl.media_cache.timeline_update_sender().cloned();
        start(info.clone(), update_sender);
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
            match action.as_widget_action().widget_uid_eq(room_screen_widget_uid).cast_ref() {
                MessageAction::React { details, reaction } => {
                    let Some(tl) = self.tl_state.as_ref() else { return };
                    submit_async_request(MatrixRequest::ToggleReaction {
                        timeline_kind: tl.kind.clone(),
                        timeline_event_id: details.timeline_event_id.clone(),
                        reaction: reaction.clone(),
                    });
                }
                MessageAction::Reply(details) => {
                    let Some(tl) = self.tl_state.as_ref() else { return };
                    if let Some(event_tl_item) = Self::find_event_in_timeline(&tl.items, details).cloned() {
                        let replied_to_info = EmbeddedEvent::from_timeline_item(&event_tl_item);
                        self.view.room_input_bar(cx, ids!(room_input_bar))
                            .show_replying_to(cx, (event_tl_item, replied_to_info), &tl.kind);
                    }
                    else {
                        enqueue_popup_notification(
                            "Could not find message in timeline to reply to. Please try again.",
                            PopupKind::Error,
                            Some(5.0),
                        );
                        error!("MessageAction::Reply: couldn't find event [{}] {:?} to reply to in room {:?}",
                            details.item_id,
                            details.timeline_event_id,
                            self.room_id(),
                        );
                    }
                }
                MessageAction::ReplyInThread(details) => {
                    let Some(room_name_id) = self.room_name_id.clone() else {
                        error!("BUG: MessageAction::ReplyInThread: room_name_id was None in room {:?}", self.room_id());
                        continue;
                    };
                    // If this message was already part of a thread, use that thread root.
                    // If not, use the message's event ID as the root for a new thread.
                    let Some(thread_root_event_id) = details.thread_root_event_id.clone()
                        .or_else(|| details.event_id().cloned())
                    else {
                        enqueue_popup_notification(
                            "Cannot reply in thread to an unsent message.",
                            PopupKind::Error,
                            Some(5.0),
                        );
                        continue;
                    };
                    let thread_kind = TimelineKind::Thread {
                        room_id: room_name_id.room_id().clone(),
                        thread_root_event_id: thread_root_event_id.clone(),
                    };
                    if self.timeline_kind.as_ref() == Some(&thread_kind) {
                        // We're already viewing this thread, so just focus the input bar.
                        self.focus_input_bar_on_show = true;
                        self.redraw(cx);
                    } else {
                        // Emit an action to open the thread's RoomScreen
                        // and tell it to grab key focus once it's drawn.
                        input_bar_focus::request(cx, thread_kind);
                        cx.widget_action(
                            room_screen_widget_uid,
                            RoomsListAction::Selected(SelectedRoom::Thread {
                                room_name_id,
                                thread_root_event_id,
                            }),
                        );
                    }
                }
                MessageAction::Edit(details) => {
                    let Some(tl) = self.tl_state.as_ref() else { return };
                    if let Some(event_tl_item) = Self::find_event_in_timeline(&tl.items, details) {
                        self.view.room_input_bar(cx, ids!(room_input_bar))
                            .show_editing_pane(
                                cx,
                                event_tl_item.clone(),
                                tl.kind.clone(),
                            );
                    }
                    else {
                        enqueue_popup_notification(
                            "Could not find message in timeline to edit. Please try again.",
                            PopupKind::Error,
                            Some(5.0),
                        );
                        error!("MessageAction::Edit: couldn't find event [{}] {:?} to edit in room {:?}",
                            details.item_id,
                            details.timeline_event_id,
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
                        self.view.room_input_bar(cx, ids!(room_input_bar))
                            .show_editing_pane(
                                cx,
                                latest_sent_msg,
                                tl.kind.clone(),
                            );
                    }
                    else {
                        enqueue_popup_notification(
                            "No recent message available to edit. Please manually select a message to edit.",
                            PopupKind::Warning,
                            Some(5.0),
                        );
                    }
                }
                MessageAction::Pin(details) => {
                    let Some(tl) = self.tl_state.as_ref() else { return };
                    if let Some(event_id) = details.event_id() {
                        submit_async_request(MatrixRequest::PinEvent {
                            room_id: tl.kind.room_id().clone(),
                            event_id: event_id.clone(),
                            pin: true,
                        });
                    } else {
                        enqueue_popup_notification(
                            "This event cannot be pinned.",
                            PopupKind::Error,
                            Some(5.0),
                        );
                    }
                }
                MessageAction::Unpin(details) => {
                    let Some(tl) = self.tl_state.as_ref() else { return };
                    if let Some(event_id) = details.event_id() {
                        confirm_unpin_message(cx, tl.kind.room_id().clone(), event_id.clone(), false);
                    } else {
                        enqueue_popup_notification(
                            "This event cannot be unpinned.",
                            PopupKind::Error,
                            Some(5.0),
                        );
                    }
                }
                MessageAction::CopyText(details) => {
                    let Some(tl) = self.tl_state.as_ref() else { return };
                    if let Some(event_tl_item) = Self::find_event_in_timeline(&tl.items, details) {
                        cx.copy_to_clipboard(&plaintext_body_of_timeline_item(event_tl_item));
                    }
                    else {
                        enqueue_popup_notification(
                            "Could not find message in timeline to copy text from. Please try again.",
                            PopupKind::Error,
                            Some(5.0),
                        );
                        error!("MessageAction::CopyText: couldn't find event [{}] {:?} to copy text from in room {}",
                            details.item_id,
                            details.timeline_event_id,
                            tl.kind.room_id(),
                        );
                    }
                }
                MessageAction::CopyHtml(details) => {
                    let Some(tl) = self.tl_state.as_ref() else { return };
                    // The logic for getting the formatted body of a message is the same
                    // as the logic used in `populate_message_view()`.
                    let mut success = false;
                    if let Some(event_tl_item) = Self::find_event_in_timeline(&tl.items, details) {
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
                        enqueue_popup_notification(
                            "Could not find message in timeline to copy HTML from. Please try again.",
                            PopupKind::Error,
                            Some(5.0),
                        );
                        error!("MessageAction::CopyHtml: couldn't find event [{}] {:?} to copy HTML from in room {}",
                            details.item_id,
                            details.timeline_event_id,
                            tl.kind.room_id(),
                        );
                    }
                }
                MessageAction::CopyLink(details) => {
                    let Some(tl) = self.tl_state.as_ref() else { return };
                    if let Some(event_id) = details.event_id() {
                        let matrix_to_uri = tl.kind.room_id().matrix_to_event_uri(event_id.clone());
                        cx.copy_to_clipboard(&matrix_to_uri.to_string());
                    } else {
                        enqueue_popup_notification(
                            "Couldn't create permalink to message. Please try again.",
                            PopupKind::Error,
                            Some(5.0),
                        );
                        error!("MessageAction::CopyLink: no `event_id`: [{}] {:?} in room {}",
                            details.item_id,
                            details.timeline_event_id,
                            tl.kind.room_id(),
                        );
                    }
                }
                MessageAction::ViewSource(details) => {
                    let Some(tl) = self.tl_state.as_ref() else { continue };
                    let Some(event_tl_item) = Self::find_event_in_timeline(&tl.items, details) else {
                        enqueue_popup_notification(
                            "Could not find message in timeline to view source.",
                            PopupKind::Error,
                            Some(5.0),
                        );
                        continue;
                    };
                    // Get the latest JSON from the event and pretty-print it
                    let latest_json: Option<String> = event_tl_item
                        .latest_json()
                        .and_then(|raw_event| serde_json::to_value(raw_event).ok())
                        .and_then(|value| serde_json::to_string_pretty(&value).ok());

                    let event_id = event_tl_item.event_id().map(|e| e.to_owned());

                    cx.action(super::event_source_modal::EventSourceModalAction::Open {
                        room_id: tl.kind.room_id().clone(),
                        event_id,
                        latest_json,
                    });
                }
                MessageAction::JumpToRelated(details) => {
                    let Some(related_event_id) = details.related_event_id.as_ref() else {
                        error!("BUG: MessageAction::JumpToRelated had no related event ID.\n{details:#?}");
                        enqueue_popup_notification(
                            "Could not find related message or event in timeline.",
                            PopupKind::Error,
                            Some(5.0),
                        );
                        continue;
                    };
                    // Get the sender of the replied-to event so we can show it in the loading pane.
                    let replied_to_sender_and_room = self.tl_state.as_ref().and_then(|tl| {
                        let sender = tl.items.get(details.item_id)
                            .and_then(|item| item.as_event())
                            .and_then(|ev| match ev.content() {
                                TimelineItemContent::MsgLike(msg_like) => msg_like.in_reply_to.as_ref(),
                                _ => None,
                            })
                            .and_then(|reply| match &reply.event {
                                TimelineDetails::Ready(replied_to) => Some(replied_to.sender.clone()),
                                _ => None,
                            })?;
                        Some((sender, tl.kind.room_id().clone()))
                    });
                    let searching_for = match replied_to_sender_and_room {
                        Some((sender, room_id)) => format!(
                            "the message from {}", display_name_or_user_id(cx, &room_id, sender),
                        ),
                        None => String::from("the replied-to message"),
                    };
                    self.jump_to_event(
                        cx,
                        related_event_id,
                        Some(details.item_id),
                        searching_for,
                        portal_list,
                        loading_pane
                    );
                }
                MessageAction::ToggleReplyPreviewExpanded(message_id) => {
                    if let Some(tl) = self.tl_state.as_mut() {
                        if !tl.expanded_reply_previews.remove(message_id) {
                            tl.expanded_reply_previews.insert(message_id.clone());
                        }
                    }
                    self.redraw(cx);
                }
                MessageAction::JumpToEvent(event_id) => {
                    self.jump_to_event(
                        cx,
                        event_id,
                        None,
                        String::from("the message you're replying to"),
                        portal_list,
                        loading_pane
                    );
                }
                MessageAction::OpenThread(thread_root_event_id) => {
                    let Some(room_name_id) = self.room_name_id.as_ref().cloned() else {
                        error!("### ERROR: MessageAction::OpenThread: thread_root_event_id: {thread_root_event_id}, but room_name_id was None!");
                        continue
                    };
                    cx.widget_action(
                        room_screen_widget_uid, 
                        RoomsListAction::Selected(SelectedRoom::Thread {
                            room_name_id,
                            thread_root_event_id: thread_root_event_id.clone(),
                        }),
                    );
                }
                MessageAction::Redact { details, reason } => {
                    let Some(tl) = self.tl_state.as_ref() else { return };
                    let timeline_event_id = details.timeline_event_id.clone();
                    let timeline_kind = tl.kind.clone();
                    let reason = reason.clone();
                    // An unsent message should just be immediately discarded without confirmation,
                    // as the user might be rushing to quickly cancel it before it actually sends.
                    if matches!(timeline_event_id, TimelineEventItemId::TransactionId(_)) {
                        submit_async_request(MatrixRequest::RedactMessage {
                            timeline_kind,
                            timeline_event_id,
                            reason,
                        });
                        continue;
                    }
                    let content = ConfirmationModalContent {
                        title_text: "Delete Message".into(),
                        body_text: "Are you sure you want to delete this message? This cannot be undone.".into(),
                        accept_button_text: Some("Delete".into()),
                        on_accept_clicked: Some(Box::new(move |_cx| {
                            submit_async_request(MatrixRequest::RedactMessage {
                                timeline_kind,
                                timeline_event_id,
                                reason,
                            });
                        })),
                        ..Default::default()
                    };
                    cx.action(ConfirmDeleteAction::Show(RefCell::new(Some(content))));
                }
                MessageAction::RetrySend(details) => {
                    let Some(tl) = self.tl_state.as_ref() else { return };
                    submit_async_request(MatrixRequest::RetrySend {
                        timeline_kind: tl.kind.clone(),
                        timeline_event_id: details.timeline_event_id.clone(),
                    });
                }
                // MessageAction::Report(details) => {
                //     // TODO
                // }

                MessageAction::DownloadAttachment(info) => {
                    self.begin_media_transfer(cx, portal_list, info, TransferKind::Download, start_attachment_download);
                }
                MessageAction::ShareAttachment(info) => {
                    self.begin_media_transfer(cx, portal_list, info, TransferKind::Share, start_attachment_share);
                }
                MessageAction::CancelDownload(mxc) => {
                    submit_async_request(MatrixRequest::CancelDownload(mxc.clone()));
                    if let Some(tl) = self.tl_state.as_mut()
                        && let Some(i) = tl.pending_downloads.iter().position(|p| &p.mxc == mxc)
                    {
                        tl.pending_downloads.swap_remove(i);
                        portal_list.redraw(cx);
                    }
                }
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

    /// Expands or collapses the state event group that contains the timeline item at `index`.
    fn toggle_state_event_group(&mut self, cx: &mut Cx, index: usize, portal_list: &PortalListRef) {
        let Some(tl) = self.tl_state.as_mut() else { return };
        let anchors = ScrollAnchors::take(&mut tl.scroll_anchors, portal_list, &tl.items, &tl.state_event_groups);
        let (groups, timeline) = tl.groups_and_timeline_info();
        let Some(group) = groups.toggle(index, &timeline) else {
            tl.scroll_anchors = Some(anchors);
            return;
        };
        tl.forget_drawn(group.ranges_to_redraw());
        let mut starts_at_summary_item = false;

        if !group.is_expanded {
            let summary_item_scrolled_off = portal_list.drawn_slot(group.range.start).is_none_or(|slot| slot.start < 0.0);
            // If it was collapsed using the "Collapse" button under its last event while its summary item
            // is scrolled up (even partly) out of view, keep the item after the group where it is on screen,
            // so the view doesn't jump. The summary item then ends up right above it.
            if index + 1 == group.range.end
                && summary_item_scrolled_off
                && let Some(slot) = portal_list.drawn_slot(group.range.end)
            {
                portal_list.set_first_id_and_scroll_in_place(group.range.end, slot.start);
            }
            // Otherwise, if the list starts at an item that's now hidden in the collapsed group,
            // start it at the group's summary item instead, since hidden items take up no space.
            else if group.items_after_summary().contains(&portal_list.first_id()) {
                portal_list.set_first_id_and_scroll_in_place(group.range.start, 0.0);
                starts_at_summary_item = true;
            }
        }
        tl.scroll_anchors = Some(if starts_at_summary_item {
            ScrollAnchors::at_list_position(portal_list, &tl.items, &tl.state_event_groups)
        } else {
            anchors.through_toggle(portal_list, &tl.items, &tl.state_event_groups, group.range.clone())
        });
        self.redraw(cx);
    }

    /// Smoothly scrolls the timeline to the event at `index` and then highlights it.
    ///
    /// If that event is hidden in a collapsed group, this expands the group instead,
    /// and the scroll happens later, once the expanded group has been drawn.
    fn scroll_to_event(&mut self, cx: &mut Cx, portal_list: &PortalListRef, index: usize, event_id: OwnedEventId) {
        let Some(tl) = self.tl_state.as_mut() else { return };
        // The items on screen, taken while the groups still match the list's last draw.
        let anchors = ScrollAnchors::take(&mut tl.scroll_anchors, portal_list, &tl.items, &tl.state_event_groups);
        let (groups, timeline) = tl.groups_and_timeline_info();
        if let Some(group) = groups.expand_containing(index, &timeline) {
            tl.forget_drawn(group.ranges_to_redraw());
            tl.scroll_anchors = Some(anchors.through_toggle(portal_list, &tl.items, &tl.state_event_groups, group.range.clone()));
            portal_list.redraw(cx);
            self.deferred_jump = Some(DeferredJump::new(DeferredJumpKind::ScrollTo { event_id }));
            return;
        }
        tl.scroll_anchors = Some(anchors);
        portal_list.smooth_scroll_to(cx, index, 50.0, None, 10.0);
        // On a far jump, the list first moves close to the target, so redraw it there right away:
        // the scroll animation needs those items drawn to know their real heights.
        portal_list.redraw(cx);
        tl.message_highlight_animation_state = MessageHighlightAnimationState::Pending { item_id: index, event_id };
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
        searching_for: String,
        portal_list: &PortalListRef,
        loading_pane: &LoadingPaneRef,
    ) {
        // Jumping to an event isn't really a user scroll action, so don't send read receipts based on jumps.
        self.read_receipt_state.cancel_timer(cx);
        // This jump replaces any jump that was waiting to happen until the timeline is drawn,
        // or that's still scrolling toward its target.
        self.deferred_jump = None;
        let Some(tl) = self.tl_state.as_mut() else { return };
        tl.message_highlight_animation_state = MessageHighlightAnimationState::Off;
        let max_tl_idx = max_tl_idx.unwrap_or_else(|| tl.items.len());

        // Attempt to find the index of replied-to message in the timeline.
        // Start from the current item's index (`tl_idx`) and search backwards,
        // since we know the related message must come before the current item.
        let related_msg_tl_index = index_of_event(&tl.items, target_event_id, max_tl_idx, MAX_ITEMS_TO_SEARCH_THROUGH);

        if let Some(index) = related_msg_tl_index {
            // log!("The related message {replied_to_event} was immediately found in room {}, scrolling to from index {reply_message_item_id} --> {index} (first ID {}).", tl.kind.room_id(), portal_list.first_id());
            self.scroll_to_event(cx, portal_list, index, target_event_id.clone());
        } else {
            log!("The related event {target_event_id} wasn't immediately available in room {}, searching for it in the background...", tl.kind.room_id());
            // The main logic is handled in `process_timeline_updates()`, the only
            // place where we receive updates to the timeline from background tasks.
            loading_pane.start_search(
                cx,
                target_event_id.clone(),
                searching_for,
                tl.kind.desc(),
                tl.request_sender.clone(),
            );

            tl.request_sender.send_if_modified(|req| {
                let request = BackwardsPaginateUntilEventRequest::new(
                    tl.kind.room_id().clone(),
                    target_event_id.clone(),
                    // Avoid searching through items we already searched through.
                    max_tl_idx.saturating_sub(MAX_ITEMS_TO_SEARCH_THROUGH),
                    tl.items.len(),
                );
                if let Some(existing) = req.backwards_paginate.iter_mut().find(|r| &r.room_id == tl.kind.room_id()) {
                    warning!("Unexpected: room {} already had an existing timeline request in progress, event: {:?}", tl.kind.room_id(), existing.target_event_id);
                    *existing = request;
                } else {
                    req.backwards_paginate.push(request);
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
        let kind = self.timeline_kind.clone()
            .expect("BUG: Timeline::show_timeline(): no timeline_kind was set.");
        let room_id = kind.room_id().clone();
        let owner = self.widget_uid();

        let (mut tl_state, is_new_tl_state) = match timeline_state_store::take(cx, &kind, owner) {
            timeline_state_store::TakeResult::Taken(existing) => (existing, false),
            timeline_state_store::TakeResult::AlreadyTaken { owner: current_owner } => {
                error!("RoomScreen::show_timeline(): timeline {kind} is already taken by widget {current_owner:?}");
                return;
            }
            timeline_state_store::TakeResult::Missing => {
                let Some(timeline_endpoints) = take_timeline_endpoints(&kind) else {
                    if let Some(thread_root_event_id) = kind.thread_root_event_id() {
                        submit_async_request(MatrixRequest::CreateThreadTimeline {
                            room_id: room_id.clone(),
                            thread_root_event_id: thread_root_event_id.clone(),
                        });
                        return;
                    }
                    if !self.is_loaded && self.all_rooms_loaded {
                        error!("BUG: timeline {kind} is not loaded, but its RoomScreen \
                        was not waiting for its timeline to be loaded either.");
                    }
                    return;
                };
                let TimelineEndpoints {
                    update_receiver,
                    update_sender,
                    request_sender,
                    successor_room,
                    is_encrypted,
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
                    kind,
                    // Initially, we assume the user has all power levels by default.
                    // This avoids unexpectedly hiding any UI elements that should be visible to the user.
                    // This doesn't mean that the user can actually perform all actions;
                    // the power levels will be updated from the homeserver once the room is opened.
                    user_power: UserPowerLevels::all(),
                    is_encrypted,
                    // Room members start as None and get populated when fetched from the server
                    room_members: None,
                    backwards_pagination: BackwardsPaginationState::default(),
                    items: Vector::new(),
                    index_of_last_own_sent: None,
                    index_of_first_own_failed: None,
                    content_drawn_since_last_update: RangeSet::new(),
                    profile_drawn_since_last_update: RangeSet::new(),
                    update_receiver,
                    request_sender,
                    media_cache: MediaCache::new(Some(update_sender.clone())),
                    link_preview_cache: LinkPreviewCache::new(Some(update_sender)),
                    fetched_thread_summaries: HashMap::new(),
                    pending_thread_summary_fetches: HashSet::new(),
                    saved_state: SavedState::default(),
                    message_highlight_animation_state: MessageHighlightAnimationState::default(),
                    paginate_again_when_done: false,
                    last_pagination_error_at: None,
                    last_sent_read_receipt: None,
                    last_sent_fully_read: None,
                    tombstone_info,
                    pending_downloads: SmallVec::new(),
                    expanded_reply_previews: HashSet::new(),
                    state_event_groups: StateEventGroups::default(),
                    pending_knocks: PendingKnocks::default(),
                    scroll_anchors: None,
                };
                timeline_state_store::mark_taken(cx, &tl_state.kind, owner);
                (tl_state, true)
            }
        };
        let mut is_first_time_being_loaded = is_new_tl_state;

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
                // log!("Detected that {}} is now loaded for the first time", tl_state.kind);
                is_first_time_being_loaded = true;
            }
            self.is_loaded = is_loaded_now;
        }

        self.view.restore_status_view(cx, ids!(restore_status_view)).set_visible(cx, !self.is_loaded);

        if is_first_time_being_loaded {
            // Even though we specify that room member profiles should be lazy-loaded,
            // the matrix server still doesn't consistently send them to our client properly.
            // So we kick off a request to fetch the room members here upon first viewing the room.
            submit_async_request(MatrixRequest::SyncRoomMemberList {
                timeline_kind: tl_state.kind.clone(),
            });
        }

        // If the room is loaded, we need to get a few key states:
        // 1. Get the current user's power levels for this room so that we can
        //    show/hide UI elements based on the user's permissions.
        // 2. Get the list of members in this room (from the SDK's local cache).
        // 3. Subscribe to our own user's read receipts so that unread counts
        //    refresh when our read position advances (from any device).
        // 4. Subscribe to typing notices again if they're enabled, now that the room is being shown.
        if self.is_loaded {
            submit_async_request(MatrixRequest::GetRoomPowerLevels {
                timeline_kind: tl_state.kind.clone(),
            });
            submit_async_request(MatrixRequest::GetRoomMembers {
                timeline_kind: tl_state.kind.clone(),
                memberships: matrix_sdk::RoomMemberships::ACTIVE,
                // Fetch from the local cache, as we already requested to sync
                // the room members from the homeserver above.
                local_only: true,
            });
            // Only main room timelines can subscribe to typing notices, pinned events,
            // and read receipt changes (the SDK has no per-thread unread counts).
            if matches!(tl_state.kind, TimelineKind::MainRoom { .. }) {
                let show_typing_notices = cx.global::<AppPreferencesGlobal>().0.show_typing_notices;
                subscribe_to_room_updates(&tl_state.kind, true, show_typing_notices);
                // The matrix spec says that opening a room should clear the marked-as-unread flag.
                if cx.global::<AppPreferencesGlobal>().0.mark_as_read_behavior != MarkAsReadBehavior::Manual {
                    submit_async_request(MatrixRequest::SetUnreadFlag {
                        room_id: room_id.clone(),
                        mark_as_unread: false,
                    });
                }
            }
        }

        // Now, restore the visual state of this timeline from its previously-saved state.
        self.restore_state(cx, &mut tl_state);
        // Until the list draws this timeline for the first time, keep its restored first item in place.
        if tl_state.scroll_anchors.is_none() {
            let list = self.portal_list(cx, ids!(timeline.list));
            tl_state.scroll_anchors = Some(ScrollAnchors::at_list_position(&list, &tl_state.items, &tl_state.state_event_groups));
        }

        // Store the tl_state for this room into this RoomScreen widget,
        // such that it can be accessed in future functions like event/draw handlers.
        self.tl_state = Some(tl_state);

        // Tell the background subscriber that this timeline is now open (if it was previously closed).
        if let Some(tl) = self.tl_state.as_ref() {
            tl.request_sender.send_if_modified(|req| !std::mem::replace(&mut req.is_timeline_open, true));
        }

        let list = self.portal_list(cx, ids!(timeline.list));
        self.read_receipt_state.on_timeline_shown(&list);

        // Now that we have restored the TimelineUiState into this RoomScreen widget,
        // we can proceed to processing pending background updates.
        // We first need to check that we have the latest endpoints, to get the latest updates.
        self.reconnect_timeline_endpoints(cx, false);
        self.process_timeline_updates(cx, &list);

        // Kick off a back pagination request if it's the first time loading this room, so the user
        // sees some messages asap. This comes after processing updates in case the rooms list already sent
        // one for this room, since that request's `PaginationCompleted` would make us think ours was done too.
        //
        // If it's NOT the first time loading this room, don't paginate, since that'll mess up our indices.
        if is_new_tl_state
            && let Some(tl) = self.tl_state.as_mut()
            && !tl.backwards_pagination.is_fully_paginated()
            && !tl.backwards_pagination.is_loading()
        {
            log!("Sending a first-time backwards pagination request for {}", tl.kind);
            tl.paginate_backwards();
        }

        self.redraw(cx);
    }

    /// Sets this timeline's endpoints to the latest ones created by the backend task,
    /// if there are any new ones.
    ///
    /// `take_timeline_endpoints()` only returns `Some` when a fresh unclaimed channel exists,
    /// so it's safe to call this repeatedly as a check.
    fn reconnect_timeline_endpoints(&mut self, cx: &mut Cx, resubscribe: bool) {
        let Some(tl) = self.tl_state.as_mut() else { return };
        // An invalidated state means we destructed the timeline on purpose
        // (e.g., for a left/banned room or a closed thread),
        // so don't take the endpoints for it even if they're available.
        if timeline_state_store::is_invalidated(&tl.kind) {
            return;
        }
        let Some(TimelineEndpoints {
            update_sender,
            update_receiver,
            request_sender,
            successor_room,
            is_encrypted,
        }) = take_timeline_endpoints(&tl.kind) else {
            // If a room timeline was rebuilt, its thread timelines were also dropped,
            // so we need to ask the backend to recreate them for us.
            if let Some(thread_root_event_id) = tl.kind.thread_root_event_id() {
                submit_async_request(MatrixRequest::CreateThreadTimeline {
                    room_id: tl.kind.room_id().clone(),
                    thread_root_event_id: thread_root_event_id.clone(),
                });
            }
            return;
        };

        log!("Reconnecting timeline {} to its newly-created backend channel.", tl.kind);

        // Transfer over any pending jump-to-event searches so it isn't silently dropped.
        let mut pending_searches = Vec::new();
        tl.request_sender.send_if_modified(|req| {
            pending_searches = std::mem::take(&mut req.backwards_paginate);
            false
        });
        tl.update_receiver = update_receiver;
        // Pagination requests sent to the old timeline report back on the old channel we just dropped,
        // so forget about them. The new timeline's first items will tell us whether it's fully paginated.
        tl.backwards_pagination.reset();
        tl.paginate_again_when_done = false;
        tl.request_sender = request_sender;
        if !pending_searches.is_empty() {
            tl.request_sender.send_if_modified(|req| {
                req.backwards_paginate = pending_searches;
                true
            });
        }
        tl.media_cache.set_timeline_update_sender(update_sender.clone());
        tl.link_preview_cache.set_timeline_update_sender(update_sender);
        tl.is_encrypted = is_encrypted;
        if tl.tombstone_info.is_none() && let Some(successor_room) = successor_room {
            submit_async_request(MatrixRequest::GetSuccessorRoomDetails {
                tombstoned_room_id: tl.kind.room_id().clone(),
            });
            tl.tombstone_info = Some(SuccessorRoomDetails::Basic(successor_room));
        }
        // If `tl_state` was Some here, it means the timeline is being shown (it's visible),
        // so inform the subscribers of that status.
        tl.request_sender.send_if_modified(|req| !std::mem::replace(&mut req.is_timeline_open, true));
        let reconnected_sender = tl.request_sender.clone();
        let timeline_kind = tl.kind.clone();
        submit_async_request(MatrixRequest::SyncRoomMemberList { timeline_kind: timeline_kind.clone() });
        // Re-subscribe to things needed for this main room timeline to be properly updated
        // while it's open. The previously-created async tasks for these things are either dead
        // or still running but with the old channel endpoints, so they're useless either way.
        if resubscribe {
            let show_typing_notices = cx.global::<AppPreferencesGlobal>().0.show_typing_notices;
            subscribe_to_room_updates(&timeline_kind, true, show_typing_notices);
        }
        let loading_pane = self.loading_pane(cx, ids!(loading_pane));
        // Also update the loading pane's timeline request sender.
        loading_pane.set_timeline_request_sender(reconnected_sender);
        // If the loading pane was searching for an older event, the in-progress pagination request
        // might've been cancelled while the timeline was being re-created. So we restart it here.
        if loading_pane.is_searching() && let Some(tl) = self.tl_state.as_mut() {
            tl.paginate_backwards();
        }
        // The bkgd upload task still holds the old channel endpoints, so let the upload
        // progress view deal with whatever upload it was showing.
        self.view.room_input_bar(cx, ids!(room_input_bar)).on_timeline_reconnected(cx);
        self.redraw(cx);
    }

    /// Invoke this when this RoomScreen/timeline is being hidden or no longer being shown.
    fn hide_timeline(&mut self) {
        let Some(timeline_kind) = self.timeline_kind.clone() else { return };
        if self.tl_state.is_none() {
            return;
        }

        // Don't send read receipts if we're hiding the timeline.
        self.read_receipt_state.clear();
        // Closing/hiding the room should cancel any pending jump/search.
        self.pending_read_receipt_jump = None;
        self.deferred_jump = None;

        // Tell the background subscriber that this timeline is now closed.
        if let Some(tl) = self.tl_state.as_ref() {
            tl.request_sender.send_if_modified(|req| std::mem::replace(&mut req.is_timeline_open, false));
        }

        self.save_state();

        // When closing a room view, we do the following with non-persistent states.
        // (This should be the inverse of what's done in `show_timeline()`.)
        // * Unsubscribe from typing notices, since we don't care about them
        //   when a given room isn't visible.
        // * Unsubscribe from updates to this room's pinned events, for the same reason.
        // * Unsubscribe from updates to our own user's read receipts, for the same reason.
        subscribe_to_room_updates(&timeline_kind, false, false);
    }

    /// Removes the current room's visual UI state from this widget
    /// and saves it to the timeline state store such that it can be restored later.
    ///
    /// Note: after calling this function, the widget's `tl_state` will be `None`.
    fn save_state(&mut self) {
        let Some(mut tl) = self.tl_state.take() else {
            error!("Timeline::save_state(): skipping due to missing state, room {:?}, {:?}", self.timeline_kind, self.room_name_id.as_ref().map(|r| r.display_name()));
            return;
        };

        let portal_list = self.child_by_path(ids!(timeline.list)).as_portal_list();
        // The list's last draw of this timeline won't be around once it's shown again, so capture its anchors now.
        if tl.scroll_anchors.is_none() {
            tl.scroll_anchors = Some(ScrollAnchors::capture(&portal_list, &tl.items, &tl.state_event_groups));
        }
        let room_input_bar = self.child_by_path(ids!(room_input_bar)).as_room_input_bar();
        log!("Saving state for room {:?}\n\t{:?}\n\tfirst_id: {:?}, scroll: {}", self.room_name_id.as_ref().map(|r| r.display_name()), self.timeline_kind, portal_list.first_id(), portal_list.scroll_position());
        let state = SavedState {
            first_index_and_scroll: Some((portal_list.first_id(), portal_list.scroll_position())),
            was_at_end: portal_list.is_at_end(),
            room_input_bar_state: room_input_bar.save_state(),
            room_panes: self.child_by_path(ids!(room_pane_dock)).as_room_pane_dock().save_state(),
        };
        tl.saved_state = state;
        // Clear room_members to avoid wasting memory (in case this room is never re-opened).
        tl.room_members = None;
        // Store this Timeline's `TimelineUiState` until a RoomScreen shows it again.
        timeline_state_store::put_back(self.widget_uid(), tl);
    }

    /// Restores the previously-saved visual UI state of this room.
    ///
    /// Note: this accepts a direct reference to the timeline's UI state,
    /// so this function must not try to re-obtain it by accessing `self.tl_state`.
    fn restore_state(&mut self, cx: &mut Cx, tl_state: &mut TimelineUiState) {
        let SavedState {
            first_index_and_scroll,
            was_at_end,
            room_input_bar_state,
            room_panes,
        } = &mut tl_state.saved_state;

        // 0. Restore this timeline's docked panes.
        if let Some(room_name_id) = self.room_name_id.as_ref() {
            self.view.room_pane_dock(cx, ids!(room_pane_dock)).show_timeline(
                cx,
                room_name_id,
                tl_state.kind.clone(),
                tl_state.room_members.clone(),
                std::mem::take(room_panes),
            );
        }

        // 1. Restore the position of the timeline.
        let portal_list = self.portal_list(cx, ids!(timeline.list));
        if let Some((first_index, scroll_from_first_id)) = first_index_and_scroll {
            log!("Restoring state for room {:?}: first_id: {:?}, scroll: {}", self.room_name_id, first_index, scroll_from_first_id);
            portal_list.set_first_id_and_scroll(*first_index, *scroll_from_first_id);
            portal_list.set_tail_range(*was_at_end);
        } else {
            // If the first index is not set, then the timeline has not yet been scrolled by the user,
            // so we reset the portal list's scroll position and set it to "tail" (track) the bottom.
            // The explicit reset is necessary when the same RoomScreen widget is reused for a
            // different room (e.g., via stack navigation view alternation), otherwise the portal list
            // would retain the previous room's scroll position which may be out of bounds.
            log!("Restoring state for room {:?}: first_id: None, scroll: None", self.room_name_id);
            portal_list.set_first_id_and_scroll(0, 0.0);
            portal_list.set_tail_range(true);
        }

        // 2. Restore the state of the room input bar.
        let room_input_bar = self.child_by_path(ids!(room_input_bar)).as_room_input_bar();
        let saved_room_input_bar_state = std::mem::take(room_input_bar_state);
        room_input_bar.restore_state(
            cx,
            tl_state.kind.clone(),
            saved_room_input_bar_state,
            tl_state.user_power,
            tl_state.tombstone_info.as_ref(),
            tl_state.is_encrypted,
        );
    }

    /// Re-fetches this timeline's members if they're shown in a docked members pane,
    /// syncing them from the server if some might be missing locally (which the SDK determines).
    fn refresh_members_pane(&mut self, cx: &mut Cx) {
        if self.is_loaded
            && let Some(tl) = self.tl_state.as_ref()
            && !timeline_state_store::is_invalidated(&tl.kind)
            && self.view.room_pane_dock(cx, ids!(room_pane_dock)).has_pane(&RoomPaneKind::Members)
        {
            submit_async_request(MatrixRequest::GetRoomMembers {
                timeline_kind: tl.kind.clone(),
                memberships: matrix_sdk::RoomMemberships::ACTIVE,
                local_only: false,
            });
        }
    }

    fn handle_room_action_bar_action(&mut self, cx: &mut Cx, action: RoomActionBarAction) {
        match action {
            RoomActionBarAction::TogglePane(kind) => {
                self.view.room_pane_dock(cx, ids!(room_pane_dock)).toggle(cx, kind);
            }
            RoomActionBarAction::Invite => {
                let Some(room_name_id) = self.room_name_id.clone() else { return };
                cx.action(InviteModalAction::Open(room_name_id));
            }
            RoomActionBarAction::LayoutChanged { .. } | RoomActionBarAction::None => {}
        }
    }

    /// Jumps to the given event in this RoomScreen's timeline once it has been drawn.
    fn jump_to_event_after_draw(&mut self, cx: &mut Cx, event_id: OwnedEventId, description: String) {
        self.deferred_jump = Some(DeferredJump::new(DeferredJumpKind::Search { event_id, description }));
        self.redraw(cx);
    }

    /// Sets this `RoomScreen` widget to display the timeline for the given room.
    pub fn set_displayed_room(
        &mut self,
        cx: &mut Cx,
        room_name_id: &RoomNameId,
        thread_root_event_id: Option<OwnedEventId>,
    ) {
        let timeline_kind = if let Some(thread_root_event_id) = thread_root_event_id {
            TimelineKind::Thread {
                room_id: room_name_id.room_id().clone(),
                thread_root_event_id,
            }
        } else {
            TimelineKind::MainRoom {
                room_id: room_name_id.room_id().clone(),
            }
        };

        // If we opened this timeline to reply in thread, give the text input key focus.
        if input_bar_focus::take_if_matches(cx, &timeline_kind) {
            self.focus_input_bar_on_show = true;
        }

        if self.timeline_kind.as_ref() != Some(&timeline_kind) {
            self.deferred_jump = None;
        }

        // If this timeline is already displayed, we don't need to do anything major,
        // but we do need update the `room_name_id` in case it has changed/cleared.
        if self.tl_state.is_some() && self.timeline_kind.as_ref().is_some_and(|k| k == &timeline_kind) {
            self.room_name_id = Some(room_name_id.clone());
            self.view.room_pane_dock(cx, ids!(room_pane_dock)).set_room_name(cx, room_name_id);
            return;
        }

        // Hiding the previous timeline saves its docked panes, so we can then clear them.
        self.hide_timeline();
        self.view.room_pane_dock(cx, ids!(room_pane_dock)).clear(cx);
        // Reset the the state of the inner loading pane.
        self.loading_pane(cx, ids!(loading_pane)).hide(cx);
        // Reset the user profile sliding pane so a previous room's open profile
        // pane doesn't remain shown when this RoomScreen is reused for a new room.
        self.user_profile_sliding_pane(cx, ids!(user_profile_sliding_pane)).reset(cx);
        // Hide any typing notice left over from the previous room.
        self.view.typing_notice(cx, ids!(typing_notice)).show_or_hide(cx, &[], Animate::No);

        self.room_input_popup_menu(cx, ids!(room_input_popup_menu)).close(cx);

        self.room_name_id = Some(room_name_id.clone());
        self.timeline_kind = Some(timeline_kind.clone());

        // Tell the room input bar which room/thread we're now displaying.
        // The list of room members is None for now, it'll get updated later.
        self.view.room_input_bar(cx, ids!(room_input_bar))
            .set_room_context(cx, self.widget_uid(), timeline_kind.clone(), None);

        self.show_timeline(cx);
    }

    pub fn hide_displayed_room(&mut self, cx: &mut Cx) {
        if self.tl_state.is_some() {
            self.hide_timeline();
        }

        // Close all overlay views before this screen is reused for another room.
        self.view.room_pane_dock(cx, ids!(room_pane_dock)).clear(cx);
        self.loading_pane(cx, ids!(loading_pane)).hide(cx); // also cancels an in-progress search
        self.user_profile_sliding_pane(cx, ids!(user_profile_sliding_pane)).reset(cx);
        self.room_input_popup_menu(cx, ids!(room_input_popup_menu)).close(cx);

        self.room_name_id = None;
        self.timeline_kind = None;
        self.deferred_jump = None;
        self.pinned_events.clear();
        self.is_loaded = false;
        self.all_rooms_loaded = false;
        self.view.restore_status_view(cx, ids!(restore_status_view)).set_visible(cx, false);
        self.redraw(cx);
    }

    /// Sends read receipts for the events the user has seen in the timeline.
    ///
    /// This should only be called if a user's direct timeline interaction has finished.
    fn send_read_receipts_for_visible_events(
        &mut self,
        cx: &mut Cx,
        portal_list: &PortalListRef,
    ) {
        if cx.global::<AppPreferencesGlobal>().0.mark_as_read_behavior == MarkAsReadBehavior::Manual {
            return;
        }
        let Some(tl_state) = self.tl_state.as_mut() else { return };
        if tl_state.items.is_empty() { return; }

        // If we're at the very bottom of the timeline, mark everything as read.
        let index_of_last_seen = if portal_list.is_at_end() {
            Some(tl_state.items.len() - 1)
        } else {
            let visible_count = portal_list.visible_items();
            if visible_count == 0 { return; }
            let first_index = portal_list.first_id();
            let list_rect = portal_list.area().rect(cx);
            if list_rect.size.y <= 0.0 { return; }
            // A small tolerance so an item flush with the bottom edge counts as seen.
            let viewport_bottom = list_rect.pos.y + list_rect.size.y + 1.0;
            // The portallist skips over anything hidden in a collapsed group, so find the items it actually drew.
            let mut drawn = Vec::with_capacity(visible_count);
            let mut index = first_index;
            while drawn.len() < visible_count && index < tl_state.items.len() {
                drawn.push(index);
                index = tl_state.next_drawn_index(index);
            }
            // A message counts as seen once its bottom edge is fully within the portallist viewport.
            let mut found = None;
            for &index in drawn.iter().rev() {
                let Some((_, item_widget)) = portal_list.get_item(index) else { continue };
                let rect = item_widget.area().rect(cx);
                if rect.size.y <= 0.0 { continue; }
                if rect.pos.y + rect.size.y <= viewport_bottom {
                    found = Some(index);
                    break;
                }
            }
            found
        };
        let Some(index_of_last_seen) = index_of_last_seen else { return };
        // If the user has seen a collapsed group's summary, we can only treat that
        // as the user having seen the entire group, which is what they certainly expect.
        let index_of_last_seen = tl_state.state_event_groups.containing(index_of_last_seen)
            .filter(|group| !group.is_expanded)
            .map_or(index_of_last_seen, |group| group.range.end - 1);

        // The read receipt target is the nearest *real* event at or above that item
        // (we ignore virtual items like day dividers, the read marker, and local echoes).
        let target_event_id = (0 ..= index_of_last_seen).rev().find_map(|index|
            tl_state.items
                .get(index)
                .and_then(|item| item.as_event())
                .and_then(|ev| ev.event_id())
                .map(|ev_id| ev_id.to_owned())
        );
        let Some(target_event_id) = target_event_id else { return };

        if tl_state.last_sent_read_receipt.as_deref() != Some(&target_event_id) {
            tl_state.last_sent_read_receipt = Some(target_event_id.clone());
            submit_async_request(MatrixRequest::ReadReceipt {
                timeline_kind: tl_state.kind.clone(),
                event_id: target_event_id.clone(),
                receipt_type: preferred_receipt_type(),
            });
        }

        // FullyRead moves the room-wide read marker; do NOT send it from a thread.
        if !matches!(tl_state.kind, TimelineKind::MainRoom { .. }) { return; }
        // A "New Messages" marker still below what we've seen means there are
        // unread messages further down, so don't claim to have read past it.
        // (No marker at all just means the fully-read event isn't loaded.)
        let num_items_below_view = tl_state.items.len() - 1 - index_of_last_seen;
        let is_marker_below_view = tl_state.items
            .iter()
            .rev()
            .take(num_items_below_view)
            .any(|item| matches!(item.kind(), TimelineItemKind::Virtual(VirtualTimelineItem::ReadMarker)));
        if is_marker_below_view { return; }

        // Never move the marker backwards past a FullyRead we already sent.
        // Look backwards from the end to find the last-sent FullyRead event.
        let mut advances = true;
        if let Some(last_sent_id) = tl_state.last_sent_fully_read.as_deref() {
            for item in tl_state.items.iter().rev() {
                let Some(ev_id) = item.as_event().and_then(|ev| ev.event_id()) else { continue };
                if ev_id == last_sent_id {
                    advances = false;
                    break;
                }
                if ev_id == target_event_id {
                    break;
                }
            }
        }
        if advances {
            tl_state.last_sent_fully_read = Some(target_event_id.clone());
            submit_async_request(MatrixRequest::ReadReceipt {
                timeline_kind: tl_state.kind.clone(),
                event_id: target_event_id,
                receipt_type: ReceiptType::FullyRead,
            });
        }
    }

    /// Returns the widget refs used on every event, caching them if None.
    fn cached_widget_refs(&mut self, cx: &mut Cx) -> RoomScreenWidgetRefs {
        if let Some(refs) = &self.cached_refs {
            return refs.clone();
        }
        let refs = RoomScreenWidgetRefs {
            portal_list: self.portal_list(cx, ids!(timeline.list)),
            user_profile_sliding_pane: self.user_profile_sliding_pane(cx, ids!(user_profile_sliding_pane)),
            loading_pane: self.loading_pane(cx, ids!(loading_pane)),
            room_input_popup_menu: self.room_input_popup_menu(cx, ids!(room_input_popup_menu)),
        };
        self.cached_refs = Some(refs.clone());
        refs
    }

    /// Asks for older history when the first item(s) in the timeline come into view.
    fn send_pagination_request_on_reached_start(
        &mut self,
        _cx: &mut Cx,
        actions: &ActionsBuf,
        portal_list: &PortalListRef,
    ) {
        if !portal_list.reached_start(actions) { return };
        let Some(tl) = self.tl_state.as_mut() else { return };
        if tl.backwards_pagination.is_fully_paginated() { return };
        log!("Timeline hit first item in {}", tl.kind);
        tl.paginate_backwards();
    }

    /// Shows the loading indicator for when we're fetching older messages.
    ///
    /// We keep the indicator visible until the UI has actually processed the new items.
    fn update_top_space_visibility(&self, cx: &mut Cx) {
        let is_loading = self.tl_state.as_ref().is_some_and(|tl| tl.backwards_pagination.is_loading());
        self.view.view(cx, ids!(top_space)).set_visible(cx, is_loading);
    }
}

impl RoomScreenRef {
    /// See [`RoomScreen::set_displayed_room()`].
    pub fn set_displayed_room(
        &self,
        cx: &mut Cx,
        room_name_id: &RoomNameId,
        thread_root_event_id: Option<OwnedEventId>,
    ) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.set_displayed_room(cx, room_name_id, thread_root_event_id);
    }

    pub fn hide_displayed_room(&self, cx: &mut Cx) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.hide_displayed_room(cx);
    }

    pub fn handle_room_action_bar_action(&self, cx: &mut Cx, action: RoomActionBarAction) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.handle_room_action_bar_action(cx, action);
    }

    /// Jumps to the given event once this RoomScreen has drawn the given timeline,
    /// e.g., right after navigating to that timeline, which could still be loading.
    ///
    /// Does nothing if this RoomScreen isn't showing the given timeline.
    pub fn jump_to_event_when_shown(
        &self,
        cx: &mut Cx,
        timeline_kind: &TimelineKind,
        event_id: OwnedEventId,
        description: String,
    ) {
        let Some(mut inner) = self.borrow_mut() else { return };
        if inner.timeline_kind.as_ref() != Some(timeline_kind) {
            error!("BUG: can't jump to event {event_id}: this RoomScreen isn't showing {timeline_kind}.");
            return;
        }
        inner.jump_to_event_after_draw(cx, event_id, description);
    }
}


/// Subscribes to or unsubscribes from room-level updates that are needed
/// while a main room's timeline is open and being shown.
///
/// Does nothing for thread-specific timelines.
fn subscribe_to_room_updates(timeline_kind: &TimelineKind, subscribe: bool, show_typing_notices: bool) {
    if !matches!(timeline_kind, TimelineKind::MainRoom { .. }) {
        return;
    }
    let room_id = timeline_kind.room_id();
    submit_async_request(MatrixRequest::SubscribeToOwnUserReadReceiptsChanged {
        room_id: room_id.clone(),
        subscribe,
    });
    submit_async_request(MatrixRequest::SubscribeToTypingNotices {
        room_id: room_id.clone(),
        subscribe: subscribe && show_typing_notices,
    });
    submit_async_request(MatrixRequest::SubscribeToPinnedEventIds {
        room_id: room_id.clone(),
        subscribe,
    });
}


/// Actions for the room screen's tooltip.
#[derive(Clone, Debug, Default)]
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
    #[default]
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
        ///
        /// This supersedes `changed_indices` and is used when the entire timeline is being redrawn.
        clear_cache: bool,
        /// Whether the UI should forget where the previous history started.
        ///
        /// Set this to `true` when history is cleared or replaced. Also set it when the backend
        /// sends the full current item list after a backwards pagination request, because that
        /// list may include a history reset the UI hasn't seen yet.
        ///
        /// On success, the following [`TimelineUpdate::PaginationCompleted`] update tells us whether
        /// we've reached the beginning of the current history.
        was_timeline_reset: bool,
        /// How many items at the end of `new_items` this update didn't touch, though they may have moved.
        num_unchanged_at_end: usize,
    },
    /// Only the upload progress of local echoes (pending message) changed.
    LocalEchoProgress {
        new_items: Vector<Arc<TimelineItem>>,
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
    /// One pagination request finished successfully.
    ///
    /// The backend sends the full current item list and any found target event *before* this notice,
    /// so the UI handles those items before deciding whether to hide the loading indicator or request another page.
    PaginationCompleted {
        /// Whether the requested end of history was reached, and no more pages are needed.
        /// This is the timeline start for backwards pagination, and the timeline end for forwards pagination.
        is_fully_paginated: bool,
        direction: PaginationDirection,
    },
    /// A notice that event details have been fetched from the server,
    /// including a `result` that indicates whether the request was successful.
    EventDetailsFetched {
        event_id: OwnedEventId,
        result: Result<(), matrix_sdk_ui::timeline::Error>,
    },
    /// A notice that fresh thread-summary details were fetched for a thread root.
    ThreadSummaryDetailsFetched {
        thread_root_event_id: OwnedEventId,
        timeline_item_index: usize,
        num_replies: Option<u32>,
        latest_reply_preview_text: Option<String>,
    },
    /// The result of a request to edit a message in this timeline.
    MessageEdited {
        timeline_event_item_id: TimelineEventItemId,
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
    /// A notice that the room's member list could not be fetched.
    RoomMembersListFetchFailed {
        error: String,
    },
    /// A notice with an option of Media Request Parameters that one or more requested media items (images, videos, etc.)
    /// that should be displayed in this timeline have now been fetched and are available.
    MediaFetched(MediaRequestParameters),
    /// A notice that one or more members of a this room are currently typing.
    TypingUsers {
        /// The list of users (their displayable name) who are currently typing in this room.
        users: Vec<String>,
    },
    /// An update containing the set of pinned events in this room.
    PinnedEventIds(Vec<OwnedEventId>),
    /// An update containing the currently logged-in user's power levels for this room.
    UserPowerLevels(UserPowerLevels),
    /// A notice that this room has been changed to use encryption.
    /// It's only possible to go from unencrypted --> encrypted, not the other way.
    RoomEncrypted,
    /// A read receipt failed to send, so the UI should allow retrying it.
    ReadReceiptSendFailed {
        receipt_type: ReceiptType,
        event_id: OwnedEventId,
    },
    /// The search for the given user's read receipt (last-seen event) finished.
    UserReadReceiptFetched {
        user_id: OwnedUserId,
        /// Their last-seen event ID (which we should now jump to),
        /// or `None` if we couldn't find it.
        event_id: Option<OwnedEventId>,
    },
    /// A notice that the given room has been tombstoned (closed)
    /// and replaced by the given successor room.
    Tombstoned(SuccessorRoomDetails),
    /// A notice that link preview data for a URL has been fetched and is now available.
    LinkPreviewFetched,
    /// A file upload has been started in the background for this timeline.
    FileUploadStarted {
        upload_id: FileUploadAttemptId,
        file_name: String,
        in_reply_to: Option<OwnedEventId>,
        abort_handle: futures_util::future::AbortHandle,
    },
    /// The file being uploaded was read from storage and handed to the send queue.
    /// This means that we have to discard the local echo message if we cancel it.
    FileUploadQueuing {
        upload_id: FileUploadAttemptId,
        transaction_id: OwnedTransactionId,
    },
    /// There's an update on the progress for a specific file upload attempt.
    FileUploadProgress {
        upload_id: FileUploadAttemptId,
        current_bytes: usize,
        total_bytes: usize,
    },
    /// A message failed before it even reached the send queue,
    /// so we should restore/redisplay it in the room input bar.
    SendFailedBeforeBeingQueued {
        message: RoomMessageEventContent,
        replied_to: Option<Reply>,
    },
    /// An error occurred during a specific file-upload attempt.
    FileUploadError {
        upload_id: FileUploadAttemptId,
        error: String,
        /// The upload to resubmit if the error was retry-able, otherwise `None`.
        retryable_upload: Option<crate::shared::file_upload_modal::AttachmentUpload>,
    },
    /// The room input bar is done showing this upload's status (either success or failure).
    ///
    /// At this point, the upload status view can be hidden, as the
    /// message's send indicator will show its status from here on out.
    FileUploadComplete {
        upload_id: FileUploadAttemptId,
    },
    /// Download finished. `Ok` if bytes hit disk, `Err(msg)` otherwise.
    /// The inline button briefly shows a success/failure indicator, then
    /// `AttachmentDownloadReset` clears the entry from `pending_downloads`.
    AttachmentDownloadFinished(OwnedMxcUri, Result<(), String>),
    /// Drop the entry so the inline button goes back to its default "Download …" label.
    AttachmentDownloadReset(OwnedMxcUri),
}

/// An action indicating that the main UI thread can now free the given set
/// of decoded images from makepad's image cache.
///
/// This is typically used for when a timeline has been closed and its
/// decoded images are no longer needed, so they can be dropped to save memory.
#[derive(Debug)]
pub struct DropDecodedImagesAction(pub Vec<PathBuf>);

/// Stores timeline UI state that is not currently owned by a `RoomScreen`.
mod timeline_state_store {
    use super::*;

    /// The current ownership state for a timeline's UI state.
    #[allow(clippy::large_enum_variant)]
    enum StateEntry {
        /// No widget is displaying this timeline, so its state is parked here,
        /// and can be taken by another `RoomScreen` in the future.
        Stored(TimelineUiState),
        /// A `RoomScreen` is displaying this timeline, so it's not available.
        Taken {
            /// The widget UID of the RoomScreen that is currently displaying this timeline.
            owner: WidgetUid,
            /// A flag indicating that this timeline's backend async task that handles
            /// timeline sync & updates was closed while the timeline was still being shown.
            /// See [`put_back()`] for more info. 
            invalidated: bool,
            /// A flag indicating that the screen showing this timeline was closed,
            /// so its docked panes can drop their loaded data once it's put back.
            was_closed: bool,
        },
    }

    /// Result of trying to take ownership of a timeline's UI state.
    #[allow(clippy::large_enum_variant)]
    pub(super) enum TakeResult {
        /// A previously-saved state existed and has been taken by the caller.
        Taken(TimelineUiState),
        /// Another widget already took this timeline state and is displaying it.
        AlreadyTaken { owner: WidgetUid },
        /// No saved state exists, so a new `TimelineUiState` can be created
        /// from newly-available timeline endpoints.
        Missing,
    }

    thread_local! {
        /// All timeline states (for timelines that have been shown),
        /// one per room/thread timeline. Only relevant from the main UI thread.
        static TIMELINE_STATES: RefCell<HashMap<TimelineKind, StateEntry>> = RefCell::new(HashMap::new());
    }

    /// Attempts to take ownership of the saved UI state for `kind`.
    pub(super) fn take(_cx: &mut Cx, kind: &TimelineKind, owner: WidgetUid) -> TakeResult {
        TIMELINE_STATES.with_borrow_mut(|states| {
            match states.remove(kind) {
                Some(StateEntry::Stored(state)) => {
                    states.insert(kind.clone(), StateEntry::Taken { owner, invalidated: false, was_closed: false });
                    TakeResult::Taken(state)
                }
                Some(StateEntry::Taken { owner: current_owner, invalidated, was_closed }) => {
                    states.insert(kind.clone(), StateEntry::Taken { owner: current_owner, invalidated, was_closed });
                    TakeResult::AlreadyTaken { owner: current_owner }
                }
                None => TakeResult::Missing,
            }
        })
    }

    /// Records that `owner` has created and taken ownership of a new timeline state.
    ///
    /// This is only supposed to be used when a new timeline is created by taking
    /// the backend timeline endpoints.
    pub(super) fn mark_taken(_cx: &mut Cx, kind: &TimelineKind, owner: WidgetUid) {
        TIMELINE_STATES.with_borrow_mut(|states| {
            match states.insert(kind.clone(), StateEntry::Taken { owner, invalidated: false, was_closed: false }) {
                Some(StateEntry::Stored(_)) => {
                    error!("RoomScreen::show_timeline(): timeline {kind} unexpectedly had a stored state while creating a new state");
                }
                Some(StateEntry::Taken { owner: current_owner, .. }) if current_owner != owner => {
                    error!("RoomScreen::show_timeline(): timeline {kind} was already taken by widget {current_owner:?}, but widget {owner:?} created a new state");
                }
                Some(StateEntry::Taken { .. }) | None => {}
            }
        });
    }

    /// Puts a timeline's UI state back into the store after a `RoomScreen` hides it,
    /// which allows a future RoomScreen to take it again.
    ///
    /// Note: this function gets called from drop handlers so it can't take `&mut Cx`,
    ///       but those drop handlers are only reachable from the main UI thread anyway.
    pub(super) fn put_back(owner: WidgetUid, mut state: TimelineUiState) {
        let kind = state.kind.clone();
        TIMELINE_STATES.with_borrow_mut(|states| {
            match states.remove(&kind) {
                Some(StateEntry::Taken { owner: current_owner, invalidated, was_closed }) if current_owner == owner => {
                    if invalidated || was_closed {
                        drop_decoded_images(&state);
                    }
                    // If it was invalidated and we (the `owner`) was the RoomScreen currently showing it,
                    // just return here to keep it removed from the TIMELINE_STATES.
                    if invalidated {
                        return;
                    }
                    if was_closed {
                        state.saved_state.room_panes.iter_mut().for_each(SavedRoomPane::drop_data);
                    }
                }
                Some(StateEntry::Taken { owner: current_owner, .. }) => {
                    error!("RoomScreen::save_state(): timeline {kind} was put back by widget {owner:?}, but it was taken by widget {current_owner:?}");
                }
                Some(StateEntry::Stored(_)) => {
                    error!("RoomScreen::save_state(): timeline {kind} was put back by widget {owner:?}, but a stored state already existed");
                }
                None => {
                    log!("RoomScreen::save_state(): timeline {kind} was put back by widget {owner:?} without a taken marker");
                }
            }
            states.insert(kind, StateEntry::Stored(state));
        });
    }

    /// Drops the loaded data of the given timeline's docked panes.
    /// Since its screen was closed, its decoded images get freed too.
    pub(super) fn drop_pane_data(_cx: &mut Cx, kind: &TimelineKind) {
        TIMELINE_STATES.with_borrow_mut(|states| match states.get_mut(kind) {
            Some(StateEntry::Stored(state)) => {
                state.saved_state.room_panes.iter_mut().for_each(SavedRoomPane::drop_data);
                drop_decoded_images(state);
            }
            Some(StateEntry::Taken { was_closed, .. }) => *was_closed = true,
            None => {}
        });
    }

    /// Drops every stored timeline state and `Taken` marker.
    ///
    /// This is used when all timeline UI state is being reset globally, such as
    /// during logout or session teardown.
    pub(super) fn clear_all(_cx: &mut Cx) {
        TIMELINE_STATES.with_borrow_mut(|states| {
            for entry in states.values() {
                if let StateEntry::Stored(state) = entry {
                    drop_decoded_images(state);
                }
            }
            states.clear();
        });
    }

    /// Marks the given timeline's cached UI state as invalidated because we closed/stopped
    /// the corresponding backend async timeline sync loop task for it.
    ///
    /// This ensures that a new timeline (and async sync task) will be re-created for it
    /// the next time that we want to show it.
    pub(super) fn invalidate(_cx: &mut Cx, kind: &TimelineKind) {
        TIMELINE_STATES.with_borrow_mut(|states| {
            if let Some(StateEntry::Taken { invalidated, .. }) = states.get_mut(kind) {
                // If this timeline is currently being shown (it was `Taken`), just set the flag
                // so it'll be dropped (see `put_back()`) when it's hidden by the RoomScreen.
                *invalidated = true;
                return;
            }

            // Otherwise, if it's not being shown, just remove it now.
            if let Some(StateEntry::Stored(state)) = states.remove(kind) {
                drop_decoded_images(&state);
            }
        });
    }

    /// Invalidates the states of a room's main timeline and all of its thread timelines,
    /// e.g., for when that room has been left or banned.
    pub(super) fn invalidate_entire_room(_cx: &mut Cx, room_id: &RoomId) {
        TIMELINE_STATES.with_borrow_mut(|states| {
            states.retain(|kind, entry| {
                if kind.room_id() != room_id {
                    return true;
                }
                match entry {
                    // Same as `invalidate()`: keep the shown UI state but flag it
                    // such that `put_back()` drops it when the RoomScreen hides it.
                    StateEntry::Taken { invalidated, .. } => {
                        *invalidated = true;
                        true
                    }
                    StateEntry::Stored(state) => {
                        drop_decoded_images(state);
                        false
                    }
                }
            });
        });
    }

    /// Frees the decoded images of a timeline that nothing shows anymore.
    fn drop_decoded_images(state: &TimelineUiState) {
        let image_keys = state.media_cache.get_image_cache_keys();
        if !image_keys.is_empty() {
            Cx::post_action(DropDecodedImagesAction(image_keys));
        }
    }

    /// Returns `true` if the given timeline's state was invalidated while a RoomScreen was still displaying it,
    /// meaning we deliberately destructed it (and therefore it shouldn't be auto-reconnected to new endpoints).
    pub(super) fn is_invalidated(kind: &TimelineKind) -> bool {
        TIMELINE_STATES.with_borrow(|states| {
            matches!(states.get(kind), Some(StateEntry::Taken { invalidated: true, .. }))
        })
    }
}

/// The UI-side state of a single room's timeline, which is only accessed/updated by the UI thread.
///
/// This struct should only include states that need to be persisted for a given room
/// across multiple `Hide`/`Show` cycles of that room's timeline within a RoomScreen.
/// If a state is more temporary and shouldn't be persisted when the timeline is hidden,
/// then it should be stored in the RoomScreen widget itself, not in this struct.
struct TimelineUiState {
    /// Info determining whether this is a main room timeline is a thread-focused timeline.
    kind: TimelineKind,

    /// The power levels of the currently logged-in user in this room.
    user_power: UserPowerLevels,

    /// Whether this room is encrypted. Once enabled it can never be disabled.
    is_encrypted: bool,

    /// The list of room members for this room.
    room_members: Option<Arc<Vec<RoomMember>>>,

    /// Tracks requests for older messages, their pending results, and whether the start was reached.
    ///
    /// Keeps the loading indicator visible until the UI has processed a page's items and result.
    backwards_pagination: BackwardsPaginationState,

    /// The list of items (events) in this room's timeline that our client currently knows about.
    items: Vector<Arc<TimelineItem>>,

    /// The index (in `items`) of our newest fully-sent message that nobody has read yet,
    /// which is the only message that should show a "Sent" status icon.
    index_of_last_own_sent: Option<usize>,

    /// The index (in `items`) of our earliest message that failed to send unrecoverably,
    /// which blocks every message queued after it until it's retried or cancelled.
    index_of_first_own_failed: Option<usize>,

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
    /// Upon a background update, only the changed items are removed from this set,
    /// plus any items whose state event group changed along with them.
    /// Toggling a group (or jumping into a collapsed one) also forgets that group's items.
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
    /// Cached fetched thread-summary details, keyed by thread-root event ID.
    fetched_thread_summaries: HashMap<OwnedEventId, FetchedThreadSummary>,
    /// Set of thread roots currently being fetched to avoid duplicate in-flight requests.
    pending_thread_summary_fetches: HashSet<OwnedEventId>,

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

    /// Whether another backwards pagination was asked for while one was already in progress.
    ///
    /// See `paginate_backwards()`, which handles this.
    paginate_again_when_done: bool,

    /// When a backwards pagination last failed; see [`RETRY_PAGINATION_AFTER_ERROR_DELAY`].
    last_pagination_error_at: Option<Instant>,

    /// The last event we sent a `Read`/`ReadPrivate` receipt for (to avoid re-sending).
    last_sent_read_receipt: Option<OwnedEventId>,
    /// The last event we sent a `FullyRead` receipt for (to avoid re-sending).
    last_sent_fully_read: Option<OwnedEventId>,

    /// If `Some`, this room has been tombstoned and the details of its successor room
    /// are contained within. If `None`, the room has not been tombstoned.
    tombstone_info: Option<SuccessorRoomDetails>,

    /// Media/file attachments in this timeline that are currently being downloaded.
    pending_downloads: SmallVec<[PendingDownload; 1]>,

    /// Reply previews the user has expanded that should be shown in full.
    /// Collapsed reply previews (their default state) are absent from this set.
    expanded_reply_previews: HashSet<TimelineEventItemId>,

    /// Contiguous groups of small state events that can be collapsed together.
    state_event_groups: StateEventGroups,

    /// Which knocks in this timeline are still waiting to be answered.
    ///
    /// Pending knocks should not be part of a collapsed group, and should show an invite button.
    pending_knocks: PendingKnocks,

    /// The items to keep in place on screen through timeline updates, until the list draws again.
    ///
    /// Right after the list draws, this is `None`, since those items are wherever that draw put them
    /// (see [`ScrollAnchors::take()`]). They're kept while the timeline is hidden too, so the updates
    /// it gets in the meantime keep those items in place.
    scroll_anchors: Option<ScrollAnchors>,
}

impl TimelineUiState {
    /// Returns this timeline's items, kind and pending knocks, which decide how items get drawn and grouped.
    fn timeline_info(&self) -> TimelineInfo<'_> {
        TimelineInfo { items: &self.items, kind: &self.kind, pending_knocks: &self.pending_knocks }
    }

    /// Returns this timeline's state event groups, plus the [`TimelineInfo`] they're grouped from.
    ///
    /// Regrouping needs the groups borrowed mutably and the items immutably at the same time,
    /// which only works by borrowing those fields separately, like this.
    fn groups_and_timeline_info(&mut self) -> (&mut StateEventGroups, TimelineInfo<'_>) {
        (&mut self.state_event_groups, TimelineInfo { items: &self.items, kind: &self.kind, pending_knocks: &self.pending_knocks })
    }

    /// Returns whether a backwards pagination failed less than [`RETRY_PAGINATION_AFTER_ERROR_DELAY`] ago.
    fn failed_recently(&self) -> bool {
        self.last_pagination_error_at.is_some_and(|at| at.elapsed() < RETRY_PAGINATION_AFTER_ERROR_DELAY)
    }

    /// Sends a back pagination request, or if one's already in progress, queues up another one for right after it.
    ///
    /// * Shows the loading indicator immediately.
    /// * Skips the request if we've reached the start or are waiting for the retry delay after an error.
    /// * Repeated requests will set `paginate_again_when_done`, which allows us to auto-ask for
    ///   another page after the current pagination request finishes.
    fn paginate_backwards(&mut self) {
        if self.backwards_pagination.is_fully_paginated() || self.failed_recently() {
            return;
        }
        if self.backwards_pagination.is_loading() {
            self.paginate_again_when_done = true;
            return;
        }
        self.backwards_pagination.mark_requested();
        submit_async_request(MatrixRequest::PaginateTimeline {
            timeline_kind: self.kind.clone(),
            num_events: 50,
            direction: PaginationDirection::Backwards,
        });
    }

    /// Returns the index of the next item after `index` that the portal list draws.
    ///
    /// The portal list skips items hidden in collapsed groups (see [`StateEventGroups::next_drawn_after()`]).
    fn next_drawn_index(&self, index: usize) -> usize {
        self.state_event_groups.next_drawn_after(index, self.items.len())
    }

    /// Returns whether there is visible older content above the viewport's `first_id` item.
    ///
    /// This skips a collapsed group of state events immediately above it.
    /// Loading more events into that group alone will not result in more message history
    /// being actually shown or available to scroll through.
    fn has_older_content(&self, first_id: usize) -> bool {
        let timeline = self.timeline_info();
        let top = self.state_event_groups.collapsed_group_right_before(&timeline, first_id).unwrap_or(first_id);
        state_event_group::shows_anything_before(&timeline, top)
    }

    /// Marks the items in the given ranges as not drawn, so they'll be fully redrawn next time.
    fn forget_drawn(&mut self, ranges: impl IntoIterator<Item = Range<usize>>) {
        for range in ranges {
            self.content_drawn_since_last_update.remove(range.clone());
            self.profile_drawn_since_last_update.remove(range);
        }
    }
}

#[derive(Default, Debug)]
enum MessageHighlightAnimationState {
    /// Highlight the item at `item_id` once the list's done scrolling to it.
    ///
    /// `event_id` tells us if older items came in during the scroll and moved that item to a different index.
    Pending { item_id: usize, event_id: OwnedEventId },
    #[default]
    Off,
}

/// A jump that must wait until the timeline has been drawn with its latest items,
/// since the timeline's PortalList can only scroll to an item that has actually been drawn.
struct DeferredJump {
    kind: DeferredJumpKind,
    /// This next frame gets triggered after the timeline was drawn,
    /// which is how the jump action actually gets started.
    frame: NextFrame,
    /// How many items the timeline had when it was drawn.
    num_items: usize,
}

impl DeferredJump {
    fn new(kind: DeferredJumpKind) -> Self {
        Self { kind, frame: NextFrame::default(), num_items: 0 }
    }
}

enum DeferredJumpKind {
    /// Jump to the given event, searching for it if needed (see `RoomScreen::jump_to_event()`).
    Search {
        event_id: OwnedEventId,
        description: String,
    },
    /// Scroll to the given event, which has already been found in the timeline.
    ScrollTo {
        event_id: OwnedEventId,
    },
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
    /// Whether the timeline was tracking the end of the list.
    was_at_end: bool,
    /// The state of all UI elements in the `RoomInputBar`.
    room_input_bar_state: RoomInputBarState,
    /// The panes docked around this timeline, e.g., the room's member list.
    room_panes: Vec<SavedRoomPane>,
}

/// The timeline's zero-height item, for hidden items, plus the odd one in a collapsed group that the list couldn't skip over.
///
/// It's a bare widget rather than a `View` for the sake of efficiency.
#[derive(Script, ScriptHook, Widget)]
pub struct ZeroHeightItem {
    #[uid] uid: WidgetUid,
    #[redraw] #[rust] area: Area,
    #[walk] walk: Walk,
}

impl Widget for ZeroHeightItem {
    fn handle_event(&mut self, _cx: &mut Cx, _event: &Event, _scope: &mut Scope) {}

    fn draw_walk(&mut self, cx: &mut Cx2d, _scope: &mut Scope, walk: Walk) -> DrawStep {
        cx.walk_turtle_with_area(&mut self.area, walk);
        DrawStep::done()
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(super) struct ItemDrawnStatus {
    /// Whether the profile info (avatar and displayable username) were drawn for this item.
    pub(super) profile_drawn: bool,
    /// Whether the content of the item was drawn (e.g., the message text, image, video, sticker, etc).
    pub(super) content_drawn: bool,
}

#[derive(Clone, Debug)]
struct FetchedThreadSummary {
    /// The server's reply count (0 if the root had no thread summary), or `None` if we couldn't fetch the root.
    num_replies: Option<u32>,
    latest_reply_preview_text: Option<String>,
    /// The SDK's own reply count when this was fetched, so its later changes apply on top.
    sdk_num_replies_at_fetch: u32,
}
impl ItemDrawnStatus {
    /// Returns a new `ItemDrawnStatus` with both `profile_drawn` and `content_drawn` set to `true`.
    const fn both_drawn() -> Self {
        Self {
            profile_drawn: true,
            content_drawn: true,
        }
    }
}

/// Searches backwards for our newest fully-sent message that nobody has read yet,
/// and for our earliest failed send, which blocks everything queued behind it.
fn own_send_indices(
    items: &Vector<Arc<TimelineItem>>,
    is_main_timeline: bool,
) -> (Option<usize>, Option<usize>) {
    let mut first_failed = None;
    for (index, item) in items.iter().enumerate().rev() {
        let Some(event) = item.as_event() else { continue };
        // Anyone who read this has read past everything older, so we can stop here.
        // Local echoes carry no receipts, so a failed send is always found first.
        if !event.read_receipts().is_empty() { return (None, first_failed) }
        if !event.is_own() { continue }
        match event.send_state() {
            Some(EventSendState::SendingFailed { is_recoverable: false, .. }) => first_failed = Some(index),
            None | Some(EventSendState::Sent { .. }) => {
                let is_message = matches!(event.content(), TimelineItemContent::MsgLike(msg_like)
                    if matches!(msg_like.kind, MsgLikeKind::Message(_) | MsgLikeKind::Sticker(_) | MsgLikeKind::Redacted)
                        && !(is_main_timeline && msg_like.thread_root.is_some())
                );
                if is_message { return (Some(index), first_failed) }
            }
            _ => { }
        }
    }
    (None, first_failed)
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
    timeline_kind: &TimelineKind,
    event_tl_item: &EventTimelineItem,
    msg_like_content: &MsgLikeContent,
    prev_event: Option<&Arc<TimelineItem>>,
    media_cache: &mut MediaCache,
    link_preview_cache: &mut LinkPreviewCache,
    fetched_thread_summaries: &HashMap<OwnedEventId, FetchedThreadSummary>,
    pending_thread_summary_fetches: &mut HashSet<OwnedEventId>,
    user_power_levels: &UserPowerLevels,
    pinned_events: &[OwnedEventId],
    pending_downloads: &[PendingDownload],
    expanded_reply_previews: &HashSet<TimelineEventItemId>,
    is_newest_sent: bool,
    is_blocked_by_failed_send: bool,
    is_room_encrypted: bool,
    item_drawn_status: ItemDrawnStatus,
    room_screen_widget_uid: WidgetUid,
) -> (WidgetRef, ItemDrawnStatus) {
    let mut new_drawn_status = item_drawn_status;
    let ts_millis = event_tl_item.timestamp();

    let mut is_notice = false; // whether this message is a Notice (automated bot message)
    let mut is_server_notice = false; // whether this message is a Server Notice

    let use_compact_view = uses_compact_view(prev_event, event_tl_item);

    let has_html_body: bool;

    // Sometimes we need to get the username/avatar up-front,
    // so we save that here to avoid calling the function twice.
    let mut set_username_and_get_avatar_retval = None;
    let mut has_room_mention = false;
    let mut download_info: Option<DownloadableAttachment> = None;

    let (item, used_cached_item) = match &msg_like_content.kind {
        MsgLikeKind::Message(msg) => {
            let room_mention_room_id = if msg.mentions().is_some_and(|m| m.room) {
                has_room_mention = true;
                Some(timeline_kind.room_id())
            } else {
                None
            };
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
                        let html_or_plaintext_ref =
                            item.html_or_plaintext(cx, ids!(content.message));
                        let mut link_preview_ref =
                            item.link_preview(cx, ids!(content.link_preview_view));
                        new_drawn_status.content_drawn = populate_text_message_content(
                            cx,
                            &html_or_plaintext_ref,
                            body,
                            formatted.as_ref(),
                            room_mention_room_id,
                            Some(&mut link_preview_ref),
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
                        let html_or_plaintext_ref = item.html_or_plaintext(cx, ids!(content.message));
                        // Apply gray color to all text styles for notice messages.
                        // This covers both rendering paths in HtmlOrPlaintext: the rich
                        // `html_view.html` widget (used when the message has an HTML body)
                        // and the `plaintext_view.pt_label` (used for plain-text notices).
                        let mut html_widget = html_or_plaintext_ref.html(cx, ids!(html_view.html));
                        script_apply_eval!(cx, html_widget, {
                            font_color: mod.widgets.COLOR_MESSAGE_NOTICE_TEXT,
                            draw_block +: {
                                quote_fg_color: mod.widgets.COLOR_MESSAGE_NOTICE_TEXT,
                            }
                        });
                        let mut pt_label = html_or_plaintext_ref.label(cx, ids!(plaintext_view.pt_label));
                        script_apply_eval!(cx, pt_label, {
                            draw_text +: {
                                color: mod.widgets.COLOR_MESSAGE_NOTICE_TEXT
                            }
                        });
                        let mut link_preview_ref =
                            item.link_preview(cx, ids!(content.link_preview_view));
                        new_drawn_status.content_drawn = populate_text_message_content(
                            cx,
                            &html_or_plaintext_ref,
                            body,
                            formatted.as_ref(),
                            room_mention_room_id,
                            Some(&mut link_preview_ref),
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
                        let html_or_plaintext_ref = item.html_or_plaintext(cx, ids!(content.message));
                        // Apply red color to all text styles for server notices.
                        let mut html_widget = html_or_plaintext_ref.html(cx, ids!(html_view.html));
                        script_apply_eval!(cx, html_widget, {
                            font_color: mod.widgets.COLOR_FG_DANGER_RED
                            draw_text +: { color: mod.widgets.COLOR_FG_DANGER_RED }
                            draw_block +: {
                                line_color: mod.widgets.COLOR_FG_DANGER_RED
                                quote_fg_color: mod.widgets.COLOR_FG_DANGER_RED
                            }
                        });
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
                        let mut link_preview_ref =
                            item.link_preview(cx, ids!(content.link_preview_view));
                        new_drawn_status.content_drawn = populate_text_message_content(
                            cx,
                            &html_or_plaintext_ref,
                            &sn.body,
                            Some(&FormattedBody {
                                format: MessageFormat::Html,
                                body: formatted,
                            }),
                            room_mention_room_id,
                            Some(&mut link_preview_ref),
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
                        let (username, profile_drawn) = item.avatar(cx, ids!(header.avatar)).set_avatar_and_get_username(
                            cx,
                            timeline_kind,
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
                                    body: format!("* {} {}", username, fb.body),
                                })
                            )
                        } else {
                            (Cow::from(format!("* {} {}", username, body)), None)
                        };
                        let html_or_plaintext_ref =
                            item.html_or_plaintext(cx, ids!(content.message));
                        let mut link_preview_ref =
                            item.link_preview(cx, ids!(content.link_preview_view));
                        let link_previews_drawn = populate_text_message_content(
                            cx,
                            &html_or_plaintext_ref,
                            &body,
                            formatted.as_ref(),
                            room_mention_room_id,
                            Some(&mut link_preview_ref),
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
                    let was_cached = existed && item_drawn_status.content_drawn;
                    let text_or_image_ref = item.text_or_image(cx, ids!(content.message.image));
                    let fallback = if was_cached {
                        // Cached path re-reads the status the widget already has.
                        text_or_image_ref.status().is_text().then(|| DownloadableAttachment {
                            media_source: image.source.clone(),
                            filename: image.filename().to_owned(),
                            size: image.info.as_ref().and_then(|i| i.size).map(u64::from),
                            kind: DownloadKind::Image,
                        })
                    } else {
                        let (is_image_fully_drawn, fallback) = populate_image_message_content_with_fallback(
                            cx,
                            &text_or_image_ref,
                            image.info.as_deref(),
                            image.source.clone(),
                            msg.body(),
                            media_cache,
                            image.filename(),
                            image.info.as_ref().and_then(|i| i.size).map(u64::from),
                            DownloadKind::Image,
                        );
                        new_drawn_status.content_drawn = is_image_fully_drawn;
                        populate_media_caption(cx, &item, image.formatted_caption(), image.caption());
                        fallback
                    };
                    download_info = fallback;
                    (item, was_cached)
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
                        let html_or_plaintext_ref =
                            item.html_or_plaintext(cx, ids!(content.message));
                        item.link_preview(cx, ids!(content.link_preview_view)).clear(cx);
                        let is_location_fully_drawn = populate_location_message_content(
                            cx,
                            &html_or_plaintext_ref,
                            location,
                        );
                        new_drawn_status.content_drawn = is_location_fully_drawn;
                        (item, false)
                    }
                }
                MessageType::File(file_content) => {
                    has_html_body = file_content.formatted.as_ref().is_some_and(|f| f.format == MessageFormat::Html);
                    download_info = Some(DownloadableAttachment {
                        media_source: file_content.source.clone(),
                        filename: file_content.filename().to_owned(),
                        size: file_content.info.as_ref().and_then(|i| i.size).map(u64::from),
                        kind: DownloadKind::File,
                    });
                    let template = if use_compact_view {
                        id!(CondensedMessage)
                    } else {
                        id!(Message)
                    };
                    let (item, existed) = list.item_with_existed(cx, item_id, template);
                    if existed && item_drawn_status.content_drawn {
                        (item, true)
                    } else {
                        let html_or_plaintext_ref =
                            item.html_or_plaintext(cx, ids!(content.message));
                        item.link_preview(cx, ids!(content.link_preview_view)).clear(cx);
                        new_drawn_status.content_drawn = populate_file_message_content(
                            cx,
                            &html_or_plaintext_ref,
                            file_content,
                        );
                        (item, false)
                    }
                }
                MessageType::Audio(audio) => {
                    has_html_body = audio.formatted.as_ref().is_some_and(|f| f.format == MessageFormat::Html);
                    download_info = Some(DownloadableAttachment {
                        media_source: audio.source.clone(),
                        filename: audio.filename().to_owned(),
                        size: audio.info.as_ref().and_then(|i| i.size).map(u64::from),
                        kind: DownloadKind::Audio,
                    });
                    let template = if use_compact_view {
                        id!(CondensedMessage)
                    } else {
                        id!(Message)
                    };
                    let (item, existed) = list.item_with_existed(cx, item_id, template);
                    if existed && item_drawn_status.content_drawn {
                        (item, true)
                    } else {
                        let html_or_plaintext_ref =
                            item.html_or_plaintext(cx, ids!(content.message));
                        item.link_preview(cx, ids!(content.link_preview_view)).clear(cx);
                        new_drawn_status.content_drawn = populate_audio_message_content(
                            cx,
                            &html_or_plaintext_ref,
                            audio,
                        );
                        (item, false)
                    }
                }
                MessageType::Video(video) => {
                    has_html_body = video.formatted.as_ref().is_some_and(|f| f.format == MessageFormat::Html);
                    download_info = Some(DownloadableAttachment {
                        media_source: video.source.clone(),
                        filename: video.filename().to_owned(),
                        size: video.info.as_ref().and_then(|i| i.size).map(u64::from),
                        kind: DownloadKind::Video,
                    });
                    let template = if use_compact_view {
                        id!(CondensedMessage)
                    } else {
                        id!(Message)
                    };
                    let (item, existed) = list.item_with_existed(cx, item_id, template);
                    if existed && item_drawn_status.content_drawn {
                        (item, true)
                    } else {
                        let html_or_plaintext_ref =
                            item.html_or_plaintext(cx, ids!(content.message));
                        item.link_preview(cx, ids!(content.link_preview_view)).clear(cx);
                        new_drawn_status.content_drawn = populate_video_message_content(
                            cx,
                            &html_or_plaintext_ref,
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
                        let html_or_plaintext_ref =
                            item.html_or_plaintext(cx, ids!(content.message));
                        let mut link_preview_ref =
                            item.link_preview(cx, ids!(content.link_preview_view));

                        new_drawn_status.content_drawn = populate_text_message_content(
                            cx,
                            &html_or_plaintext_ref,
                            &verification.body,
                            Some(&formatted),
                            room_mention_room_id,
                            Some(&mut link_preview_ref),
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
                        item.link_preview(cx, ids!(content.link_preview_view)).clear(cx);
                        item.label(cx, ids!(content.message)).set_text(
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
            let was_cached = existed && item_drawn_status.content_drawn;

            let text_or_image_ref = item.text_or_image(cx, ids!(content.message.image));
            let media_source: MediaSource = source.clone().into();
            let filename = if body.is_empty() { "sticker" } else { body.as_str() };
            let size = info.size.map(u64::from);
            download_info = if was_cached {
                text_or_image_ref.status().is_text().then(|| DownloadableAttachment {
                    media_source,
                    filename: filename.to_owned(),
                    size,
                    kind: DownloadKind::Image,
                })
            } else {
                let (is_image_fully_drawn, fallback) = populate_image_message_content_with_fallback(
                    cx,
                    &text_or_image_ref,
                    Some(info),
                    media_source,
                    body,
                    media_cache,
                    filename,
                    size,
                    DownloadKind::Image,
                );
                new_drawn_status.content_drawn = is_image_fully_drawn;
                populate_media_caption(cx, &item, None, None);
                fallback
            };
            (item, was_cached)
        }
        // Handle messages that have been redacted (deleted).
        MsgLikeKind::Redacted => {
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
                let html_or_plaintext_ref = item.html_or_plaintext(cx, ids!(content.message));
                // Redacted messages have no link preview; clear any stale one from a reused row.
                item.link_preview(cx, ids!(content.link_preview_view)).clear(cx);
                // Apply a smaller font size for redacted messages.
                let mut html_widget = html_or_plaintext_ref.html(cx, ids!(html_view.html));
                script_apply_eval!(cx, html_widget, {
                    font_size: mod.widgets.REDACTED_MESSAGE_FONT_SIZE
                    text_style_normal +: { font_size: mod.widgets.REDACTED_MESSAGE_FONT_SIZE }
                    text_style_italic +: { font_size: mod.widgets.REDACTED_MESSAGE_FONT_SIZE }
                    text_style_bold +: { font_size: mod.widgets.REDACTED_MESSAGE_FONT_SIZE }
                    text_style_bold_italic +: { font_size: mod.widgets.REDACTED_MESSAGE_FONT_SIZE }
                    text_style_fixed +: { font_size: mod.widgets.REDACTED_MESSAGE_FONT_SIZE }
                });
                new_drawn_status.content_drawn = populate_redacted_message_content(
                    cx,
                    &html_or_plaintext_ref,
                    event_tl_item,
                    timeline_kind.room_id(),
                );
                (item, false)
            }
        }
        other => {
            has_html_body = false;
            let (item, existed) = list.item_with_existed(cx, item_id, id!(Message));
            if existed && item_drawn_status.content_drawn {
                (item, true)
            } else {
                item.link_preview(cx, ids!(content.link_preview_view)).clear(cx);
                item.label(cx, ids!(content.message)).set_text(
                    cx,
                    &format!("[Unsupported {:?}] ", other),
                );
                new_drawn_status.content_drawn = true;
                (item, false)
            }
        }
    };

    let timeline_event_id = event_tl_item.identifier();

    // If we didn't use a cached item, we need to draw all other message content:
    // the reactions, the read receipts avatar row, the reply preview.
    if !used_cached_item {
        // Redacted messages must never show reactions, even if the SDK still reports some.
        let reactions = (!matches!(msg_like_content.kind, MsgLikeKind::Redacted))
            .then(|| event_tl_item.reactions());
        item.reaction_list(cx, ids!(content.reaction_list)).set_list(
            cx,
            reactions,
            timeline_kind,
            &timeline_event_id,
            item_id,
        );
        populate_read_receipts(&item, cx, timeline_kind, event_tl_item);
        let is_reply_fully_drawn = draw_replied_to_message(
            cx,
            &item.widget(cx, ids!(replied_to_message)),
            timeline_kind,
            msg_like_content.in_reply_to.as_ref(),
            event_tl_item.event_id(),
        );
        let is_thread_summary_fully_drawn = populate_thread_root_summary(
            cx,
            &item,
            item_id,
            timeline_kind,
            msg_like_content,
            event_tl_item,
            fetched_thread_summaries,
            pending_thread_summary_fetches,
        );

        // The content is only considered to be fully drawn if the logic above marked it as such
        // *and* if the reply preview was also fully drawn
        // *and* if the thread root summary (if applicable) was also fully drawn.
        new_drawn_status.content_drawn &= is_reply_fully_drawn;
        new_drawn_status.content_drawn &= is_thread_summary_fully_drawn;
    }


    // Re-set even for cached items: the portal list recycles widgets, so
    // the same template might now be showing a totally different message.
    let message_details = MessageDetails {
        thread_root_event_id: msg_like_content.thread_root.clone().or_else(|| {
            msg_like_content.thread_summary.as_ref()
                .and_then(|_| event_tl_item.event_id().map(|id| id.to_owned()))
        }),
        timeline_event_id,
        item_id,
        related_event_id: msg_like_content.in_reply_to.as_ref().map(|r| r.event_id.clone()),
        room_screen_widget_uid,
        abilities: MessageAbilities::from_user_power_and_event(
            user_power_levels,
            event_tl_item,
            msg_like_content,
            pinned_events,
            has_html_body,
            timeline_kind.thread_root_event_id().is_some(),
        ),
        should_be_highlighted: event_tl_item.is_highlighted() || has_room_mention,
    };
    let download_state = download_info.as_ref()
        .and_then(|info| {
            let mxc = media_source_mxc(&info.media_source);
            pending_downloads.iter()
                .find(|p| &p.mxc == mxc)
                .map(|p| p.state.display(p.kind))
        })
        .unwrap_or_default();
    let is_reply_expanded = expanded_reply_previews.contains(&message_details.timeline_event_id);
    item.as_message().set_data(
        cx,
        message_details,
        event_tl_item,
        download_info,
        download_state,
        is_reply_expanded,
        is_newest_sent,
        is_blocked_by_failed_send,
        is_room_encrypted,
    );


    // If `used_cached_item` is false, we should always redraw the profile, even if profile_drawn is true.
    let skip_draw_profile =
        use_compact_view || (used_cached_item && item_drawn_status.profile_drawn);
    if skip_draw_profile {
        // log!("\t --> populate_message_view(): SKIPPING profile draw for item_id: {item_id}");
        new_drawn_status.profile_drawn = true;
    } else {
        // log!("\t --> populate_message_view(): DRAWING  profile draw for item_id: {item_id}");
        let mut username_label = item.label(cx, ids!(header.username));

        if !is_server_notice { // the normal case
            let (username, profile_drawn) = set_username_and_get_avatar_retval.unwrap_or_else(||
                item.avatar(cx, ids!(header.avatar)).set_avatar_and_get_username(
                    cx,
                    timeline_kind,
                    event_tl_item.sender(),
                    Some(event_tl_item.sender_profile()),
                    event_tl_item.event_id(),
                    true,
                )
            );
            if is_notice {
                script_apply_eval!(cx, username_label, {
                    draw_text +: {
                        color: mod.widgets.COLOR_MESSAGE_NOTICE_TEXT
                    }
                });
            }
            username_label.set_text(cx, &username);
            new_drawn_status.profile_drawn = profile_drawn;
        }
        else {
            // Server notices are drawn with a red color avatar background and username.
            let avatar = item.avatar(cx, ids!(header.avatar));
            avatar.show_text(cx, Some(COLOR_FG_DANGER_RED), None, "⚠");
            username_label.set_text(cx, "Server notice");
            script_apply_eval!(cx, username_label, {
                draw_text +: {
                    color: (mod.widgets.COLOR_FG_DANGER_RED)
                }
            });
            new_drawn_status.profile_drawn = true;
        }
    }

    // If we've previously drawn the item content, skip all other steps.
    if used_cached_item && item_drawn_status.content_drawn && item_drawn_status.profile_drawn {
        return (item, new_drawn_status);
    }

    // Set the timestamp.
    if let Some(dt) = unix_time_millis_to_datetime(ts_millis) {
        item.timestamp(cx, ids!(profile.timestamp)).set_date_time(cx, dt);
    }

    // Set the "edited" indicator if this message was edited, otherwise hide it
    // (this widget may be reused for a non-edited message at the same row).
    let edited_indicator = item.edited_indicator(cx, ids!(profile.edited_indicator));
    if msg_like_content.as_message().is_some_and(|m| m.is_edited()) {
        edited_indicator.set_latest_edit(cx, event_tl_item);
    } else {
        edited_indicator.hide(cx);
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
            item.tsp_sign_indicator(cx, ids!(profile.tsp_sign_indicator))
                .show_with_state(cx, tsp_sign_state);
        } else {
            // Hide the TSP indicator (in case we reused the message widget at this item index).
            item.tsp_sign_indicator(cx, ids!(profile.tsp_sign_indicator)).hide(cx);
        }
    }

    (item, new_drawn_status)
}

/// Draws the Html or plaintext body of the given Text or Notice message into the `message_content_widget`.
/// Also populates link previews if a link_preview_ref is provided.
///
/// Returns whether the text items were fully drawn.
fn populate_text_message_content(
    cx: &mut Cx,
    message_content_widget: &HtmlOrPlaintextRef,
    body: &str,
    formatted_body: Option<&FormattedBody>,
    room_mention_room_id: Option<&OwnedRoomId>,
    link_preview_ref: Option<&mut LinkPreviewRef>,
    media_cache: Option<&mut MediaCache>,
    link_preview_cache: Option<&mut LinkPreviewCache>,
) -> bool {
    /// If this is a room mention, replace `@room` text in `html` with a pill
    /// link to the room so it renders as a red room pill with the room's avatar.
    fn apply_room_mention<'a>(html: Cow<'a, str>, room_id: Option<&OwnedRoomId>) -> Cow<'a, str> {
        if let Some(room_id) = room_id {
            // Only replace @room if it's NOT already inside an <a> tag
            // (some clients pre-link @room in the formatted_body).
            if html.contains("@room") && !html.contains("\">@room</a>") {
                return Cow::Owned(html.replace(
                    "@room",
                    &format!("<a href=\"https://matrix.to/#/{room_id}\">@room</a>"),
                ));
            }
        }
        html
    }

    // The message was HTML-formatted rich text.
    let mut links = Vec::new();
    if let Some(fb) = formatted_body.as_ref()
        .and_then(|fb| (fb.format == MessageFormat::Html).then_some(fb))
    {
        let linkified_html = utils::linkify_get_urls(
            utils::trim_start_html_line_breaks(&fb.body),
            true,
            Some(&mut links),
        );
        let html = apply_room_mention(linkified_html, room_mention_room_id);
        message_content_widget.show_html(cx, html);
    }
    // The message was non-HTML plaintext.
    else {
        let linkified_html = utils::linkify_get_urls(body, false, Some(&mut links));
        let html = apply_room_mention(linkified_html, room_mention_room_id);
        match html {
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


/// Populates the caption (and makes its view visible) for the given message `item`.
///
/// Prefers the formatted caption (HTML), with an optional plaintext caption as fallback.
fn populate_media_caption(
    cx: &mut Cx,
    item: &WidgetRef,
    formatted_caption: Option<&FormattedBody>,
    backup_caption: Option<&str>,
) {
    let caption_view = item.view(cx, ids!(content.message.caption_view));
    let caption_ref = item.html_or_plaintext(cx, ids!(content.message.caption_view.caption));
    let should_show = if let Some(fb) = formatted_caption
        .filter(|fb| fb.format == MessageFormat::Html && !fb.body.trim().is_empty())
    {
        caption_ref.show_html(cx, &fb.body);
        true
    } else if let Some(text) = backup_caption.filter(|c| !c.trim().is_empty()) {
        caption_ref.show_plaintext(cx, text);
        true
    } else {
        false
    };
    caption_view.set_visible(cx, should_show);
}

/// Like `populate_image_message_content`, but also returns metadata
/// about how to download the image if we were unable to show a preview of it.
fn populate_image_message_content_with_fallback(
    cx: &mut Cx,
    text_or_image_ref: &TextOrImageRef,
    image_info_source: Option<&ImageInfo>,
    original_source: MediaSource,
    body: &str,
    media_cache: &mut MediaCache,
    filename: &str,
    size: Option<u64>,
    kind: DownloadKind,
) -> (bool, Option<DownloadableAttachment>) {
    let fully_drawn = populate_image_message_content(
        cx,
        text_or_image_ref,
        image_info_source,
        original_source.clone(),
        body,
        media_cache,
    );
    let fallback = text_or_image_ref.status().is_text().then(|| DownloadableAttachment {
        media_source: original_source,
        filename: filename.to_owned(),
        size,
        kind,
    });
    (fully_drawn, fallback)
}

/// Draws an image into the given `text_or_image_ref`.
///
/// Returns whether it was fully drawn (meaning its content was fully loaded/available).
fn populate_image_message_content(
    cx: &mut Cx,
    text_or_image_ref: &TextOrImageRef,
    image_info_source: Option<&ImageInfo>,
    original_source: MediaSource,
    body: &str,
    media_cache: &mut MediaCache,
) -> bool {
    let (mimetype, _width, _height) = image_info_source
        .map(|info| (info.mimetype.as_deref(), info.width, info.height))
        .unwrap_or_default();

    // If the mimetype is known but isn't an image format makepad can decode,
    // show a message that it's unsupported.
    if let Some(mime) = mimetype.as_ref() {
        if !utils::is_supported_image_mimetype(mime) {
            text_or_image_ref.show_text(
                cx,
                format!("{}{}Unsupported type {:?}",
                    body,
                    if body.trim().is_empty() { "" } else { "\n" },
                    mime,
                ),
            );
            return true; // consider this as fully drawn
        }
    }

    let Some(image_info) = image_info_source else {
        text_or_image_ref.show_text(cx, format!("{body}\n\nImage message had no source URL."));
        return true;
    };

    // A still thumbnail only shows an animated image's first frame, which is often blank.
    // Encrypted media can't be thumbnailed, so asking for one downloads the whole original.
    let mut should_animate = image_info.is_animated.unwrap_or_else(||
        mimetype.is_some_and(|mime| matches!(mime, "image/gif" | "image/webp" | "image/apng"))
    );
    let is_encrypted = matches!(original_source, MediaSource::Encrypted(_));
    // Use the provided thumbnail URI if it exists; otherwise use the original URI.
    let get_still_thumbnail_source = || image_info.thumbnail_source.clone().unwrap_or_else(|| original_source.clone());
    let (mut media_source, requested_format) = if should_animate {
        (original_source.clone(), ANIMATED_MEDIA_THUMBNAIL_FORMAT.into())
    } else {
        (get_still_thumbnail_source(), MEDIA_THUMBNAIL_FORMAT.into())
    };
    let mut media_entry = media_cache.try_get_media_or_fetch(&media_source, requested_format);
    // If the original image can't be found, try its thumbnail, which may have been uploaded separately.
    if should_animate && matches!(
        media_entry,
        (MediaCacheEntry::Failed(StatusCode::NOT_FOUND), MediaFormat::Thumbnail(_))
    ) {
        should_animate = false;
        media_source = get_still_thumbnail_source();
        media_entry = media_cache.try_get_media_or_fetch(&media_source, MEDIA_THUMBNAIL_FORMAT.into());
    }
    // The server can't thumbnail this image (the spec's errors for that), so show the original instead.
    if matches!(
        media_entry,
        (MediaCacheEntry::Failed(StatusCode::BAD_REQUEST | StatusCode::PAYLOAD_TOO_LARGE | StatusCode::BAD_GATEWAY), MediaFormat::Thumbnail(_))
    ) {
        media_entry = media_cache.try_get_media_or_fetch(&media_source, MediaFormat::File);
    }

    // The image keeps the original source rather than the thumbnail's,
    // so that clicking on it opens (or downloads) the original image.
    // A placeholder texture stays visible until the new image is decoded.
    let show_loaded_image = |cx: &mut Cx, media_format: MediaFormat, data: Arc<[u8]>, placeholder: Option<Texture>| {
        let cache_key = get_image_cache_key(media_source_mxc(&media_source), &media_format);
        let show_image_result = text_or_image_ref.show_image(cx, Some(original_source.clone()), |cx, img| {
            if placeholder.is_some() {
                img.set_texture(cx, placeholder);
            }
            utils::load_image_with_cache_key(&img, cx, &cache_key, data)
                .map(|()| img.size_in_pixels(cx).unwrap_or_default())
        });
        if let Err(e) = show_image_result {
            let err_str = format!("{body}\n\nFailed to display image: {e:?}");
            error!("{err_str}");
            text_or_image_ref.show_text(cx, &err_str);
        }
    };

    match media_entry {
        // The server sent a non-animated thumbnail, so we show that while fetching
        // the original image that *can* be animated.
        (MediaCacheEntry::Loaded(data), MediaFormat::Thumbnail(settings))
            if should_animate && !is_encrypted && !is_animated_image(&data) =>
        {
            match media_cache.try_get_media_or_fetch(&media_source, MediaFormat::File) {
                (MediaCacheEntry::Loaded(full_data), MediaFormat::File) => {
                    // Keep showing the thumbnail until the original is full decoded.
                    let still = text_or_image_ref.is_showing_image_from(&original_source)
                        .then(|| text_or_image_ref.get_texture(cx))
                        .flatten();
                    show_loaded_image(cx, MediaFormat::File, full_data, still);
                    true
                }
                (MediaCacheEntry::Failed(_), _) => {
                    show_loaded_image(cx, MediaFormat::Thumbnail(settings), data, None);
                    true
                }
                _ => {
                    show_loaded_image(cx, MediaFormat::Thumbnail(settings), data, None);
                    false
                }
            }
        }
        (MediaCacheEntry::Loaded(data), media_format) => {
            show_loaded_image(cx, media_format, data, None);
            // We're done drawing the image, so mark it as fully drawn.
            true
        }
        (MediaCacheEntry::Requested, _media_format) => {
            // If the image is being fetched, we try to show its blurhash.
            // Only decode the image once, not on every draw while we're wait.
            if !text_or_image_ref.is_showing_image_from(&original_source)
                && let (Some(blurhash), Some(width), Some(height)) = (image_info.blurhash.as_deref(), image_info.width, image_info.height)
            {
                let show_image_result = text_or_image_ref.show_image(cx, Some(original_source.clone()), |cx, img| {
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
            false
        }
        (MediaCacheEntry::Failed(_status_code), _media_format) => {
            text_or_image_ref.show_text(
                cx,
                format!("{body}\n\nFailed to fetch image from {:?}", media_source_mxc(&media_source)),
            );
            // For now, we consider this as being "complete". In the future, we could support
            // retrying to fetch thumbnail of the image on a user click/tap.
            true
        }
    }
}


/// Draws a file message's content into the given `message_content_widget`.
///
/// Returns whether the file message content was fully drawn.
fn populate_file_message_content(
    cx: &mut Cx,
    message_content_widget: &HtmlOrPlaintextRef,
    file_content: &FileMessageEventContent,
) -> bool {
    let filename = htmlize::escape_text(file_content.filename());
    let size = file_content
        .info
        .as_ref()
        .and_then(|info| info.size)
        .map(|bytes| format!("  ({})", utils::format_decimal_file_size(bytes.into())))
        .unwrap_or_default();
    let caption = file_content.formatted_caption()
        .filter(|fb| fb.format == MessageFormat::Html)
        .map(|fb| format!("{}<br>", fb.body))
        .or_else(|| file_content.caption().map(|c| format!("{}<br>", htmlize::escape_text(c))))
        .unwrap_or_default();

    message_content_widget.show_html(
        cx,
        format!("<b>File: </b>{caption}{filename}{size}"),
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
    let filename = htmlize::escape_text(audio.filename());
    let (duration, mime, size) = audio
        .info
        .as_ref()
        .map(|info| (
            info.duration
                .map(|d| format!(",  {:.2} sec", d.as_secs_f64()))
                .unwrap_or_default(),
            info.mimetype
                .as_ref()
                .map(|m| format!("  {},", htmlize::escape_text(m)))
                .unwrap_or_default(),
            info.size
                .map(|bytes| format!("  ({})", utils::format_decimal_file_size(bytes.into())))
                .unwrap_or_default(),
        ))
        .unwrap_or_default();
    let caption = audio.formatted_caption()
        .filter(|fb| fb.format == MessageFormat::Html)
        .map(|fb| format!("{}<br>", fb.body))
        .or_else(|| audio.caption().map(|c| format!("{}<br>", htmlize::escape_text(c))))
        .unwrap_or_default();

    // TODO: add an audio to play the audio file

    message_content_widget.show_html(
        cx,
        format!("<b>Audio: </b>{caption}File: <i>{filename}</i>{size}{mime}{duration}<br> → <i>Video playback not yet supported.</i>"),
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
    let filename = htmlize::escape_text(video.filename());
    let (duration, mime, size, dimensions) = video
        .info
        .as_ref()
        .map(|info| (
            info.duration
                .map(|d| format!(",  {:.2} sec", d.as_secs_f64()))
                .unwrap_or_default(),
            info.mimetype
                .as_ref()
                .map(|m| format!(",  {}", htmlize::escape_text(m)))
                .unwrap_or_default(),
            info.size
                .map(|bytes| format!("  ({})", utils::format_decimal_file_size(bytes.into())))
                .unwrap_or_default(),
            info.width.and_then(|width|
                info.height.map(|height| format!(",  {width}x{height}"))
            ).unwrap_or_default(),
        ))
        .unwrap_or_default();
    let caption = video.formatted_caption()
        .filter(|fb| fb.format == MessageFormat::Html)
        .map(|fb| format!("{}<br>", fb.body))
        .or_else(|| video.caption().map(|c| format!("{}<br>", htmlize::escape_text(c))))
        .unwrap_or_default();

    // TODO: populate a video widget here, once makepad supports that

    message_content_widget.show_html(
        cx,
        format!("<b>Video: </b>{caption}File: <i>{filename}</i>{size}{mime}{duration}{dimensions}<br> → <i>Video playback not yet supported.</i>"),
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
    if let Some((lat, long)) = location.geo_uri.strip_prefix(utils::GEO_URI_SCHEME).and_then(|s| s.split_once(',')) {
        let short_lat = lat.find('.').and_then(|dot| lat.get(..dot + 7)).unwrap_or(lat);
        let short_long = long.find('.').and_then(|dot| long.get(..dot + 7)).unwrap_or(long);
        let safe_lat = htmlize::escape_attribute(lat);
        let safe_long = htmlize::escape_attribute(long);
        let safe_geo_uri = htmlize::escape_attribute(&location.geo_uri);
        let safe_short_lat = htmlize::escape_text(short_lat);
        let safe_short_long = htmlize::escape_text(short_long);
        let html_body = format!(
            "Location: <a href=\"{}\">{safe_short_lat},{safe_short_long}</a><br>\
            <ul>\
            <li><a href=\"https://www.openstreetmap.org/?mlat={safe_lat}&amp;mlon={safe_long}#map=15/{safe_lat}/{safe_long}\">Open in OpenStreetMap</a></li>\
            <li><a href=\"https://www.google.com/maps/search/?api=1&amp;query={safe_lat},{safe_long}\">Open in Google Maps</a></li>\
            <li><a href=\"https://maps.apple.com/?ll={safe_lat},{safe_long}&amp;q={safe_lat},{safe_long}\">Open in Apple Maps</a></li>\
            </ul>",
            safe_geo_uri,
        );
        message_content_widget.show_html(cx, html_body);
    } else {
        message_content_widget.show_html(
            cx,
            format!("<i>[Location invalid]</i> {}", htmlize::escape_text(&location.body))
        );
    }

    // Currently we do not fetch location thumbnail previews, so we consider this as fully drawn.
    // In the future, when we do support this, we'll return false until the thumbnail is fetched,
    // at which point we can return true.
    true
}


/// Draws the given redacted message's content into the `message_content_widget`.
///
/// Returns whether the redacted message content was fully drawn.
fn populate_redacted_message_content(
    cx: &mut Cx,
    message_content_widget: &HtmlOrPlaintextRef,
    event_tl_item: &EventTimelineItem,
    room_id: &OwnedRoomId,
) -> bool {
    let fully_drawn: bool;
    let mut redactor_id_and_reason = None;
    if let Some(redacted_msg) = event_tl_item.latest_json() {
        if let Ok(AnySyncTimelineEvent::MessageLike(
            AnySyncMessageLikeEvent::RoomMessage(
                SyncMessageLikeEvent::Redacted(redaction)
            )
        )) = redacted_msg.deserialize() {
            if let Ok(redacted_because) = redaction.unsigned.redacted_because.deserialize() {
                let reason = match &redacted_because {
                    AnyRedactionEvent::RoomRedaction(e) => e.content.reason.clone(),
                    _ => None,
                };
                redactor_id_and_reason = Some((
                    redacted_because.sender().to_owned(),
                    reason,
                ));
            }
        }
    }

    let html = if let Some((redactor, reason)) = redactor_id_and_reason {
        if redactor == event_tl_item.sender() {
            fully_drawn = true;
            match reason {
                Some(r) => format!("⛔ <i>Deleted their own message. Reason: \"{}\".</i>", htmlize::escape_text(r)),
                None => String::from("⛔ <i>Deleted their own message.</i>"),
            }
        } else {
            // Try to get the displayable name of the user who redacted this message.
            let redactor_name = user_profile_cache::get_user_display_name_for_room(
                cx,
                redactor.clone(),
                Some(room_id),
                true,
            );
            fully_drawn = redactor_name.was_found();
            let redactor_name_esc = htmlize::escape_text(redactor_name.as_deref().unwrap_or(redactor.as_str()));
            match reason {
                Some(r) => format!("⛔ <i>{} deleted this message. Reason: \"{}\".</i>",
                    redactor_name_esc,
                    htmlize::escape_text(r),
                ),
                None => format!("⛔ <i>{} deleted this message.</i>", redactor_name_esc),
            }
        }
    } else {
        fully_drawn = true;
        String::from("⛔ <i>Message deleted.</i>")
    };
    message_content_widget.show_html(cx, html);
    fully_drawn
}


/// Draws a ReplyPreview above a message if it was in-reply to another message.
///
/// ## Arguments
/// * `replied_to_message_view`: the destination `RepliedToMessage` view that will be populated.
/// * `timeline_kind`: the [`TimelineKind`] of the timeline that is being drawn.
/// * `in_reply_to`: if `Some`, the details that will be used to populate the `replied_to_message_view`.
///   If `None`, this function will mark it as non-visible and consider it fully drawn.
/// * `message_event_id`: the [`EventId`] of the message that is the reply itself (the response).
///   This is needed to fetch the details of the replied-to message (if not yet available).
///
/// Returns whether the in-reply-to information was available and fully drawn,
/// i.e., whether it can be considered cached and not needing to be redrawn later.
fn draw_replied_to_message(
    cx: &mut Cx2d,
    replied_to_message_view: &WidgetRef,
    timeline_kind: &TimelineKind,
    in_reply_to: Option<&InReplyToDetails>,
    message_event_id: Option<&EventId>,
) -> bool {
    let fully_drawn: bool;
    let show_reply: bool;

    if let Some(in_reply_to_details) = in_reply_to {
        show_reply = true;
        match &in_reply_to_details.event {
            TimelineDetails::Ready(replied_to_event) => {
                let (in_reply_to_username, is_avatar_fully_drawn) =
                    replied_to_message_view
                        .avatar(cx, ids!(preview_content.reply_preview_avatar))
                        .set_avatar_and_get_username(
                            cx,
                            timeline_kind,
                            &replied_to_event.sender,
                            Some(&replied_to_event.sender_profile),
                            Some(in_reply_to_details.event_id.as_ref()),
                            true,
                        );

                fully_drawn = is_avatar_fully_drawn;

                replied_to_message_view
                    .label(cx, ids!(preview_content.reply_preview_username))
                    .set_text(cx, in_reply_to_username.as_str());
                let msg_body = replied_to_message_view.html_or_plaintext(cx, ids!(reply_preview_body));
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
                    .label(cx, ids!(preview_content.reply_preview_username))
                    .set_text(cx, "[Error fetching username]");
                replied_to_message_view
                    .avatar(cx, ids!(preview_content.reply_preview_avatar))
                    .show_text(cx, None, None, "?");
                replied_to_message_view
                    .html_or_plaintext(cx, ids!(preview_content.reply_preview_body))
                    .show_plaintext(cx, "[Error fetching replied-to event]");
            }
            td @ TimelineDetails::Pending | td @ TimelineDetails::Unavailable => {
                // We don't have the replied-to message yet, so we can't fully draw the preview.
                fully_drawn = false;
                replied_to_message_view
                    .label(cx, ids!(preview_content.reply_preview_username))
                    .set_text(cx, "[Loading username...]");
                replied_to_message_view
                    .avatar(cx, ids!(preview_content.reply_preview_avatar))
                    .show_text(cx, None, None, "?");
                replied_to_message_view
                    .html_or_plaintext(cx, ids!(preview_content.reply_preview_body))
                    .show_plaintext(cx, "[Loading replied-to message...]");

                // Confusingly, we need to fetch the details of the `message` (the event that is the reply),
                // not the details of the original event that this `message` is replying to.
                if matches!(td, TimelineDetails::Unavailable) {
                    if let Some(event_id) = message_event_id {
                        submit_async_request(MatrixRequest::FetchDetailsForEvent {
                            timeline_kind: timeline_kind.clone(),
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
    // After we changed a reply preview's content, we need to clear its cached view and measured height.
    replied_to_message_view.view(cx, ids!(preview_content)).redraw_texture_cache();
    replied_to_message_view.as_collapsible_preview().reset_measured_height();
    fully_drawn
}

/// Draws a one-line thread summary at the bottom of a message if it is the root of a thread.
///
/// Returns whether the thread summary information was available and fully drawn,
/// i.e., whether it can be considered cached and not needing to be redrawn later.
fn populate_thread_root_summary(
    cx: &mut Cx2d,
    item: &WidgetRef,
    timeline_item_index: usize,
    timeline_kind: &TimelineKind,
    msg_like_content: &MsgLikeContent,
    event_tl_item: &EventTimelineItem,
    fetched_thread_summaries: &HashMap<OwnedEventId, FetchedThreadSummary>,
    pending_thread_summary_fetches: &mut HashSet<OwnedEventId>,
) -> bool {
    let thread_summary_view = item.view(cx, ids!(thread_root_summary));
    thread_summary_view.set_visible(cx, false); // hide by default
    let fully_drawn: bool;

    if matches!(timeline_kind, TimelineKind::Thread { .. }) {
        // If we're already drawing a message in a thread-focused timeline,
        // it doesn't make sense to show a redundant thread summary.
        fully_drawn = true;
        return fully_drawn;
    }

    let Some(thread_summary) = msg_like_content.thread_summary.as_ref() else {
        // consider this as fully drawn since there's no thread summary to show.
        fully_drawn = true;
        return fully_drawn;
    };

    let sdk_num_replies = thread_summary.num_replies;
    // The SDK only counts the replies it has seen itself, and the count the server sent with the root can be stale.
    let bundled_num_replies = event_tl_item.original_json()
        .and_then(extract_bundled_thread)
        .map_or(0, |bundled| u32::try_from(bundled.count).unwrap_or(u32::MAX));
    // Only synced data decides whether to show this, since a fetch must not change the item's height.
    if sdk_num_replies == 0 && bundled_num_replies == 0 {
        fully_drawn = true;
        return fully_drawn;
    }

    // Here, we actually need to show the thread summary.
    thread_summary_view.set_visible(cx, true);
    let thread_root_event_id = event_tl_item.event_id().map(|id| id.to_owned());
    // A fetched summary goes stale once the SDK switches between showing the server's count and its own.
    let fetched_summary = thread_root_event_id
        .as_ref()
        .and_then(|root_id| fetched_thread_summaries.get(root_id))
        .filter(|fetched| (fetched.sdk_num_replies_at_fetch == bundled_num_replies) == (sdk_num_replies == bundled_num_replies));
    // Replies the SDK added or removed since the fetch change the fetched count by as much.
    let replies_count = match fetched_summary {
        // A fetched 0 means the root had no thread summary, which only proves it has no replies
        // if the server bundles them at all, as the synced root shows.
        Some(FetchedThreadSummary { num_replies: Some(fetched_num_replies), sdk_num_replies_at_fetch, .. })
            if *fetched_num_replies > 0 || bundled_num_replies > 0 =>
            (fetched_num_replies + sdk_num_replies).saturating_sub(*sdk_num_replies_at_fetch),
        _ => sdk_num_replies.max(bundled_num_replies),
    };

    // Fetch the real count and latest reply if we can't tell them locally.
    if fetched_summary.is_none()
        && (thread_summary.latest_event.is_unavailable() || sdk_num_replies != bundled_num_replies)
        && let Some(thread_root_event_id) = thread_root_event_id
        && pending_thread_summary_fetches.insert(thread_root_event_id.clone())
    {
        submit_async_request(MatrixRequest::FetchThreadSummaryDetails {
            timeline_kind: timeline_kind.clone(),
            thread_root_event_id,
            timeline_item_index,
        });
    }

    // The SDK's latest reply is more current than the fetched one once its count has changed.
    let sdk_latest_is_newer = fetched_summary.is_none_or(|fetched| fetched.sdk_num_replies_at_fetch != sdk_num_replies);
    let fetched_preview = fetched_summary.and_then(|fetched| fetched.latest_reply_preview_text.as_deref());
    let latest_preview: Cow<str> = match (&thread_summary.latest_event, fetched_preview) {
        (TimelineDetails::Ready(embedded_event), _) if sdk_latest_is_newer || fetched_preview.is_none() => {
            fully_drawn = true;
            let sender_name = match &embedded_event.sender_profile {
                TimelineDetails::Ready(profile) => profile.display_name.as_deref().unwrap_or(embedded_event.sender.as_str()),
                _ => embedded_event.sender.as_str(),
            };
            text_preview_of_thread_reply(&embedded_event.sender, sender_name, Some(&embedded_event.content)).into()
        }
        (_, Some(preview)) => {
            fully_drawn = true;
            preview.into()
        }
        _ if replies_count == 0 => {
            fully_drawn = true;
            "".into()
        }
        (TimelineDetails::Error(_), None) => {
            fully_drawn = true; // consider this fully drawn since there's no point retrying.
            "<i>Unable to load latest reply</i>".into()
        }
        _ => {
            fully_drawn = true;
            let preview = match fetched_summary {
                Some(_) => "<i>Unable to load latest reply</i>",
                None => "<i>Loading latest reply...</i>",
            };
            preview.into()
        }
    };

    let replies_count_text = match replies_count {
        1 => Cow::Borrowed("1 reply"),
        n => Cow::Owned(format!("{n} replies"))
    };
    item.label(cx, ids!(thread_summary_count))
        .set_text(cx, &replies_count_text);
    item.html(cx, ids!(thread_summary_latest))
        .set_text(cx, &latest_preview);
    fully_drawn
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
                let _ = populate_text_message_content(cx, widget_out, body, formatted.as_ref(), None, None, None, None);
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
    ShowInviteConfirmationModal(RefCell<Option<ConfirmationModalContent>>),
}

/// The result of inviting a user to a room.
///
#[derive(Debug)]
pub enum InviteResultAction {
    /// The invite was sent successfully.
    ///
    /// This action is posted in response to the [`MatrixRequest::InviteUser`] request.
    Sent {
        room_id: OwnedRoomId,
        user_id: OwnedUserId,
    },
    /// The invite failed to be sent.
    ///
    /// This action is posted in response to the [`MatrixRequest::InviteUser`] request.
    Failed {
        room_id: OwnedRoomId,
        user_id: OwnedUserId,
        error: matrix_sdk::Error,
    },
}


/// A clicked link to a room, or space, or an event within a room.
#[derive(Clone, Debug, PartialEq)]
pub struct RoomLink {
    pub room_or_alias_id: OwnedRoomOrAliasId,
    pub event_id: Option<OwnedEventId>,
    /// The full link, which the AddRoom screen can show and search for.
    pub url: String,
}

/// The screen that should show the room, space, or event that a [`RoomLink`] points to.
#[derive(Debug)]
pub enum RoomLinkDestination {
    /// A joined room's timeline, which also contains the linked event, if there is one.
    Timeline {
        room_name_id: RoomNameId,
        timeline_kind: TimelineKind,
    },
    /// A joined space's lobby.
    Space(RoomNameId),
    /// The invite to a room or space.
    Invite(RoomNameId),
    /// A room or space that the user hasn't joined, so show it in the AddRoom screen.
    NotJoined,
}

/// The result of a [`MatrixRequest::ResolveRoomLink`] request.
///
/// This is NOT a widget action.
#[derive(Debug)]
pub struct RoomLinkResolved {
    pub link: RoomLink,
    /// The resolved link's destination, or an error message if it couldn't be resolved.
    pub result: Result<RoomLinkDestination, String>,
}

/// A request to show the room, space, or event that a clicked link leads to.
///
/// This is NOT a widget action, and is handled by `MainDesktopUI` or the mobile `HomeScreen`.
#[derive(Debug)]
pub enum NavigateToLinkAction {
    /// Show the given room, space, thread, or invite screen.
    Screen(SelectedRoom),
    /// Show the given timeline, and then jump to the given event in it.
    Event {
        room_name_id: RoomNameId,
        timeline_kind: TimelineKind,
        event_id: OwnedEventId,
        /// How to describe this event to the user while searching for it.
        description: String,
    },
}


/// Actions related to a specific message within a room timeline.
#[derive(Clone, Default, Debug)]
pub enum MessageAction {
    /// The user clicked the "react" button on a message
    /// and wants to send the given `reaction` to that message.
    React {
        details: MessageDetails,
        reaction: String,
    },
    /// The user clicked the "reply" button on a message.
    Reply(MessageDetails),
    /// The user clicked the "reply in thread" button on a message, indicating
    /// they want to open (or start) that message's thread and reply within it.
    ReplyInThread(MessageDetails),
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
    /// The user clicked the "Show more" or "Show less" button on a tall reply preview.
    ToggleReplyPreviewExpanded(TimelineEventItemId),
    /// The user clicked the thread summary on a thread-root message.
    OpenThread(OwnedEventId),
    /// The user requested to jump to a specific event in this room.
    JumpToEvent(OwnedEventId),
    /// The user clicked the "delete" button on a message.
    #[doc(alias("delete"))]
    Redact {
        details: MessageDetails,
        reason: Option<String>,
    },
    /// The user clicked the "retry sending" button on a message that failed to send.
    RetrySend(MessageDetails),

    // /// The user clicked the "report" button on a message.
    // Report(MessageDetails),

    /// The user clicked the "Download" button on a media/file message.
    DownloadAttachment(DownloadableAttachment),
    /// The user clicked the "Share" button on a media/file message.
    ShareAttachment(DownloadableAttachment),
    /// User clicked the cancel × next to the in-progress spinner.
    CancelDownload(OwnedMxcUri),
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
    #[default]
    None,
}

impl ActionDefaultRef for MessageAction {
    fn default_ref() -> &'static Self {
        static DEFAULT: MessageAction = MessageAction::None;
        &DEFAULT
    }
}

/// A widget representing a single message of any kind within a room timeline.
#[derive(Script, Widget, Animator)]
pub struct Message {
    #[source] source: ScriptObjectRef,
    #[deref] view: View,
    #[apply_default] animator: Animator,

    #[rust] details: Option<MessageDetails>,
    /// `True` while a context menu that we opened is being shown.
    #[rust] is_context_menu_open: bool,
    /// Set on file/image/audio/video messages so the download button knows
    /// what to save when the user clicks it. `None` for plain text messages,
    /// which hide the download button entirely.
    #[rust] download_info: Option<DownloadableAttachment>,
    /// Cached so `set_data` can reset_hover only on the button that just
    /// transitioned into visibility, not on every redraw.
    #[rust] download_state: DownloadDisplayState,
    /// The UID of the touch that was claimed by this message or one of its children.
    /// This is used to determine if a future long press was on this message.
    #[rust] pressed_touch_uid: Option<u64>,

    // Belowhere: cached references to child widgets, for efficiency.
    #[rust] replied_to_message_view: Option<CollapsiblePreviewRef>,
    #[rust] thread_root_summary_view: Option<ViewRef>,
    #[rust] send_status_indicator: Option<SendStatusIndicatorRef>,
}

impl ScriptHook for Message {
    fn on_after_reload(&mut self, _vm: &mut ScriptVm) {
        // A script reload changes the Message's children; invalidate the ones we cached.
        self.replied_to_message_view = None;
        self.thread_root_summary_view = None;
        self.send_status_indicator = None;
    }
}

impl Widget for Message {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if self.animator_handle_event(cx, event).must_redraw() {
            self.redraw(cx);
        }

        if !self.animator.is_track_animating(id!(highlight))
            && self.animator_in_state(cx, ids!(highlight.on))
        {
            self.animator_play(cx, ids!(highlight.off));
        }

        let Some(d) = self.details.as_ref() else { return };
        let room_screen_widget_uid = d.room_screen_widget_uid;
        let thread_root_event_id = d.thread_root_event_id.clone();

        // determine if any other ancestor widget has already claimed this pointer event.
        let claim_before = event.pointer_claimed_area();

        // A right-click or long-press anywhere on a message widget should show its context menu,
        // so we have to handle the raw events instead of relying on the hit system.
        if let Event::MouseDown(mde) = event
            && mde.button.is_secondary()
            && mde.handled.get().is_empty()
            && self.view.area().clipped_rect(cx).contains(mde.abs)
            && !self.is_within_excluded_child(cx, mde.abs, false)
        {
            mde.handled.set(self.view.area());
            let details = d.clone();
            self.animator_play(cx, ids!(bg_hover.on));
            self.open_context_menu(cx, room_screen_widget_uid, details, mde.abs);
        }
        else if let Event::LongPress(lpe) = event
            // the long press must've been claimed by us or a descendant on touch-down,
            // which also rules out presses owned by an open context menu overlay.
            && self.pressed_touch_uid == Some(lpe.uid)
        {
            let msg_rect = self.view.area().clipped_rect(cx);
            if msg_rect.contains(lpe.abs)
                && !self.is_within_excluded_child(cx, lpe.abs, true)
            {
                let details = d.clone();
                self.animator_play(cx, ids!(bg_hover.on));
                self.open_context_menu(cx, room_screen_widget_uid, details, lpe.abs);
            }
        }

        // We first handle a click on the replied-to message preview, if present,
        // because we don't want any widgets within the replied-to message to be
        // clickable or otherwise interactive.
        let reply = self.replied_to_message_view(cx);
        let reply_content_area = reply.content_area(cx);
        let reply_hit = event.hits(cx, reply_content_area);
        match reply_hit {
            Hit::FingerHoverIn(..) => {
                self.animator_play(cx, ids!(bg_hover.on));
            }
            Hit::FingerDown(_) => {
                self.animator_play(cx, ids!(bg_hover.on));
            }
            Hit::FingerUp(fe) => {
                if fe.is_over && fe.is_primary_hit() && fe.was_tap() {
                    // Tapping on a collapsed reply preview expands it.
                    // Tapping on an expanded reply preview jumps to the replied-to message.
                    let action = if reply.is_collapsed() {
                        MessageAction::ToggleReplyPreviewExpanded(
                            self.details.as_ref().unwrap().timeline_event_id.clone(), // guaranteed to be Some()
                        )
                    } else {
                        MessageAction::JumpToRelated(self.details.clone().unwrap()) // guaranteed to be Some()
                    };
                    cx.widget_action(room_screen_widget_uid, action);
                }
                // since we already captured this finger-up, the hit test on the message body itself
                // won't result in anything, so we have to un-set the hover here.
                if !self.is_context_menu_open {
                    self.animator_toggle(cx, fe.device.has_hovers() && fe.is_over, Animate::Yes, ids!(bg_hover.on), ids!(bg_hover.off));
                }
            }
            _ => { }
        }

        // Handle clicks on the thread summary shown beneath a thread-root message.
        if let Some(thread_root_event_id) = thread_root_event_id.as_ref() {
            let thread_root_summary = self.thread_root_summary_view(cx);
            let apply_hover = |cx: &mut Cx, bg_color: Vec4| {
                let mut thread_root_summary_ref = thread_root_summary.clone();
                script_apply_eval!(cx, thread_root_summary_ref, {
                    draw_bg.color: #(bg_color)
                });
            };
            let summary_hit = event.hits(cx, thread_root_summary.area());
            match summary_hit {
                Hit::FingerDown(_) => {
                    self.animator_play(cx, ids!(bg_hover.on));
                    apply_hover(cx, COLOR_THREAD_SUMMARY_BG_HOVER);
                }
                Hit::FingerHoverIn(_) => {
                    self.animator_play(cx, ids!(bg_hover.on));
                    apply_hover(cx, COLOR_THREAD_SUMMARY_BG_HOVER);
                }
                Hit::FingerHoverOut(_) => {
                    apply_hover(cx, COLOR_THREAD_SUMMARY_BG);
                }
                Hit::FingerMove(fe) if !fe.is_over => {
                    apply_hover(cx, COLOR_THREAD_SUMMARY_BG);
                }
                Hit::FingerLongPress(_) => {
                    apply_hover(cx, COLOR_THREAD_SUMMARY_BG_HOVER);
                }
                Hit::FingerUp(fe) => {
                    let still_hovered = fe.device.has_hovers() && fe.is_over;
                    apply_hover(cx, if still_hovered { COLOR_THREAD_SUMMARY_BG_HOVER } else { COLOR_THREAD_SUMMARY_BG });
                    // Same as the reply preview: this press never reaches the body's
                    // hit test, so settle the message highlight here too.
                    if !self.is_context_menu_open {
                        self.animator_toggle(cx, still_hovered, Animate::Yes, ids!(bg_hover.on), ids!(bg_hover.off));
                    }
                    if fe.is_over && fe.is_primary_hit() && fe.was_tap() {
                        cx.widget_action(
                            room_screen_widget_uid,
                            MessageAction::OpenThread(thread_root_event_id.clone()),
                        );
                    }
                }
                _ => { }
            }
        }

        // Next, we forward the event to the child view such that it has the chance
        // to handle it before the Message widget handles it.
        // This ensures that events like right-clicking/long-pressing a reaction button
        // or a link within a message will be treated as an action upon that child view
        // rather than an action upon the message itself.
        self.view.handle_event(cx, event, scope);

        // Finally, handle any hits on the rest of the message body itself.
        let message_view_area = self.view.area();
        let body_hit = handle_hover_hit(self, cx, event, message_view_area, claim_before, self.is_context_menu_open);
        match body_hit {
            Hit::FingerDown(_) => {
                cx.set_key_focus(message_view_area);
            }
            Hit::FingerHoverIn(..) => {
                // TODO: here, show the "action bar" buttons upon hover-in
            }
            Hit::FingerHoverOut(_fho) => {
                // TODO: here, hide the "action bar" buttons upon hover-out
            }
            _ => { }
        }

        // All of our claim sites have run by now, so a press claimed since our
        // entry snapshot came from within us; remember it for the LongPress guard.
        if let Event::TouchUpdate(tue) = event {
            for touch in &tue.touches {
                match touch.state {
                    TouchState::Start => {
                        self.pressed_touch_uid = (claim_before.is_empty()
                            && !touch.handled.get().is_empty())
                            .then_some(touch.uid);
                    }
                    TouchState::Stop | TouchState::Cancel if self.pressed_touch_uid == Some(touch.uid) => {
                        self.pressed_touch_uid = None;
                    }
                    _ => { }
                }
            }
        }

        // TODO: use regular animator states for the thread root summary hover too.
        if let Event::ClearHover = event && thread_root_event_id.is_some() {
            let mut summary = self.thread_root_summary_view(cx);
            script_apply_eval!(cx, summary, {
                draw_bg.color: #(COLOR_THREAD_SUMMARY_BG)
            });
        }

        if let Event::Actions(actions) = event {
            for action in actions {
                if self.is_context_menu_open && action.downcast_ref::<ContextMenuClosed>().is_some() {
                    self.is_context_menu_open = false;
                    continue;
                }

                match action.as_widget_action().widget_uid_eq(room_screen_widget_uid).cast_ref() {
                    MessageAction::HighlightMessage(id) if id == &self.details.as_ref().unwrap().item_id => { // guaranteed to be Some()
                        // Always start the highlight animation sequence from the beginning.
                        self.animator_cut(cx, ids!(highlight.off)); // stop it first
                        self.animator_play(cx, ids!(highlight.on)); // then start it over
                        self.redraw(cx);
                        continue;
                    }
                    _ => {}
                }
            }

            // Handle clicks on the reply preview's "show more" or "show less" buttons.
            let reply_expand_button = self.button(cx, ids!(replied_to_message.reply_expand_button));
            let reply_collapse_button = self.button(cx, ids!(replied_to_message.reply_collapse_button));
            if reply_expand_button.clicked(actions) || reply_collapse_button.clicked(actions)             {
                cx.widget_action(
                    room_screen_widget_uid,
                    MessageAction::ToggleReplyPreviewExpanded(
                        self.details.as_ref().unwrap().timeline_event_id.clone(), // guaranteed to be Some()
                    ),
                );
                reply_expand_button.reset_hover(cx);
                reply_collapse_button.reset_hover(cx);
            }

            // Handle clicks on the media-related buttons (download, share, cancel) beneath media messages.
            if let Some(info) = self.download_info.as_ref() {
                if self.view.button(cx, ids!(content.download_section.download_button)).clicked(actions) {
                    cx.widget_action(
                        room_screen_widget_uid,
                        MessageAction::DownloadAttachment(info.clone()),
                    );
                }
                if self.view.button(cx, ids!(content.download_section.share_button)).clicked(actions) {
                    cx.widget_action(
                        room_screen_widget_uid,
                        MessageAction::ShareAttachment(info.clone()),
                    );
                }
                if self.view.button(cx, ids!(content.download_section.downloading_view.cancel_button)).clicked(actions) {
                    cx.widget_action(
                        room_screen_widget_uid,
                        MessageAction::CancelDownload(media_source_mxc(&info.media_source).clone()),
                    );
                }
            }

            // Clicking a failed message's status indicator opens its context menu to retry or cancel it.
            let indicator = self.send_status_indicator(cx);
            if let SendStatusIndicatorAction::Clicked { abs_pos } = actions.find_widget_action(indicator.widget_uid()).cast() {
                let details = self.details.as_ref().unwrap(); // guaranteed to be Some()
                if details.abilities.intersects(MessageAbilities::CanRetrySend | MessageAbilities::CanCancelSend) {
                    let details = details.clone();
                    self.animator_play(cx, ids!(bg_hover.on));
                    self.open_context_menu(cx, room_screen_widget_uid, details, abs_pos);
                }
            }
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        if self.animator.is_animating() {
            self.animator.next_frame = cx.new_next_frame();
        }
        if self.details.as_ref().is_some_and(|d| d.should_be_highlighted) {
            script_apply_eval!(cx, self, {
                draw_bg +: {
                    color: #ffffd1,
                    color_hover: #fff9c2,
                    mentions_bar_color: #ffd54f
                }
            });
        }

        self.view.draw_walk(cx, scope, walk)
    }
}

impl Message {
    fn replied_to_message_view(&mut self, cx: &mut Cx) -> CollapsiblePreviewRef {
        if let Some(reply) = &self.replied_to_message_view {
            return reply.clone();
        }
        let reply = self.view.widget(cx, ids!(replied_to_message)).as_collapsible_preview();
        self.replied_to_message_view = Some(reply.clone());
        reply
    }

    /// Whether a click/tap at `abs` is within a child widget (within this message) that has its own hit behavior.
    ///
    /// This currently includes: reactions, download/share buttons, the read receipts row.
    /// On long-presses specifically, it also excludes timestamps, edited indicators, and TSP sign indicators.
    fn is_within_excluded_child(&self, cx: &mut Cx, abs: DVec2, is_long_press: bool) -> bool {
        self.view.widget(cx, ids!(reaction_list)).as_reaction_list().contains_button(cx, abs)
            || self.view.widget(cx, ids!(avatar_row)).area().clipped_rect(cx).contains(abs)
            || self.view.widget(cx, ids!(content.download_section)).area().clipped_rect(cx).contains(abs)
            || (is_long_press && (
                self.view.send_status_indicator(cx, ids!(send_status_indicator)).has_tooltip_at(cx, abs)
                || self.view.widget(cx, ids!(timestamp)).area().clipped_rect(cx).contains(abs)
                || self.view.widget(cx, ids!(edited_indicator)).area().clipped_rect(cx).contains(abs)
                || self.view.widget(cx, ids!(tsp_sign_indicator)).area().clipped_rect(cx).contains(abs)
            ))
    }

    fn open_context_menu(&mut self, cx: &mut Cx, room_screen_widget_uid: WidgetUid, details: MessageDetails, abs_pos: DVec2) {
        self.is_context_menu_open = true;
        cx.widget_action(
            room_screen_widget_uid,
            MessageAction::OpenMessageContextMenu {
                details,
                abs_pos,
            },
        );
    }

    fn thread_root_summary_view(&mut self, cx: &mut Cx) -> ViewRef {
        if let Some(view) = &self.thread_root_summary_view {
            return view.clone();
        }
        let view = self.view(cx, ids!(thread_root_summary));
        self.thread_root_summary_view = Some(view.clone());
        view
    }

    fn send_status_indicator(&mut self, cx: &mut Cx) -> SendStatusIndicatorRef {
        if let Some(indicator) = &self.send_status_indicator {
            return indicator.clone();
        }
        let indicator = self.view.send_status_indicator(cx, ids!(send_status_indicator));
        self.send_status_indicator = Some(indicator.clone());
        indicator
    }

    /// Called every time `populate_message_view` runs, including on cached
    /// items, so all states must be re-set unconditionally.
    fn set_data(
        &mut self,
        cx: &mut Cx,
        details: MessageDetails,
        event_tl_item: &EventTimelineItem,
        download_info: Option<DownloadableAttachment>,
        download_state: DownloadDisplayState,
        is_reply_expanded: bool,
        is_newest_sent: bool,
        is_blocked_by_failed_send: bool,
        is_room_encrypted: bool,
    ) {
        let prev_section_visible = self.download_info.is_some();
        let prev_state = self.download_state;

        // If the message details changed, reset any UI state that belongs to the old message.
        if self.details.as_ref().is_none_or(|d| d.timeline_event_id != details.timeline_event_id) {
            self.is_context_menu_open = false;
            self.pressed_touch_uid = None;
            self.animator_cut(cx, ids!(bg_hover.off));
            self.animator_cut(cx, ids!(highlight.off));
        }

        self.details = Some(details);
        self.download_info = download_info;
        self.send_status_indicator(cx).set_from_event(cx, event_tl_item, is_newest_sent, is_blocked_by_failed_send, is_room_encrypted);

        // Re-apply this every time to ensure a re-used portallist item is still correctly expanded.
        self.view.widget(cx, ids!(replied_to_message)).as_collapsible_preview().set_expanded(is_reply_expanded);

        let section_visible = self.download_info.is_some();
        self.view.view(cx, ids!(content.download_section)).set_visible(cx, section_visible);
        if section_visible {
            let download_button  = self.view.button(cx, ids!(content.download_section.download_button));
            let share_button     = self.view.button(cx, ids!(content.download_section.share_button));
            let downloading_view = self.view.view(cx, ids!(content.download_section.downloading_view));
            let cancel_button    = self.view.button(cx, ids!(content.download_section.downloading_view.cancel_button));
            let success_button   = self.view.button(cx, ids!(content.download_section.success_button));
            let failure_button   = self.view.button(cx, ids!(content.download_section.failure_button));
            let is_idle = matches!(download_state, DownloadDisplayState::Idle);
            download_button.set_visible(cx, is_idle);
            share_button.set_visible(cx, is_idle);
            downloading_view.set_visible(cx, matches!(download_state, DownloadDisplayState::InProgress));
            success_button.set_visible(cx, matches!(download_state, DownloadDisplayState::Succeeded(_)));
            failure_button.set_visible(cx, matches!(download_state, DownloadDisplayState::Failed));
            if let DownloadDisplayState::Succeeded(kind) = download_state {
                success_button.set_text(cx, match kind {
                    TransferKind::Download => "Downloaded",
                    TransferKind::Share => "Shared",
                });
            }
            // Only reset hover for the button(s) just now becoming visible.
            let newly_visible = !prev_section_visible || prev_state != download_state;
            if newly_visible {
                match download_state {
                    DownloadDisplayState::Idle => {
                        download_button.reset_hover(cx);
                        share_button.reset_hover(cx);
                    }
                    DownloadDisplayState::InProgress => cancel_button.reset_hover(cx),
                    DownloadDisplayState::Succeeded(_) => success_button.reset_hover(cx),
                    DownloadDisplayState::Failed => failure_button.reset_hover(cx),
                }
            }
        }
        self.download_state = download_state;
    }
}

impl MessageRef {
    fn set_data(
        &self,
        cx: &mut Cx,
        details: MessageDetails,
        event_tl_item: &EventTimelineItem,
        download_info: Option<DownloadableAttachment>,
        download_state: DownloadDisplayState,
        is_reply_expanded: bool,
        is_newest_sent: bool,
        is_blocked_by_failed_send: bool,
        is_room_encrypted: bool,
    ) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.set_data(cx, details, event_tl_item, download_info, download_state, is_reply_expanded, is_newest_sent, is_blocked_by_failed_send, is_room_encrypted);
    }
}

/// Clears all UI-related timeline states for all known rooms.
///
/// Takes `&mut Cx` (unused) to enforce that it's only called from the main UI thread.
pub fn clear_timeline_states(cx: &mut Cx) {
    timeline_state_store::clear_all(cx);
}

/// Invalidates the UI-side cached state for a single timeline whose backend was just closed,
/// so the next time it's shown, it'll rebuild it instead of reusing the stale cached data.
///
/// Takes `&mut Cx` (unused) to enforce that it's only called from the main UI thread.
pub fn invalidate_single_timeline_state(cx: &mut Cx, kind: &TimelineKind) {
    timeline_state_store::invalidate(cx, kind);
}

/// Invalidates the cached UI states of the given room's main timeline
/// and all of its thread timelines.
pub fn invalidate_entire_room_timeline_states(cx: &mut Cx, room_id: &RoomId) {
    timeline_state_store::invalidate_entire_room(cx, room_id);
}

/// Drops the loaded data of the given timeline's docked panes once its screen is closed for good,
/// which stops their data feeds, e.g., a threads pane's background worker.
///
/// Takes `&mut Cx` (unused) to enforce that it's only called from the main UI thread.
pub fn drop_docked_pane_data(cx: &mut Cx, kind: &TimelineKind) {
    timeline_state_store::drop_pane_data(cx, kind);
}

/// A pending "Reply In Thread" request to focus a thread's input bar once its RoomScreen is
/// shown, stored as a `Cx` global so whichever screen ends up showing it can pick it up.
mod input_bar_focus {
    use super::*;

    /// The timeline whose RoomScreen should focus its input bar when next shown.
    #[derive(Default)]
    struct PendingInputBarFocus(Option<TimelineKind>);

    /// Requests that the RoomScreen showing `kind` focus its input bar once it's shown.
    pub(super) fn request(cx: &mut Cx, kind: TimelineKind) {
        cx.global::<PendingInputBarFocus>().0 = Some(kind);
    }

    /// If a focus request is pending for `kind`, consumes it and returns `true`.
    pub(super) fn take_if_matches(cx: &mut Cx, kind: &TimelineKind) -> bool {
        let pending = cx.global::<PendingInputBarFocus>();
        if pending.0.as_ref() == Some(kind) {
            pending.0 = None;
            true
        } else {
            false
        }
    }
}
