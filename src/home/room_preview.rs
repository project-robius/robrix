use makepad_widgets::*;

use crate::{
    shared::{
        avatar::AvatarWidgetExt,
        html_or_plaintext::HtmlOrPlaintextWidgetExt,
    },
    utils::{self, relative_format},
};

use super::rooms_list::{RoomPreviewAvatar, RoomsListEntry, RoomsListScopeProps};
live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::styles::*;
    use crate::shared::helpers::*;
    use crate::shared::avatar::Avatar;
    use crate::shared::html_or_plaintext::HtmlOrPlaintext;
    pub UNREAD_HIGHLIGHT_COLOR = #FF0000;
    pub UNREAD_DEFAULT_COLOR = #AAA;

    RoomName = <Label> {
        width: Fill, height: Fit
        draw_text:{
            color: #000,
            wrap: Ellipsis,
            text_style: <USERNAME_TEXT_STYLE>{ font_size: 10. }
        }
        text: "[Room name unknown]"
    }

    Timestamp = <Label> {
        width: Fit, height: Fit
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

    RoomPreviewContent = {{RoomPreviewContent}} {
        flow: Right,
        spacing: 10., padding: 10.
        width: Fill, height: Fit
        show_bg: true
        draw_bg: {
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

    UnreadBadge = <View> {
        width: 30, height: 20,
        align: {
            x: 0.5,
            y: 0.5
        }
        visible: false,
        flow: Overlay,
        rounded_label = <View> {
            width: Fill,
            height: Fill,
            show_bg: true,
            draw_bg: {
                instance highlight: 0.0,
                instance highlight_color: (UNREAD_HIGHLIGHT_COLOR),
                instance default_color: (UNREAD_DEFAULT_COLOR),
                instance border_radius: 4.0
                // Adjust this border_size to larger value to make oval smaller 
                instance border_size: 2.0
                fn pixel(self) -> vec4 {
                    let sdf = Sdf2d::viewport(self.pos * self.rect_size)
                    sdf.box(
                        self.border_size,
                        1.0,
                        self.rect_size.x - (self.border_size * 2.0),
                        self.rect_size.y - 2.0,
                        max(1.0, self.border_radius)
                    )
                    sdf.fill_keep(mix(self.default_color, self.highlight_color, self.highlight));
                    return sdf.result;
                }
            }
        }
        // Label that displays the unread message count
        unread_messages_count = <Label> {
            width: Fit,
            height: Fit,
            text: "",
            draw_text: {
                color: #ffffff,
                text_style: {font_size: 8.0},
            }
        }
    }

    pub RoomPreview = {{RoomPreview}} {
        flow: Down, height: Fit
        cursor: Default,

        // Wrap the RoomPreviewContent in an AdaptiveView to change the displayed content
        // (and its layout) based on the available space in the sidebar.
        adaptive_preview = <AdaptiveView> {
            height: Fit

            OnlyIcon = <RoomPreviewContent> {
                align: {x: 0.5, y: 0.5}
                padding: 5.
                <View> {
                    height: Fit
                    flow: Overlay
                    align: { x: 1.0 }
                    avatar = <Avatar> {}
                    unread_badge = <UnreadBadge> {}
                }
            }
            IconAndName = <RoomPreviewContent> {
                padding: 5.
                align: {x: 0.5, y: 0.5}
                avatar = <Avatar> {}
                room_name = <RoomName> {}
                unread_badge = <UnreadBadge> {}
            }
            FullPreview = <RoomPreviewContent> {
                padding: 10
                avatar = <Avatar> {}
                <View> {
                    flow: Down
                    width: Fill, height: 56
                    align: { x: 0.0, y: 0.0 }
                    top = <View> {
                        width: Fill, height: Fit,
                        spacing: 5,
                        flow: Right,
                        room_name = <RoomName> {}
                        // Use a small top margin to align the timestamp text baseline with the room name text baseline. 
                        timestamp = <Timestamp> { margin: { top: 1.3 } }
                    }
                    bottom = <View> {
                        width: Fill, height: Fill,
                        spacing: 5,
                        margin: { top: 7. }
                        flow: Right,
                        preview = <MessagePreview> {}
                        <View> {
                            width: Fit, height: Fit
                            align: { x: 1.0 }
                            unread_badge = <UnreadBadge> {
                                margin: { top: 5. } // Align the badge with the timestamp, same as the message preview's margin top.
                            }
                        }
                    }
                }
            }
        }
    }
}

#[derive(Live, Widget)]
pub struct RoomPreview {
    #[deref]
    view: View,
}

#[derive(Clone, DefaultNone, Debug)]
pub enum RoomPreviewAction {
    None,
    Click,
}

impl LiveHook for RoomPreview {
    fn after_new_from_doc(&mut self, _cx: &mut Cx) {
        // Adapt the preview based on the available space.
        self.view
            .adaptive_view(id!(adaptive_preview))
            .set_variant_selector(|_cx, parent_size| match parent_size.x {
                width if width <= 70.0  => live_id!(OnlyIcon),
                width if width <= 200.0 => live_id!(IconAndName),
                _ => live_id!(FullPreview),
            });
    }
}

impl Widget for RoomPreview {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        let uid = self.widget_uid();
        let rooms_list_props = scope.props.get::<RoomsListScopeProps>().unwrap();

        // We handle hits on this widget first to ensure that any clicks on it
        // will just select the room, rather than resulting in a click on any child view
        // within the room preview content itself, such as links or avatars.
        match event.hits(cx, self.view.area()) {
            Hit::FingerDown(..) => {
                cx.set_key_focus(self.view.area());
            }
            Hit::FingerUp(fe) => {
                if !rooms_list_props.was_scrolling && fe.is_over && fe.is_primary_hit() && fe.was_tap() {
                    cx.widget_action(uid, &scope.path, RoomPreviewAction::Click);
                }
            }
            _ => { }
        }

        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl RoomPreviewRef {
    pub fn clicked(&self, actions: &Actions) -> bool {
        if let RoomPreviewAction::Click = actions.find_widget_action(self.widget_uid()).cast() {
            return true;
        }
        false
    }
}

#[derive(Live, LiveHook, Widget)]
pub struct RoomPreviewContent {
    #[deref]
    view: View,
}

impl Widget for RoomPreviewContent {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        if let Some(room_info) = scope.props.get::<RoomsListEntry>() {
            if let Some(ref name) = room_info.room_name {
                self.view.label(id!(room_name)).set_text(cx, name);
            }
            if let Some((ts, msg)) = room_info.latest.as_ref() {
                if let Some(human_readable_date) = relative_format(ts) {
                    self.view
                        .label(id!(timestamp))
                        .set_text(cx, &human_readable_date);
                }
                self.view
                    .html_or_plaintext(id!(latest_message))
                    .show_html(cx, msg);
            }
            match room_info.avatar {
                RoomPreviewAvatar::Text(ref text) => {
                    self.view.avatar(id!(avatar)).show_text(cx, None, text);
                }
                RoomPreviewAvatar::Image(ref img_bytes) => {
                    let _ = self.view.avatar(id!(avatar)).show_image(
                        cx,
                        None, // don't make room preview avatars clickable.
                        |cx, img| utils::load_png_or_jpg(&img, cx, img_bytes),
                    );
                }
            }

            let unread_badge = self.view(id!(unread_badge)); 
            // Helper function to format the rounded rectangle.
            //
            // The rounded rectangle needs to be wider for longer text.
            // It also adds a plus sign at the end if the unread count is greater than 99. 
            fn format_border_and_truncation(count: u64) -> (f64, &'static str) {
                let (border_size, plus_sign) = if count > 99 {
                    (0.0, "+")
                } else if count > 9 {
                    (2.0, "")
                } else {
                    (5.0, "")
                };
                (border_size, plus_sign)
            }
            if room_info.num_unread_mentions > 0 {
                let (border_size, plus_sign) = format_border_and_truncation(room_info.num_unread_mentions);
                // If there are unread mentions, show red badge and the number of unread mentions
                unread_badge
                    .label(id!(unread_messages_count))
                    .set_text(cx, &format!("{}{plus_sign}", std::cmp::min(room_info.num_unread_mentions, 99)));
                unread_badge.view(id!(rounded_label)).apply_over(cx, live!{
                    draw_bg: {
                        border_size: (border_size),
                        highlight: 1.0
                    }
                });
                unread_badge.set_visible(cx, true);
            } else if room_info.num_unread_messages > 0 {
                let (border_size, plus_sign) = format_border_and_truncation(room_info.num_unread_messages);
                // If there are no unread mentions but there are unread messages, show gray badge and the number of unread messages
                unread_badge
                    .label(id!(unread_messages_count))
                    .set_text(cx, &format!("{}{plus_sign}", std::cmp::min(room_info.num_unread_messages, 99)));
                unread_badge.view(id!(rounded_label)).apply_over(cx, live!{
                    draw_bg: {
                        border_size: (border_size),
                        highlight: 0.0
                    }
                });
                unread_badge.set_visible(cx, true);
            } else {
                // If there are no unread mentions and no unread messages, hide the badge
                unread_badge.set_visible(cx, false);
            }
            if cx.display_context.is_desktop() {
                self.update_preview_colors(cx, room_info.is_selected);
            } else if room_info.is_selected {
                // Mobile doesn't have a selected state. Always use the default colors.
                // We call the update in case the app was resized from desktop to mobile while the room was selected.
                // This can be optimized by only calling this when the app is resized.
                self.update_preview_colors(cx, false);
            }
        }
        self.view.draw_walk(cx, scope, walk)
    }
}

impl RoomPreviewContent {
    /// Updates the styling of the preview based on whether the room is selected or not.
    pub fn update_preview_colors(&mut self, cx: &mut Cx, is_selected: bool) {
        let bg_color;
        let message_text_color;
        let room_name_color;
        let timestamp_color;
        let code_bg_color;

        // TODO: This is quite verbose, makepad should provide a way to override this at a higher level.
        if is_selected {
            bg_color = vec3(0.059, 0.533, 0.996); // COLOR_PRIMARY_SELECTED
            message_text_color = vec3(1., 1., 1.); // COLOR_PRIMARY
            room_name_color = vec3(1., 1., 1.); // COLOR_PRIMARY
            timestamp_color = vec3(1., 1., 1.); // COLOR_PRIMARY
            code_bg_color = vec3(0.3, 0.3, 0.3); // a darker gray, used for `code_color` and `quote_bg_color`
        } else {
            bg_color = vec3(1., 1., 1.); // COLOR_PRIMARY
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
        if !self.view.label(id!(room_name)).is_empty() {
            self.view.label(id!(room_name)).apply_over(
                cx,
                live!(
                draw_text: {
                    color: (room_name_color)
                }
                ),
            );
        }

        if !self.view.label(id!(timestamp)).is_empty() {
            self.view.label(id!(timestamp)).apply_over(
                cx,
                live!(
                draw_text: {
                    color: (timestamp_color)
                }
                ),
            );
        }

        if !self.view.html_or_plaintext(id!(latest_message)).is_empty() {
            self.view.html_or_plaintext(id!(latest_message)).apply_over(
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
