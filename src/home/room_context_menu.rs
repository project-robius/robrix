//! A context menu that appears when the user right-clicks
//! or long-presses on a room in the room list.

use makepad_widgets::*;
use matrix_sdk::ruma::OwnedRoomId;
use crate::{app::AppState, home::invite_modal::InviteModalAction, shared::popup_list::{PopupKind, enqueue_popup_notification}, sliding_sync::{MatrixRequest, current_user_id, submit_async_request}, utils::RoomNameId};

const BUTTON_HEIGHT: f64 = 35.0;
const MENU_WIDTH: f64 = 215.0;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*


    mod.widgets.ROOM_CONTEXT_MENU_BUTTON_HEIGHT = 35
    mod.widgets.ROOM_CONTEXT_MENU_WIDTH = 215

    mod.widgets.RoomContextMenuButton = RobrixIconButton {
        height: (mod.widgets.ROOM_CONTEXT_MENU_BUTTON_HEIGHT)
        width: Fill,
        margin: 0,
        icon_walk: Walk{width: 16, height: 16, margin: Inset{right: 3}}
        // Override the blue default back to neutral for context menu items
        draw_bg +: { color: (COLOR_PRIMARY), color_hover: #EBEBEB, color_down: #DCDCDC }
        draw_icon.color: #000
        draw_text +: { color: #000, color_hover: #000, color_down: #000 }
    }

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

        main_content := RoundedView {
            flow: Down
            width: (mod.widgets.ROOM_CONTEXT_MENU_WIDTH),
            height: Fit,
            padding: 5
            spacing: 0,
            align: Align{x: 0, y: 0}

            show_bg: true
            draw_bg +: {
                color: (COLOR_PRIMARY)
                border_radius: 5.0
                border_size: 0.5
                border_color: #888
            }

            mark_unread_button := mod.widgets.RoomContextMenuButton {
                draw_icon +: { svg: (ICON_CHECKMARK) }
                text: "Mark as Unread"
            }

            favorite_button := mod.widgets.RoomContextMenuButton {
                draw_icon +: { svg: (ICON_PIN) }
                text: "Favorite"
            }

            priority_button := mod.widgets.RoomContextMenuButton {
                draw_icon +: { svg: (ICON_TOMBSTONE) } 
                text: "Set Low Priority"
            }

            copy_link_button := mod.widgets.RoomContextMenuButton {
                draw_icon +: { svg: (ICON_LINK) }
                text: "Copy Link to Room"
            }
            
            divider1 := LineH {
                margin: Inset{top: 3, bottom: 3}
                width: Fill,
            }

            room_settings_button := mod.widgets.RoomContextMenuButton {
                draw_icon +: { svg: (ICON_SETTINGS) }
                text: "Settings"
            }

            notifications_button := mod.widgets.RoomContextMenuButton {
                // TODO: use a proper bell icon
                draw_icon +: { svg: (ICON_INFO) }
                text: "Notifications"
            }

            invite_button := mod.widgets.RoomContextMenuButton {
                draw_icon +: { svg: (ICON_ADD_USER) }
                text: "Invite"
            }

            bot_binding_button := mod.widgets.RoomContextMenuButton {
                draw_icon +: { svg: (ICON_HIERARCHY) }
                text: "Bind BotFather"
            }

            divider2 := LineH {
                margin: Inset{top: 3, bottom: 3}
                width: Fill,
            }

            leave_button := RobrixNegativeIconButton {
                height: (mod.widgets.ROOM_CONTEXT_MENU_BUTTON_HEIGHT)
                width: Fill,
                margin: 0,
                icon_walk: Walk{width: 16, height: 16, margin: Inset{right: 3}}
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
    pub app_service_enabled: bool,
    pub is_bot_bound: bool,
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
                     !self.view(cx, ids!(main_content)).area().rect(cx).contains(fue.abs)
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
        else if self.button(cx, ids!(bot_binding_button)).clicked(actions) {
            if let Some(app_state) = scope.data.get::<AppState>() {
                let room_id = details.room_name_id.room_id().clone();
                match app_state.bot_settings.resolved_bot_user_id_for_room(
                    &room_id,
                    current_user_id().as_deref(),
                ) {
                    Ok(bot_user_id) => {
                        if details.is_bot_bound {
                            submit_async_request(MatrixRequest::SetRoomBotBinding {
                                room_id,
                                bound: false,
                                bot_user_id: bot_user_id.clone(),
                            });
                            enqueue_popup_notification(
                                format!("Removing BotFather {bot_user_id} from this room..."),
                                PopupKind::Info,
                                Some(4.0),
                            );
                        } else {
                            submit_async_request(MatrixRequest::SetRoomBotBinding {
                                room_id,
                                bound: true,
                                bot_user_id: bot_user_id.clone(),
                            });
                            enqueue_popup_notification(
                                format!("Inviting BotFather {bot_user_id} into this room..."),
                                PopupKind::Info,
                                Some(5.0),
                            );
                        }
                    }
                    Err(error) => {
                        enqueue_popup_notification(error, PopupKind::Error, Some(5.0));
                    }
                }
            } else {
                enqueue_popup_notification(
                    "Bot settings are unavailable right now.",
                    PopupKind::Error,
                    Some(5.0),
                );
            }
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
        let height = self.update_buttons(cx, &details);
        self.details = Some(details);
        self.visible = true;
        cx.set_key_focus(self.view.area());
        dvec2(MENU_WIDTH, height)
    }
    
    fn update_buttons(&mut self, cx: &mut Cx, details: &RoomContextMenuDetails) -> f64 {
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

        let bot_binding_button = self.button(cx, ids!(bot_binding_button));
        bot_binding_button.set_visible(cx, details.app_service_enabled);
        if details.is_bot_bound {
            bot_binding_button.set_text(cx, "Unbind BotFather");
        } else {
            bot_binding_button.set_text(cx, "Bind BotFather");
        }
        
        // Reset hover states
        mark_unread_button.reset_hover(cx);
        favorite_button.reset_hover(cx);
        priority_button.reset_hover(cx);
        self.button(cx, ids!(copy_link_button)).reset_hover(cx);
        self.button(cx, ids!(room_settings_button)).reset_hover(cx);
        self.button(cx, ids!(notifications_button)).reset_hover(cx);
        self.button(cx, ids!(invite_button)).reset_hover(cx);
        bot_binding_button.reset_hover(cx);
        self.button(cx, ids!(leave_button)).reset_hover(cx);
        
        self.redraw(cx);
        
        // Calculate height (rudimentary) - sum of visible buttons + padding
        // 8 or 9 buttons * 35.0 + 2 dividers * ~10.0 + padding
        ((if details.app_service_enabled { 9.0 } else { 8.0 }) * BUTTON_HEIGHT) + 20.0 + 10.0 // approx
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
