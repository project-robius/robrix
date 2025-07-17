//! RoomInputBar component provides a message input interface with @mention capabilities
//! Supports user mention autocomplete, avatar display, and desktop/mobile layouts

use makepad_widgets::*;
use crate::shared::styles::{COLOR_ACCEPT_GREEN, COLOR_DISABLE_GRAY};

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
        padding: 6,
        show_bg: true
        draw_bg: {color: (COLOR_PRIMARY)}

        location_button = <RobrixIconButton> {
            spacing: 0,
            draw_icon: {svg_file: (ICO_LOCATION_PERSON)},
            icon_walk: {width: Fit, height: 23, margin: {bottom: -1}}
            text: "",
        }

        message_input = <MentionableTextInput> {
            width: Fill,
            height: Fit
            margin: { bottom: 12 },

            persistent = {
                center = {
                    text_input = {
                        empty_text: "Write a message (in Markdown) ..."
                    }
                }
            }
        }

        send_message_button = <RobrixIconButton> {
            enabled: false, // is enabled when text is inputted
            spacing: 0,
            draw_icon: {
                svg_file: (ICO_SEND),
                color: (COLOR_DISABLE_GRAY),
            }
            icon_walk: {width: Fit, height: 21},
        }
    }
}

/// Main component for message input with @mention support
#[derive(Live, LiveHook, Widget)]
pub struct RoomInputBar {
    /// Base view properties
    #[deref]
    view: View,
}

impl Widget for RoomInputBar {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl RoomInputBar {
    /// Sets the send_message_button to be enabled and green, or disabled and gray.
    fn enable_send_message_button(&mut self, cx: &mut Cx, enable: bool) {
        let send_message_button = self.view.button(id!(send_message_button));
        let new_color = if enable {
            COLOR_ACCEPT_GREEN
        } else {
            COLOR_DISABLE_GRAY
        };
        send_message_button.apply_over(cx, live! {
            enabled: (enable),
            draw_icon: {
                color: (new_color),
                color_hover: (new_color),
            }
        });
    }
}

impl RoomInputBarRef {
    pub fn enable_send_message_button(&self, cx: &mut Cx, enable: bool) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.enable_send_message_button(cx, enable);
        }
    }
}
