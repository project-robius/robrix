//! A context menu that appears when the user right-clicks
//! or long-presses on a room in the room list.

use makepad_widgets::*;
use matrix_sdk::ruma::OwnedRoomId;
use crate::{home::invite_modal::InviteModalAction, shared::{context_menu::{ContextMenuClosed, expected_menu_size}, popup_list::{PopupKind, enqueue_popup_notification}}, sliding_sync::{MatrixRequest, submit_async_request}, utils::RoomNameId};

/// Nothing here is conditionally shown, so keep these matching the DSL below.
const NUM_BUTTONS: usize = 8;
const NUM_DIVIDERS: usize = 2;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.RoomContextMenu = set_type_default() do #(RoomContextMenu::register_widget(vm)) {
        ..mod.widgets.SolidView

        visible: false,
        flow: Overlay,
        width: Fill,
        height: Fill,
        cursor: MouseCursor.Default,
        align: Align{x: 0, y: 0}

        show_bg: true
        draw_bg +: {
            color: #0000004d
        }

        main_content := mod.widgets.ContextMenuContent {
            mark_unread_button := mod.widgets.ContextMenuButton {
                draw_icon +: { svg: (ICON_CHECKMARK) }
                text: "Mark as Unread"
            }

            favorite_button := mod.widgets.ContextMenuButton {
                draw_icon +: { svg: (ICON_PIN) }
                text: "Favorite"
            }

            priority_button := mod.widgets.ContextMenuButton {
                draw_icon +: { svg: (ICON_TOMBSTONE) }
                text: "Set Low Priority"
            }

            copy_link_button := mod.widgets.ContextMenuButton {
                draw_icon +: { svg: (ICON_LINK) }
                text: "Copy Link to Room"
            }

            divider1 := mod.widgets.ContextMenuDivider { }

            room_settings_button := mod.widgets.ContextMenuButton {
                draw_icon +: { svg: (ICON_SETTINGS) }
                text: "Settings"
            }

            notifications_button := mod.widgets.ContextMenuButton {
                // TODO: use a proper bell icon
                draw_icon +: { svg: (ICON_INFO) }
                text: "Notifications"
            }

            invite_button := mod.widgets.ContextMenuButton {
                draw_icon +: { svg: (ICON_ADD_USER) }
                text: "Invite"
            }

            divider2 := mod.widgets.ContextMenuDivider { }

            leave_button := mod.widgets.ContextMenuDangerButton {
                draw_icon.svg: (ICON_LOGOUT)
                text: "Leave Room"
            }
        }
    }
}

/// Details needed to populate the room context menu.
#[derive(Clone, Debug)]
pub struct RoomContextMenuDetails {
    pub room_name_id: RoomNameId,
    pub is_favorite: bool,
    pub is_low_priority: bool,
    pub is_marked_unread: bool,
}

/// Actions emitted from the RoomContextMenu widget, as they must be handled
/// by other widgets with more information (e.g., the RoomsList).
#[derive(Clone, Default, Debug)]
pub enum RoomContextMenuAction {
    Notifications(OwnedRoomId),
    OpenRoomSettings(OwnedRoomId),
    #[default]
    None,
}

#[derive(Script, ScriptHook, Widget)]
pub struct RoomContextMenu {
    #[deref] view: View,
    #[source] source: ScriptObjectRef,
    #[rust] details: Option<RoomContextMenuDetails>,
}

impl Widget for RoomContextMenu {
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        if self.details.is_none() {
            self.visible = false;
        };
        let step = self.view.draw_walk(cx, scope, walk);
        if self.visible {
            let main_content_area = self.view(cx, ids!(main_content)).area();
            cx.block_scrolling_except_within(main_content_area);
        }
        step
    }

    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if !self.visible { return; }
        self.view.handle_event(cx, event, scope);

        // Close logic similar to NewMessageContextMenu
        let area = self.view.area();
        let close_menu = {
            event.back_pressed()
            || match event.hits_with_capture_overload(cx, area, true) {
                Hit::KeyUp(key) => key.key_code == KeyCode::Escape,
                Hit::FingerUp(fue) if fue.is_over => {
                     !self.view(cx, ids!(main_content)).area().rect(cx).contains(fue.abs)
                }
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

impl WidgetMatchEvent for RoomContextMenu {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions, _scope: &mut Scope) {
        let Some(details) = self.details.as_ref() else { return };
        let mut close_menu = false;
        
        if self.button(cx, ids!(mark_unread_button)).clicked(actions) {
            submit_async_request(MatrixRequest::SetUnreadFlag {
                room_id: details.room_name_id.room_id().clone(),
                mark_as_unread: !details.is_marked_unread,
            });
            close_menu = true;
        } 
        else if self.button(cx, ids!(favorite_button)).clicked(actions) {
            submit_async_request(MatrixRequest::SetIsFavorite {
                room_id: details.room_name_id.room_id().clone(),
                is_favorite: !details.is_favorite,
            });
            close_menu = true;
        }
        else if self.button(cx, ids!(priority_button)).clicked(actions) {
            submit_async_request(MatrixRequest::SetIsLowPriority {
                room_id: details.room_name_id.room_id().clone(),
                is_low_priority: !details.is_low_priority,
            });
            close_menu = true;
        }
        else if self.button(cx, ids!(copy_link_button)).clicked(actions) {
            submit_async_request(MatrixRequest::GenerateMatrixLink {
                room_id: details.room_name_id.room_id().clone(),
                event_id: None,
                use_matrix_scheme: false,
                join_on_click: false,
            });
            close_menu = true;
        }
         else if self.button(cx, ids!(room_settings_button)).clicked(actions) {
            // TODO: handle/implement this
            enqueue_popup_notification(
                "The room settings page is not yet implemented.",
                PopupKind::Warning,
                Some(5.0),
            );
            close_menu = true;
        }
        else if self.button(cx, ids!(notifications_button)).clicked(actions) {
            // TODO: handle/implement this
            enqueue_popup_notification(
                "The room notifications page is not yet implemented.",
                PopupKind::Warning,
                Some(5.0),
            );
            close_menu = true;
        }
        else if self.button(cx, ids!(invite_button)).clicked(actions) {
            cx.action(InviteModalAction::Open(details.room_name_id.clone()));
            close_menu = true;
        }
        else if self.button(cx, ids!(leave_button)).clicked(actions) {
            use crate::join_leave_room_modal::{JoinLeaveRoomModalAction, JoinLeaveModalKind};
            use crate::room::BasicRoomDetails;
            let room_details = BasicRoomDetails::Name(details.room_name_id.clone());
            cx.action(JoinLeaveRoomModalAction::Open {
                kind: JoinLeaveModalKind::LeaveRoom(room_details),
                show_tip: false,
            });
            close_menu = true;
        }

        if close_menu {
            self.close(cx);
        }
    }
}

impl RoomContextMenu {
    pub fn is_currently_shown(&self, _cx: &mut Cx) -> bool {
        self.visible
    }

    pub fn show(&mut self, cx: &mut Cx, details: RoomContextMenuDetails) -> DVec2 {
        self.update_buttons(cx, &details);
        self.details = Some(details);
        self.visible = true;
        cx.set_key_focus(self.view.area());
        expected_menu_size(NUM_BUTTONS, NUM_DIVIDERS)
    }

    fn update_buttons(&mut self, cx: &mut Cx, details: &RoomContextMenuDetails) {
        let mark_unread_button = self.button(cx, ids!(mark_unread_button));
        if details.is_marked_unread {
            mark_unread_button.set_text(cx, "Mark as Read");
        } else {
            mark_unread_button.set_text(cx, "Mark as Unread");
        }
        
        let favorite_button = self.button(cx, ids!(favorite_button));
        if details.is_favorite {
            favorite_button.set_text(cx, "Un-favorite");
        } else {
             favorite_button.set_text(cx, "Favorite");
        }

        let priority_button = self.button(cx, ids!(priority_button));
        if details.is_low_priority {
            priority_button.set_text(cx, "Un-set Low Priority");
        } else {
            priority_button.set_text(cx, "Set Low Priority");
        }
        
        // Reset hover states
        mark_unread_button.reset_hover(cx);
        favorite_button.reset_hover(cx);
        priority_button.reset_hover(cx);
        self.button(cx, ids!(copy_link_button)).reset_hover(cx);
        self.button(cx, ids!(room_settings_button)).reset_hover(cx);
        self.button(cx, ids!(notifications_button)).reset_hover(cx);
        self.button(cx, ids!(invite_button)).reset_hover(cx);
        self.button(cx, ids!(leave_button)).reset_hover(cx);

        self.redraw(cx);
    }

    fn close(&mut self, cx: &mut Cx) {
        self.visible = false;
        self.details = None;
        cx.revert_key_focus();
        cx.unblock_scrolling();
        cx.action(ContextMenuClosed);
        cx.clear_all_hovers();
        self.redraw(cx);
    }
}

impl RoomContextMenuRef {
    pub fn is_currently_shown(&self, cx: &mut Cx) -> bool {
        let Some(inner) = self.borrow() else { return false };
        inner.is_currently_shown(cx)
    }

    pub fn show(&self, cx: &mut Cx, details: RoomContextMenuDetails) -> DVec2 {
        let Some(mut inner) = self.borrow_mut() else { return DVec2::default()};
        inner.show(cx, details)
    }
}
