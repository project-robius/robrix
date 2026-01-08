use makepad_widgets::*;
use matrix_sdk::ruma::OwnedRoomId;

use crate::{
    room::FetchedRoomAvatar, shared::{
        avatar::AvatarWidgetExt,
        html_or_plaintext::HtmlOrPlaintextWidgetExt, unread_badge::UnreadBadgeWidgetExt as _,
    }, utils::{self, relative_format}
};

use super::rooms_list::{InvitedRoomInfo, InviterInfo, JoinedRoomInfo, RoomsListScopeProps};
live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::styles::*;
    use crate::shared::helpers::*;
    use crate::shared::avatar::Avatar;
    use crate::shared::html_or_plaintext::HtmlOrPlaintext;
    use crate::shared::unread_badge::UnreadBadge;

    // A cancel icon to be displayed in the RoomsListEntry when the room is tombstoned.
    TombstoneIcon = <View> {
        width: Fit, height: Fit,
        visible: false,

        <Icon> {
            width: 19, height: 19,
            align: {x: 0.5, y: 0.5}
            draw_icon: {
                svg_file: (ICON_TOMBSTONE)
                color: (COLOR_FG_DANGER_RED)
            }
            icon_walk: { width: 15, height: 15 }
        }
    }

    RoomName = <Label> {
        width: Fill, height: Fit
        flow: Right, // do not wrap
        padding: 0,
        draw_text:{
            color: #000,
            wrap: Ellipsis,
            text_style: <USERNAME_TEXT_STYLE>{ font_size: 10. }
        }
        text: "[Room name unknown]"
    }

    Timestamp = <Label> {
        padding: {top: 1},
        width: Fit, height: Fit
        flow: Right, // do not wrap
        draw_text:{
            color: (TIMESTAMP_TEXT_COLOR)
            text_style: <TIMESTAMP_TEXT_STYLE>{
                font_size: 7.5
            },
        }
        text: "??"
    }

    MessagePreview = <View> {
        width: Fill, height: Fit
        latest_message = <HtmlOrPlaintext> {
            html_view = { html = {
                font_size: 9.3,
                draw_normal:      { text_style: { font_size: 9.3 } },
                draw_italic:      { text_style: { font_size: 9.3 } },
                draw_bold:        { text_style: { font_size: 9.3 } },
                draw_bold_italic: { text_style: { font_size: 9.3 } },
                draw_fixed:       { text_style: { font_size: 9.3 } },
                a = {
                    matrix_link_view = {
                        matrix_link = {
                            padding: { top: 2.0, bottom: 2.0, left: 4.0, right: 4.0 }
                            draw_bg: {
                                color: #000,
                                border_radius: 3.5,
                            }
                            avatar = {
                                height: 10.0, width: 10.0
                                text_view = { text = { draw_text: {
                                    text_style: <TITLE_TEXT>{ font_size: 6.3 }
                                }}}
                            }
                            title = {
                                draw_text: {
                                    color: #fff
                                    text_style: {
                                        font_size: 6.3
                                    }
                                }
                            }
                        }
                    }
                }
            } }
            plaintext_view = { pt_label = {
                draw_text: {
                    text_style: { font_size: 9.5 },
                }
                text: "[Loading latest message]"
            } }
        }
    }

    RoomsListEntryContent = {{RoomsListEntryContent}} {
        flow: Right,
        spacing: 10,
        padding: 10,
        width: Fill, height: Fit
        show_bg: true
        draw_bg: {
            color: #0000
            instance border_size: 0.0
            instance border_color: #0000
            instance inset: vec4(0.0, 0.0, 0.0, 0.0)
            instance border_radius: 4.0

            fn get_color(self) -> vec4 {
                return self.color
            }

            fn get_border_color(self) -> vec4 {
                return self.border_color
            }

            fn pixel(self) -> vec4 {
                let sdf = Sdf2d::viewport(self.pos * self.rect_size)
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

    pub RoomsListEntry = {{RoomsListEntry}} {
        flow: Down, height: Fit
        cursor: Default,
        show_bg: true,

        // Wrap the RoomsListEntryContent in an AdaptiveView to change the displayed content
        // (and its layout) based on the available space in the sidebar.
        adaptive_preview = <AdaptiveView> {
            height: Fit

            OnlyIcon = <RoomsListEntryContent> {
                align: {x: 0.5, y: 0.5}
                padding: 5.
                <View> {
                    height: Fit
                    flow: Overlay
                    align: { x: 1.0 }
                    avatar = <Avatar> {}
                    unread_badge = <UnreadBadge> {}
                    tombstone_icon = <TombstoneIcon> {}
                }
            }
            IconAndName = <RoomsListEntryContent> {
                padding: 5.
                align: {x: 0.5, y: 0.5}
                avatar = <Avatar> {}
                room_name = <RoomName> {}
                unread_badge = <UnreadBadge>  {}
                tombstone_icon = <TombstoneIcon> {}
            }
            FullPreview = <RoomsListEntryContent> {
                padding: 10
                avatar = <Avatar> {}
                <View> {
                    flow: Down
                    width: Fill, height: 56
                    align: { x: 0.0, y: 0.0 }
                    top = <View> {
                        width: Fill, height: Fit,
                        spacing: 3,
                        flow: Right,
                        room_name = <RoomName> {}
                        timestamp = <Timestamp> { }
                    }
                    bottom = <View> {
                        width: Fill, height: Fill,
                        spacing: 2,
                        flow: Right,
                        preview = <MessagePreview> {
                            margin: { top: 2.5 }
                        }
                        <View> {
                            width: Fit, height: Fit
                            align: { x: 1.0 }
                            unread_badge = <UnreadBadge> {}
                            tombstone_icon = <TombstoneIcon> {}
                        }
                    }
                }
            }
        }
    }
}

#[derive(Live, Widget)]
pub struct RoomsListEntry {
    #[deref] view: View,
    #[rust] room_id: Option<OwnedRoomId>,
}

#[derive(Clone, DefaultNone, Debug)]
pub enum RoomsListEntryAction {
    Clicked(OwnedRoomId),
    None,
}

impl LiveHook for RoomsListEntry {
    fn after_new_from_doc(&mut self, _cx: &mut Cx) {
        // Adapt the preview based on the available space.
        self.view
            .adaptive_view(ids!(adaptive_preview))
            .set_variant_selector(|_cx, parent_size| match parent_size.x {
                width if width <= 70.0  => id!(OnlyIcon),
                width if width <= 200.0 => id!(IconAndName),
                _ => id!(FullPreview),
            });
    }
}

impl Widget for RoomsListEntry {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        let uid = self.widget_uid();
        let rooms_list_props = scope.props.get::<RoomsListScopeProps>().unwrap();

        // We handle hits on this widget first to ensure that any clicks on it
        // will just select the room, rather than resulting in a click on any child view
        // within the RoomsListEntry content itself, such as links or avatars.
        match event.hits(cx, self.view.area()) {
            Hit::FingerDown(..) => {
                cx.set_key_focus(self.view.area());
            }
            Hit::FingerUp(fe) => {
                if !rooms_list_props.was_scrolling && fe.is_over && fe.is_primary_hit() && fe.was_tap() {
                    cx.widget_action(uid, &scope.path, RoomsListEntryAction::Clicked(self.room_id.clone().unwrap()));
                }
            }
            _ => { }
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

#[derive(Live, LiveHook, Widget)]
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
        self.view.label(ids!(room_name)).set_text(cx, &room_info.room_name_id.to_string());
        if let Some((ts, msg)) = room_info.latest.as_ref() {
            if let Some(human_readable_date) = relative_format(*ts) {
                self.view
                    .label(ids!(timestamp))
                    .set_text(cx, &human_readable_date);
            }
            self.view
                .html_or_plaintext(ids!(latest_message))
                .show_html(cx, msg);
        }

        self.view
            .unread_badge(ids!(unread_badge))
            .update_counts(room_info.num_unread_mentions, room_info.num_unread_messages);
        self.draw_common(cx, &room_info.room_avatar, room_info.is_selected);
        // Show tombstone icon if the room is tombstoned
        self.view.view(ids!(tombstone_icon)).set_visible(cx, room_info.is_tombstoned);
    }

    /// Populates this RoomsListEntry with info about an invited room.
    pub fn draw_invited_room(
        &mut self,
        cx: &mut Cx,
        room_info: &InvitedRoomInfo,
    ) {
        self.view.label(ids!(room_name)).set_text(cx, &room_info.room_name_id.to_string());
        // Hide the timestamp field, and use the latest message field to show the inviter.
        self.view.label(ids!(timestamp)).set_text(cx, "");
        let inviter_string = match &room_info.inviter_info {
            Some(InviterInfo { user_id, display_name: Some(dn), .. }) => format!("Invited by <b>{dn}</b> ({user_id})"),
            Some(InviterInfo { user_id, .. }) => format!("Invited by {user_id}"),
            None => String::from("You were invited"),
        };
        self.view.html_or_plaintext(ids!(latest_message)).show_html(cx, &inviter_string);

        match room_info.room_avatar {
            FetchedRoomAvatar::Text(ref text) => {
                self.view.avatar(ids!(avatar)).show_text(cx, None, None, text);
            }
            FetchedRoomAvatar::Image(ref img_bytes) => {
                let _ = self.view.avatar(ids!(avatar)).show_image(
                    cx,
                    None, // Avatars in a RoomsListEntry shouldn't be clickable.
                    |cx, img| utils::load_png_or_jpg(&img, cx, img_bytes),
                );
            }
        }

        self.view
            .unread_badge(ids!(unread_badge))
            .update_counts(1, 0);

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
                self.view.avatar(ids!(avatar)).show_text(cx, None, None, text);
            }
            FetchedRoomAvatar::Image(img_bytes) => {
                let _ = self.view.avatar(ids!(avatar)).show_image(
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
    pub fn update_preview_colors(&mut self, cx: &mut Cx, is_selected: bool) {
        let bg_color;
        let message_text_color;
        let room_name_color;
        let timestamp_color;
        let code_bg_color;

        // TODO: This is quite verbose, makepad should provide a way to override this at a higher level.
        if is_selected {
            bg_color = vec4(0.059, 0.533, 0.996, 1.0); // COLOR_PRIMARY_SELECTED
            message_text_color = vec3(1., 1., 1.); // COLOR_PRIMARY
            room_name_color = vec3(1., 1., 1.); // COLOR_PRIMARY
            timestamp_color = vec3(1., 1., 1.); // COLOR_PRIMARY
            code_bg_color = vec3(0.3, 0.3, 0.3); // a darker gray, used for `code_color` and `quote_bg_color`
        } else {
            bg_color = vec4(0.0, 0.0, 0.0, 0.0); // TRANSPARENT
            message_text_color = vec3(0.267, 0.267, 0.267); // MESSAGE_TEXT_COLOR
            room_name_color = vec3(0., 0., 0.);
            timestamp_color = vec3(0.6, 0.6, 0.6);
            code_bg_color = vec3(0.929, 0.929, 0.929); // #EDEDED, see `code_color` and `quote_bg_color`
        }

        self.view.apply_over(
            cx,
            live!(
                draw_bg: {
                    color: (bg_color)
                }
            ),
        );

        // We check that the UI elements exist to avoid unnecessary updates, and prevent error logs.
        if !self.view.label(ids!(room_name)).is_empty() {
            self.view.label(ids!(room_name)).apply_over(
                cx,
                live!(
                draw_text: {
                    color: (room_name_color)
                }
                ),
            );
        }

        if !self.view.label(ids!(timestamp)).is_empty() {
            self.view.label(ids!(timestamp)).apply_over(
                cx,
                live!(
                draw_text: {
                    color: (timestamp_color)
                }
                ),
            );
        }

        if !self.view.html_or_plaintext(ids!(latest_message)).is_empty() {
            self.view.html_or_plaintext(ids!(latest_message)).apply_over(
                cx,
                live!(
                html_view = {
                    html = {
                        font_color: (message_text_color),
                        draw_normal:      { color: (message_text_color) },
                        draw_italic:      { color: (message_text_color) },
                        draw_bold:        { color: (message_text_color) },
                        draw_bold_italic: { color: (message_text_color) },
                        draw_block: {
                            quote_bg_color: (code_bg_color),
                            code_color: (code_bg_color),
                        }
                    }
                }
                plaintext_view = {
                    pt_label = {
                        draw_text: {
                            color: (message_text_color)
                        }
                    }
                }
                ),
            );
        }
    }
}
