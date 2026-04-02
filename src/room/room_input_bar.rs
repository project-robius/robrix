//! The RoomInputBar widget contains all components related to sending messages/content to a room.
//!
//! The RoomInputBar is capped to a maximum height of 75% of the containing RoomScreen's height.
//!
//! The widgets included in the RoomInputBar are:
//! * a preview of the message the user is replying to.
//! * the location preview (which allows you to send your current location to the room),
//!   and a location card to show the location preview.
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
use ruma::{events::room::message::{LocationMessageEventContent, MessageType, ReplyWithinThread, RoomMessageEventContent}, OwnedRoomId, OwnedUserId};
use crate::{home::{editing_pane::{EditingPaneState, EditingPaneWidgetExt, EditingPaneWidgetRefExt}, location_preview::{LocationPreviewWidgetExt, LocationPreviewWidgetRefExt}, room_screen::{MessageAction, RoomScreenProps, populate_preview_of_timeline_item}, tombstone_footer::{SuccessorRoomDetails, TombstoneFooterWidgetExt}}, i18n::AppLanguage, location::init_location_subscriber, shared::{avatar::AvatarWidgetRefExt, html_or_plaintext::HtmlOrPlaintextWidgetRefExt, mentionable_text_input::MentionableTextInputWidgetExt, popup_list::{PopupKind, enqueue_popup_notification}, styles::*}, sliding_sync::{MatrixRequest, TimelineKind, UserPowerLevels, submit_async_request}, utils};

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*


    mod.widgets.ICO_LOCATION_PERSON = crate_resource("self://resources/icons/location-person.svg")
    mod.widgets.ICO_MENU = crate_resource("self://resources/icons/menu.svg")

    mod.widgets.RoomEmojiButton = mod.widgets.RobrixIconButton {
        spacing: 0
        text: ""
        margin: 0
        padding: Inset{left: 8, right: 8, top: 6, bottom: 6}
        icon_walk: Walk{width: 0, height: 0}
        draw_text +: {
            color: (COLOR_TEXT)
            color_hover: (COLOR_TEXT)
            color_down: (COLOR_TEXT)
            text_style: MESSAGE_TEXT_STYLE { font_size: 15.0 }
        }
        draw_bg +: {
            color: (COLOR_PRIMARY)
            color_hover: #F4F7FC
            color_down: #E8EEF8
            border_size: 1.0
            border_color: (COLOR_SECONDARY)
        }
    }


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
                flow: Down
                padding: 6,
                spacing: 4

                location_card_button := RobrixIconButton {
                    visible: false
                    width: 230
                    align: Align{x: 0.0, y: 0.5}
                    margin: Inset{top: 1, bottom: 1}
                    padding: Inset{left: 10, right: 10, top: 8, bottom: 8}
                    spacing: 8
                    draw_icon +: {
                        svg: (mod.widgets.ICO_LOCATION_PERSON)
                        color: (COLOR_ACTIVE_PRIMARY_DARKER)
                    },
                    draw_bg +: {
                        color: (COLOR_BG_PREVIEW)
                        color_hover: #E0E8F0
                        color_down: #D0D8E8
                        border_size: 1.0
                        border_color: (COLOR_SECONDARY)
                    }
                    draw_text +: {
                        color: (COLOR_TEXT)
                        color_hover: (COLOR_TEXT)
                        color_down: (COLOR_TEXT)
                        text_style: MESSAGE_TEXT_STYLE { font_size: 10.5 }
                    }
                    icon_walk: Walk{width: 20, height: 20}
                    text: "Share your current location",
                }

                emoji_picker_popup := View {
                    visible: false
                    width: Fit
                    height: Fit
                    flow: Right{wrap: true}
                    align: Align{x: 0.0, y: 0.5}
                    margin: Inset{left: 5, top: 1, bottom: 1}
                    padding: Inset{left: 0, right: 0, top: 0, bottom: 0}
                    spacing: 6

                    emoji_smile_button := mod.widgets.RoomEmojiButton { text: "😀" }
                    emoji_joy_button := mod.widgets.RoomEmojiButton { text: "😂" }
                    emoji_thumbsup_button := mod.widgets.RoomEmojiButton { text: "👍" }
                    emoji_heart_button := mod.widgets.RoomEmojiButton { text: "❤️" }
                    emoji_fire_button := mod.widgets.RoomEmojiButton { text: "🔥" }
                    emoji_party_button := mod.widgets.RoomEmojiButton { text: "🎉" }
                    emoji_think_button := mod.widgets.RoomEmojiButton { text: "🤔" }
                    emoji_clap_button := mod.widgets.RoomEmojiButton { text: "👏" }
                }

                input_row := View {
                    width: Fill,
                    height: Fit{max: FitBound.Rel{base: Base.Full, factor: 0.75}}
                    flow: Right
                    // Bottom-align everything to ensure that buttons always stick to the bottom
                    // even when the mentionable_text_input box is very tall.
                    align: Align{y: 1.0},

                    // A checkbox that enables TSP signing for the outgoing message.
                    // If TSP is not enabled, this will be an empty invisible view.
                    tsp_sign_checkbox := TspSignAnycastCheckbox {
                        margin: Inset{bottom: 9, left: 6, right: 0}
                    }

                    emoji_picker_button := RobrixIconButton {
                        margin: Inset{left: 3, right: 1, top: 4, bottom: 4}
                        spacing: 0,
                        draw_icon +: {
                            svg: (ICON_ADD_REACTION)
                            color: (COLOR_ACTIVE_PRIMARY_DARKER)
                        },
                        draw_bg +: {
                            color: (COLOR_BG_PREVIEW)
                            color_hover: #E0E8F0
                            color_down: #D0D8E8
                        }
                        icon_walk: Walk{width: 19, height: 19}
                        text: "",
                    }

                    mentionable_text_input := MentionableTextInput {
                        width: Fill,
                        height: Fit{max: FitBound.Rel{base: Base.Full, factor: 0.75}}
                        margin: Inset {
                            top: 3, // add some space between the top border of the text input and the top border of this row
                            bottom: 5.75, // to line up the middle of the text input with the middle of the buttons
                            left: 3, right: 3 // to give a bit of breathing room between the text input and the buttons on the sides
                        },

                        persistent +: {
                            center +: {
                                text_input := RobrixTextInput {
                                    empty_text: "Write a message (in Markdown) ..."
                                }
                            }
                        }
                    }

                    send_message_button := RobrixPositiveIconButton {
                        visible: false,
                        // Disabled by default; enabled when text is inputted
                        enabled: false,
                        spacing: 0,
                        text: "",
                        margin: 4
                        draw_icon +: { svg: (ICON_SEND) }
                        icon_walk: Walk{width: 21, height: 21},
                    }

                    more_actions_button := RobrixIconButton {
                        spacing: 0,
                        text: "",
                        margin: 4
                        draw_icon +: { svg: (mod.widgets.ICO_MENU) }
                        draw_bg +: {
                            color: (COLOR_ACTIVE_PRIMARY)
                            color_hover: (COLOR_ACTIVE_PRIMARY_DARKER)
                            color_down: #0C5DAA
                        }
                        icon_walk: Walk{width: 19, height: 19},
                    }
                }
            }

            can_not_send_message_notice := SolidView {
                visible: false
                padding: Inset{left: 50, right: 50, top: 20, bottom: 20}
                align: Align{y: 0.5}
                width: Fill, height: Fit

                show_bg: true
                draw_bg.color: (COLOR_SECONDARY)

                text := Label {
                    width: Fill,
                    flow: Flow.Right{wrap: true},
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
#[derive(Script, ScriptHook, Widget)]
pub struct RoomInputBar {
    #[source] source: ScriptObjectRef,
    #[deref] view: View,

    /// Whether the `ReplyingPreview` was visible when the `EditingPane` was shown.
    /// If true, when the `EditingPane` gets hidden, we need to re-show the `ReplyingPreview`.
    #[rust] was_replying_preview_visible: bool,
    /// Info about the message event that the user is currently replying to, if any.
    #[rust] replying_to: Option<(EventTimelineItem, EmbeddedEvent)>,
    /// The most recently selected explicit bot target for this room.
    #[rust] active_target_user_id: Option<OwnedUserId>,
    /// Whether the location card is currently expanded.
    #[rust] is_location_card_expanded: bool,
    /// Whether the emoji picker popup is currently expanded.
    #[rust] is_emoji_picker_expanded: bool,
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
            self.handle_actions(cx, actions, room_screen_props);
        }

        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl RoomInputBar {
    fn resolve_target_user_id(
        &mut self,
        explicit_target_user_id: Option<OwnedUserId>,
        reply_target_user_id: Option<OwnedUserId>,
        fallback_target_user_id: Option<OwnedUserId>,
    ) -> Option<OwnedUserId> {
        if let Some(explicit_target_user_id) = explicit_target_user_id {
            self.active_target_user_id = Some(explicit_target_user_id.clone());
            Some(explicit_target_user_id)
        } else if let Some(reply_target_user_id) = reply_target_user_id {
            self.active_target_user_id = Some(reply_target_user_id.clone());
            Some(reply_target_user_id)
        } else {
            self.active_target_user_id.clone().or(fallback_target_user_id)
        }
    }

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

        // Handle the more actions button being clicked.
        if self.button(cx, ids!(more_actions_button)).clicked(actions) {
            self.is_location_card_expanded = !self.is_location_card_expanded;
            self.button(cx, ids!(location_card_button)).set_visible(cx, self.is_location_card_expanded);
            self.redraw(cx);
        }

        // Handle the emoji picker button being clicked.
        if self.button(cx, ids!(emoji_picker_button)).clicked(actions) {
            self.is_emoji_picker_expanded = !self.is_emoji_picker_expanded;
            self.view.view(cx, ids!(emoji_picker_popup)).set_visible(cx, self.is_emoji_picker_expanded);
            self.redraw(cx);
        }

        let picked_emoji = if self.button(cx, ids!(emoji_smile_button)).clicked(actions) {
            Some("😀")
        } else if self.button(cx, ids!(emoji_joy_button)).clicked(actions) {
            Some("😂")
        } else if self.button(cx, ids!(emoji_thumbsup_button)).clicked(actions) {
            Some("👍")
        } else if self.button(cx, ids!(emoji_heart_button)).clicked(actions) {
            Some("❤️")
        } else if self.button(cx, ids!(emoji_fire_button)).clicked(actions) {
            Some("🔥")
        } else if self.button(cx, ids!(emoji_party_button)).clicked(actions) {
            Some("🎉")
        } else if self.button(cx, ids!(emoji_think_button)).clicked(actions) {
            Some("🤔")
        } else if self.button(cx, ids!(emoji_clap_button)).clicked(actions) {
            Some("👏")
        } else {
            None
        };

        if let Some(emoji) = picked_emoji {
            let mut text = mentionable_text_input.text();
            text.push_str(emoji);
            mentionable_text_input.set_text(cx, &text);
            self.enable_send_message_button(cx, !text.trim().is_empty());
            submit_async_request(MatrixRequest::SendTypingNotice {
                room_id: room_screen_props.timeline_kind.room_id().clone(),
                typing: !text.is_empty(),
            });
            self.is_emoji_picker_expanded = false;
            self.view.view(cx, ids!(emoji_picker_popup)).set_visible(cx, false);
            self.text_input(cx, ids!(input_bar.input_row.mentionable_text_input.text_input)).set_key_focus(cx);
            self.redraw(cx);
        }

        // Handle the location card being clicked.
        if self.button(cx, ids!(location_card_button)).clicked(actions) {
            log!("Location card clicked; requesting current location...");
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
                let reply_target_user_id = self
                    .replying_to
                    .as_ref()
                    .map(|(event_tl_item, _emb)| event_tl_item.sender().to_owned());
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
                        }
                    })
                ).or_else(||
                    room_screen_props.timeline_kind.thread_root_event_id().map(|thread_root_event_id|
                        Reply {
                            event_id: thread_root_event_id.clone(),
                            enforce_thread: EnforceThread::Threaded(ReplyWithinThread::No),
                        }
                    )
                );
                submit_async_request(MatrixRequest::SendMessage {
                    timeline_kind: room_screen_props.timeline_kind.clone(),
                    message,
                    replied_to,
                    target_user_id: self.resolve_target_user_id(
                        None,
                        reply_target_user_id,
                        room_screen_props.bound_bot_user_id.clone(),
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
        if self.button(cx, ids!(send_message_button)).clicked(actions)
            || text_input.returned(actions).is_some_and(|(_, m)| m.is_primary())
        {
            let entered_text = mentionable_text_input.text().trim().to_string();
            if !entered_text.is_empty() {
                if self.try_handle_bot_shortcut(cx, &entered_text, room_screen_props) {
                    self.clear_replying_to(cx);
                    mentionable_text_input.set_text(cx, "");
                    submit_async_request(MatrixRequest::SendTypingNotice {
                        room_id: room_screen_props.timeline_kind.room_id().clone(),
                        typing: false,
                    });
                    self.enable_send_message_button(cx, false);
                    self.redraw(cx);
                    return;
                }
                let reply_target_user_id = self
                    .replying_to
                    .as_ref()
                    .map(|(event_tl_item, _emb)| event_tl_item.sender().to_owned());
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
                        }
                    })
                ).or_else(||
                    room_screen_props.timeline_kind.thread_root_event_id().map(|thread_root_event_id|
                        Reply {
                            event_id: thread_root_event_id.clone(),
                            enforce_thread: EnforceThread::Threaded(ReplyWithinThread::No),
                        }
                    )
                );
                submit_async_request(MatrixRequest::SendMessage {
                    timeline_kind: room_screen_props.timeline_kind.clone(),
                    message,
                    replied_to,
                    target_user_id: self.resolve_target_user_id(
                        None,
                        reply_target_user_id,
                        room_screen_props.bound_bot_user_id.clone(),
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

        // If the EditingPane has been hidden, handle that.
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
            AppLanguage::default(),
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
            self.text_input(cx, ids!(input_bar.input_row.mentionable_text_input.text_input)).set_key_focus(cx);
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
        // We must hide the input_bar while the editing pane is shown,
        // otherwise a very-tall inputted message might show up underneath a shorter editing pane.
        self.view.view(cx, ids!(input_bar)).set_visible(cx, false);

        // Similarly, we must hide the replying preview and location preview,
        // since those are not relevant to editing an existing message,
        // so keeping them visible might confuse the user.
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
        // In `show_editing_pane()` above, we hid the input_bar while the editing pane
        // was being shown, so here we need to make it visible again.
        // Same goes for the replying_preview, if it was previously shown.
        self.view.view(cx, ids!(input_bar)).set_visible(cx, true);
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
            if !self.editing_pane(cx, ids!(editing_pane)).is_currently_shown(cx) {
                input_bar.set_visible(cx, true);
            }
        }
    }

    /// Sets the send_message_button to be shown/enabled and green, or hidden/disabled and gray.
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
            visible: #(enable),
            enabled: #(enable),
            draw_icon.color: #(fg_color),
            draw_bg.color: #(bg_color),
        });
    }

    fn try_handle_bot_shortcut(
        &mut self,
        cx: &mut Cx,
        entered_text: &str,
        room_screen_props: &RoomScreenProps,
    ) -> bool {
        if !(entered_text == "/bot" || entered_text.starts_with("/bot ")) {
            return false;
        }

        let popup_message = if room_screen_props.timeline_kind.thread_root_event_id().is_some() {
            Some((
                "Bot commands are only supported in the main room timeline.",
                PopupKind::Warning,
            ))
        } else if entered_text != "/bot" {
            Some((
                "Only `/bot` is supported right now. Use `/bot` and choose an action from the room panel.",
                PopupKind::Info,
            ))
        } else if !room_screen_props.app_service_enabled {
            Some((
                "Enable App Service in Settings before using /bot.",
                PopupKind::Warning,
            ))
        } else if !room_screen_props.app_service_room_bound {
            Some((
                "Bind BotFather to this room before using /bot.",
                PopupKind::Warning,
            ))
        } else {
            None
        };

        if let Some((message, kind)) = popup_message {
            enqueue_popup_notification(message, kind, Some(4.0));
        } else {
            cx.widget_action(
                room_screen_props.room_screen_widget_uid,
                MessageAction::ToggleAppServiceActions,
            );
        }

        true
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
            active_target_user_id: inner.active_target_user_id.clone(),
            editing_pane_state: inner.child_by_path(ids!(editing_pane)).as_editing_pane().save_state(),
            text_input_state: inner.child_by_path(ids!(input_bar.input_row.mentionable_text_input.text_input)).as_text_input().save_state(),
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
            active_target_user_id,
            editing_pane_state,
        } = saved_state;

        // Note: we do *not* restore the location preview state here; see `save_state()`.

        // 0. Update select views based on user power levels from the RoomScreen (the `TimelineUiState`).
        //    This must happen before we restore the state of the `EditingPane`,
        //    because the call to `show_editing_pane()` might re-update the `input_bar`'s visibility.
        inner.update_user_power_levels(cx, user_power_levels);

        // 1. Restore the state of the TextInput within the MentionableTextInput.
        inner.text_input(cx, ids!(input_bar.input_row.mentionable_text_input.text_input))
            .restore_state(cx, text_input_state);
        let is_text_input_empty = inner.text_input(cx, ids!(input_bar.input_row.mentionable_text_input.text_input))
            .text()
            .is_empty();
        inner.enable_send_message_button(cx, !is_text_input_empty);
        inner.is_location_card_expanded = false;
        inner.button(cx, ids!(location_card_button)).set_visible(cx, false);
        inner.is_emoji_picker_expanded = false;
        inner.view.view(cx, ids!(emoji_picker_popup)).set_visible(cx, false);

        // 2. Restore the state of the replying-to preview.
        if let Some(replying_to) = replying_to {
            inner.show_replying_to(cx, replying_to, &timeline_kind, false);
        } else {
            inner.clear_replying_to(cx);
        }
        inner.was_replying_preview_visible = was_replying_preview_visible;
        inner.active_target_user_id = active_target_user_id;

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
    /// The most recently selected explicit bot target for this room.
    active_target_user_id: Option<OwnedUserId>,
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
