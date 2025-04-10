//! RoomInputBar component provides a message input interface with @mention capabilities
//! Supports user mention autocomplete, avatar display, and desktop/mobile layouts

use crate::room::room_member_manager::{RoomMemberSubscriber, RoomMemberSubscription};
use crate::shared::mentionable_text_input::{
    MentionableTextInputAction, MentionableTextInputWidgetExt,
};
use crate::sliding_sync::{MatrixRequest, submit_async_request};
use makepad_widgets::*;
use matrix_sdk::room::RoomMember;
use matrix_sdk::ruma::OwnedRoomId;
use std::sync::{Arc, Mutex};

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;
    use crate::shared::styles::*;
    use crate::shared::icon_button::*;
    use crate::shared::mentionable_text_input::MentionableTextInput;

    ICO_LOCATION_PERSON = dep("crate://self/resources/icons/location-person.svg")
    ICO_SEND = dep("crate://self/resources/icon_send.svg")

    pub RoomInputBar = {{RoomInputBar}} {
        width: Fill,
        height: Fit
        flow: Right
        // Bottom-align everything to ensure that buttons always stick to the bottom
        // even when the message_input box is very tall.
        align: {y: 1.0},
        padding: 8.
        show_bg: true
        draw_bg: {color: (COLOR_PRIMARY)}

        location_button = <IconButton> {
            draw_icon: {svg_file: (ICO_LOCATION_PERSON)},
            icon_walk: {width: 22.0, height: Fit, margin: {left: 0, right: 5}},
            text: "",
        }

        message_input = <MentionableTextInput> {
            width: Fill,
            height: Fit
            margin: 0
            align: {y: 0.5}

            persistent = {
                center = {
                    text_input = {
                        empty_message: "Write a message (in Markdown) ..."
                    }
                }
            }
        }

        send_message_button = <IconButton> {
            draw_icon: {svg_file: (ICO_SEND)},
            icon_walk: {width: 18.0, height: Fit},
        }
    }
}

/// Actions emitted by the RoomInputBar component
#[allow(dead_code)]
#[derive(Clone, Debug, DefaultNone)]
pub enum RoomInputBarAction {
    /// Triggered when message content changes
    MessageChanged(String),
    /// Triggered when a user is specifically mentioned
    UserMentioned(String),
    /// Room members data has been updated
    RoomMembersUpdated(OwnedRoomId, Arc<Vec<RoomMember>>),
    /// Default empty action
    None,
}

/// Create subscriber adapter for RoomInputBar
/// Create subscriber adapter for RoomInputBar
struct RoomInputBarSubscriber {
    widget_uid: WidgetUid,
    current_room_id: Option<OwnedRoomId>,
}

/// Implement `RoomMemberSubscriber` trait, receive member update notifications
impl RoomMemberSubscriber for RoomInputBarSubscriber {
    fn on_room_members_updated(
        &mut self, cx: &mut Cx, room_id: &OwnedRoomId, members: Arc<Vec<RoomMember>>,
    ) {
        if let Some(current_room_id) = &self.current_room_id {
            if current_room_id == room_id {
                // Use stable identifier for logging
                log!(
                    "RoomInputBarSubscriber({:?}) received members update for room {}",
                    self.widget_uid,
                    room_id
                );

                // cx.action(RoomInputBarAction::RoomMembersUpdated(room_id.clone(), members.clone()));
                cx.widget_action(
                    self.widget_uid,
                    &Scope::empty().path,
                    RoomInputBarAction::RoomMembersUpdated(room_id.clone(), members.clone())
                );
            }else{
                log!("Ignoring update for different room {} (current: {})", room_id, current_room_id);
            }
        }
    }
}

/// Main component for message input with @mention support
#[derive(Live, LiveHook, Widget)]
pub struct RoomInputBar {
    /// Base view properties
    #[deref]
    view: View,
    /// Current Matrix room ID
    #[rust]
    room_id: Option<OwnedRoomId>,
    /// Room member subscription
    #[rust]
    member_subscription: Option<RoomMemberSubscription>,
}

impl Widget for RoomInputBar {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);

        if let Event::Actions(actions) = event {
            for action in actions {

                if let Some(widget_action) = action.as_widget_action().widget_uid_eq(self.widget_uid())  {
                    log!("Found widget action for my widget_uid: {:?}", self.widget_uid());
                    log!("Widget action type: {}", std::any::type_name_of_val(&widget_action));

                    if let Some(update_action) = widget_action.downcast_ref::<RoomInputBarAction>() {
                        if let RoomInputBarAction::RoomMembersUpdated(room_id, members) = update_action
                        {
                            log!(
                                "RoomInputBar received RoomInputBarAction RoomMembersUpdated action for room {}",
                                room_id
                            );
                            self.handle_members_updated(members.clone());
                        }
                        continue;
                    }
                }

                // Check for MentionableTextInputAction::RoomIdChanged action
                if let Some(room_id) =
                    action.downcast_ref::<MentionableTextInputAction>().and_then(|a| {
                        if let MentionableTextInputAction::RoomIdChanged(room_id) = a {
                            Some(room_id)
                        } else {
                            None
                        }
                    })
                {
                    // Create subscription
                    self.create_room_subscription(cx, room_id.clone());
                }

                // // Check for text input actions
                // if let Some(text_action) = action.as_widget_action().cast() {
                //     match text_action {
                //         MentionableTextInputAction::TextChanged(text) => {
                //             cx.widget_action(
                //                 self.widget_uid(),
                //                 &scope.path,
                //                 RoomInputBarAction::MessageChanged(text),
                //             );
                //         },
                //         MentionableTextInputAction::UserMentioned(username) => {
                //             cx.widget_action(
                //                 self.widget_uid(),
                //                 &scope.path,
                //                 RoomInputBarAction::UserMentioned(username),
                //             );
                //         },
                //         _ => {},
                //     }
                // }

                if let Some(widget_action) =
                    action.as_widget_action().widget_uid_eq(self.widget_uid())
                {
                    if let Some(RoomInputBarAction::RoomMembersUpdated(_room_id, members)) =
                        widget_action.cast()
                    {
                        self.handle_members_updated(members);
                    }
                }
            }
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl RoomInputBar {
    /// Create room member subscription
    fn create_room_subscription(&mut self, cx: &mut Cx, room_id: OwnedRoomId) {
        // Save room ID
        self.room_id = Some(room_id.clone());

        // Cancel previous subscription (if any)
        self.member_subscription = None;

        // Create new subscriber and subscribe
        let subscriber = Arc::new(Mutex::new(RoomInputBarSubscriber {
            widget_uid: self.widget_uid(),
            current_room_id: Some(room_id.clone()),
        }));

        log!("Creating subscription, RoomInputBar ID: {:?}", self.widget_uid());

        // Create and save subscription
        self.member_subscription =
            Some(RoomMemberSubscription::new(cx, room_id.clone(), subscriber));

        // Request data after subscription is confirmed
        submit_async_request(MatrixRequest::GetRoomMembers {
            room_id,
            memberships: matrix_sdk::RoomMemberships::JOIN,
            local_only: false,
        });
    }

    /// Handle room members update event
    fn handle_members_updated(&mut self, members: Arc<Vec<RoomMember>>) {
        if let Some(current_room_id) = &self.room_id {
            let message_input = self.view.mentionable_text_input(id!(message_input));

            if message_input.get_room_id().as_ref() == Some(current_room_id) {
                log!("RoomInputBar: Updating {} members to MentionableTextInput (Room {})",
                        members.len(), current_room_id);
                // Pass data to MentionableTextInput
                message_input.set_room_members(members);
            }
        }
    }
}
