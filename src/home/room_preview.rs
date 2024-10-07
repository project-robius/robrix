use makepad_widgets::*;

use crate::{
    shared::{
        adaptive_view::{AdaptiveViewWidgetExt, DisplayContext}, avatar::AvatarWidgetExt,
        html_or_plaintext::HtmlOrPlaintextWidgetExt,
    },
    utils::{self, relative_format},
};

use super::rooms_list::{RoomPreviewAvatar, RoomPreviewEntry};

live_design! {
    import makepad_draw::shader::std::*;
    import makepad_widgets::view::*;
    import makepad_widgets::base::*;
    import makepad_widgets::theme_desktop_dark::*;

    import crate::shared::styles::*;
    import crate::shared::helpers::*;
    import crate::shared::avatar::Avatar;
    import crate::shared::adaptive_view::AdaptiveView;
    import crate::shared::html_or_plaintext::HtmlOrPlaintext;

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
        flow: Down, spacing: 5.

        latest_message = <HtmlOrPlaintext> {
            padding: {top: 3.0}
            html_view = { html = {
                font_size: 9.3,
                draw_normal:      { text_style: { font_size: 9.3 } },
                draw_italic:      { text_style: { font_size: 9.3 } },
                draw_bold:        { text_style: { font_size: 9.3 } },
                draw_bold_italic: { text_style: { font_size: 9.3 } },
                draw_fixed:       { text_style: { font_size: 9.3 } },
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
        flow: Right, spacing: 10., padding: 10.
        width: Fill, height: Fit
        show_bg: true
        draw_bg: {
            instance border_width: 0.0
            instance border_color: #0000
            instance inset: vec4(0.0, 0.0, 0.0, 0.0)
            instance radius: 4.0

            fn get_color(self) -> vec4 {
                return self.color
            }

            fn get_border_color(self) -> vec4 {
                return self.border_color
            }

            fn pixel(self) -> vec4 {
                let sdf = Sdf2d::viewport(self.pos * self.rect_size)
                sdf.box(
                    self.inset.x + self.border_width,
                    self.inset.y + self.border_width,
                    self.rect_size.x - (self.inset.x + self.inset.z + self.border_width * 2.0),
                    self.rect_size.y - (self.inset.y + self.inset.w + self.border_width * 2.0),
                    max(1.0, self.radius)
                )
                sdf.fill_keep(self.get_color())
                if self.border_width > 0.0 {
                    sdf.stroke(self.get_border_color(), self.border_width)
                }
                return sdf.result;
            }
        }
    }
    
    RoomPreview = {{RoomPreview}} {
        // Wraps the RoomPreviewContent in an AdaptiveView
        // to change the displayed content (and its layout) based on the available space in the sidebar.
        adaptive_preview = <AdaptiveView> {
            OnlyIcon = <RoomPreviewContent> {
                align: {x: 0.5, y: 0.5}
                padding: 5.
                avatar = <Avatar> {}
            }
            IconAndName = <RoomPreviewContent> {
                padding: 5.
                align: {x: 0.5, y: 0.5}
                avatar = <Avatar> {}
                room_name = <RoomName> {}
            }
            FullPreview = <RoomPreviewContent> {
                avatar = <Avatar> {}
                <View> {
                    flow: Down
                    width: Fill, height: Fit
                    header = <View> {
                        width: Fill, height: Fit
                        flow: Right
                        spacing: 10.
                        align: {y: 0.5}

                        room_name = <RoomName> {}
                        timestamp = <Timestamp> {}
                    }
                    preview = <MessagePreview> {}
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
    fn after_new_from_doc(&mut self, cx: &mut Cx) {
        // Adapt the preview based on the available space.
        self.view
            .adaptive_view(id!(adaptive_preview))
            .set_variant_selector(cx, |_cx, parent_size| {
                match parent_size.x {
                    x if x <= 100. => live_id!(OnlyIcon),
                    x if x <= 250. => live_id!(IconAndName),
                    _ => live_id!(FullPreview),
                }
            });
    }
}

impl Widget for RoomPreview {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        let uid = self.widget_uid().clone();

        match event.hits(cx, self.view.area()) {
            Hit::FingerDown(_fe) => {
                cx.set_key_focus(self.view.area());
            }
            Hit::FingerUp(fe) => {
                if fe.was_tap() {
                    cx.widget_action(uid, &scope.path, RoomPreviewAction::Click);
                }
            }
            _ => (),
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
        if let Some(room_info) = scope.props.get::<RoomPreviewEntry>() {
            if let Some(ref name) = room_info.room_name {
                self.view.label(id!(room_name)).set_text(name);
            }
            if let Some((ts, msg)) = room_info.latest.as_ref() {
                if let Some(human_readable_date) = relative_format(ts) {
                    self.view
                        .label(id!(timestamp))
                        .set_text(&human_readable_date);
                }
                self.view
                    .html_or_plaintext(id!(latest_message))
                    .show_html(msg);
            }
            match room_info.avatar {
                RoomPreviewAvatar::Text(ref text) => {
                    self.view.avatar(id!(avatar)).show_text(None, text);
                }
                RoomPreviewAvatar::Image(ref img_bytes) => {
                    let _ = self.view.avatar(id!(avatar)).show_image(
                        None, // don't make room preview avatars clickable.
                        |img| utils::load_png_or_jpg(&img, cx, img_bytes),
                    );
                }
            }

            if cx.get_global::<DisplayContext>().is_desktop() {
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

        // TODO: This is quite verbose, makepad should provide a way to override this at a higher level.
        if is_selected {
            bg_color = vec3(0.059, 0.533, 0.996); // COLOR_PRIMARY_SELECTED
            message_text_color = vec3(1., 1., 1.); // COLOR_PRIMARY
            room_name_color = vec3(1., 1., 1.); // COLOR_PRIMARY
            timestamp_color = vec3(1., 1., 1.); // COLOR_PRIMARY
        } else {
            bg_color = vec3(1., 1., 1.); // COLOR_PRIMARY
            message_text_color = vec3(0.267, 0.267, 0.267); // MESSAGE_TEXT_COLOR
            room_name_color = vec3(0., 0., 0.);
            timestamp_color = vec3(0.6, 0.6, 0.6);
        }

        self.view.apply_over(
            cx,
            live!(
                draw_bg: {
                    color: (bg_color)
                }
            ),
        );

        self.view.label(id!(room_name)).apply_over(
            cx,
            live!(
                draw_text: {
                    color: (room_name_color)
                }
            ),
        );

        self.view.label(id!(timestamp)).apply_over(
            cx,
            live!(
                    draw_text: {
                        color: (timestamp_color)
                    }
            ),
        );

        self.html_or_plaintext(id!(latest_message)).apply_over(
            cx,
            live!(
                    html_view = {
                        html = {
                            font_color: (message_text_color),
                            draw_normal:      { color: (message_text_color) },
                            draw_italic:      { color: (message_text_color) },
                            draw_bold:        { color: (message_text_color) },
                            draw_bold_italic: { color: (message_text_color) },
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
