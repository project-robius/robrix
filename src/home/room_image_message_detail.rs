//! A room image message detail widget that displays a user's avatar, username, and message date.

use makepad_widgets::*;
use matrix_sdk::ruma::{MilliSecondsSinceUnixEpoch, OwnedRoomId, OwnedUserId};
use matrix_sdk_ui::timeline::{Profile, TimelineDetails};

use crate::{
    shared::{
        avatar::AvatarWidgetExt,
        timestamp::TimestampWidgetExt,
    },
    utils::unix_time_millis_to_datetime,
};
use matrix_sdk::ruma::OwnedEventId;

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::styles::*;
    use crate::shared::avatar::Avatar;
    use crate::shared::timestamp::Timestamp;

    pub RoomImageMessageDetail = {{RoomImageMessageDetail}} {
        width: Fill, height: Fill
        flow: Right

        top_left_container = <View> {
            width: 150, height: Fit,
            flow: Right,
            spacing: 10,
            margin: {left: 20, top: 20}
            align: { y: 0.5}

            avatar = <Avatar> {
                width: 40,
                height: 40,
            }

            content = <View> {
                width: Fill, height: Fit,
                flow: Down,
                spacing: 4,
                align: { x: 0.0 }

                username = <Label> {
                    width: Fill, height: Fit,
                    draw_text: {
                        text_style: <REGULAR_TEXT>{font_size: 14},
                        color: (COLOR_TEXT)
                    }
                    text: ""
                }
                timestamp_view = <View> {
                    width: Fill, height: Fit
                    timestamp = <Timestamp> {
                        width: Fill, height: Fit,
                        margin: { left: 5}
                    }
                }
                
            }
        }
        image_name_and_size = <Label> {
            width: Fill, height: Fit,
            margin: {top: 40}
            align: { x: 0.5, }
            draw_text: {
                text_style: <REGULAR_TEXT>{font_size: 14},
                color: (COLOR_TEXT),
                wrap: Word
            }
        }
        empty_right_container = <View> {
            // equal width as the top-left container to keep the image name centered.
            width: 150, height: Fit,
        }
    }
}

#[derive(Live, LiveHook, Widget)]
pub struct RoomImageMessageDetail {
    #[deref] view: View,    
    #[rust] sender: Option<OwnedUserId>,
    #[rust] sender_profile: Option<TimelineDetails<Profile>>,
    #[rust] room_id: Option<OwnedRoomId>,
    #[rust] event_id: Option<OwnedEventId>,
    #[rust] avatar_drawn: bool,
}

/// Convert bytes to human-readable file size format
fn format_file_size(bytes: i32) -> String {
    if bytes < 0 {
        return "Unknown size".to_string();
    }
    
    let bytes = bytes as u64;
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    
    if bytes == 0 {
        return "0 B".to_string();
    }
    
    let mut size = bytes as f64;
    let mut unit_index = 0;
    
    while size >= 1024.0 && unit_index < UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }
    
    if unit_index == 0 {
        format!("{} {}", bytes, UNITS[unit_index])
    } else {
        format!("{:.1} {}", size, UNITS[unit_index])
    }
}

impl Widget for RoomImageMessageDetail {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        self.match_event(cx, event);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        if !self.avatar_drawn {
            let avatar_ref = self.avatar(id!(top_left_container.avatar));
            let Some(room_id) = &self.room_id else { return DrawStep::done() };
            let Some(sender) = &self.sender else { return DrawStep::done() };
            let (username, avatar_drawn) = avatar_ref.set_avatar_and_get_username(cx, room_id, sender, self.sender_profile.as_ref(), self.event_id.as_deref());
            self.label(id!(top_left_container.username)).set_text(cx, &username);
            self.avatar_drawn = avatar_drawn;
        }
        self.view.draw_walk(cx, scope, walk)
    }
}

impl MatchEvent for RoomImageMessageDetail {
    fn handle_action(&mut self, cx: &mut Cx, action:&Action) {
        match action.as_widget_action().cast() {
            RoomImageMessageDetailAction::SetImageDetail { 
                room_id, 
                sender, 
                sender_profile, 
                event_id, 
                timestamp_millis,
                image_name,
                image_size
             } => {
                self.room_id = room_id.clone();
                self.sender = sender.clone();
                self.sender_profile = sender_profile.clone();
                self.event_id = event_id.clone();
                self.avatar_drawn = false;
                // Format and display image name and size
                let human_readable_size = format_file_size(image_size);
                let display_text = format!("{} ({})", image_name, human_readable_size);
                self.label(id!(image_name_and_size)).set_text(cx, &display_text);
                if let Some(dt) = unix_time_millis_to_datetime(timestamp_millis) {
                    self.view(id!(timestamp_view)).set_visible(cx, true);
                    self.timestamp(id!(timestamp)).set_date_time(cx, dt);
                }
            }
            _ => {}
        }
    }
}

impl RoomImageMessageDetail {
    /// Reset the widget state to its default values
    pub fn reset_state(&mut self, cx: &mut Cx) {
        self.sender = None;
        self.sender_profile = None;
        self.room_id = None;
        self.event_id = None;
        self.avatar_drawn = false;
        
        // Clear the UI elements
        self.label(id!(top_left_container.username)).set_text(cx, "");
        self.label(id!(image_name_and_size)).set_text(cx, "");
        self.view(id!(timestamp_view)).set_visible(cx, false);
    }
}

impl RoomImageMessageDetailRef {
    /// See [`RoomImageMessageDetail::reset_state()`]
    pub fn reset_state(&self, cx: &mut Cx) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.reset_state(cx);
        }
    }
}

/// Actions handled by the `RoomImageMessageDetail`
#[derive(Debug, Clone, DefaultNone)]
pub enum RoomImageMessageDetailAction {
    /// Set the image detail onto image viewer modal.
    SetImageDetail {
        /// Room ID
        room_id: Option<OwnedRoomId>,
        /// User ID for the sender of the image
        sender: Option<OwnedUserId>,
        /// Profile details for the sender
        sender_profile: Option<TimelineDetails<Profile>>,
        /// Event ID
        event_id: Option<OwnedEventId>,
        /// Timestamp of the message
        timestamp_millis: MilliSecondsSinceUnixEpoch,
        /// Image name
        image_name: String,
        /// Image size in bytes.
        image_size: i32
    },
    None,
}