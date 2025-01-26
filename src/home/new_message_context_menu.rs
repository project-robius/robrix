use bitflags::bitflags;
use makepad_widgets::*;
use matrix_sdk::ruma::OwnedEventId;
use matrix_sdk_ui::timeline::EventTimelineItem;

use crate::sliding_sync::UserPowerLevels;

use super::room_screen::MessageOrSticker;

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::helpers::*;
    use crate::shared::styles::*;
    use crate::shared::avatar::*;
    use crate::shared::icon_button::*;

    pub NewMessageContextMenu = {{NewMessageContextMenu}} {
        visible: false,
        flow: Overlay,
        width: Fill,
        height: Fill,
        align: {x: 0.5, y: 0.5}

        // Show a clear background such that we can capture hits
        // on the background to close the context menu.
        // TODO: test removing this; it may not be necessary,
        //       because the parent view already has width: Fill, height: Fill.
        show_bg: true
        draw_bg: {
            fn pixel(self) -> vec4 {
                return vec4(0., 0., 0., 0.)
            }
        }

        main_content = <RoundedView> {
            flow: Down
            width: Fit
            height: Fit
            padding: 15
            spacing: 10

            show_bg: true
            draw_bg: {
                color: #fff
                radius: 3.0
            }

            react_button = <RobrixIconButton> {
                draw_icon: {
                    svg_file: (ICON_ADD_REACTION)
                }
                icon_walk: {width: 16, height: 16, margin: {right: 5}}
                text: "Add Reaction"
            }

            reply_button = <RobrixIconButton> {
                draw_icon: {
                    svg_file: (ICON_REPLY)
                }
                icon_walk: {width: 16, height: 16, margin: {right: 5}}
                text: "Reply"
            }

            edit_message_button = <RobrixIconButton> {
                draw_icon: {
                    svg_file: (ICON_EDIT)
                }
                icon_walk: {width: 16, height: 16, margin: {right: -2} }
                text: "Edit Message"
            }

            // TODO: change text to "Unpin Message" if the message is already pinned,
            //       using https://matrix-org.github.io/matrix-rust-sdk/matrix_sdk/struct.RoomInfo.html#method.is_pinned_event.
            //       The caller of `show()` will also need to check if the current user is allowed to 
            //       pin/unpin messages using: https://matrix-org.github.io/matrix-rust-sdk/matrix_sdk_base/struct.RoomMember.html#method.can_pin_or_unpin_event
            pin_message_button = <RobrixIconButton> {
                draw_icon: {
                    svg_file: (ICON_PIN)
                }
                icon_walk: {width: 16, height: 16, margin: {right: -2} }
                text: "" // set dynamically to "Pin Message" or "Unpin Message"
            }

            copy_text_button = <RobrixIconButton> {
                draw_icon: {
                    svg_file: (ICON_COPY)
                }
                icon_walk: {width: 16, height: 16, margin: {right: -2} }
                text: "Copy Text"
            }

            copy_html_button = <RobrixIconButton> {
                draw_icon: {
                    svg_file: (ICON_HTML_FILE)
                }
                icon_walk: {width: 16, height: 16, margin: {right: -2} }
                text: "Copy Text as HTML"
            }

            copy_link_to_message_button = <RobrixIconButton> {
                draw_icon: {
                    svg_file: (ICON_LINK)
                }
                icon_walk: {width: 16, height: 16, margin: {right: -2} }
                text: "Copy Link to Message"
            }

            view_source_button = <RobrixIconButton> {
                draw_icon: {
                    svg_file: (ICON_VIEW_SOURCE)
                }
                icon_walk: {width: 16, height: 16, margin: {right: -2} }
                text: "View Source"
            }

            jump_to_related_button = <RobrixIconButton> {
                draw_icon: {
                    svg_file: (ICON_JUMP)
                }
                icon_walk: {width: 16, height: 16, margin: {right: -2} }
                text: "Jump to Related Event"
            }

            // report_button = <RobrixIconButton> {
            //     draw_icon: {
            //         svg_file: (ICON_TRASH) // TODO: ICON_REPORT/WARNING/FLAG
            //         color: (COLOR_DANGER_RED),
            //     }
            //     icon_walk: {width: 16, height: 16, margin: {left: -2, right: -1} }
            //
            //     draw_bg: {
            //         border_color: (COLOR_DANGER_RED),
            //         color: #fff0f0
            //     }
            //     text: "Report"
            //     draw_text:{
            //         color: (COLOR_DANGER_RED),
            //     }
            // }

            // Note: we don't yet support deleting others' messages via admin/moderator power levels.
            //       For now we only consider whether its the user's own message.
            //       The caller needs to use `can_redact_own()` or `can_redact_other()`:
            //       https://matrix-org.github.io/matrix-rust-sdk/matrix_sdk_base/struct.RoomMember.html#method.can_redact_own

            delete_button = <RobrixIconButton> {
                draw_icon: {
                    svg_file: (ICON_TRASH)
                    color: (COLOR_DANGER_RED),
                }
                icon_walk: {width: 16, height: 16, margin: {left: -2, right: -1} }

                draw_bg: {
                    border_color: (COLOR_DANGER_RED),
                    color: #fff0f0
                }
                text: "Delete"
                draw_text:{
                    color: (COLOR_DANGER_RED),
                }
            }
        }
    }
}


/// Actions that can be emitted from a message context menu.
///
/// These are handled by the parent RoomScreen widget.
#[derive(Clone, Debug, DefaultNone)]
pub enum MessageContextMenuAction {
    React {
        details: MessageDetails,
        reaction: String,
    },
    Reply(MessageDetails),
    Edit(MessageDetails),
    Pin(MessageDetails),
    Unpin(MessageDetails),
    CopyText(MessageDetails),
    CopyHtml(MessageDetails),
    CopyLink(MessageDetails),
    ViewSource(MessageDetails),
    JumpToRelated(MessageDetails),
    Delete(MessageDetails),
    None,
}

bitflags! {
    /// Possible actions that the user can perform on a message.
    ///
    /// This is used to determine which buttons to show in the message context menu.
    #[derive(Copy, Clone, Debug)]
    pub struct MessageAbilities: u8 {
        /// Whether this message was sent by the current logged-in user.
        const IsOwn = 1 << 0;
        /// Whether the user can edit this message.
        const CanEdit = 1 << 1;
        /// Whether the user can reply to this message.
        const CanReplyTo = 1 << 2;
        /// Whether the user can pin this message.
        const CanPin = 1 << 3;
        /// Whether the user can unpin this message.
        const CanUnpin = 1 << 4;
        /// Whether the user can delete/redact this message.
        const CanDelete = 1 << 5;
        /// Whether the user can react to this message.
        const CanReact = 1 << 6;
        /// Whether this message contains HTML content that the user can copy.
        const HasHtml = 1 << 7;
    }
}
impl MessageAbilities {
    pub fn from_user_power_and_event(
        user_power_levels: &UserPowerLevels,
        event_tl_item: &EventTimelineItem,
        message: &MessageOrSticker,
        has_html: bool,
    ) -> Self {
        let mut abilities = Self::empty();
        let is_own = event_tl_item.is_own();
        abilities.set(Self::IsOwn, is_own);
        // Currently we only support deleting and editing one's own messages.
        if is_own {
            abilities.set(Self::CanEdit, true);
            abilities.set(Self::CanDelete, user_power_levels.can_redact_own());
        }

        abilities.set(Self::CanReplyTo, message.in_reply_to().is_some());
        abilities.set(Self::CanPin, user_power_levels.can_pin());
        // TODO: currently we don't differentiate between pin and unpin,
        //       but we should first check whether the given message is already pinned
        //       before deciding which ability to set.
        // abilities.set(Self::CanUnPin, user_power_levels.can_pin_unpin());
        abilities.set(Self::CanReact, user_power_levels.can_send_reaction());
        abilities.set(Self::HasHtml, has_html);
        abilities
    }

}

/// Details about the message that define its context menu content.
#[derive(Clone, Debug)]
pub struct MessageDetails {
    /// The Event ID of the message. If `None`, it is an unsent local event.
    pub event_id: Option<OwnedEventId>,
    /// The index of this message in its room's timeline.
    pub item_id: usize,
    /// The event ID of the message that this message is related to, if any,
    /// such as the replied-to message.
    pub related_event_id: Option<OwnedEventId>,
    /// The widget ID of the RoomScreen that contains this message.
    pub room_screen_widget_uid: WidgetUid,
    /// Whether this message mentions the current user.
    pub mentions_user: bool,
    /// The abilities that the user has on this message.
    pub abilities: MessageAbilities,
}

#[derive(Live, LiveHook, Widget)]
pub struct NewMessageContextMenu {
    #[deref] view: View,
    #[rust] details: Option<MessageDetails>,
}

impl Widget for NewMessageContextMenu {
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        if self.details.is_none() {
            self.visible = false;
        };

        self.view.draw_walk(cx, scope, walk)
    }

    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        if !self.visible { return; }
        
        let area = self.view.area();

        // Close the menu if:
        // 1. The cancel button is clicked,
        // 2. The back navigational gesture/action occurs (e.g., Back on Android),
        // 3. The escape key is pressed if this menu has key focus,
        // 4. The user clicks/touches outside the main_content view area.
        let close_menu = match event {
            Event::Actions(actions) => self.button(id!(cancel_button)).clicked(actions), // 1
            Event::BackPressed => true,                                                  // 2
            _ => false,
        } || match event.hits_with_capture_overload(cx, area, true) {
            Hit::KeyUp(key) => key.key_code == KeyCode::Escape,                          // 3
            Hit::FingerDown(_fde) => {
                cx.set_key_focus(area);
                false
            }
            Hit::FingerUp(fue) if fue.is_over => {
                !self.view(id!(main_content)).area().rect(cx).contains(fue.abs)       // 4
            }
            _ => false,
        };
        if close_menu {
            self.details = None;
            cx.revert_key_focus();
            self.visible = false;
        }

        self.widget_match_event(cx, event, scope); 
    }
}

impl WidgetMatchEvent for NewMessageContextMenu {
    fn handle_actions(&mut self, cx: &mut Cx, actions :&Actions, scope: &mut Scope) {
        let Some(details) = self.details.as_ref() else { return };

        if self.button(id!(react_button)).clicked(actions) {
            cx.widget_action(
                details.room_screen_widget_uid,
                &scope.path,
                MessageContextMenuAction::React {
                    details: details.clone(),
                    // TODO: show a dialog to choose the reaction (or a TextInput for custom),
                    //       which itself should send this action (instead of doing it here).
                    reaction: "Test Reaction".to_string(),
                },
            );
        }
        if self.button(id!(reply_button)).clicked(actions) {
            cx.widget_action(
                details.room_screen_widget_uid,
                &scope.path,
                MessageContextMenuAction::Reply(details.clone()),
            );
        }
        if self.button(id!(edit_message_button)).clicked(actions) {
            cx.widget_action(
                details.room_screen_widget_uid,
                &scope.path,
                MessageContextMenuAction::Edit(details.clone()),
            );
        }
        if self.button(id!(pin_message_button)).clicked(actions) {
            if details.abilities.contains(MessageAbilities::CanPin) {
                cx.widget_action(
                    details.room_screen_widget_uid,
                    &scope.path,
                    MessageContextMenuAction::Pin(details.clone()),
                );
            } else if details.abilities.contains(MessageAbilities::CanUnpin) {
                cx.widget_action(
                    details.room_screen_widget_uid,
                    &scope.path,
                    MessageContextMenuAction::Unpin(details.clone()),
                );
            }
        }
        if self.button(id!(copy_text_button)).clicked(actions) {
            cx.widget_action(
                details.room_screen_widget_uid,
                &scope.path,
                MessageContextMenuAction::CopyText(details.clone()),
            );
        }
        if self.button(id!(copy_html_button)).clicked(actions) {
            cx.widget_action(
                details.room_screen_widget_uid,
                &scope.path,
                MessageContextMenuAction::CopyHtml(details.clone()),
            );
        }
        if self.button(id!(copy_link_to_message_button)).clicked(actions) {
            cx.widget_action(
                details.room_screen_widget_uid,
                &scope.path,
                MessageContextMenuAction::CopyLink(details.clone()),
            );
        }
        if self.button(id!(view_source_button)).clicked(actions) {
            cx.widget_action(
                details.room_screen_widget_uid,
                &scope.path,
                MessageContextMenuAction::ViewSource(details.clone()),
            );
        }
        if self.button(id!(jump_to_related_button)).clicked(actions) {
            cx.widget_action(
                details.room_screen_widget_uid,
                &scope.path,
                MessageContextMenuAction::JumpToRelated(details.clone()),
            );
        }
        // if self.button(id!(report_button)).clicked(actions) {
        //     cx.widget_action(
        //         details.room_screen_widget_uid,
        //         &scope.path,
        //         // TODO: display a dialog to confirm the report reason.
        //         MessageContextMenuAction::Report {
        //             event_id: details.event_id.clone(),
        //             item_id: details.item_id,
        //         },
        //     );
        // }
        if self.button(id!(delete_button)).clicked(actions) {
            cx.widget_action(
                details.room_screen_widget_uid,
                &scope.path,
                MessageContextMenuAction::Delete(details.clone()),
            );
        }
    }
}

impl NewMessageContextMenu {
    /// Returns `true` if this menu is currently being shown.
    pub fn is_currently_shown(&self, _cx: &mut Cx) -> bool {
        self.visible
    }

    pub fn show(&mut self, cx: &mut Cx, details: MessageDetails) {
        self.visible = true;
        cx.set_key_focus(self.view.area());

        // Set all of the buttons based on the message's abilities.
        // Note that some buttons are always enabled:
        // copy_text_button, copy_link_to_message_button, and view_source_button
        self.view.button(id!(react_button))
            .set_visible(cx, details.abilities.contains(MessageAbilities::CanReact));
        self.view.button(id!(reply_button))
            .set_visible(cx, details.abilities.contains(MessageAbilities::CanReplyTo));
        self.view.button(id!(edit_message_button))
            .set_visible(cx, details.abilities.contains(MessageAbilities::CanEdit));
        let pin_button = self.view.button(id!(pin_message_button));
        if details.abilities.contains(MessageAbilities::CanPin) {
            pin_button.set_text(cx, "Pin Message");
            pin_button.set_visible(cx, true);
        } else if details.abilities.contains(MessageAbilities::CanUnpin) {
            pin_button.set_text(cx, "Unpin Message");
            pin_button.set_visible(cx, true);
        } else {
            pin_button.set_visible(cx, false);
        }
        self.view.button(id!(copy_html_button))
            .set_visible(cx, details.abilities.contains(MessageAbilities::HasHtml));
        self.view.button(id!(jump_to_related_button))
            .set_visible(cx, details.related_event_id.is_some());
        self.view.button(id!(delete_button))
            .set_visible(cx, details.abilities.contains(MessageAbilities::CanDelete));

        self.redraw(cx);
    }
}

impl NewMessageContextMenuRef {
    /// See [`NewMessageContextMenu::is_currently_shown()`]
    pub fn is_currently_shown(&self, cx: &mut Cx) -> bool {
        let Some(inner) = self.borrow() else { return false };
        inner.is_currently_shown(cx)
    }

    /// See [`NewMessageContextMenu::show()`]
    pub fn show(&self, cx: &mut Cx, details: MessageDetails) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.show(cx, details);
    }
}
