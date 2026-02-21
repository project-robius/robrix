use makepad_widgets::*;
use matrix_sdk::ruma::OwnedRoomId;

use crate::{
    room::FetchedRoomAvatar, shared::{
        avatar::AvatarWidgetExt,
        html_or_plaintext::HtmlOrPlaintextWidgetExt, unread_badge::UnreadBadgeWidgetExt as _,
    }, utils::{self, relative_format}
};

use super::rooms_list::{InvitedRoomInfo, InviterInfo, JoinedRoomInfo, RoomsListScopeProps};
script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*


    // A cancel icon to be displayed in the RoomsListEntry when the room is tombstoned.
    mod.widgets.TombstoneIcon = View {
        width: Fit, height: Fit,
        visible: false,

        Icon {
            width: 19, height: 19,
            align: Align{x: 0.5, y: 0.5}
            draw_icon +: {
                svg_file: (ICON_TOMBSTONE)
                color: (COLOR_FG_DANGER_RED)
            }
            icon_walk: Walk{ width: 15, height: 15 }
        }
    }

    mod.widgets.RoomName = Label {
        width: Fill, height: Fit
        flow: Right, // do not wrap
        padding: 0,
        draw_text +: {
            color: #000,
            flow: Flow.Right{wrap: true},
            text_style: USERNAME_TEXT_STYLE { font_size: 10. }
        }
        text: "[Room name unknown]"
    }

    mod.widgets.Timestamp = Label {
        padding: Inset{top: 1},
        width: Fit, height: Fit
        flow: Right, // do not wrap
        draw_text +: {
            color: (TIMESTAMP_TEXT_COLOR)
            text_style: TIMESTAMP_TEXT_STYLE {
                font_size: 7.5
            },
        }
    }

    mod.widgets.MessagePreview = View {
        width: Fill, height: Fit
        latest_message := HtmlOrPlaintext {
            html_view: { html := mod.widgets.MessageHtml {
                font_size: 9.3
                text_style_normal: theme.font_regular { font_size: 9.3 }
                text_style_italic: theme.font_italic { font_size: 9.3 }
                text_style_bold: theme.font_bold { font_size: 9.3 }
                text_style_bold_italic: theme.font_bold_italic { font_size: 9.3 }
                text_style_fixed: theme.font_code { font_size: 9.3 }
            } }
            plaintext_view: { pt_label := Label {
                draw_text +: {
                    text_style: theme.font_regular { font_size: 9.5 },
                }
                text: "[No recent messages]"
            } }
        }
    }

    mod.widgets.RoomsListEntryContent = #(RoomsListEntryContent::register_widget(vm)) {
        flow: Right,
        spacing: 10,
        padding: 10,
        width: Fill, height: Fit
        show_bg: true
        draw_bg +: {
            color: uniform(#0000)
            border_size: instance(0.0)
            border_color: instance(#0000)
            inset: instance(vec4(0.0))
            border_radius: instance(4.0)

            get_color: fn() -> vec4 {
                return self.color
            }

            get_border_color: fn() -> vec4 {
                return self.border_color
            }

            pixel: fn() -> vec4 {
                let sdf = Sdf2d.viewport(self.pos * self.rect_size)
                sdf.box(
                    self.inset.x + self.border_size,
                    self.inset.y + self.border_size,
                    self.rect_size.x - (self.inset.x + self.inset.z + self.border_size * 2.0),
                    self.rect_size.y - (self.inset.y + self.inset.w + self.border_size * 2.0),
                    max(1.0, self.border_radius)
                )
                sdf.fill_keep(self.get_color())
                if self.border_size > 0.0 {
                    sdf.stroke(self.get_border_color(), self.border_size)
                }
                return sdf.result;
            }
        }
    }

    mod.widgets.RoomsListEntry = #(RoomsListEntry::register_widget(vm)) {
        flow: Down, height: Fit
        cursor: Default,
        show_bg: true,

        // Wrap the RoomsListEntryContent in an AdaptiveView to change the displayed content
        // (and its layout) based on the available space in the sidebar.
        adaptive_preview := AdaptiveView {
            height: Fit

            OnlyIcon := mod.widgets.RoomsListEntryContent {
                align: Align{x: 0.5, y: 0.5}
                padding: 5.
                View {
                    height: Fit
                    flow: Overlay
                    align: Align{ x: 1.0 }
                    avatar := Avatar {}
                    unread_badge := UnreadBadge {}
                    tombstone_icon := mod.widgets.TombstoneIcon {}
                }
            }
            IconAndName := mod.widgets.RoomsListEntryContent {
                padding: 5.
                align: Align{x: 0.5, y: 0.5}
                avatar := Avatar {}
                room_name := mod.widgets.RoomName {}
                unread_badge := UnreadBadge {}
                tombstone_icon := mod.widgets.TombstoneIcon {}
            }
            FullPreview := mod.widgets.RoomsListEntryContent {
                padding: 10
                avatar := Avatar {}
                View {
                    flow: Down
                    width: Fill, height: 56
                    align: Align{ x: 0.0, y: 0.0 }
                    top := View {
                        width: Fill, height: Fit,
                        spacing: 3,
                        flow: Right,
                        room_name := mod.widgets.RoomName {}
                        timestamp := mod.widgets.Timestamp { }
                    }
                    bottom := View {
                        width: Fill, height: Fill,
                        spacing: 2,
                        flow: Right,
                        preview := mod.widgets.MessagePreview {
                            margin: Inset{ top: 2.5 }
                        }
                        View {
                            width: Fit, height: Fit
                            align: Align{ x: 1.0 }
                            unread_badge := UnreadBadge {}
                            tombstone_icon := mod.widgets.TombstoneIcon {}
                        }
                    }
                }
            }
        }
    }
}

/// An entry in the rooms list.
#[derive(Script, ScriptHook, Widget)]
pub struct RoomsListEntry {
    #[deref] view: View,
    #[rust] room_id: Option<OwnedRoomId>,
}

/// Widget actions that are emitted by a RoomsListEntry.
#[derive(Clone, Default, Debug)]
pub enum RoomsListEntryAction {
    /// This RoomsListEntry was primary-clicked or tapped.
    PrimaryClicked(OwnedRoomId),
    /// This RoomsListEntry was right-clicked or long-pressed.
    SecondaryClicked(OwnedRoomId, DVec2),
    #[default]
    None,
}

impl Widget for RoomsListEntry {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view
            .adaptive_view(cx, ids!(adaptive_preview))
            .set_variant_selector(|_cx, parent_size| match parent_size.x {
                width if width <= 70.0 => id!(OnlyIcon),
                width if width <= 200.0 => id!(IconAndName),
                _ => id!(FullPreview),
            });

        let uid = self.widget_uid();
        let rooms_list_props = scope.props.get::<RoomsListScopeProps>().unwrap();

        // We handle hits on this widget first to ensure that any clicks on it
        // will just select the room, rather than resulting in a click on any child view
        // within the RoomsListEntry content itself, such as links or avatars.
        if let Some(room_id) = &self.room_id {
            let area = self.view.area();
            match event.hits(cx, area) {
                Hit::FingerDown(fe) => {
                    cx.set_key_focus(area);
                    if fe.device.mouse_button().is_some_and(|b| b.is_secondary()) {
                        cx.widget_action(
                            uid, 
                            RoomsListEntryAction::SecondaryClicked(room_id.clone(), fe.abs),
                        );
                    }
                }
                Hit::FingerLongPress(fe) => {
                    cx.widget_action(
                        uid, 
                        RoomsListEntryAction::SecondaryClicked(room_id.clone(), fe.abs),
                    );
                }
                Hit::FingerUp(fe) if !rooms_list_props.was_scrolling && fe.is_over && fe.is_primary_hit() && fe.was_tap() => {
                    cx.widget_action(uid,  RoomsListEntryAction::PrimaryClicked(room_id.clone()));
                }
                _ => { }
            }
        }

        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        if let Some(room_info) = scope.props.get::<JoinedRoomInfo>() {
            self.room_id = Some(room_info.room_name_id.room_id().clone());
        }
        else if let Some(room_info) = scope.props.get::<InvitedRoomInfo>() {
            self.room_id = Some(room_info.room_name_id.room_id().clone());
        }

        self.view.draw_walk(cx, scope, walk)
    }
}

#[derive(Script, ScriptHook, Widget)]
pub struct RoomsListEntryContent {
    #[deref] view: View,
}

impl Widget for RoomsListEntryContent {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        if let Some(joined_room_info) = scope.props.get::<JoinedRoomInfo>() {
            self.draw_joined_room(cx, joined_room_info);
        } else if let Some(invited_room_info) = scope.props.get::<InvitedRoomInfo>() {
            self.draw_invited_room(cx, invited_room_info);
        }

        self.view.draw_walk(cx, scope, walk)
    }
}

impl RoomsListEntryContent {
    /// Populates this RoomsListEntry with info about a joined room.
    pub fn draw_joined_room(
        &mut self,
        cx: &mut Cx,
        room_info: &JoinedRoomInfo,
    ) {
        self.view.label(cx, ids!(room_name)).set_text(cx, &room_info.room_name_id.to_string());
        if let Some((ts, msg)) = room_info.latest.as_ref() {
            if let Some(human_readable_date) = relative_format(*ts) {
                self.view
                    .label(cx, ids!(timestamp))
                    .set_text(cx, &human_readable_date);
            }
            self.view
                .html_or_plaintext(cx, ids!(latest_message))
                .show_html(cx, msg);
        }

        self.view.unread_badge(cx, ids!(unread_badge)).update_counts(
            room_info.is_marked_unread,
            room_info.num_unread_mentions,
            room_info.num_unread_messages,
        );
        self.draw_common(cx, &room_info.room_avatar, room_info.is_selected);
        // Show tombstone icon if the room is tombstoned
        self.view.view(cx, ids!(tombstone_icon)).set_visible(cx, room_info.is_tombstoned);
    }

    /// Populates this RoomsListEntry with info about an invited room.
    pub fn draw_invited_room(
        &mut self,
        cx: &mut Cx,
        room_info: &InvitedRoomInfo,
    ) {
        self.view.label(cx, ids!(room_name)).set_text(cx, &room_info.room_name_id.to_string());
        // Hide the timestamp field, and use the latest message field to show the inviter.
        self.view.label(cx, ids!(timestamp)).set_text(cx, "");
        let inviter_string = match &room_info.inviter_info {
            Some(InviterInfo { user_id, display_name: Some(dn), .. }) => format!("Invited by <b>{dn}</b> ({user_id})"),
            Some(InviterInfo { user_id, .. }) => format!("Invited by {user_id}"),
            None => String::from("You were invited"),
        };
        self.view.html_or_plaintext(cx, ids!(latest_message)).show_html(cx, &inviter_string);

        match room_info.room_avatar {
            FetchedRoomAvatar::Text(ref text) => {
                self.view.avatar(cx, ids!(avatar)).show_text(cx, None, None, text);
            }
            FetchedRoomAvatar::Image(ref img_bytes) => {
                let _ = self.view.avatar(cx, ids!(avatar)).show_image(
                    cx,
                    None, // Avatars in a RoomsListEntry shouldn't be clickable.
                    |cx, img| utils::load_png_or_jpg(&img, cx, img_bytes),
                );
            }
        }

        self.view
            .unread_badge(cx, ids!(unread_badge))
            .update_counts(false, 1, 0);

        self.draw_common(cx, &room_info.room_avatar, room_info.is_selected);
    }

    /// Populates the widgets common to both invited and joined rooms list entries.
    pub fn draw_common(
        &mut self,
        cx: &mut Cx,
        room_avatar: &FetchedRoomAvatar,
        is_selected: bool,
    ) {
        match room_avatar {
            FetchedRoomAvatar::Text(text) => {
                self.view.avatar(cx, ids!(avatar)).show_text(cx, None, None, text);
            }
            FetchedRoomAvatar::Image(img_bytes) => {
                let _ = self.view.avatar(cx, ids!(avatar)).show_image(
                    cx,
                    None, // Avatars in a RoomsListEntry shouldn't be clickable.
                    |cx, img| utils::load_png_or_jpg(&img, cx, img_bytes),
                );
            }
        }

        if cx.display_context.is_desktop() {
            self.update_preview_colors(cx, is_selected);
        } else {
            // Mobile doesn't have a selected state. Always use the default colors.
            // We call the update in case the app was resized from desktop to mobile while the room was selected.
            // This can be optimized by only calling this when the app is resized.
            self.update_preview_colors(cx, false);
        }
    }

    /// Updates the styling of the preview based on whether the room is selected or not.
    pub fn update_preview_colors(&mut self, _cx: &mut Cx, _is_selected: bool) {
        // Dynamic runtime recoloring is temporarily disabled because nested adaptive variants
        // can resolve IDs to incompatible targets during script_apply_eval!, which triggers
        // Splash runtime errors and crashes the draw path.
    }
}
