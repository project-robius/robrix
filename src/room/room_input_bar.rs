//! The RoomInputBar widget contains all components related to sending messages/content to a room.
//!
//! The RoomInputBar is capped to a maximum height of 75% of the containing RoomScreen's height.
//!
//! The widgets included in the RoomInputBar are:
//! * a preview of the message the user is replying to.
//! * the location preview (which allows you to send your current location to the room),
//!   and a button to show the location preview.
//! * If TSP is enabled, a checkbox to enable TSP signing for the outgoing message.
//! * A MentionableTextInput, which allows the user to type a message
//!   and mention other users via the `@` key.
//! * A button to send the message.
//! * The editing pane, which is shown when the user is editing a previous message.
//! * A tombstone footer, which is shown if the room has been tombstoned (replaced).
//! * A "cannot-send-message" notice, which is shown if the user cannot send messages to the room.
//!


#[cfg(not(any(target_os = "ios", target_os = "android")))]
use bytesize::ByteSize;
use makepad_widgets::*;
use matrix_sdk::room::reply::{EnforceThread, Reply};
use ruma::events::room::message::AddMentions;
use matrix_sdk_ui::timeline::{EmbeddedEvent, EventTimelineItem, TimelineEventItemId};
use ruma::{events::room::message::{LocationMessageEventContent, MessageType, ReplyWithinThread, RoomMessageEventContent}, OwnedRoomId};
#[cfg(not(any(target_os = "ios", target_os = "android")))]
use std::sync::Arc;
use crate::{home::{editing_pane::{EditingPaneState, EditingPaneWidgetExt, EditingPaneWidgetRefExt}, location_preview::{LocationPreviewWidgetExt, LocationPreviewWidgetRefExt}, room_screen::{MessageAction, RoomScreenProps, populate_preview_of_timeline_item}, tombstone_footer::{SuccessorRoomDetails, TombstoneFooterWidgetExt}, upload_progress::UploadProgressViewWidgetRefExt}, location::init_location_subscriber, settings::app_preferences::{AppPreferencesGlobal, AppPreferencesAction}, shared::{avatar::AvatarWidgetRefExt, file_upload_modal::{FileData, FileLoadedData, FilePreviewerAction, TimelineUpdateSender}, html_or_plaintext::HtmlOrPlaintextWidgetRefExt, mentionable_text_input::MentionableTextInputWidgetExt, popup_list::{PopupKind, enqueue_popup_notification}, styles::*}, sliding_sync::{MatrixRequest, TimelineKind, UserPowerLevels, submit_async_request}, utils};
#[cfg(not(any(target_os = "ios", target_os = "android")))]
use crate::shared::file_upload_modal::FilePreviewerMetaData;
// Check file size limit (100 MB - homeservers typically cap at 50-100 MB)
#[cfg(not(any(target_os = "ios", target_os = "android")))]
const MAX_FILE_SIZE: u64 = 100 * 1024 * 1024; // 100 MB

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*


    mod.widgets.ICO_LOCATION_PERSON = crate_resource("self://resources/icons/location-person.svg")


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

                // Attachment button for uploading files/images
                send_attachment_button := RobrixIconButton {
                    margin: 4
                    spacing: 0,
                    draw_icon +: {
                        svg: (ICON_ADD_ATTACHMENT)
                        color: (COLOR_ACTIVE_PRIMARY_DARKER)
                    },
                    draw_bg +: {
                        color: (COLOR_BG_PREVIEW)
                        color_hover: #E0E8F0
                        color_down: #D0D8E8
                    }
                    icon_walk: Walk{width: 21, height: 21}
                    text: "",
                }

                location_button := RobrixIconButton {
                    margin: 4
                    spacing: 0,
                    draw_icon +: {
                        svg: (mod.widgets.ICO_LOCATION_PERSON)
                        color: (COLOR_ACTIVE_PRIMARY_DARKER)
                    },
                    draw_bg +: {
                        color: (COLOR_BG_PREVIEW)
                        color_hover: #E0E8F0
                        color_down: #D0D8E8
                    }
                    icon_walk: Walk{width: 23, height: 23, margin: Inset{bottom: -1}}
                    text: "",
                }

                // A checkbox that enables TSP signing for the outgoing message.
                // If TSP is not enabled, this will be an empty invisible view.
                tsp_sign_checkbox := TspSignAnycastCheckbox {
                    margin: Inset{bottom: 9, left: 6, right: 0}
                }

                mentionable_text_input := MentionableTextInput {
                    width: Fill,
                    height: Fit
                    margin: Inset {
                        top: 3, // add some space between the top border of the text input and the top border of the room input bar
                        bottom: 5.75, // to line up the middle of the text input with the middle of the buttons
                        left: 3, right: 3 // to give a bit of breathing room between the text input and the buttons on the sides
                    },

                    persistent +: {
                        center +: {
                            text_input := RobrixTextInput {
                                empty_text: "Write a message (in Markdown) ..."
                                is_multiline: true,
                            }
                        }
                    }
                }

                send_message_button := RobrixPositiveIconButton {
                    // Disabled by default; enabled when text is inputted
                    enabled: false,
                    spacing: 0,
                    text: "",
                    margin: 4
                    draw_icon +: { svg: (ICON_SEND) }
                    icon_walk: Walk{width: 21, height: 21},
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
    /// The pending file load operation, if any. Contains the receiver channel
    /// for receiving the loaded file data from a background thread.
    #[rust] pending_file_load: Option<crate::shared::file_upload_modal::FileLoadReceiver>,
    /// The timeline update sender captured when a file picker is opened, to ensure the file
    /// is uploaded to the correct room/thread even if the user switches rooms.
    #[rust] pending_file_update_sender: Option<TimelineUpdateSender>,
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
        let room_screen_props = scope
            .props
            .get::<RoomScreenProps>()
            .expect("BUG: RoomScreenProps should be available in Scope::props for RoomInputBar");

        match event.hits(cx, self.view.view(cx, ids!(replying_preview.reply_preview_content)).area()) {
            // If the hit occurred on the replying message preview, jump to it.
            Hit::FingerUp(fe) if fe.is_over && fe.is_primary_hit() && fe.was_tap() => {
                if let Some(event_id) = self.replying_to.as_ref()
                    .and_then(|(event_tl_item, _)| event_tl_item.event_id().map(ToOwned::to_owned))
                {
                    cx.widget_action(
                        room_screen_props.room_screen_widget_uid,
                        MessageAction::JumpToEvent(event_id),
                    );
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
            // Handle changes to the `send_on_enter` preference.
            for action in actions {
                if let Some(AppPreferencesAction::SendOnEnterChanged(v)) = action.downcast_ref() {
                    self.mentionable_text_input(cx, ids!(mentionable_text_input))
                        .text_input_ref()
                        .set_submit_on_enter(*v);
                }
            }

            self.handle_actions(cx, actions, room_screen_props);
        }

        // Handle signal events for pending file loads from background threads
        if let Event::Signal = event {
            if let Some(receiver) = &self.pending_file_load {
                let mut remove_receiver = false;
                match receiver.try_recv() {
                    Ok(Some(loaded_data)) => {
                        // Convert FileLoadedData to FileData for the modal
                        let file_data = convert_loaded_data_to_file_data(loaded_data);
                        // Use the captured sender from when the file picker was opened
                        if let Some(timeline_update_sender) = self.pending_file_update_sender.take() {
                            Cx::post_action(FilePreviewerAction::Show { file_data, timeline_update_sender });
                        }
                        remove_receiver = true;
                    }
                    Ok(None) => {
                        // File loading failed
                        self.pending_file_update_sender = None;
                        remove_receiver = true;
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => {
                        // Still waiting for data
                    }
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        // Channel disconnected
                        self.pending_file_update_sender = None;
                        remove_receiver = true;
                    }
                }
                if remove_receiver {
                    self.pending_file_load = None;
                    self.redraw(cx);
                }
            }
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
}

impl RoomInputBar {
    fn handle_actions(
        &mut self,
        cx: &mut Cx,
        actions: &Actions,
        room_screen_props: &RoomScreenProps,
    ) {
        let mentionable_text_input = self.mentionable_text_input(cx, ids!(mentionable_text_input));
        let text_input = mentionable_text_input.text_input_ref();

        // Clear the replying-to preview pane if the "cancel reply" button was clicked
        // or if the `Escape` key was pressed within the message input box.
        if self.button(cx, ids!(cancel_reply_button)).clicked(actions)
            || text_input.escaped(actions)
        {
            self.clear_replying_to(cx);
            self.redraw(cx);
        }

        // Handle the add attachment button being clicked.
        if self.button(cx, ids!(send_attachment_button)).clicked(actions) {
            log!("Add attachment button clicked; opening file picker...");
            self.open_file_picker(cx, room_screen_props.timeline_update_sender.clone());
        }

        // Handle the add location button being clicked.
        if self.button(cx, ids!(location_button)).clicked(actions) {
            log!("Add location button clicked; requesting current location...");
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
                        let enforce_thread = if room_screen_props.timeline_kind.thread_root_event_id().is_some() {
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
                    room_screen_props.timeline_kind.thread_root_event_id().map(|thread_root_event_id|
                        Reply {
                            event_id: thread_root_event_id.clone(),
                            enforce_thread: EnforceThread::Threaded(ReplyWithinThread::No),
                            add_mentions: AddMentions::No,
                        }
                    )
                );
                submit_async_request(MatrixRequest::SendMessage {
                    timeline_kind: room_screen_props.timeline_kind.clone(),
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
        // from the message text input. The text input only emits `Returned`
        // for the key combination chosen by the user in App Settings (plus
        // Cmd/Ctrl+Enter, which always submits).
        if self.button(cx, ids!(send_message_button)).clicked(actions)
            || text_input.returned(actions).is_some()
        {
            let entered_text = mentionable_text_input.text().trim().to_string();
            if !entered_text.is_empty() {
                let message = mentionable_text_input.create_message_with_mentions(&entered_text);
                let replied_to = self.replying_to.take().and_then(|(event_tl_item, _emb)|
                    event_tl_item.event_id().map(|event_id| {
                        let enforce_thread = if room_screen_props.timeline_kind.thread_root_event_id().is_some() {
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
                    room_screen_props.timeline_kind.thread_root_event_id().map(|thread_root_event_id|
                        Reply {
                            event_id: thread_root_event_id.clone(),
                            enforce_thread: EnforceThread::Threaded(ReplyWithinThread::No),
                            add_mentions: AddMentions::No,
                        }
                    )
                );
                submit_async_request(MatrixRequest::SendMessage {
                    timeline_kind: room_screen_props.timeline_kind.clone(),
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
                room_id: room_screen_props.timeline_kind.room_id().clone(),
                typing: !is_empty,
            });
            is_empty
        } else {
            text_input.text().is_empty()
        };
        self.enable_send_message_button(cx, !is_text_input_empty);

        // Handle the user pressing the up arrow in an empty message input box
        // to edit their latest sent message.
        if is_text_input_empty {
            if let Some(KeyEvent {
                key_code: KeyCode::ArrowUp,
                modifiers: KeyModifiers { shift: false, control: false, alt: false, logo: false },
                ..
            }) = text_input.key_down_unhandled(actions) {
                cx.widget_action(
                    room_screen_props.room_screen_widget_uid, 
                    MessageAction::EditLatest,
                );
            }
        }

        // When the hide animation fully completes, restore the replying preview.
        if self.view.editing_pane(cx, ids!(editing_pane)).was_hidden(actions) {
            self.on_editing_pane_hidden(cx);
        }
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

    /// Sets the send_message_button to be enabled and green, or disabled and gray.
    ///
    /// This should be called to update the button state when the message TextInput content changes.
    fn enable_send_message_button(&mut self, cx: &mut Cx, enable: bool) {
        let mut send_message_button = self.view.button(cx, ids!(send_message_button));
        let (fg_color, bg_color) = if enable {
            (COLOR_FG_ACCEPT_GREEN, COLOR_BG_ACCEPT_GREEN)
        } else {
            (COLOR_FG_DISABLED, COLOR_BG_DISABLED)
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
    }

    /// Returns true if the TSP signing checkbox is checked, false otherwise.
    ///
    /// If TSP is not enabled, this will always return false.
    #[cfg(feature = "tsp")]
    fn is_tsp_signing_enabled(&self, cx: &mut Cx) -> bool {
        self.view.check_box(cx, ids!(tsp_sign_checkbox)).active(cx)
    }

    /// Opens the native file picker dialog to select a file for upload.
    ///
    /// The timeline update sender is captured at this moment to ensure the file is uploaded
    /// to the correct room/thread, even if the user switches rooms while the modal is open.
    #[cfg(not(any(target_os = "ios", target_os = "android")))]
    fn open_file_picker(&mut self, cx: &mut Cx, timeline_update_sender: Option<TimelineUpdateSender>) {
        // Get the timeline update sender - it's passed from RoomScreenProps
        let Some(timeline_update_sender) = timeline_update_sender else {
            enqueue_popup_notification(
                "Cannot upload file: timeline not available.",
                PopupKind::Error,
                None,
            );
            return;
        };

        // Run file dialog on main thread (required for non-windowed environments)
        let dialog = rfd::FileDialog::new()
            .set_title("Select file to upload")
            .add_filter("All files", &["*"])
            .add_filter("Images", &["png", "jpg", "jpeg", "gif", "webp", "bmp"])
            .add_filter("Documents", &["pdf", "doc", "docx", "txt", "rtf"]);

        if let Some(selected_file_path) = dialog.pick_file() {
            // Store the sender for when the file finishes loading
            self.pending_file_update_sender = Some(timeline_update_sender);
            // Get file metadata
            let file_size = match std::fs::metadata(&selected_file_path) {
                Ok(metadata) => metadata.len(),
                Err(e) => {
                    makepad_widgets::error!("Failed to read file metadata: {e}");
                    enqueue_popup_notification(
                        format!("Unable to access file: {e}"),
                        PopupKind::Error,
                        None,
                    );
                    return;
                }
            };

            // Check for empty files
            if file_size == 0 {
                enqueue_popup_notification("Cannot upload empty file", PopupKind::Error, None);
                return;
            }

            if file_size > MAX_FILE_SIZE {
                enqueue_popup_notification(
                    format!(
                        "File too large ({}). Maximum upload size is 100 MB.",
                        ByteSize::b(file_size)
                    ),
                    PopupKind::Error,
                    None,
                );
                return;
            }

            // Detect the MIME type from the file extension
            let mime = mime_guess::from_path(&selected_file_path)
                .first_or_octet_stream();

            // Create channel for receiving loaded file data
            let (sender, receiver) = std::sync::mpsc::channel();
            self.pending_file_load = Some(receiver);

            // Spawn background thread to read file and generate thumbnail (for images)
            let path_clone = selected_file_path.clone();
            let mime_clone = mime.clone();
            cx.spawn_thread(move || {
                // Read the file data in the background thread (not on UI thread)
                let file_data = match std::fs::read(&path_clone) {
                    Ok(data) => data,
                    Err(e) => {
                        makepad_widgets::error!("Failed to read file: {e}");
                        if sender.send(None).is_err() {
                            makepad_widgets::error!("Failed to send error to UI: receiver dropped");
                        }
                        SignalToUI::set_ui_signal();
                        return;
                    }
                };

                // Wrap file data in Arc to avoid copying when passed through channels
                let file_data = Arc::new(file_data);

                let loaded_data = FileLoadedData {
                    metadata: FilePreviewerMetaData {
                        mime: mime_clone,
                        file_size,
                        file_path: path_clone,
                    },
                    data: file_data,
                };

                if sender.send(Some(loaded_data)).is_err() {
                    makepad_widgets::error!("Failed to send file data to UI: receiver dropped");
                }
                SignalToUI::set_ui_signal();
            });
        }
    }

    /// Shows a "not supported" message on mobile platforms.
    #[cfg(any(target_os = "ios", target_os = "android"))]
    fn open_file_picker(&mut self, _cx: &mut Cx, _timeline_update_sender: Option<TimelineUpdateSender>) {
        enqueue_popup_notification(
            "File uploads are not yet supported on this platform.",
            PopupKind::Error,
            None,
        );
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
            text_input_state: inner.child_by_path(ids!(input_bar.mentionable_text_input.text_input)).as_text_input().save_state(),
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
    ) {
        let Some(mut inner) = self.borrow_mut() else { return };
        let RoomInputBarState {
            was_replying_preview_visible,
            text_input_state,
            replying_to,
            editing_pane_state,
        } = saved_state;

        // Note: we do *not* restore the location preview state here; see `save_state()`.

        // 0. Update select views based on user power levels from the RoomScreen (the `TimelineUiState`).
        //    This must happen before we restore the state of the `EditingPane`,
        //    because the call to `show_editing_pane()` might re-update the `input_bar`'s visibility.
        inner.update_user_power_levels(cx, user_power_levels);

        // 1. Restore the state of the TextInput within the MentionableTextInput.
        inner.text_input(cx, ids!(input_bar.mentionable_text_input.text_input))
            .restore_state(cx, text_input_state);

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

    /// Shows the upload progress view for a file upload.
    pub fn show_upload_progress(&self, cx: &mut Cx, file_name: &str) {
        let Some(inner) = self.borrow() else { return };
        inner.child_by_path(ids!(upload_progress_view))
            .as_upload_progress_view()
            .show(cx, file_name);
    }

    /// Hides the upload progress view.
    pub fn hide_upload_progress(&self, cx: &mut Cx) {
        let Some(inner) = self.borrow() else { return };
        inner.child_by_path(ids!(upload_progress_view))
            .as_upload_progress_view()
            .hide(cx);
    }

    /// Updates the upload progress.
    pub fn set_upload_progress(&self, cx: &mut Cx, current: u64, total: u64) {
        let Some(inner) = self.borrow() else { return };
        inner.child_by_path(ids!(upload_progress_view))
            .as_upload_progress_view()
            .set_progress(cx, current, total);
    }

    /// Sets the abort handle for the current upload.
    pub fn set_upload_abort_handle(&self, handle: tokio::task::AbortHandle) {
        let Some(inner) = self.borrow_mut() else { return };
        inner.child_by_path(ids!(upload_progress_view))
            .as_upload_progress_view()
            .set_abort_handle(handle);
    }

    /// Shows an upload error with retry option.
    pub fn show_upload_error(&self, cx: &mut Cx, error: &str, file_data: FileData) {
        let Some(inner) = self.borrow() else { return };
        inner.child_by_path(ids!(upload_progress_view))
            .as_upload_progress_view()
            .show_error(cx, error, file_data);
    }

    /// Handles a confirmed file upload from the file upload modal.
    ///
    /// This method:
    /// - Shows the upload progress view
    /// - Gets and clears any "replying to" state
    /// - Returns the reply metadata (None if not replying or widget unavailable)
    pub fn handle_file_upload_confirmed(&self, cx: &mut Cx, file_name: &str) -> Option<Reply> {
        let mut inner = self.borrow_mut()?;

        // Get the reply metadata if replying to a message
        let replied_to = inner
            .replying_to
            .take()
            .and_then(|(event_tl_item, _embedded_event)| {
                event_tl_item.event_id().map(|event_id| Reply {
                    event_id: event_id.to_owned(),
                    enforce_thread: EnforceThread::MaybeThreaded,
                    add_mentions: AddMentions::Yes
                })
            });

        // Show the upload progress view
        inner.child_by_path(ids!(upload_progress_view))
            .as_upload_progress_view()
            .show(cx, file_name);

        // Clear the replying-to state
        inner.clear_replying_to(cx);

        replied_to
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
    /// The state of the `TextInput` within the `mentionable_text_input`.
    text_input_state: TextInputState,
    /// The event that the user is currently replying to, if any.
    replying_to: Option<(EventTimelineItem, EmbeddedEvent)>,
    /// The state of the `EditingPane`, if any message was being edited.
    editing_pane_state: Option<EditingPaneState>,
}

/// Defines what to do when showing the `EditingPane` from the `RoomInputBar`.
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

/// Converts `FileLoadedData` from background thread to `FileData` for the modal.
///
/// The file data has already been read in the background thread,
/// so this is a cheap conversion that doesn't block the UI thread.
fn convert_loaded_data_to_file_data(loaded: FileLoadedData) -> FileData {
    let name = loaded.metadata.file_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    FileData {
        path: loaded.metadata.file_path,
        name,
        mime_type: loaded.metadata.mime.to_string(),
        data: loaded.data,
        size: loaded.metadata.file_size,
        thumbnail: None, // Thumbnail generation is not currently implemented
    }
}
