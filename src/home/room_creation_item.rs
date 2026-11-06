//! A `RoomCreationItem` shows the room's creation event at the very beginning of its timeline.
//!
//! Above that event (and the rest of the room's setup, if it's grouped with it), it shows the room's avatar, name,
//! topic and settings so it's obvious you've reached the top, plus a link to the older room that it replaced, if any.

use std::sync::Arc;
use imbl::Vector;
use makepad_widgets::*;
use matrix_sdk::ruma::{
    OwnedRoomId,
    events::{StateEventContentChange, room::{guest_access::GuestAccess, history_visibility::HistoryVisibility, join_rules::JoinRule}},
};
use matrix_sdk_ui::timeline::{AnyOtherStateEventContentChange, EventTimelineItem, TimelineItem, TimelineItemContent};

use crate::{
    app::AppStateAction,
    home::{
        room_read_receipt::populate_read_receipts,
        room_screen::ItemDrawnStatus,
        rooms_list::RoomAvatarUpdated,
        new_message_context_menu::MessageDetails,
        small_state_event::{populate_group_summary, populate_small_state_event_widget},
        state_event_group::{SmallStateEventWidgetRefExt, StateEventGroup, StateEventGroupHeaderWidgetRefExt},
        timeline_items::{ChangedItems, SmallStateContent, TimelineInfo, is_room_create, items_in},
    },
    room::{PredecessorRoomDetails, room_info::get_rooms_list_avatar},
    shared::avatar::AvatarWidgetExt,
    utils::RoomNameId,
};

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    // A small outlined pill for one of the room's settings, which is colored by how open that setting is.
    let SettingPill = RoundedView {
        visible: false
        width: Fit, height: Fit
        padding: Inset{top: 3, bottom: 3, left: 8, right: 8}
        show_bg: true
        draw_bg +: {
            color: (COLOR_PRIMARY)
            border_size: 1.0
            border_radius: 9.0
        }
        label := Label {
            draw_text +: { text_style: REGULAR_TEXT {font_size: 9} }
            text: ""
        }
    }
    let GreenPill = SettingPill {
        draw_bg +: { border_color: (COLOR_FG_ACCEPT_GREEN) }
        label +: { draw_text +: { color: (COLOR_FG_ACCEPT_GREEN) } }
    }
    let BluePill = SettingPill {
        draw_bg +: { border_color: (COLOR_ACTIVE_PRIMARY_DARKER) }
        label +: { draw_text +: { color: (COLOR_ACTIVE_PRIMARY_DARKER) } }
    }
    let OrangePill = SettingPill {
        draw_bg +: { border_color: (COLOR_TEXT_WARNING_NOT_FOUND) }
        label +: { draw_text +: { color: (COLOR_TEXT_WARNING_NOT_FOUND) } }
    }
    let PurplePill = SettingPill {
        draw_bg +: { border_color: (COLOR_TEXT_PURPLE) }
        label +: { draw_text +: { color: (COLOR_TEXT_PURPLE) } }
    }
    let GrayPill = SettingPill {
        draw_bg +: { border_color: (COLOR_TEXT_SUBTLE) }
        label +: { draw_text +: { color: (COLOR_TEXT_SUBTLE) } }
    }

    mod.widgets.RoomCreationItem = #(RoomCreationItem::register_widget(vm)) {
        width: Fill,
        height: Fit,
        flow: Down,

        intro := RoundedView {
            width: Fill, height: Fit
            flow: Down
            align: Align{x: 0.5}
            margin: Inset{top: 10, bottom: 6, left: 10, right: 10}
            padding: Inset{top: 18, bottom: 18, left: 16, right: 16}
            spacing: 6
            show_bg: true
            draw_bg +: {
                color: (COLOR_SECONDARY)
                border_radius: 8.0
            }

            room_avatar := Avatar {
                width: 48, height: 48
                margin: Inset{bottom: 4}
                cursor: MouseCursor.Default
                text_view +: {
                    text +: {
                        draw_text +: { text_style: TITLE_TEXT {font_size: 16} }
                    }
                }
            }

            room_name := Label {
                width: Fill, height: Fit
                flow: Flow.Right{wrap: true}
                align: Align{x: 0.5}
                draw_text +: {
                    text_style: TITLE_TEXT {font_size: 14}
                    color: (ROOM_NAME_TEXT_COLOR)
                }
                text: ""
            }

            room_alias := Label {
                visible: false
                width: Fill, height: Fit
                flow: Flow.Right{wrap: true}
                align: Align{x: 0.5}
                margin: Inset{top: -4}
                draw_text +: {
                    text_style: TITLE_TEXT {font_size: 10}
                    color: (COLOR_TEXT_FAINT)
                }
                text: ""
            }

            room_topic := CollapsibleHtml {
                visible: false
                collapsed_lines: 3
                align: Align{x: 0.5, y: 0}
                html +: {
                    align: Align{x: 0.5, y: 0.5}
                    font_size: 10.5
                    font_color: (MESSAGE_TEXT_COLOR)
                }
            }

            description := Label {
                width: Fill, height: Fit
                flow: Flow.Right{wrap: true}
                align: Align{x: 0.5}
                draw_text +: {
                    text_style: REGULAR_TEXT {font_size: 10}
                    color: (COLOR_TEXT_SUBTLE)
                }
                text: "This is the beginning of the room."
            }

            older_room_button := mod.widgets.OlderRoomButton {
                margin: Inset{bottom: 6}
                // Only a tap opens the older room, not a press that scrolls the timeline it's in.
                enable_long_press: true
                reset_hover_on_click: true
            }

            settings := View {
                width: Fill, height: Fit
                flow: Flow.Right{wrap: true}
                align: Align{x: 0.5}
                margin: Inset{top: 6}
                spacing: 6
                wrap_spacing: 6

                encrypted := GreenPill { label +: { text: "🔒 End-to-end encrypted" } }
                not_encrypted := OrangePill { label +: { text: "🔓 Not encrypted" } }
                invite_only := GreenPill { label +: { text: "📩 Invite only" } }
                space_members_can_join := BluePill { label +: { text: "🧩 Space members can join" } }
                ask_to_join := BluePill { label +: { text: "🙋 Ask to join" } }
                anyone_can_join := BluePill { label +: { text: "🌐 Anyone can join" } }
                anyone_can_read := OrangePill { label +: { text: "👀 Anyone can read" } }
                full_history := BluePill { label +: { text: "📜 Members see all history" } }
                history_since_invite := GreenPill { label +: { text: "📜 History since invite" } }
                history_since_joining := GreenPill { label +: { text: "📜 History since joining" } }
                guests_allowed := OrangePill { label +: { text: "🧳 Guests allowed" } }
                limited_posting := PurplePill { label +: { text: "📣 Limited posting" } }
                local_only := GrayPill {}
                room_version := GrayPill {}
            }
        }

        header := mod.widgets.StateEventGroupHeader {}

        first_event := mod.widgets.SmallStateEvent {
            visible: false
            margin: Inset{ top: 0.0, bottom: 4.0 }
        }
    }
}

#[derive(Script, ScriptHook, Widget)]
pub struct RoomCreationItem {
    #[deref] view: View,
    #[rust] room_name_id: Option<RoomNameId>,
    #[rust] predecessor_room: Option<PredecessorRoomDetails>,
}

impl Widget for RoomCreationItem {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if let Event::Actions(actions) = event {
            if self.view.button(cx, ids!(older_room_button)).clicked(actions)
                && let Some(predecessor_room) = self.predecessor_room.clone()
            {
                cx.action(AppStateAction::NavigateToRoom {
                    room_to_close: None,
                    destination_room: predecessor_room.into(),
                });
            }
            for action in actions {
                if let Some(AppStateAction::RoomNameUpdated(new_room_name)) = action.downcast_ref()
                    && self.is_showing_room(new_room_name.room_id())
                {
                    self.room_name_id = Some(new_room_name.clone());
                    self.show_room(cx);
                }
                if let Some(RoomAvatarUpdated { room_id }) = action.downcast_ref()
                    && self.is_showing_room(room_id)
                {
                    self.show_room(cx);
                }
            }
        }
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl RoomCreationItem {
    fn is_showing_room(&self, room_id: &OwnedRoomId) -> bool {
        self.room_name_id.as_ref().is_some_and(|r| r.room_id() == room_id)
    }

    /// Shows the given room's name, avatar and settings, plus a link to the older room that it replaced, if any.
    ///
    /// The settings come from the room's state events in `items`, which are all loaded once its creation event is.
    fn set_room(
        &mut self,
        cx: &mut Cx,
        room_name_id: Option<&RoomNameId>,
        items: &Vector<Arc<TimelineItem>>,
        create_event: &EventTimelineItem,
        create_event_content: &SmallStateContent,
    ) {
        use AnyOtherStateEventContentChange as C;
        use StateEventContentChange::Original;

        let create_content = if let SmallStateContent::OtherState(other) = create_event_content
            && let C::RoomCreate(Original { content, .. }) = other.content()
        {
            Some(content)
        } else {
            None
        };
        self.room_name_id = room_name_id.cloned();
        self.predecessor_room = create_content
            .and_then(|content| content.predecessor.as_ref())
            .map(|predecessor| PredecessorRoomDetails::new(
                predecessor.room_id.clone(),
                room_name_id.map(|room_name_id| room_name_id.display_name().clone()),
                Some(create_event.sender()),
            ));
        self.show_room(cx);
        self.view.label(cx, ids!(description)).set_text(cx, match self.predecessor_room {
            Some(_) => "This room is a continuation of an older room.",
            None => "This is the beginning of the room.",
        });
        self.view.button(cx, ids!(older_room_button)).set_visible(cx, self.predecessor_room.is_some());

        // Each setting's latest value is the one that's in effect.
        let (mut join_rule, mut history_visibility, mut guest_access) = (None, None, None);
        let (mut topic, mut alias, mut only_some_can_send) = (None, None, None);
        let mut is_encrypted = false;
        for event in items.iter().rev().filter_map(|item| item.as_event()) {
            let TimelineItemContent::OtherState(other) = event.content() else { continue };
            match other.content() {
                C::RoomJoinRules(Original { content, .. }) => { join_rule.get_or_insert(&content.join_rule); }
                C::RoomHistoryVisibility(Original { content, .. }) => { history_visibility.get_or_insert(&content.history_visibility); }
                C::RoomGuestAccess(Original { content, .. }) => { guest_access.get_or_insert(&content.guest_access); }
                C::RoomTopic(Original { content, .. }) => { topic.get_or_insert(content.topic.trim()); }
                C::RoomCanonicalAlias(Original { content, .. }) => { alias.get_or_insert(content.alias.as_ref().or(content.alt_aliases.first())); }
                C::RoomPowerLevels(Original { content, .. }) => { only_some_can_send.get_or_insert(content.events_default > content.users_default); }
                // Encryption can't be turned off once it's on.
                C::RoomEncryption(_) => is_encrypted = true,
                _ => {}
            }
        }

        let alias = alias.flatten();
        let room_alias = self.view.label(cx, ids!(room_alias));
        room_alias.set_text(cx, alias.map_or("", |alias| alias.as_str()));
        room_alias.set_visible(cx, alias.is_some());
        let topic = topic.filter(|topic| !topic.is_empty());
        let room_topic = self.view.widget(cx, ids!(room_topic));
        room_topic.set_text(cx, topic.unwrap_or_default());
        room_topic.set_visible(cx, topic.is_some());

        let settings = self.view.widget(cx, ids!(settings));
        // A room without these settings uses the defaults, which are invite-only with shared history.
        for (pill, is_shown) in [
            (id!(encrypted), is_encrypted),
            (id!(not_encrypted), !is_encrypted),
            (id!(invite_only), matches!(join_rule, None | Some(JoinRule::Invite | JoinRule::Private))),
            (id!(space_members_can_join), matches!(join_rule, Some(JoinRule::Restricted(_) | JoinRule::KnockRestricted(_)))),
            (id!(ask_to_join), matches!(join_rule, Some(JoinRule::Knock | JoinRule::KnockRestricted(_)))),
            (id!(anyone_can_join), matches!(join_rule, Some(JoinRule::Public))),
            (id!(anyone_can_read), matches!(history_visibility, Some(HistoryVisibility::WorldReadable))),
            (id!(full_history), matches!(history_visibility, None | Some(HistoryVisibility::Shared))),
            (id!(history_since_invite), matches!(history_visibility, Some(HistoryVisibility::Invited))),
            (id!(history_since_joining), matches!(history_visibility, Some(HistoryVisibility::Joined))),
            (id!(guests_allowed), matches!(guest_access, Some(GuestAccess::CanJoin))),
            (id!(limited_posting), only_some_can_send == Some(true)),
            (id!(local_only), create_content.is_some_and(|content| !content.federate)),
            (id!(room_version), create_content.is_some()),
        ] {
            settings.child(pill).set_visible(cx, is_shown);
        }
        if let Some(content) = create_content {
            self.view.label(cx, ids!(local_only.label)).set_text(cx, &format!("🏠 {} users only", create_event.sender().server_name()));
            self.view.label(cx, ids!(room_version.label)).set_text(cx, &format!("🔢 Room v{}", content.room_version));
        }
    }

    fn show_room(&mut self, cx: &mut Cx) {
        let Some(room_name_id) = self.room_name_id.as_ref() else { return };
        let room_avatar = get_rooms_list_avatar(cx, room_name_id);
        self.view.avatar(cx, ids!(room_avatar)).show_room_avatar(cx, room_avatar.as_ref(), room_name_id);
        let room_name = self.view.label(cx, ids!(room_name));
        room_name.set_text(cx, &room_name_id.display());
        room_name.set_visible(cx, !room_name_id.is_empty());
    }
}

/// Creates, populates, and adds the room's creation item at `item_id`, for the room's creation event.
///
/// If `group` is `Some`, it's the room's setup, which this item sums up just like a group's summary item does.
pub(super) fn populate_room_creation_item(
    cx: &mut Cx,
    list: &mut PortalList,
    item_id: usize,
    timeline: TimelineInfo,
    group: Option<&StateEventGroup>,
    create_event: &EventTimelineItem,
    create_event_content: &SmallStateContent,
    create_event_details: MessageDetails,
    room_name_id: Option<&RoomNameId>,
    item_drawn_status: ItemDrawnStatus,
) -> (WidgetRef, ItemDrawnStatus) {
    let (item, existed) = list.item_with_existed(cx, item_id, id!(RoomCreationItem));
    if !(existed && item_drawn_status.content_drawn)
        && let Some(mut inner) = item.as_room_creation_item().borrow_mut()
    {
        inner.set_room(cx, room_name_id, timeline.items, create_event, create_event_content);
    }
    item.state_event_group_header(cx, ids!(header)).set_shown(group.is_some());
    if let Some(group) = group {
        return populate_group_summary(cx, item, existed, timeline, group, create_event, create_event_content, create_event_details, item_drawn_status);
    }
    let first_event = item.small_state_event(cx, ids!(first_event));
    first_event.set_details(cx, create_event_details);
    let first_event_was_shown = existed && first_event.visible();
    first_event.set_shown(true);
    populate_read_receipts(&first_event, cx, timeline.kind, create_event);
    let (_, first_event_status) = populate_small_state_event_widget(
        cx,
        (*first_event).clone(),
        first_event_was_shown,
        timeline.kind,
        create_event,
        create_event_content,
        item_drawn_status,
        false,
    );
    (item, first_event_status)
}

/// Returns the index of the room's creation item if the given changes added or changed a setting that it shows.
pub(super) fn get_creation_item_to_redraw(items: &Vector<Arc<TimelineItem>>, changes: &[ChangedItems]) -> Option<usize> {
    use AnyOtherStateEventContentChange as C;
    let changed_settings = changes.iter()
        .flat_map(|change| items_in(items, &change.new))
        .filter_map(|item| item.as_event())
        .any(|event| matches!(event.content(), TimelineItemContent::OtherState(other) if matches!(other.content(),
            C::RoomJoinRules(_) | C::RoomHistoryVisibility(_) | C::RoomGuestAccess(_) | C::RoomTopic(_)
                | C::RoomCanonicalAlias(_) | C::RoomPowerLevels(_) | C::RoomEncryption(_)
        )));
    if !changed_settings {
        return None;
    }
    // The creation event is always the room's very first event.
    let (index, first_event) = items.iter().enumerate().find_map(|(index, item)| Some((index, item.as_event()?)))?;
    matches!(first_event.content(), TimelineItemContent::OtherState(other) if is_room_create(other)).then_some(index)
}
