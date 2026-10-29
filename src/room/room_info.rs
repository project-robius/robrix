//! A view of a room's details, e.g., its avatar, topic, and member count, with buttons for common actions.
//! It subscribes to its room's info itself, so it can be docked within a RoomScreen or popped out.

use makepad_widgets::*;
use matrix_sdk::{notification_settings::RoomNotificationMode, ruma::{OwnedMxcUri, OwnedRoomAliasId, OwnedRoomId}};

use crate::{
    home::{invite_modal::InviteModalAction, rooms_list::{RoomAvatarUpdated, RoomsListRef}},
    join_leave_room_modal::{JoinLeaveModalKind, JoinLeaveRoomModalAction},
    room::pane_dock::FRAME_PADDING,
    shared::avatar::AvatarWidgetExt,
    sliding_sync::{MatrixRequest, RoomDataKind, TimelineEndpointsRecreated, submit_async_request},
    utils::RoomNameId,
};
use super::{BasicRoomDetails, FetchedRoomAvatar, message_list::RoomDataSubscription, room_pane::RoomPaneKind};

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    let RoomInfoButton = mod.widgets.ContextMenuButton {
        padding: Inset{left: 10, right: 27, top: 9, bottom: 9}
        label_walk: Walk{width: Fill, height: Fit}
        draw_text +: { max_lines: 1, text_overflow: TextOverflow.Ellipsis }
        draw_bg +: {
            // We fade the hover color by its own alpha, since mix()ing it with a transparent
            // background pulls it toward black, which flashes darker as it fades out.
            pixel: fn() {
                let sdf = Sdf2d.viewport(self.pos * self.rect_size)
                sdf.box(0.0, 0.0, self.rect_size.x, self.rect_size.y, self.border_radius)
                let hover_color = vec4(self.color_hover.xyz, self.color_hover.w * self.hover)
                sdf.fill(mix(hover_color, self.color_down, self.down))
                return sdf.result
            }
        }
    }

    let RoomInfoPaneButton = View {
        width: Fill, height: Fit
        flow: Overlay
        align: Align{x: 1.0, y: 0.5}
    }

    let RoomInfoPaneArrow = Icon {
        margin: Inset{right: 12}
        icon_walk: Walk{width: 7, height: 12}
        draw_icon +: {
            svg: crate_resource("self://resources/icons/forward.svg")
            color: (COLOR_TEXT_SUBTLE)
        }
    }

    let RoomInfoDetailsLabel = Label {
        width: Fit, height: Fit
        draw_text +: {
            text_style: REGULAR_TEXT {font_size: 10}
            color: (COLOR_TEXT_SUBTLE)
        }
        text: ""
    }

    mod.widgets.RoomInfoView = #(RoomInfoView::register_widget(vm)) {
        width: Fill, height: Fill
        flow: Down

        room_info_scroll := ScrollYView {
            width: Fill, height: Fill
            flow: Down, spacing: 2
            // This has the right padding instead of the pane, so its scroll bar sits at the pane's edge.
            padding: Inset{right: #(FRAME_PADDING)}

            room_header := View {
                width: Fill, height: Fit
                flow: Down, spacing: 8
                align: Align{x: 0.5, y: 0}
                padding: Inset{top: 4, bottom: 10}

                room_avatar := Avatar {
                    width: 56, height: 56
                    text_view +: {
                        text +: {
                            draw_text +: { text_style: TITLE_TEXT {font_size: 18} }
                        }
                    }
                }

                room_name := Label {
                    width: Fill, height: Fit
                    align: Align{x: 0.5, y: 0}
                    flow: Flow.Right{wrap: true}
                    draw_text +: {
                        text_style: TITLE_TEXT {font_size: 14}
                        color: (ROOM_NAME_TEXT_COLOR)
                    }
                    text: ""
                }

                room_alias := Label {
                    width: Fill, height: Fit
                    align: Align{x: 0.5, y: 0}
                    margin: Inset{top: -4}
                    flow: Flow.Right{wrap: true}
                    draw_text +: {
                        text_style: TITLE_TEXT {font_size: 10}
                        color: (COLOR_TEXT_FAINT)
                    }
                    text: ""
                }

                room_details := View {
                    width: Fill, height: Fit
                    flow: Flow.Right{wrap: true}
                    align: Align{x: 0.5, y: 0}
                    spacing: 14
                    // The same spacing between two regular wrapped lines of text.
                    wrap_spacing: 3.52
                    room_encryption := RoomInfoDetailsLabel {}
                    room_member_count := RoomInfoDetailsLabel {}
                }

                room_topic := CollapsibleLabel {
                    margin: Inset{top: 4}
                    label +: {
                        align: Align{x: 0.5, y: 0}
                        draw_text +: {
                            text_style: REGULAR_TEXT {font_size: 10.5}
                            color: (MESSAGE_TEXT_COLOR)
                        }
                    }
                }
            }

            LineH { margin: Inset{bottom: 6} }

            members := RoomInfoPaneButton {
                button := RoomInfoButton {
                    draw_icon.svg: (ICON_MEMBERS)
                    text: "Members"
                }
                RoomInfoPaneArrow {}
            }
            pinned_messages := RoomInfoPaneButton {
                button := RoomInfoButton {
                    draw_icon.svg: (ICON_PIN)
                    text: "Pinned messages"
                }
                RoomInfoPaneArrow {}
            }
            threads := RoomInfoPaneButton {
                button := RoomInfoButton {
                    draw_icon.svg: (ICON_REPLY_IN_THREAD)
                    text: "Threads"
                }
                RoomInfoPaneArrow {}
            }

            LineH { margin: Inset{top: 6, bottom: 6} }

            notifications_label := Label {
                width: Fill, height: Fit
                // A Label's padding counts twice, so this lines its text up with the dropdown's text.
                padding: Inset{left: 9, top: 4}
                draw_text +: {
                    text_style: REGULAR_TEXT {font_size: 10}
                    color: (COLOR_TEXT_SUBTLE)
                }
                text: "Notifications"
            }
            notification_mode_dropdown := mod.widgets.RobrixSettingsDropDown {
                width: Fill {max: 320}
                margin: Inset{left: 6, top: 4, bottom: 4}
                popup_min_width: 220
            }
            Label {
                width: Fill, height: Fit
                flow: Flow.Right{wrap: true}
                padding: Inset{left: 9, bottom: 8}
                draw_text +: {
                    text_style: REGULAR_TEXT {font_size: 9}
                    color: (COLOR_TEXT_FAINT)
                }
                text: "Robrix doesn't yet display notifications."
            }

            room_actions := View {
                width: Fill, height: Fit
                flow: Down, spacing: 10
                padding: Inset{left: 6, top: 4, bottom: 10}
                copy_link_button := RobrixNeutralIconButton {
                    padding: Inset{top: 10, bottom: 10, left: 12, right: 15}
                    margin: 0
                    draw_icon.svg: (ICON_LINK)
                    icon_walk: Walk{width: 16, height: 16, margin: Inset{left: -1, right: -1}}
                    text: "Copy Link to Room"
                }
                invite_button := RobrixPositiveIconButton {
                    padding: Inset{top: 10, bottom: 10, left: 12, right: 15}
                    margin: 0
                    draw_icon.svg: (ICON_ADD_USER)
                    icon_walk: Walk{width: 16, height: 16, margin: Inset{left: -2, right: -1}}
                    text: "Invite"
                }
                leave_button := RobrixNegativeIconButton {
                    padding: Inset{top: 10, bottom: 10, left: 12, right: 15}
                    margin: 0
                    draw_icon.svg: (ICON_LOGOUT)
                    icon_walk: Walk{width: 16, height: 16, margin: Inset{left: -1, right: -1}}
                    text: "Leave Room"
                }
            }
        }
    }
}

pub(crate) const NOTIFICATION_MODES: [(Option<RoomNotificationMode>, &str); 4] = [
    (None, "Default"),
    (Some(RoomNotificationMode::AllMessages), "All messages"),
    (Some(RoomNotificationMode::MentionsAndKeywordsOnly), "Mentions and keywords"),
    (Some(RoomNotificationMode::Mute), "Mute"),
];

#[derive(Clone, Debug, PartialEq)]
pub struct RoomInfoDetails {
    pub topic: Option<String>,
    /// The room's canonical alias, or else its first alternative alias.
    pub alias: Option<OwnedRoomAliasId>,
    pub avatar_url: Option<OwnedMxcUri>,
    pub is_encrypted: Option<bool>,
    pub num_joined_members: u64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RoomNotificationModes {
    pub user_defined: Option<RoomNotificationMode>,
    pub default: RoomNotificationMode,
}

/// A room's info, as posted by the worker whenever it changes, see [`RoomDataKind::RoomInfo`].
///
/// This is NOT a widget action.
#[derive(Debug)]
pub enum RoomInfoAction {
    Updated {
        room_id: OwnedRoomId,
        details: RoomInfoDetails,
    },
    NotificationModesUpdated {
        room_id: OwnedRoomId,
        modes: RoomNotificationModes,
    },
}

/// Widget actions emitted by a [`RoomInfoView`].
#[derive(Clone, Debug, Default)]
pub enum RoomInfoViewAction {
    /// The user clicked the button for the given kind of pane,
    /// which should be shown in place of this room info pane.
    ShowPane(RoomPaneKind),
    #[default]
    None,
}

/// The state of a [`RoomInfoView`] that is saved and restored.
#[derive(Default)]
pub struct SavedRoomInfoView {
    pub(super) subscription: Option<RoomDataSubscription>,
    pub(super) details: Option<RoomInfoDetails>,
    notification_modes: Option<RoomNotificationModes>,
}

#[derive(Script, Widget)]
pub struct RoomInfoView {
    #[deref] view: View,

    #[rust] room_name_id: Option<RoomNameId>,
    #[rust] subscription: Option<RoomDataSubscription>,
    #[rust] details: Option<RoomInfoDetails>,
    #[rust] notification_modes: Option<RoomNotificationModes>,
}

impl ScriptHook for RoomInfoView {
    fn on_after_apply(&mut self, vm: &mut ScriptVm, apply: &Apply, _scope: &mut Scope, _value: ScriptValue) {
        // A reload resets everything we set on our children back to its DSL default, so we set it all again.
        if apply.is_reload() {
            vm.with_cx_mut(|cx| {
                if let Some(details) = self.details.take() {
                    self.set_details(cx, details);
                }
                self.show_notification_modes(cx);
                self.show_room_name(cx);
                self.show_avatar(cx);
            });
        }
    }
}

impl Widget for RoomInfoView {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if self.room_name_id.is_none() { return }
        self.view.handle_event(cx, event, scope);

        let Event::Actions(actions) = event else { return };
        for action in actions {
            match action.downcast_ref() {
                Some(RoomInfoAction::Updated { room_id, details }) if self.is_showing_room(room_id) => {
                    self.set_details(cx, details.clone());
                }
                Some(RoomInfoAction::NotificationModesUpdated { room_id, modes }) if self.is_showing_room(room_id) => {
                    self.notification_modes = Some(*modes);
                    self.show_notification_modes(cx);
                }
                _ => {}
            }
            if let Some(TimelineEndpointsRecreated { room_id }) = action.downcast_ref()
                && self.is_showing_room(room_id)
            {
                self.subscribe();
            }
            if let Some(RoomAvatarUpdated { room_id }) = action.downcast_ref()
                && self.is_showing_room(room_id)
            {
                self.show_avatar(cx);
            }
        }

        let Some(room_name_id) = self.room_name_id.as_ref() else { return };
        let scroll = self.view.child(id!(room_info_scroll));
        for (id, kind) in [
            (id!(members), RoomPaneKind::Members),
            (id!(pinned_messages), RoomPaneKind::PinnedMessages),
            (id!(threads), RoomPaneKind::Threads),
        ] {
            if scroll.child(id).child(id!(button)).as_button().clicked(actions) {
                cx.widget_action(self.widget_uid(), RoomInfoViewAction::ShowPane(kind));
            }
        }
        if let Some(index) = scroll.child(id!(notification_mode_dropdown)).as_drop_down2().changed(actions)
            && let Some((mode, _)) = NOTIFICATION_MODES.get(index)
        {
            submit_async_request(MatrixRequest::SetRoomNotificationMode {
                room_id: room_name_id.room_id().clone(),
                mode: *mode,
            });
        }
        let room_actions = scroll.child(id!(room_actions));
        if room_actions.child(id!(copy_link_button)).as_button().clicked(actions) {
            submit_async_request(MatrixRequest::GenerateMatrixLink {
                room_id: room_name_id.room_id().clone(),
                event_id: None,
                use_matrix_scheme: false,
                join_on_click: false,
                copy_to_clipboard: true,
            });
        }
        if room_actions.child(id!(invite_button)).as_button().clicked(actions) {
            cx.action(InviteModalAction::Open(room_name_id.clone()));
        }
        if room_actions.child(id!(leave_button)).as_button().clicked(actions) {
            cx.action(JoinLeaveRoomModalAction::Open {
                kind: JoinLeaveModalKind::LeaveRoom(BasicRoomDetails::Name(room_name_id.clone())),
                show_tip: false,
            });
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl RoomInfoView {
    fn is_showing_room(&self, room_id: &OwnedRoomId) -> bool {
        self.room_name_id.as_ref().is_some_and(|r| r.room_id() == room_id)
    }

    fn subscribe(&mut self) {
        let Some(room_name_id) = self.room_name_id.as_ref() else { return };
        RoomDataSubscription::subscribe(&mut self.subscription, room_name_id.room_id(), RoomDataKind::RoomInfo);
    }

    /// Shows the info of the given room.
    ///
    /// Call this again with the same room whenever its name changes.
    fn set_room(&mut self, cx: &mut Cx, room_name_id: &RoomNameId) {
        if !self.is_showing_room(room_name_id.room_id()) {
            self.reset(cx);
        }
        self.show_room(cx, room_name_id);
    }

    fn show_room(&mut self, cx: &mut Cx, room_name_id: &RoomNameId) {
        self.room_name_id = Some(room_name_id.clone());
        self.subscribe();
        self.show_room_name(cx);
        self.show_avatar(cx);
    }

    fn set_details(&mut self, cx: &mut Cx, details: RoomInfoDetails) {
        let encryption = match details.is_encrypted {
            Some(true) => "🔒  Encrypted",
            Some(false) => "🔓  Not encrypted",
            None => "",
        };
        self.view.label(cx, ids!(room_encryption)).set_text(cx, encryption);
        let member_count = match details.num_joined_members {
            1 => String::from("👥  1 member"),
            n => format!("👥  {n} members"),
        };
        self.view.label(cx, ids!(room_member_count)).set_text(cx, &member_count);
        let topic = details.topic.as_deref().map_or("", str::trim);
        let room_topic = self.view.widget(cx, ids!(room_topic));
        room_topic.set_text(cx, topic);
        room_topic.set_visible(cx, !topic.is_empty());
        if let Some(room_name_id) = self.room_name_id.as_ref()
            && details.avatar_url.is_some()
            && self.details.as_ref().is_none_or(|old| old.avatar_url != details.avatar_url)
            && !matches!(get_rooms_list_avatar(cx, room_name_id), Some(FetchedRoomAvatar::Image(_)))
        {
            submit_async_request(MatrixRequest::FetchRoomAvatar { room_name_id: room_name_id.clone() });
        }
        self.details = Some(details);
        self.show_room_name(cx);
        self.redraw(cx);
    }

    fn show_notification_modes(&mut self, cx: &mut Cx) {
        let default_label = self.notification_modes.and_then(|modes|
            NOTIFICATION_MODES.iter().find(|(mode, _)| *mode == Some(modes.default)).map(|(_, label)| *label)
        );
        let labels = NOTIFICATION_MODES.iter()
            .map(|(mode, label)| match (mode, default_label) {
                (None, Some(default_label)) => format!("{label} ({default_label})"),
                _ => label.to_string(),
            })
            .collect();
        let index = self.notification_modes
            .and_then(|modes| NOTIFICATION_MODES.iter().position(|(mode, _)| *mode == modes.user_defined))
            .unwrap_or(0);
        let dropdown = self.view.child(id!(room_info_scroll)).child(id!(notification_mode_dropdown)).as_drop_down2();
        dropdown.set_labels(cx, labels);
        dropdown.set_selected_item(cx, index);
    }

    fn show_room_name(&mut self, cx: &mut Cx) {
        let Some(room_name_id) = self.room_name_id.as_ref() else { return };
        let alias = self.details.as_ref().and_then(|details| details.alias.as_ref());
        let name = match (room_name_id.is_empty(), alias) {
            (false, _) => room_name_id.display(),
            (true, Some(alias)) => alias.as_str().into(),
            (true, None) => room_name_id.room_id().as_str().into(),
        };
        self.view.label(cx, ids!(room_name)).set_text(cx, &name);
        let alias = alias.map(|alias| alias.as_str()).filter(|alias| *alias != name);
        let room_alias = self.view.label(cx, ids!(room_alias));
        room_alias.set_text(cx, alias.unwrap_or_default());
        room_alias.set_visible(cx, alias.is_some());
    }

    fn show_avatar(&mut self, cx: &mut Cx) {
        let Some(room_name_id) = self.room_name_id.as_ref() else { return };
        let rooms_list_avatar = get_rooms_list_avatar(cx, room_name_id);
        self.view.avatar(cx, ids!(room_avatar)).show_room_avatar(cx, rooms_list_avatar.as_ref(), room_name_id);
    }

    /// Clears this view and unsubscribes from its room's info,
    /// such that it shows nothing until `set_room()` is called.
    fn reset(&mut self, cx: &mut Cx) {
        self.room_name_id = None;
        self.subscription = None;
        self.details = None;
        self.notification_modes = None;
        for id in [ids!(room_name), ids!(room_alias), ids!(room_encryption), ids!(room_member_count)] {
            self.view.label(cx, id).set_text(cx, "");
        }
        let room_topic = self.view.widget(cx, ids!(room_topic));
        room_topic.set_text(cx, "");
        room_topic.set_visible(cx, false);
        self.show_notification_modes(cx);
        self.view.view(cx, ids!(room_info_scroll)).set_scroll_pos(cx, Vec2d::default());
        self.redraw(cx);
    }
}

impl RoomInfoViewRef {
    /// See [`RoomInfoView::set_room()`].
    pub fn set_room(&self, cx: &mut Cx, room_name_id: &RoomNameId) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_room(cx, room_name_id);
        }
    }

    /// See [`RoomInfoView::reset()`].
    pub fn reset(&self, cx: &mut Cx) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.reset(cx);
        }
    }

    /// Saves and returns this view's state, which takes its subscription with it.
    pub fn save_state(&self) -> SavedRoomInfoView {
        let Some(mut inner) = self.borrow_mut() else { return SavedRoomInfoView::default() };
        SavedRoomInfoView {
            subscription: inner.subscription.take(),
            details: inner.details.clone(),
            notification_modes: inner.notification_modes,
        }
    }

    /// Restores the given room's info from a saved snapshot, until the worker posts it again.
    pub fn restore_state(&self, cx: &mut Cx, room_name_id: &RoomNameId, saved: SavedRoomInfoView) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.reset(cx);
        inner.subscription = saved.subscription;
        inner.show_room(cx, room_name_id);
        if let Some(details) = saved.details {
            inner.set_details(cx, details);
        }
        inner.notification_modes = saved.notification_modes;
        inner.show_notification_modes(cx);
    }
}

fn get_rooms_list_avatar(cx: &mut Cx, room_name_id: &RoomNameId) -> Option<FetchedRoomAvatar> {
    cx.has_global::<RoomsListRef>()
        .then(|| cx.get_global::<RoomsListRef>().get_room_avatar(room_name_id.room_id()))
        .flatten()
}
