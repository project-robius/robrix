//! A context menu that appears when the user right-clicks
//! or long-presses on a room in the room list.

use makepad_widgets::*;
use matrix_sdk::ruma::OwnedRoomId;

const BUTTON_HEIGHT: f64 = 35.0;
const MENU_WIDTH: f64 = 215.0;

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::helpers::*;
    use crate::shared::styles::*;
    use crate::shared::icon_button::*;

    BUTTON_HEIGHT = 35
    MENU_WIDTH = 215

    ContextMenuButton = <RobrixIconButton> {
        height: (BUTTON_HEIGHT)
        width: Fill,
        margin: 0,
        icon_walk: {width: 16, height: 16, margin: {right: 3}}
    }

    pub RoomContextMenu = {{RoomContextMenu}} {
        visible: false,
        flow: Overlay,
        width: Fill,
        height: Fill,
        cursor: Default,
        align: {x: 0, y: 0}

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
            padding: 5
            spacing: 0,
            align: {x: 0, y: 0}

            show_bg: true
            draw_bg: {
                color: #fff
                border_radius: 5.0
                border_size: 0.5
                border_color: #888
            }

            mark_unread_button = <ContextMenuButton> {
                draw_icon: { svg_file: (ICON_CHECKMARK) }
                text: "Mark as Read"
            }

            favorite_button = <ContextMenuButton> {
                draw_icon: { svg_file: (ICON_PIN) }
                text: "Favorite"
            }

            priority_button = <ContextMenuButton> {
                draw_icon: { svg_file: (ICON_TOMBSTONE) } 
                text: "Set Low Priority"
            }

            share_button = <ContextMenuButton> {
                draw_icon: { svg_file: (ICON_LINK) }
                text: "Copy Link to Room"
            }
            
            divider1 = <LineH> {
                margin: {top: 3, bottom: 3}
                width: Fill,
            }

            settings_button = <ContextMenuButton> {
                draw_icon: { svg_file: (ICON_SETTINGS) }
                text: "Settings"
            }

            notifications_button = <ContextMenuButton> {
                // TODO: use a proper bell icon
                draw_icon: { svg_file: (ICON_INFO) }
                text: "Notifications"
            }

            invite_button = <ContextMenuButton> {
                draw_icon: { svg_file: (ICON_ADD_USER) }
                text: "Invite"
            }

            divider2 = <LineH> {
                margin: {top: 3, bottom: 3}
                width: Fill,
            }

            leave_button = <ContextMenuButton> {
                draw_icon: {
                    svg_file: (ICON_LOGOUT)
                    color: (COLOR_FG_DANGER_RED),
                }
                draw_bg: {
                    border_color: (COLOR_FG_DANGER_RED),
                    color: (COLOR_BG_DANGER_RED)
                }
                text: "Leave Room"
                draw_text:{
                    color: (COLOR_FG_DANGER_RED),
                }
            }
        }
    }
}

use crate::{sliding_sync::{MatrixRequest, submit_async_request}, utils::RoomNameId};

#[derive(Clone, Debug)]
pub struct RoomContextMenuDetails {
    pub room_id: OwnedRoomId,
    pub room_name_id: RoomNameId,
    pub is_favorite: bool,
    pub is_low_priority: bool,
    pub is_unread: bool,
}

#[derive(Clone, DefaultNone, Debug)]
pub enum RoomContextMenuAction {
    SetFavorite(OwnedRoomId, bool),
    SetLowPriority(OwnedRoomId, bool),
    Notifications(OwnedRoomId),
    Invite(OwnedRoomId),
    CopyLink(OwnedRoomId),
    // LeaveRoom is handled directly by emitting JoinLeaveRoomModalAction
    OpenSettings(OwnedRoomId),
    None,
}

#[derive(Live, LiveHook, Widget)]
pub struct RoomContextMenu {
    #[deref] view: View,
    #[rust] details: Option<RoomContextMenuDetails>,
}

impl Widget for RoomContextMenu {
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        if self.details.is_none() {
            self.visible = false;
        };
        self.view.draw_walk(cx, scope, walk)
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
                     !self.view(ids!(main_content)).area().rect(cx).contains(fue.abs)
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

impl WidgetMatchEvent for RoomContextMenu {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions, scope: &mut Scope) {
        let Some(details) = self.details.as_ref() else { return };
        let mut action_to_dispatch = RoomContextMenuAction::None;
        let mut close_menu = false;
        
        if self.button(ids!(mark_unread_button)).clicked(actions) {
            // Toggle unread status
            submit_async_request(MatrixRequest::SetUnreadFlag {
                room_id: details.room_id.clone(),
                is_unread: !details.is_unread,
            });
            close_menu = true;
        } 
        else if self.button(ids!(favorite_button)).clicked(actions) {
            action_to_dispatch = RoomContextMenuAction::SetFavorite(details.room_id.clone(), !details.is_favorite);
            close_menu = true;
        }
        else if self.button(ids!(priority_button)).clicked(actions) {
            action_to_dispatch = RoomContextMenuAction::SetLowPriority(details.room_id.clone(), !details.is_low_priority);
            close_menu = true;
        }
        else if self.button(ids!(share_button)).clicked(actions) {
            action_to_dispatch = RoomContextMenuAction::CopyLink(details.room_id.clone());
            close_menu = true;
        }
         else if self.button(ids!(settings_button)).clicked(actions) {
            action_to_dispatch = RoomContextMenuAction::OpenSettings(details.room_id.clone());
            close_menu = true;
        }
        else if self.button(ids!(notifications_button)).clicked(actions) {
            action_to_dispatch = RoomContextMenuAction::Notifications(details.room_id.clone());
            close_menu = true;
        }
        else if self.button(ids!(invite_button)).clicked(actions) {
            action_to_dispatch = RoomContextMenuAction::Invite(details.room_id.clone());
            close_menu = true;
        }
        else if self.button(ids!(leave_button)).clicked(actions) {
            use crate::join_leave_room_modal::{JoinLeaveRoomModalAction, JoinLeaveModalKind};
            use crate::room::BasicRoomDetails;
            let room_details = BasicRoomDetails::Name(details.room_name_id.clone());
            cx.action(JoinLeaveRoomModalAction::Open {
                kind: JoinLeaveModalKind::LeaveRoom(room_details),
                show_tip: false,
            });
            close_menu = true;
        }

        if !matches!(action_to_dispatch, RoomContextMenuAction::None) {
            cx.widget_action(self.widget_uid(), &scope.path, action_to_dispatch);
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
        self.details = Some(details.clone());
        self.visible = true;
        cx.set_key_focus(self.view.area());
        
        let height = self.update_buttons(cx, &details);
        dvec2(MENU_WIDTH, height)
    }
    
    fn update_buttons(&mut self, cx: &mut Cx, details: &RoomContextMenuDetails) -> f64 {
        let mark_read_btn = self.button(ids!(mark_unread_button));
        if details.is_unread {
            mark_read_btn.set_text(cx, "Mark as Read");
            // mark_read_btn.draw_icon.svg_file = ...; // Optional: change icon
        } else {
             mark_read_btn.set_text(cx, "Mark as Unread");
        }
        
        let fav_btn = self.button(ids!(favorite_button));
        if details.is_favorite {
            fav_btn.set_text(cx, "Unfavorite");
        } else {
             fav_btn.set_text(cx, "Favorite");
        }

        let priority_btn = self.button(ids!(priority_button));
        if details.is_low_priority {
            priority_btn.set_text(cx, "Set Standard Priority");
        } else {
            priority_btn.set_text(cx, "Set Low Priority");
        }
        
        // Reset hover states
        mark_read_btn.reset_hover(cx);
        fav_btn.reset_hover(cx);
        priority_btn.reset_hover(cx);
        self.button(ids!(share_button)).reset_hover(cx);
        self.button(ids!(share_button)).reset_hover(cx);
        self.button(ids!(settings_button)).reset_hover(cx);
        self.button(ids!(notifications_button)).reset_hover(cx);
        self.button(ids!(invite_button)).reset_hover(cx);
        self.button(ids!(leave_button)).reset_hover(cx);
        
        self.redraw(cx);
        
        // Calculate height (rudimentary) - sum of visible buttons + padding
        // 8 buttons * 35.0 + 2 dividers * ~10.0 + padding
        (8.0 * BUTTON_HEIGHT) + 20.0 + 10.0 // approx
    }

    fn close(&mut self, cx: &mut Cx) {
        self.visible = false;
        self.details = None;
        cx.revert_key_focus();
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
