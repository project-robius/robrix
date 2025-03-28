//! A context menu that appears when the user right-clicks
//! or long-presses on a message/event in a room timeline.

use bitflags::bitflags;
use makepad_widgets::*;
use matrix_sdk::ruma::OwnedEventId;
use matrix_sdk_ui::timeline::EventTimelineItem;

use crate::sliding_sync::UserPowerLevels;

use super::room_screen::{MessageAction, MessageOrSticker};

const BUTTON_HEIGHT: f64 = 30.0; // KEEP IN SYNC WITH BUTTON_HEIGHT BELOW
const MENU_WIDTH: f64 = 215.0;   // KEEP IN SYNC WITH MENU_WIDTH BELOW

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::helpers::*;
    use crate::shared::styles::*;
    use crate::shared::avatar::*;
    use crate::shared::icon_button::*;

    BUTTON_HEIGHT = 30  // KEEP IN SYNC WITH BUTTON_HEIGHT ABOVE
    MENU_WIDTH = 215    // KEEP IN SYNC WITH MENU_WIDTH ABOVE

    pub NewMessageContextMenu = {{NewMessageContextMenu}} {
        visible: false,
        flow: Overlay,
        width: Fill,
        height: Fill,
        cursor: Default,
        // Align to top-left such that our coordinate adjustment
        // when showing this menu pane will work correctly.
        align: {x: 0, y: 0}

        // Show a slightly darkened translucent background to make the menu stand out.
        show_bg: true
        draw_bg: {
            fn pixel(self) -> vec4 {
                return vec4(0., 0., 0., 0.3)
            }
        }

        main_content = <RoundedView> {
            flow: Down
            width: (MENU_WIDTH),
            height: Fit,
            padding: 15
            spacing: 2
            align: {x: 0, y: 0}

            show_bg: true
            draw_bg: {
                color: #fff
                radius: 5.0
                border_width: 0.5
                border_color: #888
            }

            // Shows either the "Add Reaction" button or a reaction text input.
            react_view = <View> {
                flow: Overlay
                height: (BUTTON_HEIGHT)
                react_button = <RobrixIconButton> {
                    height: (BUTTON_HEIGHT)
                    width: Fill,
                    draw_icon: {
                        svg_file: (ICON_ADD_REACTION)
                    }
                    icon_walk: {width: 16, height: 16, margin: {right: 3}}
                    text: "Add Reaction"
                }

                reaction_input_view = <View> {
                    width: Fill,
                    height: (BUTTON_HEIGHT)
                    flow: Right,
                    visible: false, // will be shown once the react_button is clicked

                    reaction_text_input = <RobrixTextInput> {
                        width: Fill,
                        height: Fit,
                        align: {x: 0, y: 0.5}
                        empty_message: "Enter reaction..."
                        draw_text: {
                            // TODO: we want the TextInput flow to show all text
                            // within the single-line box by scrolling horizontally
                            // when the text is too long, upon a user typing/pasting
                            // or navigating with the mouse or arrow keys.
                            // However, makepad doesn't yet support this feature,
                            // so Ellipsis is the closest we can get.
                            wrap: Ellipsis,
                        }
                    }
                    reaction_send_button = <RobrixIconButton> {
                        height: (BUTTON_HEIGHT)
                        align: {x: 0.5, y: 0.5}
                        padding: {left: 10, right: 10}
                        draw_icon: {
                            svg_file: (ICON_SEND)
                            color: (COLOR_ACCEPT_GREEN),
                        }
                        icon_walk: {width: 16, height: 16, margin: {left: -2, right: -1} }

                        draw_bg: {
                            border_color: (COLOR_ACCEPT_GREEN),
                            color: #f0fff0 // light green
                        }
                        text: ""
                        draw_text:{
                            color: (COLOR_ACCEPT_GREEN),
                        }
                    }
                }
            }

            reply_button = <RobrixIconButton> {
                height: (BUTTON_HEIGHT)
                width: Fill,
                draw_icon: {
                    svg_file: (ICON_REPLY)
                }
                icon_walk: {width: 16, height: 16, margin: {right: 3}}
                text: "Reply"
            }

            divider_after_react_reply = <LineH> {
                margin: {top: 3, bottom: 3}
                draw_bg: {color: (COLOR_DIVIDER_DARK)}
                width: Fill,
            }

            edit_message_button = <RobrixIconButton> {
                height: (BUTTON_HEIGHT)
                width: Fill,
                draw_icon: {
                    svg_file: (ICON_EDIT)
                }
                icon_walk: {width: 16, height: 16, margin: {top: -3, right: 3} }
                text: "Edit Message"
            }

            // TODO: change text to "Unpin Message" if the message is already pinned,
            //       using https://matrix-org.github.io/matrix-rust-sdk/matrix_sdk/struct.RoomInfo.html#method.is_pinned_event.
            //       The caller of `show()` will also need to check if the current user is allowed to 
            //       pin/unpin messages using: https://matrix-org.github.io/matrix-rust-sdk/matrix_sdk_base/struct.RoomMember.html#method.can_pin_or_unpin_event
            pin_button = <RobrixIconButton> {
                height: (BUTTON_HEIGHT)
                width: Fill,
                draw_icon: {
                    svg_file: (ICON_PIN)
                }
                icon_walk: {width: 16, height: 16, margin: {right: 3} }
                text: "" // set dynamically to "Pin Message" or "Unpin Message"
            }

            copy_text_button = <RobrixIconButton> {
                height: (BUTTON_HEIGHT)
                width: Fill,
                draw_icon: {
                    svg_file: (ICON_COPY)
                }
                icon_walk: {width: 16, height: 16, margin: {right: 3} }
                text: "Copy Text"
            }

            copy_html_button = <RobrixIconButton> {
                height: (BUTTON_HEIGHT)
                width: Fill,
                draw_icon: {
                    svg_file: (ICON_HTML_FILE)
                }
                icon_walk: {width: 16, height: 16, margin: {left: 1.5, right: 1.5} }
                text: "Copy Text as HTML"
            }

            copy_link_to_message_button = <RobrixIconButton> {
                height: (BUTTON_HEIGHT)
                width: Fill,
                draw_icon: {
                    svg_file: (ICON_LINK)
                }
                icon_walk: {width: 16, height: 16, margin: {right: 3} }
                text: "Copy Link to Message"
            }

            view_source_button = <RobrixIconButton> {
                height: (BUTTON_HEIGHT)
                width: Fill,
                draw_icon: {
                    svg_file: (ICON_VIEW_SOURCE)
                }
                icon_walk: {width: 16, height: 16, margin: {top: 6, right: 3} }
                text: "View Source"
            }

            jump_to_related_button = <RobrixIconButton> {
                height: (BUTTON_HEIGHT)
                width: Fill,
                draw_icon: {
                    svg_file: (ICON_JUMP)
                }
                icon_walk: {width: 16, height: 16, margin: {right: 3} }
                text: "Jump to Related Event"
            }

            divider_before_report_delete = <LineH> {
                margin: {top: 3, bottom: 3}
                draw_bg: {color: (COLOR_DIVIDER_DARK)}
                width: Fill,
            }

            // report_button = <RobrixIconButton> {
            //     height: (BUTTON_HEIGHT)
            //     width: Fill,
            //     draw_icon: {
            //         svg_file: (ICON_TRASH) // TODO: ICON_REPORT/WARNING/FLAG
            //         color: (COLOR_DANGER_RED),
            //     }
            //     icon_walk: {width: 16, height: 16, margin: {left: -2, right: 3} }
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
                height: (BUTTON_HEIGHT)
                width: Fill,
                draw_icon: {
                    svg_file: (ICON_TRASH)
                    color: (COLOR_DANGER_RED),
                }
                icon_walk: {width: 16, height: 16, margin: {right: 3} }

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


bitflags! {
    /// Possible actions that the user can perform on a message.
    ///
    /// This is used to determine which buttons to show in the message context menu.
    #[derive(Copy, Clone, Debug)]
    pub struct MessageAbilities: u8 {
        /// Whether the user can react to this message.
        const CanReact = 1 << 0;
        /// Whether the user can reply to this message.
        const CanReplyTo = 1 << 1;
        /// Whether the user can edit this message.
        const CanEdit = 1 << 2;
        /// Whether the user can pin this message.
        const CanPin = 1 << 3;
        /// Whether the user can unpin this message.
        const CanUnpin = 1 << 4;
        /// Whether the user can delete/redact this message.
        const CanDelete = 1 << 5;
        /// Whether this message contains HTML content that the user can copy.
        const HasHtml = 1 << 6;
    }
}
impl MessageAbilities {
    pub fn from_user_power_and_event(
        user_power_levels: &UserPowerLevels,
        event_tl_item: &EventTimelineItem,
        _message: &MessageOrSticker,
        has_html: bool,
    ) -> Self {
        let mut abilities = Self::empty();
        abilities.set(Self::CanEdit, event_tl_item.is_editable());
        // Currently we only support deleting one's own messages.
        if event_tl_item.is_own() {
            abilities.set(Self::CanDelete, user_power_levels.can_redact_own());
        }
        abilities.set(Self::CanReplyTo, event_tl_item.can_be_replied_to());
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
        if !self.visible { return; }
        self.view.handle_event(cx, event, scope);

        let area = self.view.area();

        // Close the menu if:
        // 1. The back navigational gesture/action occurs (e.g., Back on Android),
        // 2. The escape key is pressed if this menu has key focus,
        // 3. The user clicks/touches outside the main_content view area.
        // 4. The user scrolls anywhere.
        let close_menu = {
            event.back_pressed()
            || match event.hits_with_capture_overload(cx, area, true) {
                Hit::KeyUp(key) => key.key_code == KeyCode::Escape,
                Hit::FingerDown(fde) => {
                    let reaction_text_input = self.view.text_input(id!(reaction_input_view.reaction_text_input));
                    if reaction_text_input.area().rect(cx).contains(fde.abs) {
                        reaction_text_input.set_key_focus(cx);
                    } else {
                        cx.set_key_focus(area);
                    }
                    false
                }
                Hit::FingerUp(fue) if fue.is_over => {
                    !self.view(id!(main_content)).area().rect(cx).contains(fue.abs)
                }
                Hit::FingerScroll(_) => true,
                _ => false,
            }
        };
        if close_menu {
            self.close(cx);
            return;
        }

        self.widget_match_event(cx, event, scope); 
    }
}

impl WidgetMatchEvent for NewMessageContextMenu {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions, scope: &mut Scope) {
        let Some(details) = self.details.as_ref() else { return };
        let mut close_menu = false;

        let reaction_text_input = self.view.text_input(id!(reaction_input_view.reaction_text_input));
        let reaction_send_button = self.view.button(id!(reaction_input_view.reaction_send_button));
        if reaction_send_button.clicked(actions)
            || reaction_text_input.returned(actions).is_some()
        {
            cx.widget_action(
                details.room_screen_widget_uid,
                &scope.path,
                MessageAction::React {
                    details: details.clone(),
                    reaction: reaction_text_input.text(),
                },
            );
            close_menu = true;
        }
        else if reaction_text_input.escape(actions) {
            close_menu = true;
        }
        else if self.button(id!(react_button)).clicked(actions) {
            // Show a box to allow the user to input the reaction.
            // In the future, we'll show an emoji chooser.
            self.view.button(id!(react_button)).set_visible(cx, false);
            self.view.view(id!(reaction_input_view)).set_visible(cx, true);
            self.text_input(id!(reaction_input_view.reaction_text_input)).set_key_focus(cx);
            self.redraw(cx);
            close_menu = false;
        }
        else if self.button(id!(reply_button)).clicked(actions) {
            cx.widget_action(
                details.room_screen_widget_uid,
                &scope.path,
                MessageAction::Reply(details.clone()),
            );
            close_menu = true;
        }
        else if self.button(id!(edit_message_button)).clicked(actions) {
            cx.widget_action(
                details.room_screen_widget_uid,
                &scope.path,
                MessageAction::Edit(details.clone()),
            );
            close_menu = true;
        }
        else if self.button(id!(pin_button)).clicked(actions) {
            if details.abilities.contains(MessageAbilities::CanPin) {
                cx.widget_action(
                    details.room_screen_widget_uid,
                    &scope.path,
                    MessageAction::Pin(details.clone()),
                );
            } else if details.abilities.contains(MessageAbilities::CanUnpin) {
                cx.widget_action(
                    details.room_screen_widget_uid,
                    &scope.path,
                    MessageAction::Unpin(details.clone()),
                );
            }
            close_menu = true;
        }
        else if self.button(id!(copy_text_button)).clicked(actions) {
            cx.widget_action(
                details.room_screen_widget_uid,
                &scope.path,
                MessageAction::CopyText(details.clone()),
            );
            close_menu = true;
        }
        else if self.button(id!(copy_html_button)).clicked(actions) {
            cx.widget_action(
                details.room_screen_widget_uid,
                &scope.path,
                MessageAction::CopyHtml(details.clone()),
            );
            close_menu = true;
        }
        else if self.button(id!(copy_link_to_message_button)).clicked(actions) {
            cx.widget_action(
                details.room_screen_widget_uid,
                &scope.path,
                MessageAction::CopyLink(details.clone()),
            );
            close_menu = true;
        }
        else if self.button(id!(view_source_button)).clicked(actions) {
            cx.widget_action(
                details.room_screen_widget_uid,
                &scope.path,
                MessageAction::ViewSource(details.clone()),
            );
            close_menu = true;
        }
        else if self.button(id!(jump_to_related_button)).clicked(actions) {
            cx.widget_action(
                details.room_screen_widget_uid,
                &scope.path,
                MessageAction::JumpToRelated(details.clone()),
            );
            close_menu = true;
        }
        // else if self.button(id!(report_button)).clicked(actions) {
        //     cx.widget_action(
        //         details.room_screen_widget_uid,
        //         &scope.path,
        //         // TODO: display a dialog to confirm the report reason.
        //         MessageAction::Report {
        //             event_id: details.event_id.clone(),
        //             item_id: details.item_id,
        //         },
        //     );
        //    close_menu = true;
        // }
        else if self.button(id!(delete_button)).clicked(actions) {
            cx.widget_action(
                details.room_screen_widget_uid,
                &scope.path,
                MessageAction::Redact {
                    details: details.clone(),
                    // TODO: show a Modal to confirm deletion, and get the reason.
                    reason: None,
                },
            );
            close_menu = true;
        }

        if close_menu {
            self.close(cx);
        }
    }
}

impl NewMessageContextMenu {
    /// Returns `true` if this menu is currently being shown.
    pub fn is_currently_shown(&self, _cx: &mut Cx) -> bool {
        self.visible
    }

    /// Shows this context menu with the given message details.
    ///
    /// Returns the expected (approximate) dimensions of the context menu,
    /// which can be used to proactively reposition it such that it fits on screen.
    pub fn show(&mut self, cx: &mut Cx, details: MessageDetails) -> DVec2 {
        self.details = Some(details);
        self.visible = true;
        cx.set_key_focus(self.view.area());

        // log!("Showing context menu for message: {:?}", self.details);
        let height = self.set_button_visibility(cx);

        dvec2(MENU_WIDTH, height)
    }

    /// Sets up all of the buttons based this context menu's inner details.
    ///
    /// Returns the total height of all visible items.
    fn set_button_visibility(&mut self, cx: &mut Cx) -> f64 {
        let Some(details) = self.details.as_ref() else { return 0.0 };

        let react_button = self.view.button(id!(react_button));
        let reply_button = self.view.button(id!(reply_button));
        let edit_button = self.view.button(id!(edit_message_button));
        let pin_button = self.view.button(id!(pin_button));
        let copy_text_button = self.view.button(id!(copy_text_button));
        let copy_html_button = self.view.button(id!(copy_html_button));
        let copy_link_button = self.view.button(id!(copy_link_to_message_button));
        let view_source_button = self.view.button(id!(view_source_button));
        let jump_to_related_button = self.view.button(id!(jump_to_related_button));
        // let report_button = self.view.button(id!(report_button));
        let delete_button = self.view.button(id!(delete_button));

        // Determine which buttons should be shown.
        // Note that some buttons are always enabled:
        // `copy_text_button`, `copy_link_to_message_button`, and `view_source_button`
        let show_react = details.abilities.contains(MessageAbilities::CanReact);
        let show_reply_to = details.abilities.contains(MessageAbilities::CanReplyTo);
        let show_divider_after_react_reply = show_react || show_reply_to;
        let show_edit = details.abilities.contains(MessageAbilities::CanEdit);
        let show_pin: bool;
        let show_copy_text = true;
        let show_copy_html = details.abilities.contains(MessageAbilities::HasHtml);
        let show_copy_link = true;
        let show_view_source = true;
        let show_jump_to_related = details.related_event_id.is_some();
        // let show_report = true;
        let show_delete = details.abilities.contains(MessageAbilities::CanDelete);
        let show_divider_before_report_delete = show_delete; // || show_report;

        // Actually set the buttons' visibility.
        self.view.view(id!(react_view)).set_visible(cx, show_react);
        react_button.set_visible(cx, show_react);
        reply_button.set_visible(cx, show_reply_to);
        self.view.view(id!(divider_after_react_reply)).set_visible(cx, show_divider_after_react_reply);
        edit_button.set_visible(cx, show_edit);
        if details.abilities.contains(MessageAbilities::CanPin) {
            pin_button.set_text(cx, "Pin Message");
            show_pin = true;
        } else if details.abilities.contains(MessageAbilities::CanUnpin) {
            pin_button.set_text(cx, "Unpin Message");
            show_pin = true;
        } else {
            show_pin = false;
        }
        pin_button.set_visible(cx, show_pin);
        copy_html_button.set_visible(cx, show_copy_html);
        jump_to_related_button.set_visible(cx, show_jump_to_related);
        self.view.view(id!(divider_before_report_delete)).set_visible(cx, show_divider_before_report_delete);
        // report_button.set_visible(cx, show_report);
        delete_button.set_visible(cx, show_delete);

        // Reset the hover state of each button.
        react_button.reset_hover(cx);
        reply_button.reset_hover(cx);
        edit_button.reset_hover(cx);
        pin_button.reset_hover(cx);
        copy_text_button.reset_hover(cx);
        copy_html_button.reset_hover(cx);
        copy_link_button.reset_hover(cx);
        view_source_button.reset_hover(cx);
        jump_to_related_button.reset_hover(cx);
        // report_button.reset_hover(cx);
        delete_button.reset_hover(cx);

        // Reset reaction input view stuff.
        self.view.view(id!(reaction_input_view)).set_visible(cx, false); // hide until the react_button is clicked
        self.text_input(id!(reaction_input_view.reaction_text_input)).set_text(cx, "");

        self.redraw(cx);

        let num_visible_buttons = 
            show_react as u8
            + show_reply_to as u8
            + show_edit as u8
            + show_pin as u8
            + show_copy_text as u8
            + show_copy_html as u8
            + show_copy_link as u8
            + show_view_source as u8
            + show_jump_to_related as u8
            // + show_report as u8
            + show_delete as u8;

        // Calculate and return the total expected height:
        (num_visible_buttons as f64 * (BUTTON_HEIGHT + 2.0 + 2.0))
            + if show_divider_after_react_reply { 10.0 } else { 0.0 }
            + if show_divider_before_report_delete { 10.0 } else { 0.0 }
            + 20.0  // top and bottom padding
            + 1.0   // top and bottom border
            - 4.0   // no 2.0 spacers at the top and bottom
    }

    fn close(&mut self, cx: &mut Cx) {
        self.visible = false;
        self.details = None;
        cx.revert_key_focus();
        self.redraw(cx);
    }
}

impl NewMessageContextMenuRef {
    /// See [`NewMessageContextMenu::is_currently_shown()`].
    pub fn is_currently_shown(&self, cx: &mut Cx) -> bool {
        let Some(inner) = self.borrow() else { return false };
        inner.is_currently_shown(cx)
    }

    /// See [`NewMessageContextMenu::show()`].
    pub fn show(&self, cx: &mut Cx, details: MessageDetails) -> DVec2 {
        let Some(mut inner) = self.borrow_mut() else { return DVec2::default()};
        inner.show(cx, details)
    }
}
