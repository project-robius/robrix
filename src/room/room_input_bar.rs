//! The RoomInputBar widget contains all components related to sending messages/content to a room.
//!
//! The RoomInputBar is capped to a maximum height of 62.5% of the containing RoomScreen's height.
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

use makepad_widgets::*;
use matrix_sdk::room::reply::{EnforceThread, Reply};
use matrix_sdk_ui::timeline::{EmbeddedEvent, EventTimelineItem, TimelineEventItemId};
use ruma::{events::room::message::{LocationMessageEventContent, MessageType, RoomMessageEventContent}, OwnedRoomId};
use crate::{home::{editing_pane::{EditingPaneState, EditingPaneWidgetExt}, location_preview::LocationPreviewWidgetExt, room_screen::{populate_preview_of_timeline_item, MessageAction, RoomScreenProps}, tombstone_footer::{SuccessorRoomDetails, TombstoneFooterWidgetExt}}, location::init_location_subscriber, shared::{avatar::AvatarWidgetRefExt, html_or_plaintext::HtmlOrPlaintextWidgetRefExt, mentionable_text_input::MentionableTextInputWidgetExt, popup_list::{enqueue_popup_notification, PopupItem, PopupKind}, styles::*}, sliding_sync::{submit_async_request, MatrixRequest, UserPowerLevels}, utils};

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;
    use crate::shared::styles::*;
    use crate::shared::icon_button::*;
    use crate::shared::avatar::Avatar;
    use crate::shared::html_or_plaintext::*;
    use crate::shared::mentionable_text_input::MentionableTextInput;
    use crate::room::reply_preview::*;
    use crate::home::location_preview::*;
    use crate::home::tombstone_footer::TombstoneFooter;
    use crate::home::editing_pane::*;

    use link::tsp_link::TspSignAnycastCheckbox;

    ICO_LOCATION_PERSON = dep("crate://self/resources/icons/location-person.svg")


    pub RoomInputBar = {{RoomInputBar}}<RoundedView> {
        width: Fill,
        height: Fit { max: Rel { base: Full, factor: 0.625 } }
        flow: Down,

        // These margins are a hack to make the borders of the RoomInputBar
        // line up with the boundaries of its parent widgets.
        // This only works if the border_color is the same as its parents,
        // which is currently `COLOR_SECONDARY`.
        margin: {left: -4, right: -4, bottom: -4 }

        show_bg: true,
        draw_bg: {
            color: (COLOR_PRIMARY)
            border_radius: 5.0,
            border_color: (COLOR_SECONDARY),
            border_size: 2.0
            // uniform shadow_color: #0006
            // shadow_radius: 0.0,
            // shadow_offset: vec2(0.0,0.0)
        }

        // The top-most element is a preview of the message that the user is replying to, if any.
        replying_preview = <ReplyingPreview> { }

        // Below that, display a preview of the current location that a user is about to send.
        location_preview = <LocationPreview> { }

        // Below that, display one of multiple possible views:
        // * the message input bar (buttons and message TextInput).
        // * a notice that the user can't send messages to this room.
        // * if this room was tombstoned, a "footer" view showing the successor room info.
        // * the EditingPane, which slides up as an overlay in front of the other views below.
        overlay_wrapper = <View> {
            width: Fill,
            height: Fit { max: Rel { base: Full, factor: 0.625 } }
            flow: Overlay,

            // Below that, display a view that holds the message input bar and send button.
            input_bar = <View> {
                width: Fill,
                height: Fit { max: Rel { base: Full, factor: 0.625 } }
                flow: Right
                // Bottom-align everything to ensure that buttons always stick to the bottom
                // even when the mentionable_text_input box is very tall.
                align: {y: 1.0},
                padding: 6,

                location_button = <RobrixIconButton> {
                    margin: {left: 4}
                    spacing: 0,
                    draw_icon: {
                        svg_file: (ICO_LOCATION_PERSON)
                        color: (COLOR_ACTIVE_PRIMARY_DARKER)
                    },
                    draw_bg: {
                        color: (COLOR_LOCATION_PREVIEW_BG),
                    }
                    icon_walk: {width: Fit, height: 23, margin: {bottom: -1}}
                    text: "",
                }

                // A checkbox that enables TSP signing for the outgoing message.
                // If TSP is not enabled, this will be an empty invisible view.
                tsp_sign_checkbox = <TspSignAnycastCheckbox> {
                    margin: {bottom: 9, left: 6, right: 0}
                }

                mentionable_text_input = <MentionableTextInput> {
                    width: Fill,
                    height: Fit { max: Rel { base: Full, factor: 0.625 } }
                    margin: { top: 5, bottom: 12, left: 1, right: 1 },

                    persistent = {
                        center = {
                            text_input = {
                                empty_text: "Write a message (in Markdown) ..."
                            }
                        }
                    }
                }

                send_message_button = <RobrixIconButton> {
                    // Disabled by default; enabled when text is inputted
                    enabled: false,
                    spacing: 0,
                    margin: {right: 4}
                    draw_icon: {
                        svg_file: (ICON_SEND),
                        color: (COLOR_FG_DISABLED),
                    }
                    icon_walk: {width: Fit, height: 21},
                    draw_bg: {
                        color: (COLOR_BG_DISABLED),
                    }
                }
            }

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
                    text: "You don't have permission to post to this room.",
                }
            }

            tombstone_footer = <TombstoneFooter> { }

            editing_pane = <EditingPane> { }
        }
    }
}

/// Main component for message input with @mention support
#[derive(Live, LiveHook, Widget)]
pub struct RoomInputBar {
    #[deref] view: View,

    /// Whether the `ReplyingPreview` was visible when the `EditingPane` was shown.
    /// If true, when the `EditingPane` gets hidden, we need to re-show the `ReplyingPreview`.
    #[rust] was_replying_preview_visible: bool,
    /// Info about the message event that the user is currently replying to, if any.
    #[rust] replying_to: Option<(EventTimelineItem, EmbeddedEvent)>,
}

impl Widget for RoomInputBar {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        let room_screen_props = scope
            .props
            .get::<RoomScreenProps>()
            .expect("BUG: RoomScreenProps should be available in Scope::props for RoomInputBar");

        match event.hits(cx, self.view.view(ids!(replying_preview.reply_preview_content)).area()) {
            // If the hit occurred on the replying message preview, jump to it.
            Hit::FingerUp(fe) if fe.is_over && fe.is_primary_hit() && fe.was_tap() => {
                if let Some(event_id) = self.replying_to.as_ref()
                    .and_then(|(event_tl_item, _)| event_tl_item.event_id().map(ToOwned::to_owned))
                {
                    cx.widget_action(
                        room_screen_props.room_screen_widget_uid,
                        &scope.path,
                        MessageAction::JumpToEvent(event_id),
                    );
                } else {
                    enqueue_popup_notification(PopupItem {
                        message: String::from("BUG: couldn't find the message you're replying to."),
                        kind: PopupKind::Error,
                        auto_dismissal_duration: None
                    });
                }
            }
            _ => {}
        }

        if let Event::Actions(actions) = event {
            self.handle_actions(cx, actions, room_screen_props);
        }

        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
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
        let mentionable_text_input = self.mentionable_text_input(ids!(mentionable_text_input));
        let text_input = mentionable_text_input.text_input_ref();

        // Clear the replying-to preview pane if the "cancel reply" button was clicked
        // or if the `Escape` key was pressed within the message input box.
        if self.button(ids!(cancel_reply_button)).clicked(actions)
            || text_input.escaped(actions)
        {
            self.clear_replying_to(cx);
            self.redraw(cx);
        }

        // Handle the add location button being clicked.
        if self.button(ids!(location_button)).clicked(actions) {
            log!("Add location button clicked; requesting current location...");
            if let Err(_e) = init_location_subscriber(cx) {
                error!("Failed to initialize location subscriber");
                enqueue_popup_notification(PopupItem {
                    message: String::from("Failed to initialize location services."),
                    kind: PopupKind::Error,
                    auto_dismissal_duration: None
                });
            }
            self.view.location_preview(ids!(location_preview)).show();
            self.redraw(cx);
        }

        // Handle the send location button being clicked.
        if self.button(ids!(location_preview.send_location_button)).clicked(actions) {
            let location_preview = self.location_preview(ids!(location_preview));
            if let Some((coords, _system_time_opt)) = location_preview.get_current_data() {
                let geo_uri = format!("{}{},{}", utils::GEO_URI_SCHEME, coords.latitude, coords.longitude);
                let message = RoomMessageEventContent::new(
                    MessageType::Location(
                        LocationMessageEventContent::new(geo_uri.clone(), geo_uri)
                    )
                );
                submit_async_request(MatrixRequest::SendMessage {
                    room_id: room_screen_props.room_name_id.room_id().clone(),
                    message,
                    replied_to: self.replying_to.take().and_then(|(event_tl_item, _emb)|
                        event_tl_item.event_id().map(|event_id|
                            Reply {
                                event_id: event_id.to_owned(),
                                enforce_thread: EnforceThread::MaybeThreaded,
                            }
                        )
                    ),
                    #[cfg(feature = "tsp")]
                    sign_with_tsp: self.is_tsp_signing_enabled(cx),
                });

                self.clear_replying_to(cx);
                location_preview.clear();
                location_preview.redraw(cx);
            }
        }

        // Handle the send message button being clicked or Cmd/Ctrl + Return being pressed.
        if self.button(ids!(send_message_button)).clicked(actions)
            || text_input.returned(actions).is_some_and(|(_, m)| m.is_primary())
        {
            let entered_text = mentionable_text_input.text().trim().to_string();
            if !entered_text.is_empty() {
                let message = mentionable_text_input.create_message_with_mentions(&entered_text);
                submit_async_request(MatrixRequest::SendMessage {
                    room_id: room_screen_props.room_name_id.room_id().clone(),
                    message,
                    replied_to: self.replying_to.take().and_then(|(event_tl_item, _emb)|
                        event_tl_item.event_id().map(|event_id|
                            Reply {
                                event_id: event_id.to_owned(),
                                enforce_thread: EnforceThread::MaybeThreaded,
                            }
                        )
                    ),
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
                room_id: room_screen_props.room_name_id.room_id().clone(),
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
                    &HeapLiveIdPath::default(),
                    MessageAction::EditLatest,
                );
            }
        }

        // If the EditingPane has been hidden, handle that.
        if self.view.editing_pane(ids!(editing_pane)).was_hidden(actions) {
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
        room_id: &OwnedRoomId,
        grab_key_focus: bool,
    ) {
        // When the user clicks the reply button next to a message, we need to:
        // 1. Populate and show the ReplyingPreview, of course.
        let replying_preview = self.view(ids!(replying_preview));
        let (replying_preview_username, _) = replying_preview
            .avatar(ids!(reply_preview_content.reply_preview_avatar))
            .set_avatar_and_get_username(
                cx,
                room_id,
                replying_to.0.sender(),
                Some(replying_to.0.sender_profile()),
                replying_to.0.event_id(),
                true,
            );

        replying_preview
            .label(ids!(reply_preview_content.reply_preview_username))
            .set_text(cx, replying_preview_username.as_str());

        populate_preview_of_timeline_item(
            cx,
            &replying_preview.html_or_plaintext(ids!(reply_preview_content.reply_preview_body)),
            replying_to.0.content(),
            replying_to.0.sender(),
            &replying_preview_username,
        );

        replying_preview.set_visible(cx, true);
        self.replying_to = Some(replying_to);

        // 2. Hide other views that are irrelevant to a reply, e.g.,
        //    the `EditingPane` would improperly cover up the ReplyPreview.
        self.editing_pane(ids!(editing_pane)).force_reset_hide(cx);
        self.on_editing_pane_hidden(cx);
        // 3. Automatically focus the keyboard on the message input box
        //    so that the user can immediately start typing their reply
        //    without having to manually click on the message input box.
        if grab_key_focus {
            self.text_input(ids!(input_bar.mentionable_text_input.text_input)).set_key_focus(cx);
        }
        self.redraw(cx);
    }

    /// Clears (and makes invisible) the preview of the message
    /// that the user is currently replying to.
    fn clear_replying_to(&mut self, cx: &mut Cx) {
        self.view(ids!(replying_preview)).set_visible(cx, false);
        self.replying_to = None;
    }

    /// Shows the editing pane to allow the user to edit the given event.
    fn show_editing_pane(
        &mut self,
        cx: &mut Cx,
        behavior: ShowEditingPaneBehavior,
        room_id: OwnedRoomId,
    ) {
        // We must hide the input_bar while the editing pane is shown,
        // otherwise a very-tall inputted message might show up underneath a shorter editing pane.
        self.view.view(ids!(input_bar)).set_visible(cx, false);

        // Similarly, we must hide the replying preview and location preview,
        // since those are not relevant to editing an existing message,
        // so keeping them visible might confuse the user.
        let replying_preview = self.view.view(ids!(replying_preview));
        self.was_replying_preview_visible = replying_preview.visible();
        replying_preview.set_visible(cx, false);
        self.view.location_preview(ids!(location_preview)).clear();

        let editing_pane = self.view.editing_pane(ids!(editing_pane));
        match behavior {
            ShowEditingPaneBehavior::ShowNew { event_tl_item } => {
                editing_pane.show(cx, event_tl_item, room_id);
            }
            ShowEditingPaneBehavior::RestoreExisting { editing_pane_state } => {
                editing_pane.restore_state(cx, editing_pane_state, room_id);
            }
        };

        self.redraw(cx);
    }

    /// This should be invoked after the EditingPane has been fully hidden.
    fn on_editing_pane_hidden(&mut self, cx: &mut Cx) {
        // In `show_editing_pane()` above, we hid the input_bar while the editing pane
        // was being shown, so here we need to make it visible again.
        // Same goes for the replying_preview, if it was previously shown.
        self.view.view(ids!(input_bar)).set_visible(cx, true);
        if self.was_replying_preview_visible && self.replying_to.is_some() {
            self.view.view(ids!(replying_preview)).set_visible(cx, true);
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
        let tombstone_footer = self.tombstone_footer(ids!(tombstone_footer));
        let input_bar = self.view(ids!(input_bar));

        if let Some(srd) = successor_room_details {
            tombstone_footer.show(cx, tombstoned_room_id, srd);
            input_bar.set_visible(cx, false);
        } else {
            tombstone_footer.hide(cx);
            if !self.editing_pane(ids!(editing_pane)).is_currently_shown(cx) {
                input_bar.set_visible(cx, true);
            }
        }
    }

    /// Sets the send_message_button to be enabled and green, or disabled and gray.
    ///
    /// This should be called to update the button state when the message TextInput content changes.
    fn enable_send_message_button(&mut self, cx: &mut Cx, enable: bool) {
        let send_message_button = self.view.button(ids!(send_message_button));
        let (fg_color, bg_color) = if enable {
            (COLOR_FG_ACCEPT_GREEN, COLOR_BG_ACCEPT_GREEN)
        } else {
            (COLOR_FG_DISABLED, COLOR_BG_DISABLED)
        };
        send_message_button.apply_over(cx, live! {
            enabled: (enable),
            draw_icon: {
                color: (fg_color),
                // color_hover: (fg_color),
            }
            draw_bg: {
                color: (bg_color),
            }
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
        self.view.view(ids!(input_bar)).set_visible(cx, can_send);
        self.view.view(ids!(can_not_send_message_notice)).set_visible(cx, !can_send);
    }

    /// Returns true if the TSP signing checkbox is checked, false otherwise.
    ///
    /// If TSP is not enabled, this will always return false.
    #[cfg(feature = "tsp")]
    fn is_tsp_signing_enabled(&self, cx: &mut Cx) -> bool {
        self.view.check_box(ids!(tsp_sign_checkbox)).active(cx)
    }
}

impl RoomInputBarRef {
    /// Shows a preview of the given event that the user is currently replying to
    /// above the message input bar.
    pub fn show_replying_to(
        &self,
        cx: &mut Cx,
        replying_to: (EventTimelineItem, EmbeddedEvent),
        room_id: &OwnedRoomId,
    ) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.show_replying_to(cx, replying_to, room_id, true);
    }

    /// Shows the editing pane to allow the user to edit the given event.
    pub fn show_editing_pane(
        &self,
        cx: &mut Cx,
        event_tl_item: EventTimelineItem,
        room_id: OwnedRoomId,
    ) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.show_editing_pane(
            cx,
            ShowEditingPaneBehavior::ShowNew { event_tl_item },
            room_id,
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
        inner.editing_pane(ids!(editing_pane))
            .handle_edit_result(cx, timeline_event_item_id, edit_result);
    }

    /// Save a snapshot of the UI state of this `RoomInputBar`.
    pub fn save_state(&self) -> RoomInputBarState {
        let Some(inner) = self.borrow() else { return Default::default() };
        // Clear the location preview. We don't save this state because the
        // current location might change by the next time the user opens this same room.
        inner.location_preview(ids!(location_preview)).clear();
        RoomInputBarState {
            was_replying_preview_visible: inner.was_replying_preview_visible,
            replying_to: inner.replying_to.clone(),
            editing_pane_state: inner.editing_pane(ids!(editing_pane)).save_state(),
            text_input_state: inner.text_input(ids!(input_bar.mentionable_text_input.text_input)).save_state(),
        }
    }

    /// Restore the UI state of this `RoomInputBar` from the given state snapshot.
    pub fn restore_state(
        &self,
        cx: &mut Cx,
        room_id: &OwnedRoomId,
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
        inner.text_input(ids!(input_bar.mentionable_text_input.text_input))
            .restore_state(cx, text_input_state);

        // 2. Restore the state of the replying-to preview.
        if let Some(replying_to) = replying_to {
            inner.show_replying_to(cx, replying_to, room_id, false);
        } else {
            inner.clear_replying_to(cx);
        }
        inner.was_replying_preview_visible = was_replying_preview_visible;

        // 3. Restore the state of the editing pane.
        if let Some(editing_pane_state) = editing_pane_state {
            inner.show_editing_pane(
                cx,
                ShowEditingPaneBehavior::RestoreExisting { editing_pane_state },
                room_id.clone(),
            );
        } else {
            inner.editing_pane(ids!(editing_pane)).force_reset_hide(cx);
            inner.on_editing_pane_hidden(cx);
        }

        // 4. Restore the state of the tombstone footer.
        //    This depends on the `EditingPane` state, so it must be done after Step 3.
        inner.update_tombstone_footer(cx, room_id, tombstone_info);
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
