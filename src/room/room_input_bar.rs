//! The RoomInputBar widget contains all components related to sending messages/content to a room.
//!
//! The RoomInputBar is capped to a maximum height of 75% of the containing RoomScreen's height,
//! but it always shows at least one full line of its text input, and its warning banners in full.
//!
//! The widgets included in the RoomInputBar are:
//! * banners that warn about a room member's identity being reset or a message that failed to send.
//! * a preview of the message the user is replying to.
//! * the location preview (which allows you to send your current location to the room),
//!   plus a menu item to show the location preview.
//! * a button that opens the RoomScreen-level popup menu for uploads/location.
//! * If TSP is enabled, a checkbox to enable TSP signing for the outgoing message.
//! * A MentionableTextInput, which allows the user to type a message
//!   and mention other users via the `@` key.
//! * Native streaming speech-to-text dictation, via a microphone button inside the text input.
//! * A button to send the message.
//! * The editing pane, which is shown when the user is editing a previous message.
//! * A tombstone footer, which is shown if the room has been tombstoned (replaced).
//! * A membership footer, which is shown if the user is no longer in the room.
//! * A "cannot-send-message" notice, which is shown if the user cannot send messages to the room.
//!


use std::sync::Arc;
use makepad_widgets::{text::selection::Cursor, *};
use matrix_sdk::room::RoomMember;
use matrix_sdk::room::reply::{EnforceThread, Reply};
use ruma::events::room::message::AddMentions;
use matrix_sdk_ui::timeline::{EmbeddedEvent, EventTimelineItem, TimelineEventItemId};
use ruma::{events::room::message::{LocationMessageEventContent, MessageType, ReplyWithinThread, RoomMessageEventContent}, OwnedEventId, OwnedRoomId, OwnedTransactionId};
use crate::{block_user_modal::{BlockUserModalAction, BlockUserRequest}, home::{editing_pane::{EditingPaneRef, EditingPaneState, EditingPaneWidgetExt, EditingPaneWidgetRefExt}, location_preview::{LocationPreviewWidgetExt, LocationPreviewWidgetRefExt}, room_screen::{MessageAction, display_name_or_user_id, populate_preview_of_timeline_item}, rooms_list::RoomsListRef, tombstone_footer::{SuccessorRoomDetails, TombstoneFooterWidgetExt}, upload_progress::{UploadProgressViewWidgetRefExt, UploadState}}, join_leave_room_modal::{JoinLeaveModalKind, JoinLeaveRoomModalAction}, location::init_location_subscriber, moderation::MemberModeration, moderation_modal::{ModerationModalAction, ModerationRequest}, profile::user_profile::{ShowUserProfileAction, UserProfile, UserProfileAndRoomId}, room::{BasicRoomDetails, attachment_buttons::{AttachmentButtonsRef, AttachmentButtonsWidgetExt}}, settings::app_preferences::{AppPreferencesAction, AppPreferencesGlobal}, shared::{avatar::{AvatarState, AvatarWidgetRefExt}, file_upload_modal::{AttachmentUpload, FileUploadAttemptId, PendingUpload, handle_picked_file, handle_picker_launch_errors}, html_or_plaintext::HtmlOrPlaintextWidgetRefExt, mentionable_text_input::{MentionableTextInputRef, MentionableTextInputWidgetExt, MentionableTextInputWidgetRefExt, MentionableTextInputState}, popup_list::{PopupKind, enqueue_popup_notification}, room_input_popup_menu::RoomInputPopupMenuAction, slash_commands::{SlashCommandAction, SlashCommandOutcome}, styles::{apply_positive_button_style, apply_primary_button_style}}, sliding_sync::{MatrixRequest, TimelineKind, UserPowerLevels, submit_async_request}, utils};
use crate::room::{membership::RoomMembership, room_membership_footer::RoomMembershipFooterWidgetExt};
use crate::room::reply_preview::CollapsiblePreviewWidgetRefExt;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*


    mod.widgets.RoomInputBar = set_type_default() do #(RoomInputBar::register_widget(vm)) {
        ..mod.widgets.RoundedView

        width: Fill,
        height: Fit{max: FitBound.Rel{base: Base.Full, factor: 0.75}}
        flow: Down,
        new_batch: true

        // These margins are a hack to make the borders of the RoomInputBar
        // line up with the boundaries of its parent widgets.
        // This only works if the border_color is the same as its parents,
        // which is currently `COLOR_SECONDARY`.
        margin: Inset{left: -4, right: -4, bottom: -4 }
        show_bg: true,
        draw_bg +: {
            color: (COLOR_PRIMARY)
            border_radius: 5.0
            border_color: (COLOR_SECONDARY)
            border_size: 2.0
            // shadow_color: #0006
            // shadow_radius: 0.0
            // shadow_offset: vec2(0.0,0.0)
        }

        // Everything above the message input, which always leaves room for one line of it.
        banners_and_previews := View {
            width: Fill, height: Fit
            flow: Down

            // The top-most elements warn about any room member whose identity was reset,
            // and about a failed message that is holding up this room's send queue.
            identity_reset_banner := IdentityResetBanner { }
            failed_send_banner := FailedSendBanner { }

            // Below that, display a preview of the current location that a user is about to send.
            location_preview := LocationPreview { }

            // Upload progress view (shown when a file upload is in progress)
            upload_progress_view := UploadProgressView { }

            // Below that, a preview of the message that the user is replying to, if any,
            // which collapses to fit in the height that's left.
            replying_preview := ReplyingPreview {
                height: Fit{max: FitBound.Rel{base: Base.Unused, factor: 1.0}}
            }
        }

        // Below that, if the user can't post here, display one of these notices instead of the message input bar.
        // They sit outside the overlay below so that an in-progress edit can't cover them.
        notices := RoundedYView {
            width: Fill, height: Fit
            flow: Down

            // Rounded at the top with the same outline as the RoomInputBar and EditingPane.
            show_bg: true
            draw_bg +: {
                color: (COLOR_SECONDARY)
                border_radius: vec2(5.0, 0.0)
                border_size: 2.0
                border_color: (COLOR_SECONDARY)
            }

            // A notice that the user can't send messages to this room.
            can_not_send_message_notice := View {
                visible: false
                // Uneven so the text looks centered: the RoomInputBar's -4 bottom margin hides 4 of the bottom padding.
                padding: Inset{top: 18, bottom: 22, left: 20, right: 20}
                align: Align{x: 0.5, y: 0.5}
                width: Fill, height: Fit

                text := Label {
                    width: Fill,
                    flow: Flow.Right{wrap: true},
                    align: Align{x: 0.5, y: 0.5}
                    draw_text +: {
                        color: (COLOR_TEXT)
                        text_style: theme.font_italic {font_size: 12.2}
                    }
                    text: "You don't have permission to post to this room.",
                }
            }

            // If this room was tombstoned, a "footer" view showing the successor room info.
            tombstone_footer := TombstoneFooter { }

            // If the user isn't in this room anymore, a footer saying why, with any way to re-join.
            membership_footer := RoomMembershipFooter { }
        }

        // Below that, display the message input bar (buttons and message TextInput),
        // or the EditingPane, which slides up as an overlay in front of it.
        overlay_wrapper := View {
            width: Fill,
            // The banners and previews above take up some of the input bar's max height,
            // so this only gets what's left of it.
            height: Fit{max: FitBound.Rel{base: Base.Unused, factor: 1.0}}
            flow: Overlay,

            // Below that, display a view that holds the message input bar and send button.
            input_bar := View {
                width: Fill,
                height: Fit
                flow: Right
                // Bottom-align everything to ensure that buttons always stick to the bottom
                // even when the mentionable_text_input box is very tall.
                align: Align{y: 1.0},
                padding: 6,

                attachment_buttons := AttachmentButtons {
                    margin: Inset { top: 4, left: 4, right: 4, bottom: 5}
                }

                // A checkbox that enables TSP signing for the outgoing message.
                // If TSP is not enabled, this will be an empty invisible view.
                tsp_sign_checkbox := TspSignAnycastCheckbox {
                    margin: Inset{bottom: 9, left: 6, right: 0}
                }

                mentionable_text_input := MentionableTextInput {
                    width: Fill,
                    margin: Inset {
                        top: 3, // add some space between the top border of the text input and the top border of the room input bar
                        bottom: 5.75, // to line up the middle of the text input with the middle of the buttons
                        left: 3, right: 3 // to give a bit of breathing room between the text input and the buttons on the sides
                    },

                    speech_text_input +: {
                        text_input +: {
                            empty_text: "Write a message (in Markdown) ..."
                            keep_focus_on_submit: true
                        }
                    }
                }

                send_message_button := RobrixPositiveIconButton {
                    // Disabled by default; enabled when text is inputted
                    enabled: false,
                    hide_keyboard_on_tap: false
                    padding: 8
                    margin: Inset { top: 4, left: 4, right: 4, bottom: 5}
                    spacing: 0,
                    draw_icon +: { svg: (ICON_SEND) }
                    icon_walk: Walk{width: 23, height: 23},
                }
            }

            editing_pane := EditingPane { }
        }
    }
}

/// Which view the `RoomInputBar` should show, based on room state.
enum RoomInputBarMode<'a> {
    /// The user can send messages, so show the regular message input bar.
    Input,
    /// The user cannot send messages, so show a notice saying so.
    CannotSend,
    /// This room was replaced, so show the tombstone footer.
    Tombstoned {
        room_id: &'a OwnedRoomId,
        successor: &'a SuccessorRoomDetails,
    },
    /// The user isn't in this room anymore, so show the membership footer.
    NotJoined {
        room_id: &'a OwnedRoomId,
        membership: &'a RoomMembership,
    },
}

/// The input bar at the bottom of a RoomScreen that allows the user to send messages, upload files, etc.
///
/// It also shows the tombstone footer for rooms that have been tombstoned,
/// the membership footer for rooms that the user is no longer in,
/// or a notice that the user cannot send messages to this room.
#[derive(Script, Widget)]
pub struct RoomInputBar {
    #[source] source: ScriptObjectRef,
    #[deref] view: View,

    /// Whether the `ReplyingPreview` was visible when the `EditingPane` was shown.
    /// If true, when the `EditingPane` gets hidden, we need to re-show the `ReplyingPreview`.
    #[rust] was_replying_preview_visible: bool,
    /// Info about the message event that the user is currently replying to, if any.
    #[rust] replying_to: Option<(EventTimelineItem, EmbeddedEvent)>,
    /// Cached natural Fit height of the input_bar, used as the animation
    /// target when the editing pane is being hidden.
    #[rust] input_bar_natural_height: f64,
    /// The height of the banners and previews with the replying preview's content fully collapsed,
    /// as of when we last drew them.
    #[rust] min_banners_and_previews_height: f64,
    /// The height of the views below the replying preview's content, within the replying preview.
    #[rust] height_below_reply_preview_content: f64,
    /// Used to redraw once more after the banners and previews changed height.
    #[rust] next_frame: NextFrame,
    /// Whether the currently-displayed room is encrypted, used to style the send button.
    #[rust] is_encrypted: bool,
    /// Whether the send button is currently enabled (the message input is non-empty).
    #[rust] is_send_enabled: bool,
    /// The room or thread that this RoomInputBar is currently within.
    #[rust] timeline_kind: Option<TimelineKind>,
    /// The widget UID of the RoomScreen containing this RoomInputBar.
    #[rust] room_screen_widget_uid: Option<WidgetUid>,
    /// The child widgets we use on every draw and event, looked up once.
    #[rust] child_refs: Option<ChildRefs>,
}

#[derive(Clone)]
struct ChildRefs {
    banners_and_previews: ViewRef,
    replying_preview: ViewRef,
    reply_preview_content: WidgetRef,
    reply_preview_bottom_line: WidgetRef,
    notices: ViewRef,
    overlay_wrapper: ViewRef,
    input_bar: ViewRef,
    mentionable_text_input: MentionableTextInputRef,
    attachment_buttons: AttachmentButtonsRef,
    send_message_button: ButtonRef,
    cancel_reply_button: ButtonRef,
    send_location_button: ButtonRef,
    editing_pane: EditingPaneRef,
}

impl ScriptHook for RoomInputBar {
    fn on_after_new(&mut self, vm: &mut ScriptVm) {
        vm.with_cx_mut(|cx| {
            let send_on_enter = cx.global::<AppPreferencesGlobal>().0.send_on_enter;
            let mentionable_text_input = self.mentionable_text_input(cx, ids!(mentionable_text_input));
            mentionable_text_input.text_input_ref().set_submit_on_enter(send_on_enter);
            self.attachment_buttons(cx, ids!(attachment_buttons)).set_input(mentionable_text_input.speech_text_input_ref());
        });
    }

    fn on_after_reload(&mut self, _vm: &mut ScriptVm) {
        self.child_refs = None;
    }
}

impl Widget for RoomInputBar {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        // Redraw after the banners and previews changed height (see draw_walk).
        if self.next_frame.is_event(event).is_some() {
            self.redraw(cx);
        }

        let reply_preview_content_area = self.child_refs(cx).reply_preview_content.area();
        match event.hits(cx, reply_preview_content_area) {
            // If the hit occurred on the replying message preview, jump to it.
            Hit::FingerUp(fe) if fe.is_over && fe.is_primary_hit() && fe.was_tap() => {
                if let Some(event_id) = self.replying_to.as_ref()
                    .and_then(|(event_tl_item, _)| event_tl_item.event_id().map(ToOwned::to_owned))
                {
                    if let Some(room_screen_widget_uid) = self.room_screen_widget_uid {
                        cx.widget_action(
                            room_screen_widget_uid,
                            MessageAction::JumpToEvent(event_id),
                        );
                    }
                } else {
                    enqueue_popup_notification(
                        "BUG: couldn't find the message you're replying to.",
                        PopupKind::Error,
                        None,
                    );
                }
            }
            _ => {}
        }

        if let Event::Actions(actions) = event {
            self.handle_actions(cx, actions);
        }

        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, mut walk: Walk) -> DrawStep {
        let ChildRefs {
            banners_and_previews,
            replying_preview,
            reply_preview_content,
            reply_preview_bottom_line,
            notices,
            overlay_wrapper,
            input_bar,
            editing_pane,
            ..
        } = self.child_refs(cx).clone();
        // Where we reach into the screen's bottom safe area (e.g., under iOS's home indicator),
        // our bottom-most view extends into it, so that its own color fills that area.
        let safe_area_overlap = {
            let parent = cx.turtle().rect();
            let insets = cx.display_context.safe_area_insets;
            let safe_area_top = cx.display_context.screen_size.y - insets.bottom;
            (parent.pos.y + parent.size.y - safe_area_top).max(0.0).min(insets.bottom)
        };
        let is_notice_at_bottom = !input_bar.visible() && !editing_pane.is_currently_shown(cx);
        if let Some(mut inner) = notices.borrow_mut() {
            inner.layout.padding.bottom = if is_notice_at_bottom { safe_area_overlap } else { 0.0 };
        }
        if let Some(mut inner) = overlay_wrapper.borrow_mut() {
            inner.layout.padding.bottom = if is_notice_at_bottom { 0.0 } else { safe_area_overlap };
        }

        // Shrink the input_bar's height as the editing pane slides in,
        // and grow it back as the editing pane slides out.
        // slide=1.0 → editing pane hidden → input_bar at full Fit height.
        // slide=0.0 → editing pane shown → input_bar at zero height.
        let slide = editing_pane.slide();

        // Remap slide through a steeper curve so the input_bar reaches
        // its full target height before the ExpDecay tail.
        let remapped = (slide as f64 * 1.25).min(1.0);
        if remapped >= 1.0 {
            // Input_bar has reached its full natural height: switch to Fit
            // so it can respond to content changes normally.
            // Update the cached height for future animations.
            let h = input_bar.area().rect(cx).size.y;
            if h > 0.0 {
                self.input_bar_natural_height = h;
            }
            if let Some(mut inner) = input_bar.borrow_mut() {
                inner.walk.height = Size::fit();
            }
        } else {
            let target = self.input_bar_natural_height;
            if let Some(mut inner) = input_bar.borrow_mut() {
                inner.walk.height = Size::Fixed((target * remapped).max(0.0));
            }
        }

        reply_preview_content.set_visible(cx, true);

        // Views are laid out top-down, so we reserve the input's minimum height before drawing the
        // banners and previews above it. Those are shown in full even past our usual max height,
        // except for the replying preview's content, which collapses to fit in what's left.
        let min_input_height = self.get_min_input_height(cx);
        let available_height = {
            let parent = cx.turtle();
            parent.unused_height() - parent.padding().bottom - walk.margin.height()
        };
        let max_height = cx.walk_max_height(walk)
            .unwrap_or(available_height)
            .max(min_input_height + self.min_banners_and_previews_height)
            .min(available_height);
        walk.height = Size::fit();
        walk.max_height = Some(FitBound::Abs(max_height));
        if let Some(mut inner) = banners_and_previews.borrow_mut() {
            inner.walk.max_height = Some(FitBound::Abs((max_height - min_input_height).max(0.0)));
        }
        // A replying preview without room for its header is hidden rather than cut off.
        let does_everything_fit = max_height >= min_input_height + self.min_banners_and_previews_height;
        if let Some(mut inner) = replying_preview.borrow_mut() {
            inner.walk.max_height = (!does_everything_fit).then_some(FitBound::Abs(0.0));
        }
        reply_preview_content.as_collapsible_preview()
            .set_reserved_height_below(self.height_below_reply_preview_content);

        let step = self.view.draw_walk(cx, scope, walk);

        // Measure the views that the next draw has to make room for.
        // A clipped view's children still have their full heights, so we measure those.
        let mut min_banners_and_previews_height = 0.0;
        banners_and_previews.children(&mut |id, child| {
            if id != live_id!(replying_preview) && child.visible() {
                min_banners_and_previews_height += child.area().rect(cx).size.y
                    + child.walk(cx).margin.height();
            }
        });
        if replying_preview.visible() {
            let preview_rect = replying_preview.area().rect(cx);
            let line_rect = reply_preview_bottom_line.area().rect(cx);
            let content_rect = reply_preview_content.area().rect(cx);
            let preview_bottom = line_rect.pos.y + line_rect.size.y;
            min_banners_and_previews_height += preview_bottom - preview_rect.pos.y - content_rect.size.y;
            self.height_below_reply_preview_content = preview_bottom - content_rect.pos.y - content_rect.size.y;
        }
        if min_banners_and_previews_height != self.min_banners_and_previews_height {
            self.min_banners_and_previews_height = min_banners_and_previews_height;
            self.next_frame = cx.new_next_frame();
        }
        // We only learn a notice's height once it's drawn, and a squeezed edit only regrows a bit per draw,
        // so we redraw until the height we reserve for them stops changing.
        if (self.get_min_input_height(cx) - min_input_height).abs() > 0.5 {
            self.next_frame = cx.new_next_frame();
        }
        step
    }

    /// Gives key focus to the inner message text input.
    fn set_key_focus(&self, cx: &mut Cx) {
        // The editing pane slides the input bar out of view without hiding it.
        if !self.view.view(cx, ids!(input_bar)).visible()
            || self.editing_pane(cx, ids!(editing_pane)).is_currently_shown(cx)
        {
            return;
        }
        self.child_by_path(ids!(input_bar.mentionable_text_input))
            .as_mentionable_text_input()
            .set_key_focus(cx);
    }

    /// Returns whether the inner message text input has key focus.
    fn key_focus(&self, cx: &Cx) -> bool {
        self.child_by_path(ids!(input_bar.mentionable_text_input.text_input)).key_focus(cx)
    }
}

impl RoomInputBar {
    /// Stops any speech-to-text dictation in either the message input or the editing pane.
    ///
    /// Any words already in a text input will stay there, they won't be cleared.
    fn cancel_dictation(&mut self, cx: &mut Cx) {
        self.mentionable_text_input(cx, ids!(mentionable_text_input)).cancel_dictation(cx);
        self.mentionable_text_input(cx, ids!(editing_pane.editing_content.edit_text_input)).cancel_dictation(cx);
    }

    fn handle_actions(
        &mut self,
        cx: &mut Cx,
        actions: &Actions,
    ) {
        let ChildRefs {
            input_bar,
            mentionable_text_input,
            attachment_buttons,
            send_message_button,
            cancel_reply_button,
            send_location_button,
            editing_pane,
            ..
        } = self.child_refs(cx).clone();
        let text_input = mentionable_text_input.text_input_ref();
        let attachment_action = attachment_buttons.handle_actions(cx, actions);

        for action in actions {
            // Handle changes to the `send_on_enter` preference.
            if let Some(AppPreferencesAction::SendOnEnterChanged(v)) = action.downcast_ref() {
                text_input.set_submit_on_enter(*v);
                continue;
            }
            // If the user opts out of sending typing notices, immediately stop sending them.
            if let Some(AppPreferencesAction::SendTypingNoticesChanged(false)) = action.downcast_ref()
                && let Some(timeline_kind) = self.timeline_kind.as_ref()
            {
                submit_async_request(MatrixRequest::SendTypingNotice {
                    room_id: timeline_kind.room_id().clone(),
                    typing: false,
                });
                continue;
            }
        }

        // Clear the replying-to preview pane if the "cancel reply" button was clicked
        // or if the `Escape` key was pressed within the message input box.
        if cancel_reply_button.clicked(actions)
            || text_input.escaped(actions)
        {
            self.clear_replying_to(cx);
            self.redraw(cx);
        }

        // Everything below here requires the current room/thread kind.
        let (Some(timeline_kind), Some(room_screen_widget_uid)) =
            (self.timeline_kind.clone(), self.room_screen_widget_uid)
        else {
            return;
        };

        if attachment_action != RoomInputPopupMenuAction::None {
            cx.widget_action(room_screen_widget_uid, attachment_action);
        }

        // Handle the send location button being clicked.
        if send_location_button.clicked(actions) {
            let location_preview = self.location_preview(cx, ids!(location_preview));
            if let Some((coords, _system_time_opt)) = location_preview.get_current_data() {
                let geo_uri = format!("{}{},{}", utils::GEO_URI_SCHEME, coords.latitude, coords.longitude);
                let message = RoomMessageEventContent::new(
                    MessageType::Location(
                        LocationMessageEventContent::new(geo_uri.clone(), geo_uri)
                    )
                );
                let replied_to = self.replying_to.take().and_then(|(event_tl_item, _emb)|
                    event_tl_item.event_id().map(|event_id| {
                        let enforce_thread = if timeline_kind.thread_root_event_id().is_some() {
                            EnforceThread::Threaded(ReplyWithinThread::Yes)
                        } else {
                            EnforceThread::MaybeThreaded
                        };
                        Reply {
                            event_id: event_id.to_owned(),
                            enforce_thread,
                            add_mentions: AddMentions::Yes,
                        }
                    })
                ).or_else(||
                    timeline_kind.thread_root_event_id().map(|thread_root_event_id|
                        Reply {
                            event_id: thread_root_event_id.clone(),
                            enforce_thread: EnforceThread::Threaded(ReplyWithinThread::No),
                            add_mentions: AddMentions::No,
                        }
                    )
                );
                submit_async_request(MatrixRequest::SendMessage {
                    timeline_kind: timeline_kind.clone(),
                    message,
                    replied_to,
                    #[cfg(feature = "tsp")]
                    sign_with_tsp: self.is_tsp_signing_enabled(cx),
                });

                self.clear_replying_to(cx);
                location_preview.clear();
                location_preview.redraw(cx);
            }
        }

        // Keyboard events still reach a hidden input bar (like the send msg shortcut),
        // so we have to ignore them unless the user is allowed to send a message.
        let can_post = input_bar.visible();

        // Handle the send message button being clicked, or a `Returned` action
        // from the message text input, which already respects the user's app setting.
        if can_post && (
            send_message_button.clicked(actions)
            || text_input.returned(actions).is_some()
        ) {
            self.cancel_dictation(cx);
            let entered_text = mentionable_text_input.text().trim().to_string();
            if !entered_text.is_empty() {
                match mentionable_text_input.parse_input(&entered_text) {
                    SlashCommandOutcome::Message(message) => {
                        let replied_to = self.replying_to.take().and_then(|(event_tl_item, _emb)|
                            event_tl_item.event_id().map(|event_id| {
                                let enforce_thread = if timeline_kind.thread_root_event_id().is_some() {
                                    EnforceThread::Threaded(ReplyWithinThread::Yes)
                                } else {
                                    EnforceThread::MaybeThreaded
                                };
                                Reply {
                                    event_id: event_id.to_owned(),
                                    enforce_thread,
                                    add_mentions: AddMentions::Yes,
                                }
                            })
                        ).or_else(||
                            timeline_kind.thread_root_event_id().map(|thread_root_event_id|
                                Reply {
                                    event_id: thread_root_event_id.clone(),
                                    enforce_thread: EnforceThread::Threaded(ReplyWithinThread::No),
                                    add_mentions: AddMentions::No,
                                }
                            )
                        );
                        submit_async_request(MatrixRequest::SendMessage {
                            timeline_kind: timeline_kind.clone(),
                            message,
                            replied_to,
                            #[cfg(feature = "tsp")]
                            sign_with_tsp: self.is_tsp_signing_enabled(cx),
                        });

                        self.clear_replying_to(cx);
                        mentionable_text_input.set_text(cx, "");
                        self.enable_send_message_button(cx, false);
                        submit_async_request(MatrixRequest::SendTypingNotice {
                            room_id: timeline_kind.room_id().clone(),
                            typing: false,
                        });
                    }
                    SlashCommandOutcome::Action(action) => {
                        self.run_slash_command_action(cx, action, timeline_kind.room_id(), room_screen_widget_uid);
                        mentionable_text_input.set_text(cx, "");
                        self.enable_send_message_button(cx, false);
                        submit_async_request(MatrixRequest::SendTypingNotice {
                            room_id: timeline_kind.room_id().clone(),
                            typing: false,
                        });
                    }
                    // Keep the text around so the user can fix the command and try again.
                    SlashCommandOutcome::Error(error) => {
                        enqueue_popup_notification(error, PopupKind::Error, Some(8.0));
                    }
                }
            }
            attachment_buttons.update_for_input(cx, true);
        }

        // If the user starts/stops typing in the message input box,
        // send a typing notice to the room and update the send_message_button state.
        let is_text_input_empty = if let Some(new_text) = text_input.changed(actions) {
            let is_empty = new_text.is_empty();
            if can_post && cx.global::<AppPreferencesGlobal>().0.send_typing_notices {
                submit_async_request(MatrixRequest::SendTypingNotice {
                    room_id: timeline_kind.room_id().clone(),
                    typing: !is_empty,
                });
            }
            is_empty
        } else {
            text_input.text().is_empty()
        };
        // Only restyle the button when its enabled state actually changes,
        // not on every actions pass.
        let should_enable = !is_text_input_empty;
        if should_enable != self.is_send_enabled {
            self.enable_send_message_button(cx, should_enable);
        }

        // Handle the user pressing the up arrow in an empty message input box
        // to edit their latest sent message.
        if can_post && is_text_input_empty {
            if let Some(KeyEvent {
                key_code: KeyCode::ArrowUp,
                modifiers: KeyModifiers { shift: false, control: false, alt: false, logo: false },
                ..
            }) = text_input.key_down_unhandled(actions) {
                cx.widget_action(
                    room_screen_widget_uid, 
                    MessageAction::EditLatest,
                );
            }
        }

        // When the hide animation fully completes, restore the replying preview.
        if editing_pane.was_hidden(actions) {
            self.on_editing_pane_hidden(cx);
        }
    }

    fn child_refs(&mut self, cx: &mut Cx) -> &ChildRefs {
        self.child_refs.get_or_insert_with(|| ChildRefs {
            banners_and_previews: self.view.view(cx, ids!(banners_and_previews)),
            replying_preview: self.view.view(cx, ids!(replying_preview)),
            reply_preview_content: self.view.widget(cx, ids!(replying_preview.reply_preview_content)),
            reply_preview_bottom_line: self.view.widget(cx, ids!(replying_preview.bottom_line)),
            notices: self.view.view(cx, ids!(notices)),
            overlay_wrapper: self.view.view(cx, ids!(overlay_wrapper)),
            input_bar: self.view.view(cx, ids!(input_bar)),
            mentionable_text_input: self.view.mentionable_text_input(cx, ids!(mentionable_text_input)),
            attachment_buttons: self.view.attachment_buttons(cx, ids!(attachment_buttons)),
            send_message_button: self.view.button(cx, ids!(send_message_button)),
            cancel_reply_button: self.view.button(cx, ids!(cancel_reply_button)),
            send_location_button: self.view.button(cx, ids!(location_preview.send_location_button)),
            editing_pane: self.view.editing_pane(cx, ids!(editing_pane)),
        })
    }

    /// Returns the height that the views below the banners and previews need.
    ///
    /// That's enough for the text input there to show one full line,
    /// or for a notice shown there instead to be shown in full, plus any in-progress edit below it.
    fn get_min_input_height(&mut self, cx: &mut Cx) -> f64 {
        let ChildRefs {
            notices,
            overlay_wrapper,
            input_bar,
            mentionable_text_input,
            editing_pane,
            ..
        } = self.child_refs(cx).clone();
        // At most one notice is shown, and an in-progress edit can stay shown below it.
        let notices_height = notices.area().rect(cx).size.y;
        let safe_area_padding = overlay_wrapper.borrow().map_or(0.0, |inner| inner.layout.padding.bottom);
        if editing_pane.is_currently_shown(cx) {
            return notices_height + editing_pane.get_min_height(cx) + safe_area_padding;
        }
        if input_bar.visible() {
            let speech_text_input = mentionable_text_input.speech_text_input_ref();
            let paddings_height = mentionable_text_input.borrow()
                .map_or(0.0, |inner| inner.layout.padding.height())
                + input_bar.borrow().map_or(0.0, |inner| inner.layout.padding.height());
            let min_height = speech_text_input.get_min_height_for_ancestors(cx) + paddings_height;
            // The buttons beside the text input can make the input bar even taller than that.
            let extra_text_height = mentionable_text_input.text_input_ref().area().rect(cx).size.y
                - speech_text_input.get_min_text_input_height(cx);
            return min_height.max(self.input_bar_natural_height - extra_text_height.max(0.0)) + safe_area_padding;
        }
        notices_height
    }

    /// Runs the given slash command action within this room.
    fn run_slash_command_action(
        &mut self,
        cx: &mut Cx,
        action: SlashCommandAction,
        room_id: &OwnedRoomId,
        room_screen_widget_uid: WidgetUid,
    ) {
        let profile_of = |user_id| UserProfile {
            user_id,
            username: None,
            avatar_state: AvatarState::Unknown,
        };
        match action {
            SlashCommandAction::LeaveRoom => {
                let room_details = match cx.get_global::<RoomsListRef>().get_room_name(room_id) {
                    Some(room_name_id) => BasicRoomDetails::Name(room_name_id),
                    None => BasicRoomDetails::RoomId(utils::RoomNameId::empty(room_id.clone())),
                };
                cx.action(JoinLeaveRoomModalAction::Open {
                    kind: JoinLeaveModalKind::LeaveRoom(room_details),
                    show_tip: false,
                });
            }
            SlashCommandAction::InviteUser(user_id) => {
                submit_async_request(MatrixRequest::InviteUser {
                    room_id: room_id.clone(),
                    user_id,
                })
            }
            SlashCommandAction::OpenDirectMessage(user_id) => {
                submit_async_request(MatrixRequest::OpenOrCreateDirectMessage {
                    user_profile: profile_of(user_id),
                    // Don't just create a new DM room, we want to first get confirmation
                    // from the user just like we do for the normal Direct Message button.
                    allow_create: false,
                })
            }
            SlashCommandAction::ShowUserProfile(user_id) => {
                // Only the RoomScreen owns the user profile pane, so it has to handle this.
                cx.widget_action(
                    room_screen_widget_uid,
                    ShowUserProfileAction::ShowUserProfile(UserProfileAndRoomId {
                        user_profile: profile_of(user_id),
                        room_id: room_id.clone(),
                    }),
                )
            }
            SlashCommandAction::BlockUser { user_id, block } => {
                cx.action(BlockUserModalAction::Open(BlockUserRequest {
                    user_id,
                    display_name: None,
                    block,
                    reject_invite_to: None,
                }))
            }
            SlashCommandAction::ModerateMember { user_id, action, reason, can_remove_messages } => {
                let room_name_id = cx.get_global::<RoomsListRef>().get_room_name(room_id)
                    .unwrap_or_else(|| utils::RoomNameId::empty(room_id.clone()));
                let user_name = display_name_or_user_id(cx, room_id, user_id.clone());
                cx.action(ModerationModalAction::Open(ModerationRequest::Member {
                    moderation: MemberModeration {
                        room_id: room_id.clone(),
                        room_name: room_name_id.to_string(),
                        user_id,
                        user_name,
                        action,
                    },
                    initial_reason: reason,
                    can_remove_messages,
                }))
            }
            SlashCommandAction::SetDisplayName(new_display_name) => {
                submit_async_request(MatrixRequest::SetDisplayName {
                    new_display_name: Some(new_display_name),
                })
            }
        }
    }

    fn show_current_location_preview(&mut self, cx: &mut Cx) {
        if let Err(_e) = init_location_subscriber(cx) {
            error!("Failed to initialize location subscriber");
            enqueue_popup_notification(
                "Couldn't start location services. Please try again.",
                PopupKind::Error,
                None,
            );
        }
        self.view.location_preview(cx, ids!(location_preview)).show(cx);
        self.redraw(cx);
    }

    /// Shows a preview of the given event that the user is currently replying to
    /// above the message input bar.
    ///
    /// If `grab_key_focus` is true, this will also automatically focus the keyboard
    /// on the message input box so that the user can immediately start typing their reply.
    fn show_replying_to(
        &mut self,
        cx: &mut Cx,
        replying_to: (EventTimelineItem, EmbeddedEvent),
        timeline_kind: &TimelineKind,
        grab_key_focus: bool,
    ) {
        // When the user clicks the reply button next to a message, we need to:
        // 1. Populate and show the ReplyingPreview, of course.
        let replying_preview = self.view(cx, ids!(replying_preview));
        let (replying_preview_username, _) = replying_preview
            .avatar(cx, ids!(replying_to_sender.reply_preview_avatar))
            .set_avatar_and_get_username(
                cx,
                timeline_kind,
                replying_to.0.sender(),
                Some(replying_to.0.sender_profile()),
                replying_to.0.event_id(),
                true,
            );

        replying_preview
            .label(cx, ids!(replying_to_sender.reply_preview_username))
            .set_text(cx, replying_preview_username.as_str());

        populate_preview_of_timeline_item(
            cx,
            &replying_preview.html_or_plaintext(cx, ids!(reply_preview_content.preview_content.reply_preview_body)),
            replying_to.0.content(),
            replying_to.0.sender(),
            &replying_preview_username,
        );

        replying_preview.set_visible(cx, true);
        let reply_preview_content = replying_preview.widget(cx, ids!(reply_preview_content));
        reply_preview_content.set_visible(cx, true);
        reply_preview_content.as_collapsible_preview().reset_measured_height();
        reply_preview_content.view(cx, ids!(preview_content)).redraw_texture_cache();
        self.replying_to = Some(replying_to);

        // 2. Hide other views that are irrelevant to a reply, e.g.,
        //    the `EditingPane` would improperly cover up the ReplyPreview.
        self.editing_pane(cx, ids!(editing_pane)).force_reset_hide(cx);
        self.on_editing_pane_hidden(cx);
        // 3. Automatically focus the keyboard on the message input box
        //    so that the user can immediately start typing their reply
        //    without having to manually click on the message input box.
        if grab_key_focus {
            self.text_input(cx, ids!(input_bar.mentionable_text_input.text_input)).set_key_focus(cx);
        }
        self.button(cx, ids!(cancel_reply_button)).reset_hover(cx);
        self.redraw(cx);
    }

    /// Fills the input bar with the content of a message that failed to send (so the user can retry it).
    fn restore_unsent_message(
        &mut self,
        cx: &mut Cx,
        message: &RoomMessageEventContent,
        replied_to: Option<(EventTimelineItem, EmbeddedEvent)>,
        timeline_kind: &TimelineKind,
    ) {
        self.cancel_dictation(cx);
        match &message.msgtype {
            // don't do this for locations, which might already be outdates
            MessageType::Location(_) => { }
            msgtype => {
                let restored = match msgtype {
                    MessageType::Emote(emote) => format!("/me {}", emote.body),
                    MessageType::Notice(notice) => format!("/notice {}", notice.body),
                    // Escape a leading slash so it isn't treated as a command
                    MessageType::Text(text) if text.body.starts_with('/') => format!("/{}", text.body),
                    other => other.body().to_owned(),
                };
                let mentionable_text_input = self.mentionable_text_input(cx, ids!(mentionable_text_input));
                let existing = mentionable_text_input.text();
                let was_draft_blank = existing.trim().is_empty();
                let new_text = if was_draft_blank {
                    restored
                } else {
                    format!("{existing}\n{restored}")
                };
                mentionable_text_input.set_text(cx, &new_text);
                // A blank draft leaves the caret at the start, so move it after the restored message.
                if was_draft_blank {
                    mentionable_text_input.text_input_ref().set_cursor(
                        cx,
                        Cursor { index: new_text.len(), prefer_next_row: false },
                        false,
                    );
                }
                self.enable_send_message_button(cx, true);
            }
        }

        if let Some(replied_to) = replied_to {
            self.show_replying_to(cx, replied_to, timeline_kind, false);
        }
        self.redraw(cx);
    }

    /// Clears (and makes invisible) the preview of the message
    /// that the user is currently replying to.
    fn clear_replying_to(&mut self, cx: &mut Cx) {
        self.view(cx, ids!(replying_preview)).set_visible(cx, false);
        self.replying_to = None;
    }

    /// Shows the editing pane to allow the user to edit the given event.
    fn show_editing_pane(
        &mut self,
        cx: &mut Cx,
        behavior: ShowEditingPaneBehavior,
        timeline_kind: TimelineKind,
    ) {
        // The editing pane covers the message box, so dictation into it ends here.
        self.cancel_dictation(cx);
        // Cache the input_bar's natural height before the animation shrinks it.
        let input_bar_height = self.view.view(cx, ids!(input_bar)).area().rect(cx).size.y;
        if input_bar_height > 0.0 {
            self.input_bar_natural_height = input_bar_height;
        }

        // Hide the replying preview and location preview while the editing
        // pane is shown. The input_bar is not hidden; instead it is slid out
        // of view in draw_walk using the EditingPane's slide value.
        let replying_preview = self.view.view(cx, ids!(replying_preview));
        self.was_replying_preview_visible = replying_preview.visible();
        replying_preview.set_visible(cx, false);
        self.view.location_preview(cx, ids!(location_preview)).clear();

        let editing_pane = self.view.editing_pane(cx, ids!(editing_pane));
        match behavior {
            ShowEditingPaneBehavior::ShowNew { event_tl_item } => {
                editing_pane.show(cx, event_tl_item, timeline_kind);
            }
            ShowEditingPaneBehavior::RestoreExisting { editing_pane_state } => {
                editing_pane.restore_state(cx, editing_pane_state, timeline_kind);
            }
            ShowEditingPaneBehavior::RestoreFailed { event_tl_item, edited_text } => {
                editing_pane.show_failed_edit(cx, event_tl_item, timeline_kind, &edited_text);
            }
        };

        self.redraw(cx);
    }

    /// Puts an edit that failed to send back into the editing pane, so the user doesn't lose what they typed.
    ///
    /// The `event_tl_item` is the message that was being edited, if this timeline still has it.
    fn restore_failed_edit(
        &mut self,
        cx: &mut Cx,
        event_tl_item: Option<EventTimelineItem>,
        timeline_kind: TimelineKind,
        edited_text: String,
    ) {
        let editing_pane = self.view.editing_pane(cx, ids!(editing_pane));
        // We must not replace another edit that the user has started since then.
        if let Some(event_tl_item) = event_tl_item.filter(|_| !editing_pane.is_editing()) {
            let behavior = ShowEditingPaneBehavior::RestoreFailed { event_tl_item, edited_text: edited_text.clone() };
            self.show_editing_pane(cx, behavior, timeline_kind);
            if editing_pane.is_editing() { return; }
        }
        cx.copy_to_clipboard(&edited_text);
        enqueue_popup_notification(
            "Your edit couldn't be put back in the editor, so it was copied to the clipboard.",
            PopupKind::Warning,
            None,
        );
    }

    /// This should be invoked after the EditingPane has been fully hidden.
    fn on_editing_pane_hidden(&mut self, cx: &mut Cx) {
        // Restore the replying_preview (which would've been hidden when the editing pane was shown).
        if self.was_replying_preview_visible && self.replying_to.is_some() {
            self.view.view(cx, ids!(replying_preview)).set_visible(cx, true);
            self.view.widget(cx, ids!(replying_preview.reply_preview_content)).set_visible(cx, true);
        }
        self.redraw(cx);
        // We don't need to do anything with the editing pane itself here,
        // because it has already been hidden by the time this function gets called.
    } 

    /// Updates the room state to set which mode and views are shown in the RoomInputBar.
    ///
    /// The `not_joined` membership should only be `Some` if the user isn't in this room (anymore).
    fn update_room_state(
        &mut self,
        cx: &mut Cx,
        room_id: &OwnedRoomId,
        tombstone_info: Option<&SuccessorRoomDetails>,
        user_power_levels: UserPowerLevels,
        not_joined: Option<&RoomMembership>,
    ) {
        // If the room is tombstoned, that takes precedence over anything else.
        let mode = match (tombstone_info, not_joined) {
            (Some(successor), _) => RoomInputBarMode::Tombstoned { room_id, successor },
            (None, Some(membership)) => RoomInputBarMode::NotJoined { room_id, membership },
            (None, None) if user_power_levels.can_send_message() => RoomInputBarMode::Input,
            (None, None) => RoomInputBarMode::CannotSend,
        };
        self.view.view(cx, ids!(input_bar))
            .set_visible(cx, matches!(mode, RoomInputBarMode::Input));
        self.view.view(cx, ids!(can_not_send_message_notice))
            .set_visible(cx, matches!(mode, RoomInputBarMode::CannotSend));

        let tombstone_footer = self.tombstone_footer(cx, ids!(tombstone_footer));
        if let RoomInputBarMode::Tombstoned { room_id, successor } = mode {
            tombstone_footer.show(cx, room_id, successor);
        } else {
            tombstone_footer.hide(cx);
        }

        let membership_footer = self.room_membership_footer(cx, ids!(membership_footer));
        if let RoomInputBarMode::NotJoined { room_id, membership } = mode {
            membership_footer.show(cx, room_id, membership);
        } else {
            membership_footer.hide(cx);
        }

        // An edit that the user is in the middle of stays shown so they don't lose it, but it can't be sent.
        let is_input_mode = matches!(mode, RoomInputBarMode::Input);
        self.editing_pane(cx, ids!(editing_pane)).set_submit_blocked(!is_input_mode);
        // The user can't post here anymore, so cancel any in-progress reply, dictation, etc.
        if !is_input_mode {
            self.cancel_dictation(cx);
            self.was_replying_preview_visible = false;
            self.clear_replying_to(cx);
            self.view.location_preview(cx, ids!(location_preview)).clear();
            if self.key_focus(cx) {
                cx.set_key_focus(Area::Empty);
            }
        }

        self.mentionable_text_input(cx, ids!(mentionable_text_input))
            .set_user_power_levels(cx, user_power_levels);
        self.mentionable_text_input(cx, ids!(editing_pane.editing_content.edit_text_input))
            .set_user_power_levels(cx, user_power_levels);
    }

    /// Enables or disables (grays out) the send_message_button.
    ///
    /// When enabled, the button color is set based on the room's encryption state.
    fn enable_send_message_button(&mut self, cx: &mut Cx, enable: bool) {
        self.is_send_enabled = enable;
        let mut send_message_button = self.view.button(cx, ids!(send_message_button));
        if !enable {
            // A button that we disable while it's hovered shouldn't stay highlighted.
            send_message_button.reset_hover(cx);
            script_apply_eval!(cx, send_message_button, {
                enabled: false,
                draw_icon.color: mod.widgets.COLOR_FG_DISABLED,
                draw_bg.color: mod.widgets.COLOR_BG_DISABLED,
            });
        } else if self.is_encrypted {
            script_apply_eval!(cx, send_message_button, {
                enabled: true,
                draw_icon.color: mod.widgets.COLOR_TEXT_ON_ACCENT,
                draw_bg.color: mod.widgets.COLOR_ACTIVE_PRIMARY,
            });
        } else {
            script_apply_eval!(cx, send_message_button, {
                enabled: true,
                draw_icon.color: mod.widgets.COLOR_FG_ACCEPT_GREEN,
                draw_bg.color: mod.widgets.COLOR_BG_ACCEPT_GREEN,
            });
        }
    }

    /// Updates the send button (icon + color style) and empty message text
    /// based on this room's encryption status.
    fn update_encryption_state(&mut self, cx: &mut Cx, is_encrypted: bool) {
        self.is_encrypted = is_encrypted;

        // The send button is a blue "primary" button with a closed-lock badge when encrypted,
        // and a "positive" green with an opened-lock badge when not encrypted.
        let mut send_message_button = self.view.button(cx, ids!(send_message_button));
        let empty_text: &str;
        if is_encrypted {
            apply_primary_button_style(cx, &mut send_message_button);
            script_apply_eval!(cx, send_message_button, {
                draw_icon.svg: mod.widgets.ICON_SEND_ENCRYPTED,
            });
            empty_text = "Send encrypted message…";
            // empty_text = "Message…";
        } else {
            apply_positive_button_style(cx, &mut send_message_button);
            script_apply_eval!(cx, send_message_button, {
                draw_icon.svg: mod.widgets.ICON_SEND_UNENCRYPTED,
            });
            empty_text = "Send unencrypted message…";
            // empty_text = "Message…";
        }

        self.text_input(cx, ids!(input_bar.mentionable_text_input.text_input))
            .set_empty_text(cx, empty_text.to_string());

        let enable = self.is_send_enabled;
        self.enable_send_message_button(cx, enable);
    }

    /// Returns true if the TSP signing checkbox is checked, false otherwise.
    ///
    /// If TSP is not enabled, this will always return false.
    #[cfg(feature = "tsp")]
    fn is_tsp_signing_enabled(&self, cx: &mut Cx) -> bool {
        self.view.check_box(cx, ids!(tsp_sign_checkbox)).active(cx)
    }

    /// Checks and returns whether this room/timeline already has an in-progress upload.
    ///
    /// If so, it returns true and shows the user a warning popup.
    fn check_if_upload_in_progress(&self) -> bool {
        if !self.view.child_by_path(ids!(upload_progress_view)).as_upload_progress_view().has_active_upload() {
            return false;
        }
        enqueue_popup_notification(
            "Finish or cancel your current upload before starting another one.",
            PopupKind::Warning,
            Some(7.0),
        );
        true
    }

    /// Shows the native file picker dialog to select a file to be uploaded.
    fn open_file_picker(
        &mut self,
        cx: &mut Cx,
        timeline_kind: TimelineKind,
    ) {
        if self.check_if_upload_in_progress() {
            return;
        }

        let on_picked = self.upload_picker_callback(cx, timeline_kind);
        handle_picker_launch_errors(
            robius_file_picker::FileDialog::new().pick_file(on_picked)
        );
    }

    /// Shows the native media picker dialog to select a photo or video to be uploaded.
    fn open_photo_video_picker(
        &mut self,
        cx: &mut Cx,
        timeline_kind: TimelineKind,
    ) {
        if self.check_if_upload_in_progress() {
            return;
        }

        let on_picked = self.upload_picker_callback(cx, timeline_kind);
        handle_picker_launch_errors(
            robius_file_picker::FileDialog::new().pick_image_or_video(on_picked)
        );
    }

    fn upload_picker_callback(
        &self,
        _cx: &mut Cx,
        timeline_kind: TimelineKind,
    ) -> impl FnOnce(robius_file_picker::Result<Option<robius_file_picker::PickedFile>>) + Send + 'static {
        let in_reply_to = self.replying_to
            .as_ref()
            .and_then(|(event_tl_item, _embedded_event)| event_tl_item.event_id().map(ToOwned::to_owned));
        #[cfg(feature = "tsp")]
        let sign_with_tsp = self.is_tsp_signing_enabled(_cx);

        // `robius-file-picker` ensures that this `on_picked` callback runs on a bg thread.
        move |result| handle_picked_file(result, move |file_data| {
            Ok(PendingUpload::Attachment(AttachmentUpload {
                timeline_kind,
                file_data,
                in_reply_to,
                #[cfg(feature = "tsp")]
                sign_with_tsp,
            }))
        })
    }
}

impl RoomInputBarRef {
    /// Shows a preview of the given event that the user is currently replying to
    /// above the message input bar.
    pub fn show_replying_to(
        &self,
        cx: &mut Cx,
        replying_to: (EventTimelineItem, EmbeddedEvent),
        timeline_kind: &TimelineKind,
    ) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.show_replying_to(cx, replying_to, timeline_kind, true);
    }

    /// Fills the input bar with the content of a message that failed to send (so the user can retry it).
    pub fn restore_unsent_message(
        &self,
        cx: &mut Cx,
        message: &RoomMessageEventContent,
        replied_to: Option<(EventTimelineItem, EmbeddedEvent)>,
        timeline_kind: &TimelineKind,
    ) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.restore_unsent_message(cx, message, replied_to, timeline_kind);
    }

    /// Shows the editing pane to allow the user to edit the given event.
    pub fn show_editing_pane(
        &self,
        cx: &mut Cx,
        event_tl_item: EventTimelineItem,
        timeline_kind: TimelineKind,
    ) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.show_editing_pane(
            cx,
            ShowEditingPaneBehavior::ShowNew { event_tl_item },
            timeline_kind,
        );
    }

    /// See [`RoomInputBar::update_room_state()`].
    pub fn update_room_state(
        &self,
        cx: &mut Cx,
        room_id: &OwnedRoomId,
        tombstone_info: Option<&SuccessorRoomDetails>,
        user_power_levels: UserPowerLevels,
        not_joined: Option<&RoomMembership>,
    ) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.update_room_state(cx, room_id, tombstone_info, user_power_levels, not_joined);
    }

    /// Updates the message input's placeholder based on this room's encryption status.
    pub fn update_encryption_state(&self, cx: &mut Cx, is_encrypted: bool) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.update_encryption_state(cx, is_encrypted);
    }

    /// Opens the native picker to upload a photo or video into this room.
    pub fn open_photo_video_picker(
        &self,
        cx: &mut Cx,
        timeline_kind: TimelineKind,
    ) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.open_photo_video_picker(cx, timeline_kind);
    }

    /// Opens the native picker to upload a file into this room.
    pub fn open_file_picker(
        &self,
        cx: &mut Cx,
        timeline_kind: TimelineKind,
    ) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.open_file_picker(cx, timeline_kind);
    }

    /// Shows the preview flow for sending the current location into this room.
    pub fn show_current_location_preview(&self, cx: &mut Cx) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.show_current_location_preview(cx);
    }

    /// See [`RoomInputBar::restore_failed_edit()`].
    pub fn restore_failed_edit(
        &self,
        cx: &mut Cx,
        event_tl_item: Option<EventTimelineItem>,
        timeline_kind: TimelineKind,
        edited_text: String,
    ) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.restore_failed_edit(cx, event_tl_item, timeline_kind, edited_text);
    }

    /// Forwards the result of an edit request to the `EditingPane` widget
    /// within this `RoomInputBar`.
    pub fn handle_edit_result(
        &self,
        cx: &mut Cx,
        timeline_event_item_id: TimelineEventItemId,
        edit_result: Result<(), matrix_sdk_ui::timeline::Error>,
    ) {
        let Some(inner) = self.borrow_mut() else { return };
        inner.editing_pane(cx, ids!(editing_pane))
            .handle_edit_result(cx, timeline_event_item_id, edit_result);
    }

    /// Tells this RoomInputBar which room or thread it is being shown beneath.
    pub fn set_room_context(
        &self,
        cx: &mut Cx,
        room_screen_widget_uid: WidgetUid,
        timeline_kind: TimelineKind,
        room_members: Option<Arc<Vec<RoomMember>>>,
    ) {
        let Some(mut inner) = self.borrow_mut() else { return };
        let room_id = timeline_kind.room_id().to_owned();
        if inner.timeline_kind.as_ref() != Some(&timeline_kind) {
            inner.cancel_dictation(cx);
            // The previous timeline's edit was saved with its state, and must not be shown for this one.
            inner.editing_pane(cx, ids!(editing_pane)).force_reset_hide(cx);
        }
        inner.room_screen_widget_uid = Some(room_screen_widget_uid);
        inner.timeline_kind = Some(timeline_kind);
        inner.mentionable_text_input(cx, ids!(mentionable_text_input))
            .set_room_context(cx, room_id.clone(), room_members.clone());
        inner.mentionable_text_input(cx, ids!(editing_pane.editing_content.edit_text_input))
            .set_room_context(cx, room_id, room_members);
    }

    /// Save a snapshot of the UI state of this `RoomInputBar`.
    ///
    /// If speech recognition is running, this will stop it and release the microphone.
    pub fn save_state(&self) -> RoomInputBarState {
        let Some(inner) = self.borrow() else { return Default::default() };
        let saved = RoomInputBarState {
            was_replying_preview_visible: inner.was_replying_preview_visible,
            replying_to: inner.replying_to.clone(),
            editing_pane_state: inner.child_by_path(ids!(editing_pane)).as_editing_pane().save_state(),
            mentionable_input_state: inner.child_by_path(ids!(input_bar.mentionable_text_input)).as_mentionable_text_input().save_state(),
            upload: inner.child_by_path(ids!(upload_progress_view)).as_upload_progress_view().save_state(),
        };

        inner.child_by_path(ids!(input_bar.mentionable_text_input)).as_mentionable_text_input().release_microphone();
        // Note: the editing pane's `save_state()` call above already released its microphone.

        // Clear the location preview. We don't save this state because the
        // current location might change by the next time the user opens this same room.
        inner.child_by_path(ids!(location_preview)).as_location_preview().clear();
        saved
    }

    /// Restore the UI state of this `RoomInputBar` from the given state snapshot.
    pub fn restore_state(
        &self,
        cx: &mut Cx,
        timeline_kind: TimelineKind,
        saved_state: RoomInputBarState,
        user_power_levels: UserPowerLevels,
        tombstone_info: Option<&SuccessorRoomDetails>,
        not_joined: Option<&RoomMembership>,
        is_encrypted: bool,
    ) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.cancel_dictation(cx);
        let RoomInputBarState {
            was_replying_preview_visible,
            mentionable_input_state,
            replying_to,
            editing_pane_state,
            upload,
        } = saved_state;

        // Note: we do *not* restore the location preview state here; see `save_state()`.

        // 0. Apply this room's encryption state.
        inner.update_encryption_state(cx, is_encrypted);

        // 1. Restore the state of the MentionableTextInput.
        inner.mentionable_text_input(cx, ids!(input_bar.mentionable_text_input))
            .restore_state(cx, mentionable_input_state);
        inner.attachment_buttons(cx, ids!(input_bar.attachment_buttons)).update_for_input(cx, false);

        // 2. Restore the state of the replying-to preview.
        if let Some(replying_to) = replying_to {
            inner.show_replying_to(cx, replying_to, &timeline_kind, false);
        } else {
            inner.clear_replying_to(cx);
        }
        inner.was_replying_preview_visible = was_replying_preview_visible;

        // 3. Restore the state of the editing pane.
        if let Some(editing_pane_state) = editing_pane_state {
            inner.show_editing_pane(
                cx,
                ShowEditingPaneBehavior::RestoreExisting { editing_pane_state },
                timeline_kind.clone(),
            );
        } else {
            inner.editing_pane(cx, ids!(editing_pane)).force_reset_hide(cx);
            inner.on_editing_pane_hidden(cx);
        }

        // 4. Restore the state of the upload progress view.
        inner.child_by_path(ids!(upload_progress_view))
            .as_upload_progress_view()
            .restore_state(cx, upload);

        // 5. Apply this room's tombstone state, the user's power levels, and their membership.
        inner.update_room_state(cx, timeline_kind.room_id(), tombstone_info, user_power_levels, not_joined);
    }

    /// See [`UploadProgressView::on_timeline_reconnected()`].
    pub fn on_timeline_reconnected(&self, cx: &mut Cx) {
        let Some(inner) = self.borrow() else { return };
        inner.child_by_path(ids!(upload_progress_view))
            .as_upload_progress_view()
            .on_timeline_reconnected(cx);
    }

    /// Hides the upload progress view for the given upload attempt.
    pub fn hide_upload_progress(&self, cx: &mut Cx, upload_id: FileUploadAttemptId) {
        let Some(inner) = self.borrow() else { return };
        inner.child_by_path(ids!(upload_progress_view))
            .as_upload_progress_view()
            .hide(cx, upload_id);
    }

    /// Marks the given upload as having been added to the send queue.
    pub fn set_upload_as_queued(
        &self,
        cx: &mut Cx,
        upload_id: FileUploadAttemptId,
        transaction_id: OwnedTransactionId,
        is_encrypted: bool,
    ) {
        let Some(inner) = self.borrow() else { return };
        inner.child_by_path(ids!(upload_progress_view))
            .as_upload_progress_view()
            .set_as_queued(cx, upload_id, transaction_id, is_encrypted);
    }

    /// Updates the upload progress bar for the given upload attempt.
    pub fn set_upload_progress(
        &self,
        cx: &mut Cx,
        upload_id: FileUploadAttemptId,
        current_bytes: usize,
        total_bytes: usize,
    ) {
        let Some(inner) = self.borrow() else { return };
        inner.child_by_path(ids!(upload_progress_view))
            .as_upload_progress_view()
            .set_progress(cx, upload_id, current_bytes, total_bytes);
    }

    /// Shows an upload error, offering a Retry button if `retryable_upload` is `Some`.
    pub fn show_upload_error(
        &self,
        cx: &mut Cx,
        upload_id: FileUploadAttemptId,
        error: &str,
        retryable_upload: Option<AttachmentUpload>,
    ) {
        let Some(inner) = self.borrow() else { return };
        inner.child_by_path(ids!(upload_progress_view))
            .as_upload_progress_view()
            .show_error(cx, upload_id, error, retryable_upload);
    }

    /// Handles a started file upload and clears only the reply captured for this upload.
    pub fn handle_file_upload_started(
        &self,
        cx: &mut Cx,
        upload_id: FileUploadAttemptId,
        file_name: &str,
        in_reply_to: Option<&OwnedEventId>,
        abort_handle: futures_util::future::AbortHandle,
        timeline_kind: TimelineKind,
    ) {
        let Some(mut inner) = self.borrow_mut() else { return };

        inner.child_by_path(ids!(upload_progress_view))
            .as_upload_progress_view()
            .show(cx, upload_id, file_name, abort_handle, timeline_kind);

        if let Some(in_reply_to) = in_reply_to {
            let should_clear_reply = inner
                .replying_to
                .as_ref()
                .and_then(|(event_tl_item, _embedded_event)| event_tl_item.event_id())
                .is_some_and(|current_reply_event_id| current_reply_event_id == in_reply_to);
            if should_clear_reply {
                inner.clear_replying_to(cx);
            }
        }
    }

    /// Returns whether TSP signing is enabled.
    #[cfg(feature = "tsp")]
    pub fn is_tsp_signing_enabled(&self, cx: &mut Cx) -> bool {
        let Some(inner) = self.borrow() else { return false };
        inner.is_tsp_signing_enabled(cx)
    }
}

/// The saved UI state of a `RoomInputBar` widget.
#[derive(Default)]
pub struct RoomInputBarState {
    /// Whether or not the `replying_preview` widget was shown.
    was_replying_preview_visible: bool,
    /// The state of the MentionableTextInput within this input bar.
    mentionable_input_state: MentionableTextInputState,
    /// The event that the user is currently replying to, if any.
    replying_to: Option<(EventTimelineItem, EmbeddedEvent)>,
    /// The state of the `EditingPane`, if any message was being edited.
    editing_pane_state: Option<EditingPaneState>,
    /// The upload in progress for this room, if any,
    /// which keeps working in the background even while the room is hidden.
    upload: Option<UploadState>,
}

/// Defines what to do when showing the `EditingPane` from the `RoomInputBar`.
#[allow(clippy::large_enum_variant)]
enum ShowEditingPaneBehavior {
    /// Show a new edit session, e.g., when first clicking "edit" on a message.
    ShowNew {
        event_tl_item: EventTimelineItem,
    },
    /// Restore the state of an `EditingPane` that already existed, e.g., when
    /// reopening a room that had an `EditingPane` open when it was closed.
    RestoreExisting {
        editing_pane_state: EditingPaneState,
    },
    /// Show an edit that failed to send, so the user gets back what they had typed.
    RestoreFailed {
        event_tl_item: EventTimelineItem,
        edited_text: String,
    },
}
