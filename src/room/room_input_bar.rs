//! The RoomInputBar widget contains all components related to sending messages/content to a room.
//!
//! The RoomInputBar is capped to a maximum height of 75% of the containing RoomScreen's height.
//!
//! The widgets included in the RoomInputBar are:
//! * a preview of the message the user is replying to.
//! * the location preview (which allows you to send your current location to the room),
//!   plus a menu item to show the location preview.
//! * a button that opens the RoomScreen-level popup menu for uploads/location.
//! * If TSP is enabled, a checkbox to enable TSP signing for the outgoing message.
//! * A MentionableTextInput, which allows the user to type a message
//!   and mention other users via the `@` key.
//! * A button to send the message.
//! * The editing pane, which is shown when the user is editing a previous message.
//! * A tombstone footer, which is shown if the room has been tombstoned (replaced).
//! * A "cannot-send-message" notice, which is shown if the user cannot send messages to the room.
//!


use std::sync::Arc;
use makepad_widgets::*;
use matrix_sdk::room::RoomMember;
use matrix_sdk::room::reply::{EnforceThread, Reply};
use ruma::events::room::message::AddMentions;
use matrix_sdk_ui::timeline::{EmbeddedEvent, EventTimelineItem, TimelineEventItemId};
use ruma::{events::room::message::{LocationMessageEventContent, MessageType, ReplyWithinThread, RoomMessageEventContent}, OwnedEventId, OwnedRoomId};
use crate::{home::{editing_pane::{EditingPaneState, EditingPaneWidgetExt, EditingPaneWidgetRefExt}, location_preview::{LocationPreviewWidgetExt, LocationPreviewWidgetRefExt}, room_screen::{MessageAction, populate_preview_of_timeline_item}, tombstone_footer::{SuccessorRoomDetails, TombstoneFooterWidgetExt}, upload_progress::UploadProgressViewWidgetRefExt}, location::init_location_subscriber, settings::app_preferences::{AppPreferencesAction, AppPreferencesGlobal}, shared::{avatar::AvatarWidgetRefExt, file_upload_modal::{AttachmentUpload, FileUploadModalAction, FileUploadAttemptId, PreviewPayload, load_file_metadata}, html_or_plaintext::HtmlOrPlaintextWidgetRefExt, mentionable_text_input::{MentionableTextInputWidgetExt, MentionableTextInputWidgetRefExt, MentionableTextInputState}, popup_list::{PopupKind, enqueue_popup_notification}, room_input_popup_menu::RoomInputPopupMenuAction, styles::*}, sliding_sync::{MatrixRequest, TimelineKind, UserPowerLevels, submit_async_request}, utils};

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*


    mod.widgets.RoomInputBar = set_type_default() do #(RoomInputBar::register_widget(vm)) {
        ..mod.widgets.RoundedView

        width: Fill,
        height: Fit{max: FitBound.Rel{base: Base.Full, factor: 0.75}}
        flow: Down,

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

        // The top-most element is a preview of the message that the user is replying to, if any.
        replying_preview := ReplyingPreview { }

        // Below that, display a preview of the current location that a user is about to send.
        location_preview := LocationPreview { }

        // Upload progress view (shown when a file upload is in progress)
        upload_progress_view := UploadProgressView { }

        // Below that, display one of multiple possible views:
        // * the message input bar (buttons and message TextInput).
        // * a notice that the user can't send messages to this room.
        // * if this room was tombstoned, a "footer" view showing the successor room info.
        // * the EditingPane, which slides up as an overlay in front of the other views below.
        overlay_wrapper := View {
            width: Fill,
            height: Fit{max: FitBound.Rel{base: Base.Full, factor: 0.75}}
            flow: Overlay,

            // Below that, display a view that holds the message input bar and send button.
            input_bar := View {
                width: Fill,
                height: Fit{max: FitBound.Rel{base: Base.Full, factor: 0.75}}
                flow: Right
                // Bottom-align everything to ensure that buttons always stick to the bottom
                // even when the mentionable_text_input box is very tall.
                align: Align{y: 1.0},
                padding: 6,

                open_popup_menu_button := RobrixIconButton {
                    padding: 9
                    margin: Inset { top: 4, left: 4, right: 4, bottom: 5}
                    spacing: 0,
                    draw_icon +: {
                        svg: (ICON_ADD)
                        color: (COLOR_ACTIVE_PRIMARY_DARKER)
                    },
                    draw_bg +: {
                        color: (COLOR_BG_PREVIEW)
                        color_hover: #xE0E8F0
                        color_down: #xD0D8E8
                    }
                    icon_walk: Walk{width: 21, height: 21}
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

                    text_input := RobrixTextInput {
                        empty_text: "Write a message (in Markdown) ..."
                        is_multiline: true,
                    }
                }

                send_message_button := RobrixPositiveIconButton {
                    // Disabled by default; enabled when text is inputted
                    enabled: false,
                    padding: 8
                    margin: Inset { top: 4, left: 4, right: 4, bottom: 5}
                    spacing: 0,
                    draw_icon +: { svg: (ICON_SEND) }
                    icon_walk: Walk{width: 23, height: 23},
                }
            }

            can_not_send_message_notice := SolidView {
                visible: false
                padding: 20
                align: Align{x: 0.5, y: 0.5}
                width: Fill, height: Fit

                show_bg: true
                draw_bg.color: (COLOR_SECONDARY)

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

            tombstone_footer := TombstoneFooter { }

            editing_pane := EditingPane { }
        }
    }
}

/// Main component for message input with @mention support
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
    /// Whether the currently-displayed room is encrypted, used to style the send button.
    #[rust] is_encrypted: bool,
    /// Whether the send button is currently enabled (the message input is non-empty).
    #[rust] is_send_enabled: bool,
    /// The room or thread that this RoomInputBar is currently within.
    #[rust] timeline_kind: Option<TimelineKind>,
    /// The widget UID of the RoomScreen containing this RoomInputBar.
    #[rust] room_screen_widget_uid: Option<WidgetUid>,
}

impl ScriptHook for RoomInputBar {
    fn on_after_new(&mut self, vm: &mut ScriptVm) {
        vm.with_cx_mut(|cx| {
            let send_on_enter = cx.global::<AppPreferencesGlobal>().0.send_on_enter;
            self.mentionable_text_input(cx, ids!(mentionable_text_input))
                .text_input_ref()
                .set_submit_on_enter(send_on_enter);
        });
    }
}

impl Widget for RoomInputBar {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        match event.hits(cx, self.view.view(cx, ids!(replying_preview.reply_preview_content)).area()) {
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

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        // Shrink the input_bar's height as the editing pane slides in,
        // and grow it back as the editing pane slides out.
        // slide=1.0 → editing pane hidden → input_bar at full Fit height.
        // slide=0.0 → editing pane shown → input_bar at zero height.
        let slide = self.editing_pane(cx, ids!(editing_pane)).slide();
        let input_bar = self.view.view(cx, ids!(input_bar));

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

        self.view.draw_walk(cx, scope, walk)
    }

    /// Gives key focus to the inner message text input.
    fn set_key_focus(&self, cx: &mut Cx) {
        self.child_by_path(ids!(input_bar.mentionable_text_input))
            .as_mentionable_text_input()
            .set_key_focus(cx);
    }
}

impl RoomInputBar {
    fn handle_actions(
        &mut self,
        cx: &mut Cx,
        actions: &Actions,
    ) {
        let mentionable_text_input = self.mentionable_text_input(cx, ids!(mentionable_text_input));
        let text_input = mentionable_text_input.text_input_ref();

        for action in actions {
            // Handle changes to the `send_on_enter` preference.
            if let Some(AppPreferencesAction::SendOnEnterChanged(v)) = action.downcast_ref() {
                text_input.set_submit_on_enter(*v);
                continue;
            }
        }

        // Clear the replying-to preview pane if the "cancel reply" button was clicked
        // or if the `Escape` key was pressed within the message input box.
        if self.button(cx, ids!(cancel_reply_button)).clicked(actions)
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

        let open_popup_menu_button = self.button(cx, ids!(open_popup_menu_button));
        if open_popup_menu_button.clicked(actions) {
            let button_rect = open_popup_menu_button.area().rect(cx);
            cx.widget_action(
                room_screen_widget_uid,
                RoomInputPopupMenuAction::Show { button_rect },
            );
        }

        // Handle the send location button being clicked.
        if self.button(cx, ids!(location_preview.send_location_button)).clicked(actions) {
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

        // Handle the send message button being clicked, or a `Returned` action
        // from the message text input, which already respects the user's app setting.
        if self.button(cx, ids!(send_message_button)).clicked(actions)
            || text_input.returned(actions).is_some()
        {
            let entered_text = mentionable_text_input.text().trim().to_string();
            if !entered_text.is_empty() {
                let message = mentionable_text_input.create_message_with_mentions(&entered_text);
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
            }
        }

        // If the user starts/stops typing in the message input box,
        // send a typing notice to the room and update the send_message_button state.
        let is_text_input_empty = if let Some(new_text) = text_input.changed(actions) {
            let is_empty = new_text.is_empty();
            submit_async_request(MatrixRequest::SendTypingNotice {
                room_id: timeline_kind.room_id().clone(),
                typing: !is_empty,
            });
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
        if is_text_input_empty {
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
        if self.view.editing_pane(cx, ids!(editing_pane)).was_hidden(actions) {
            self.on_editing_pane_hidden(cx);
        }
    }

    fn show_current_location_preview(&mut self, cx: &mut Cx) {
        if let Err(_e) = init_location_subscriber(cx) {
            error!("Failed to initialize location subscriber");
            enqueue_popup_notification(
                "Failed to initialize location services.",
                PopupKind::Error,
                None,
            );
        }
        self.view.location_preview(cx, ids!(location_preview)).show();
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
            .avatar(cx, ids!(reply_preview_content.reply_preview_avatar))
            .set_avatar_and_get_username(
                cx,
                timeline_kind,
                replying_to.0.sender(),
                Some(replying_to.0.sender_profile()),
                replying_to.0.event_id(),
                true,
            );

        replying_preview
            .label(cx, ids!(reply_preview_content.reply_preview_username))
            .set_text(cx, replying_preview_username.as_str());

        populate_preview_of_timeline_item(
            cx,
            &replying_preview.html_or_plaintext(cx, ids!(reply_preview_content.reply_preview_body)),
            replying_to.0.content(),
            replying_to.0.sender(),
            &replying_preview_username,
        );

        replying_preview.set_visible(cx, true);
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
        };

        self.redraw(cx);
    }

    /// This should be invoked after the EditingPane has been fully hidden.
    fn on_editing_pane_hidden(&mut self, cx: &mut Cx) {
        // Restore the replying_preview.
        if self.was_replying_preview_visible && self.replying_to.is_some() {
            self.view.view(cx, ids!(replying_preview)).set_visible(cx, true);
        }
        self.redraw(cx);
        // We don't need to do anything with the editing pane itself here,
        // because it has already been hidden by the time this function gets called.
    } 

    /// Updates (populates and shows or hides) this room's tombstone footer
    /// based on the given successor room details.
    fn update_tombstone_footer(
        &mut self,
        cx: &mut Cx,
        tombstoned_room_id: &OwnedRoomId,
        successor_room_details: Option<&SuccessorRoomDetails>,
    ) {
        let tombstone_footer = self.tombstone_footer(cx, ids!(tombstone_footer));
        let input_bar = self.view(cx, ids!(input_bar));

        if let Some(srd) = successor_room_details {
            tombstone_footer.show(cx, tombstoned_room_id, srd);
            input_bar.set_visible(cx, false);
        } else {
            tombstone_footer.hide(cx);
            input_bar.set_visible(cx, true);
        }
    }

    /// Enables or disables (grays out) the send_message_button.
    ///
    /// When enabled, the button color is set based on the room's encryption state.
    fn enable_send_message_button(&mut self, cx: &mut Cx, enable: bool) {
        self.is_send_enabled = enable;
        let mut send_message_button = self.view.button(cx, ids!(send_message_button));
        let (fg_color, bg_color) = if !enable {
            (COLOR_FG_DISABLED, COLOR_BG_DISABLED)
        } else if self.is_encrypted {
            (COLOR_PRIMARY, COLOR_ACTIVE_PRIMARY)
        } else {
            (COLOR_FG_ACCEPT_GREEN, COLOR_BG_ACCEPT_GREEN)
        };
        script_apply_eval!(cx, send_message_button, {
            enabled: #(enable),
            draw_icon.color: #(fg_color),
            draw_bg.color: #(bg_color),
        });
    }

    /// Updates the visibility of select views based on the user's new power levels.
    ///
    /// This will show/hide the `input_bar` and the `can_not_send_message_notice` views.
    fn update_user_power_levels(
        &mut self,
        cx: &mut Cx,
        user_power_levels: UserPowerLevels,
    ) {
        let can_send = user_power_levels.can_send_message();
        self.view.view(cx, ids!(input_bar)).set_visible(cx, can_send);
        self.view.view(cx, ids!(can_not_send_message_notice)).set_visible(cx, !can_send);

        // Forward the updated power levels to the two mentionable text inputs within this widget.
        let can_notify = user_power_levels.can_notify_room();
        self.mentionable_text_input(cx, ids!(mentionable_text_input))
            .set_can_notify_room(cx, can_notify);
        self.mentionable_text_input(cx, ids!(editing_pane.editing_content.edit_text_input))
            .set_can_notify_room(cx, can_notify);
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
        } else {
            apply_positive_button_style(cx, &mut send_message_button);
            script_apply_eval!(cx, send_message_button, {
                draw_icon.svg: mod.widgets.ICON_SEND_UNENCRYPTED,
            });
            empty_text = "Send unencrypted message…";
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

    /// Shows the native file picker dialog to select a file to be uploaded.
    fn open_file_picker(
        &mut self,
        cx: &mut Cx,
        timeline_kind: TimelineKind,
    ) {
        if self.view.view(cx, ids!(upload_progress_view)).visible() {
            enqueue_popup_notification(
                "Finish or cancel the current upload before starting another one.",
                PopupKind::Warning,
                Some(7.0),
            );
            return;
        }

        let on_picked = self.upload_picker_callback(cx, timeline_kind);
        Self::handle_picker_launch_result(
            robius_file_picker::FileDialog::new().pick_file(on_picked)
        );
    }

    /// Shows the native media picker dialog to select a photo or video to be uploaded.
    fn open_photo_video_picker(
        &mut self,
        cx: &mut Cx,
        timeline_kind: TimelineKind,
    ) {
        if self.view.view(cx, ids!(upload_progress_view)).visible() {
            enqueue_popup_notification(
                "Finish or cancel the current upload before starting another one.",
                PopupKind::Warning,
                Some(7.0),
            );
            return;
        }

        let on_picked = self.upload_picker_callback(cx, timeline_kind);
        Self::handle_picker_launch_result(
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
        move |result: robius_file_picker::Result<Option<robius_file_picker::PickedFile>>| {
            match result {
                Ok(Some(picked)) => match picked.into_local_file() {
                    Ok(local_file) => {
                        match load_file_metadata(
                            local_file,
                            timeline_kind,
                            in_reply_to,
                            #[cfg(feature = "tsp")]
                            sign_with_tsp,
                        ) {
                            Ok((upload, preview_source, preview_id)) => {
                                // Show the preview modal instantly, and then re-use this bg thread
                                // to read the file and generate the preview.
                                Cx::post_action(FileUploadModalAction::Show { upload, preview_id });
                                let preview = preview_source.build();
                                Cx::post_action(FileUploadModalAction::PreviewReady {
                                    preview_id,
                                    preview: PreviewPayload::new(preview),
                                });
                            }
                            Err(e) => enqueue_popup_notification(e, PopupKind::Error, None),
                        }
                    }
                    Err(e) => enqueue_popup_notification(
                        format!("Failed to read selected file: {e}"),
                        PopupKind::Error,
                        None,
                    ),
                },
                // User dismissed the picker, do nothing.
                Ok(None) => {}
                Err(err) => enqueue_popup_notification(
                    format!("Error selecting a file: {err}"),
                    PopupKind::Error,
                    None,
                ),
            }
        }
    }

    fn handle_picker_launch_result(result: robius_file_picker::Result<()>) {
        match result {
            Ok(()) => {}
            Err(robius_file_picker::Error::AlreadyOpen) => {
                enqueue_popup_notification(
                    "A file picker is already open.",
                    PopupKind::Error,
                    Some(4.0),
                );
            }
            Err(err) => {
                makepad_widgets::error!("Failed to launch file picker: {err}");
                enqueue_popup_notification(
                    format!("Failed to open file picker: {err}"),
                    PopupKind::Error,
                    None,
                );
            }
        }
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

    /// Updates the visibility of select views based on the user's new power levels.
    ///
    /// This will show/hide the `input_bar` and the `can_not_send_message_notice` views.
    pub fn update_user_power_levels(
        &self,
        cx: &mut Cx,
        user_power_levels: UserPowerLevels,
    ) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.update_user_power_levels(cx, user_power_levels);
    }

    /// Updates this room's tombstone footer based on the given `tombstone_state`.
    pub fn update_tombstone_footer(
        &self,
        cx: &mut Cx,
        tombstoned_room_id: &OwnedRoomId,
        successor_room_details: Option<&SuccessorRoomDetails>,
    ) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.update_tombstone_footer(cx, tombstoned_room_id, successor_room_details);
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
        inner.room_screen_widget_uid = Some(room_screen_widget_uid);
        inner.timeline_kind = Some(timeline_kind);
        inner.mentionable_text_input(cx, ids!(mentionable_text_input))
            .set_room_context(cx, room_id.clone(), room_members.clone());
        inner.mentionable_text_input(cx, ids!(editing_pane.editing_content.edit_text_input))
            .set_room_context(cx, room_id, room_members);
    }

    /// Save a snapshot of the UI state of this `RoomInputBar`.
    pub fn save_state(&self) -> RoomInputBarState {
        let Some(inner) = self.borrow() else { return Default::default() };
        // Clear the location preview. We don't save this state because the
        // current location might change by the next time the user opens this same room.
        inner.child_by_path(ids!(location_preview)).as_location_preview().clear();
        RoomInputBarState {
            was_replying_preview_visible: inner.was_replying_preview_visible,
            replying_to: inner.replying_to.clone(),
            editing_pane_state: inner.child_by_path(ids!(editing_pane)).as_editing_pane().save_state(),
            mentionable_input_state: inner.child_by_path(ids!(input_bar.mentionable_text_input)).as_mentionable_text_input().save_state(),
        }
    }

    /// Restore the UI state of this `RoomInputBar` from the given state snapshot.
    pub fn restore_state(
        &self,
        cx: &mut Cx,
        timeline_kind: TimelineKind,
        saved_state: RoomInputBarState,
        user_power_levels: UserPowerLevels,
        tombstone_info: Option<&SuccessorRoomDetails>,
        is_encrypted: bool,
    ) {
        let Some(mut inner) = self.borrow_mut() else { return };
        let RoomInputBarState {
            was_replying_preview_visible,
            mentionable_input_state,
            replying_to,
            editing_pane_state,
        } = saved_state;

        // Note: we do *not* restore the location preview state here; see `save_state()`.

        // 0. Update select views based on user power levels from the RoomScreen (the `TimelineUiState`).
        //    This must happen before we restore the state of the `EditingPane`,
        //    because the call to `show_editing_pane()` might re-update the `input_bar`'s visibility.
        inner.update_user_power_levels(cx, user_power_levels);
        inner.update_encryption_state(cx, is_encrypted);

        // 1. Restore the state of the MentionableTextInput.
        inner.mentionable_text_input(cx, ids!(input_bar.mentionable_text_input))
            .restore_state(cx, mentionable_input_state);

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

        // 4. Restore the state of the tombstone footer.
        //    This depends on the `EditingPane` state, so it must be done after Step 3.
        inner.update_tombstone_footer(cx, timeline_kind.room_id(), tombstone_info);
    }

    /// Hides the upload progress view for the given upload attempt.
    pub fn hide_upload_progress(&self, cx: &mut Cx, upload_id: FileUploadAttemptId) {
        let Some(inner) = self.borrow() else { return };
        inner.child_by_path(ids!(upload_progress_view))
            .as_upload_progress_view()
            .hide(cx, upload_id);
    }

    /// Updates the upload progress.
    pub fn set_upload_progress(&self, cx: &mut Cx, upload_id: FileUploadAttemptId, current: u64, total: u64) {
        let Some(inner) = self.borrow() else { return };
        inner.child_by_path(ids!(upload_progress_view))
            .as_upload_progress_view()
            .set_progress(cx, upload_id, current, total);
    }

    /// Shows an upload error with retry option.
    pub fn show_upload_error(&self, cx: &mut Cx, upload_id: FileUploadAttemptId, error: &str, upload: AttachmentUpload, retryable: bool) {
        let Some(inner) = self.borrow() else { return };
        inner.child_by_path(ids!(upload_progress_view))
            .as_upload_progress_view()
            .show_error(cx, upload_id, error, upload, retryable);
    }

    /// Handles a started file upload and clears only the reply captured for this upload.
    pub fn handle_file_upload_started(
        &self,
        cx: &mut Cx,
        upload_id: FileUploadAttemptId,
        file_name: &str,
        in_reply_to: Option<&OwnedEventId>,
        abort_handle: futures_util::future::AbortHandle,
    ) {
        let Some(mut inner) = self.borrow_mut() else { return };

        inner.child_by_path(ids!(upload_progress_view))
            .as_upload_progress_view()
            .show(cx, upload_id, file_name, abort_handle);

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
}
