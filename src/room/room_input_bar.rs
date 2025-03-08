//! RoomInputBar component provides a message input interface with @mention capabilities
//! Supports user mention autocomplete, avatar display, and desktop/mobile layouts

use crate::sliding_sync::{submit_async_request, MatrixRequest};
use crate::shared::mentionable_text_input::{MentionableTextInputAction, MentionableTextInputWidgetExt};
use makepad_widgets::*;
use matrix_sdk::room::RoomMember;
use matrix_sdk::ruma::OwnedRoomId;
use std::sync::{Arc, Mutex};
use crate::room::room_member_manager::{RoomMemberSubscriber, RoomMemberSubscription};


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
        align: {y: 0.5}
        padding: 10.
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
struct RoomInputBarSubscriber {
    // Store a stable identifier instead of widget_uid
    bar_id: String,
    widget_uid: WidgetUid,
}

impl RoomMemberSubscriber for RoomInputBarSubscriber {
    fn on_room_members_updated(&mut self, cx: &mut Cx, room_id: &OwnedRoomId, members: Arc<Vec<RoomMember>>) {
        // Use stable identifier for logging
        log!("RoomInputBarSubscriber({}) received members update for room {}", self.bar_id, room_id);

        // IMPORTANT: This sends the action directly to the global action queue
        cx.action(RoomInputBarAction::RoomMembersUpdated(room_id.clone(), members.clone()));

        // Also send as a widget action for backward compatibility
        cx.widget_action(
            self.widget_uid,
            &Scope::empty().path,
            RoomInputBarAction::RoomMembersUpdated(room_id.clone(), members)
        );
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
    #[rust] member_subscription: Option<RoomMemberSubscription>,
}

impl Widget for RoomInputBar {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);

        if let Event::Actions(actions) = event {
            // Log the widget uid for debugging
            log!("RoomInputBar handle_event - my widget_uid: {:?}", self.widget_uid());

            for action in actions {
                // First, check for a direct RoomMembersUpdated action in the global actions queue
                if let Some(update_action) = action.downcast_ref::<RoomInputBarAction>() {
                    if let RoomInputBarAction::RoomMembersUpdated(room_id, members) = update_action {
                        log!("RoomInputBar received global RoomMembersUpdated action for room {}", room_id);
                        self.handle_members_updated(cx, members.clone());
                    }
                    continue;
                }

                // Check for MentionableTextInputAction::RoomIdChanged action
                if let Some(room_id) = action.downcast_ref::<MentionableTextInputAction>()
                    .and_then(|a| if let MentionableTextInputAction::RoomIdChanged(room_id) = a {
                        Some(room_id)
                    } else {
                        None
                    })
                {
                    log!("Received RoomIdChanged: {}", room_id);

                    // 1. Create subscription
                    self.create_room_subscription(cx, room_id.clone());

                    // 2. Request data after subscription is created
                    submit_async_request(MatrixRequest::GetRoomMembers {
                        room_id: room_id.clone(),
                        memberships: matrix_sdk::RoomMemberships::JOIN,
                        use_cache: true,
                        from_server: true
                    });
                }

                // Check for text input actions
                if let Some(text_action) = action.as_widget_action().cast() {
                    match text_action {
                        MentionableTextInputAction::TextChanged(text) => {
                            cx.widget_action(
                                self.widget_uid(),
                                &scope.path,
                                RoomInputBarAction::MessageChanged(text),
                            );
                        },
                        MentionableTextInputAction::UserMentioned(username) => {
                            cx.widget_action(
                                self.widget_uid(),
                                &scope.path,
                                RoomInputBarAction::UserMentioned(username),
                            );
                        },
                        _ => {}
                    }
                }

                // As a fallback, also check for widget-specific actions
                // Log the action and its widget uid for debugging
                log!("Checking action: {:?}, widget_uid_eq? {}",
                     action,
                     action.as_widget_action().widget_uid_eq(self.widget_uid()).is_some());

                if let Some(widget_action) = action.as_widget_action().widget_uid_eq(self.widget_uid()) {
                    if let Some(RoomInputBarAction::RoomMembersUpdated(room_id, members)) = widget_action.cast() {
                        log!("RoomInputBar ID: {:?} processing update from widget action for room {}",
                             self.widget_uid(), room_id);
                        self.handle_members_updated(cx, members);
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
    /// Returns the current text content of the input bar
    pub fn text(&self) -> String {
        self.view.mentionable_text_input(id!(message_input)).text()
    }

    /// Sets the text content of the input bar
    pub fn set_text(&mut self, cx: &mut Cx, text: &str) {
        let message_input = self.view.mentionable_text_input(id!(message_input));
        message_input.set_text(cx, text);
        self.redraw(cx);
    }

    /// Create room member subscription
    fn create_room_subscription(&mut self, cx: &mut Cx, room_id: OwnedRoomId) {
        // Save room ID
        self.room_id = Some(room_id.clone());

        // Cancel previous subscription (if any)
        self.member_subscription = None;

        // Use stable identifier when creating subscriber
        let bar_id = format!("RoomInputBar-{:?}", self.widget_uid());

        // Create new subscriber and subscribe
        let subscriber = Arc::new(Mutex::new(RoomInputBarSubscriber {
            bar_id,
            widget_uid: self.widget_uid(),
        }));

        log!("Creating subscription, RoomInputBar ID: {:?}", self.widget_uid());

        // Create and save subscription
        self.member_subscription = Some(
            RoomMemberSubscription::new(cx, room_id.clone(), subscriber)
        );

        // Request data after subscription is confirmed
        submit_async_request(MatrixRequest::GetRoomMembers {
            room_id,
            memberships: matrix_sdk::RoomMemberships::JOIN,
            use_cache: true,
            from_server: true
        });
    }

    /// Handle room members update event
    fn handle_members_updated(&mut self, cx: &mut Cx, members: Arc<Vec<RoomMember>>) {
        // Pass room member data to internal MentionableTextInput component
        let message_input = self.view.mentionable_text_input(id!(message_input));

        log!("RoomInputBar::handle_members_updated - passing {} members to MentionableTextInput",
             members.len());

        // Pass data to MentionableTextInput
        message_input.set_room_members(members);
    }
}

impl RoomInputBarRef {
    pub fn set_text(&self, cx: &mut Cx, text: &str) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_text(cx, text);
        }
    }

    pub fn text(&self) -> Option<String> {
        self.borrow().map(|inner| inner.text())
    }

}
