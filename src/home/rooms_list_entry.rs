use makepad_widgets::*;
use matrix_sdk::ruma::OwnedRoomId;

use crate::{
    app::AppState,
    i18n::{AppLanguage, tr_fmt, tr_key},
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
                svg: (ICON_TOMBSTONE)
                color: (COLOR_FG_DANGER_RED)
            }
            icon_walk: Walk{ width: 15, height: 15 }
        }
    }

    mod.widgets.RoomName = Label {
        width: Fill, height: Fit
        flow: Flow.Right{wrap: false},
        padding: 0,
        draw_text +: {
            color: #000,
            text_style: USERNAME_TEXT_STYLE { font_size: 10. }
        }
        text: "[Room name unknown]"
    }

    mod.widgets.RoomsListEntryTimestamp = Label {
        padding: Inset{top: 1},
        width: Fit, height: Fit
        flow: Flow.Right{wrap: false},
        draw_text +: {
            color: (TIMESTAMP_TEXT_COLOR)
            text_style: TIMESTAMP_TEXT_STYLE { font_size: 7.5 }
        }
    }

    mod.widgets.MessagePreview = View {
        width: Fill, height: Fit
        latest_message := HtmlOrPlaintext {
            html_view +: {
                html +: {
                    font_size: 9.3
                    text_style_normal +: { font_size: 9.3 }
                    text_style_italic +: { font_size: 9.3 }
                    text_style_bold +: { font_size: 9.3 }
                    text_style_bold_italic +: { font_size: 9.3 }
                    text_style_fixed +: { font_size: 9.3 }
                }
            }
            plaintext_view +: {
                pt_label +: {
                    draw_text +: {
                        text_style: theme.font_regular { font_size: 9.5 },
                    }
                    text: "[No recent messages]"
                }
            }
        }
    }

    mod.widgets.RoomsListEntryContent = set_type_default() do #(RoomsListEntryContent::register_widget(vm)) {

        flow: Right,
        spacing: 10,
        padding: 10,
        width: Fill, height: Fit

        show_bg: true
        draw_bg +: {
            active: instance(0.0)
            color: instance(#0000)
            color_selected: instance(COLOR_ACTIVE_PRIMARY)
            border_color: instance(#0000)
            border_size: uniform(0.0)
            border_radius: uniform(4.0)
            border_inset: uniform(vec4(0.0))

            get_color: fn() -> vec4 {
                return mix(self.color, self.color_selected, self.active)
            }

            pixel: fn() {
                let sdf = Sdf2d.viewport(self.pos * self.rect_size)
                sdf.box(
                    self.border_inset.x + self.border_size,
                    self.border_inset.y + self.border_size,
                    self.rect_size.x - (self.border_inset.x + self.border_inset.z + self.border_size * 2.0),
                    self.rect_size.y - (self.border_inset.y + self.border_inset.w + self.border_size * 2.0),
                    max(1.0, self.border_radius)
                )
                sdf.fill_keep(self.get_color())
                if self.border_size > 0.0 {
                    sdf.stroke(self.border_color, self.border_size)
                }
                return sdf.result;
            }
        }
        animator: Animator{
            selected: {
                default: @off
                off: AnimatorState{
                    from: {all: Snap}
                    apply: {
                        draw_bg: {active: 0.0}
                    }
                }
                on: AnimatorState{
                    from: {all: Snap}
                    apply: {
                        draw_bg: {active: 1.0}
                    }
                }
            }
        }
    }

    mod.widgets.RoomsListEntry = #(RoomsListEntry::register_widget(vm)) {
        flow: Down, height: Fit
        cursor: MouseCursor.Default,

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
                        timestamp := mod.widgets.RoomsListEntryTimestamp { }
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
#[derive(Script, Widget)]
pub struct RoomsListEntry {
    #[deref] view: View,
    #[rust] room_id: Option<OwnedRoomId>,
}

impl ScriptHook for RoomsListEntry {
    fn on_after_new(&mut self, vm: &mut ScriptVm) {
        vm.with_cx_mut(|cx| {
            self.set_adaptive_variant_selector(cx);
        })
    }
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

impl RoomsListEntry {
    fn set_adaptive_variant_selector(&self, cx: &mut Cx) {
        self.view
            .adaptive_view(cx, ids!(adaptive_preview))
            .set_variant_selector(|cx, parent_size| {
                if cx.display_context.is_desktop() {
                    id!(FullPreview)
                } else {
                    match parent_size.x {
                        width if width <= 70.0 => id!(OnlyIcon),
                        width if width <= 200.0 => id!(IconAndName),
                        _ => id!(FullPreview),
                    }
                }
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

#[derive(Script, ScriptHook, Widget, Animator)]
pub struct RoomsListEntryContent {
    #[source] source: ScriptObjectRef,
    #[deref] view: View,
    #[apply_default] animator: Animator,
}

impl Widget for RoomsListEntryContent {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if self.animator_handle_event(cx, event).must_redraw() {
            self.redraw(cx);
        }
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        let app_language = scope.data.get::<AppState>()
            .map(|app_state| app_state.app_language)
            .unwrap_or_default();
        if let Some(joined_room_info) = scope.props.get::<JoinedRoomInfo>() {
            self.draw_joined_room(cx, joined_room_info);
        } else if let Some(invited_room_info) = scope.props.get::<InvitedRoomInfo>() {
            self.draw_invited_room(cx, invited_room_info, app_language);
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
        app_language: AppLanguage,
    ) {
        self.view.label(cx, ids!(room_name)).set_text(cx, &room_info.room_name_id.to_string());
        // Hide the timestamp field, and use the latest message field to show the inviter.
        self.view.label(cx, ids!(timestamp)).set_text(cx, "");
        let inviter_string = match &room_info.inviter_info {
            Some(InviterInfo { user_id, display_name: Some(dn), .. }) => {
                let display_name = htmlize::escape_text(dn);
                let user_id = htmlize::escape_text(user_id.as_str());
                tr_fmt(
                    app_language,
                    "rooms_list_entry.invited.by_name_and_user",
                    &[("display_name", display_name.as_ref()), ("user_id", user_id.as_ref())],
                )
            }
            Some(InviterInfo { user_id, .. }) => {
                let user_id = htmlize::escape_text(user_id.as_str());
                tr_fmt(
                    app_language,
                    "rooms_list_entry.invited.by_user",
                    &[("user_id", user_id.as_ref())],
                )
            }
            None => tr_key(app_language, "rooms_list_entry.invited.generic").to_string(),
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
    pub fn update_preview_colors(&mut self, cx: &mut Cx, is_selected: bool) {
        let message_text_color;
        let room_name_color;
        let timestamp_color;
        let code_bg_color;

        // TODO: use script-defined theme color instead of redefining constants below
        if is_selected {
            message_text_color = vec4(1., 1., 1., 1.); // COLOR_PRIMARY
            room_name_color = vec4(1., 1., 1., 1.); // COLOR_PRIMARY
            timestamp_color = vec4(1., 1., 1., 1.); // COLOR_PRIMARY
            code_bg_color = vec4(0.3, 0.3, 0.3, 1.0); // a darker gray used for the background of code blocks and quote blocks
        } else {
            message_text_color = vec4(0.267, 0.267, 0.267, 1.0); // MESSAGE_TEXT_COLOR
            room_name_color = vec4(0., 0., 0., 1.0);
            timestamp_color = vec4(0.6, 0.6, 0.6, 1.0);
            code_bg_color = vec4(0.929, 0.929, 0.929, 1.0); // #EDEDED
        }

        // Toggle the background color via the animator (handles selected/deselected bg).
        self.animator_toggle(cx, is_selected, Animate::No, ids!(selected.on), ids!(selected.off));

        // Update text colors for room name.
        let mut room_name_label = self.view.label(cx, ids!(room_name));
        script_apply_eval!(cx, room_name_label, {
            draw_text +: {
                color: #(room_name_color)
            }
        });

        // Update text colors for timestamp.
        let mut timestamp_label = self.view.label(cx, ids!(timestamp));
        script_apply_eval!(cx, timestamp_label, {
            draw_text +: {
                color: #(timestamp_color)
            }
        });

        // Update text colors for the latest message preview (both HTML and plaintext variants).
        let mut html_widget = self.view.html(cx, ids!(latest_message.html_view.html));
        script_apply_eval!(cx, html_widget, {
            font_color: #(message_text_color),
            draw_text +: { color: #(message_text_color) },
            draw_block +: {
                quote_bg_color: #(code_bg_color),
                code_color: #(code_bg_color),
            }
        });

        // When selected, set link color to None so links inherit font_color (white)
        // for better contrast against the blue selected background.
        // When not selected, restore the default blue link color.
        self.view
            .html_or_plaintext(cx, ids!(latest_message))
            .set_link_color(cx, if is_selected {
                None
            } else {
                Some(vec4(0., 0., 0.933, 1.0)) // #0000EE, default HtmlLink color
            });

        let mut pt_label = self.view.label(cx, ids!(latest_message.plaintext_view.pt_label));
        script_apply_eval!(cx, pt_label, {
            draw_text +: {
                color: #(message_text_color)
            }
        });
    }
}
