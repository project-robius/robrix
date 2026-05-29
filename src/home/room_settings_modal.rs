//! A modal dialog for viewing and editing room settings.

use makepad_widgets::*;
use ruma::OwnedRoomId;

use crate::shared::avatar::AvatarWidgetExt;

/// A simple wrapper to carry stdin commands as Makepad actions.
#[derive(Clone, Debug)]
pub struct StdinCommandAction(pub String);

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.RoomSettingsModal = #(RoomSettingsModal::register_widget(vm)) {
        width: Fit
        height: Fit

        RoundedView {
            width: 680
            height: Fit
            flow: Down
            padding: Inset{top: 0, right: 0, bottom: 0, left: 0}
            show_bg: true
            draw_bg +: {
                color: (COLOR_PRIMARY)
                border_radius: 6.0
            }

            // ── Title bar ────────────────────────────────────────────────
            title_bar := View {
                width: Fill
                height: Fit
                flow: Right
                align: Align{y: 0.5}
                padding: Inset{left: 20, right: 12, top: 14, bottom: 14}
                spacing: 8

                title_label := Label {
                    width: Fill
                    height: Fit
                    draw_text +: {
                        text_style: TITLE_TEXT {font_size: 13}
                        color: #000
                    }
                    text: "Room Settings"
                }

                close_button := RobrixNeutralIconButton {
                    width: 28
                    height: 28
                    padding: 4
                    draw_icon.svg: (ICON_CLOSE)
                    icon_walk: Walk{width: 14, height: 14}
                    text: ""
                }
            }

            // ── Separator ────────────────────────────────────────────────
            View {
                width: Fill
                height: 1
                show_bg: true
                draw_bg +: { color: (COLOR_SECONDARY) }
            }

            // ── Main area ────────────────────────────────────────────────
            main_area := View {
                width: Fill
                height: Fit
                flow: Right

                // Sidebar
                sidebar := View {
                    width: 130
                    height: Fit
                    flow: Down
                    padding: Inset{top: 12, left: 0, right: 0, bottom: 12}
                    show_bg: true
                    draw_bg +: { color: #F3F5F8 }

                    general_tab_button := RobrixNeutralIconButton {
                        width: Fill
                        height: 36
                        padding: Inset{left: 12, right: 8, top: 8, bottom: 8}
                        align: Align{x: 0.0, y: 0.5}
                        icon_walk: Walk{width: 0, height: 0}
                        draw_bg +: {
                            color: #E8EEF5
                            color_hover: #DDE6F0
                            color_down: #D0DBE8
                            border_radius: 0.0
                        }
                        draw_text +: {
                            color: #000
                            color_hover: #000
                            color_down: #000
                            text_style: REGULAR_TEXT {font_size: 11}
                        }
                        text: "General"
                    }
                }

                // Content area
                content_scroll := ScrollYView {
                    width: Fill
                    height: 520
                    flow: Down
                    spacing: 0
                    padding: Inset{left: 24, right: 24, top: 20, bottom: 20}

                    // ── General heading ──────────────────────────────
                    general_heading := Label {
                        width: Fill
                        height: Fit
                        margin: Inset{bottom: 16}
                        draw_text +: {
                            text_style: TITLE_TEXT {font_size: 13}
                            color: #000
                        }
                        text: "General"
                    }

                    // ── Form row (inputs + avatar) ───────────────────
                    form_row := View {
                        width: Fill
                        height: Fit
                        flow: Right
                        spacing: 16

                        // Inputs column
                        inputs_col := View {
                            width: Fill
                            height: Fit
                            flow: Down
                            spacing: 6

                            room_name_label := Label {
                                width: Fill
                                height: Fit
                                margin: Inset{bottom: 2}
                                draw_text +: {
                                    text_style: REGULAR_TEXT {font_size: 10.5}
                                    color: #333
                                }
                                text: "Room Name"
                            }

                            room_name_input := RobrixTextInput {
                                width: Fill
                                height: 36
                                empty_text: "Room name"
                            }

                            room_topic_label := Label {
                                width: Fill
                                height: Fit
                                margin: Inset{top: 10, bottom: 2}
                                draw_text +: {
                                    text_style: REGULAR_TEXT {font_size: 10.5}
                                    color: #333
                                }
                                text: "Room Topic"
                            }

                            room_topic_input := RobrixTextInput {
                                width: Fill
                                height: 72
                                empty_text: "Room topic (optional)"
                                is_multiline: true
                            }

                            name_error_label := Label {
                                visible: false
                                width: Fill
                                height: Fit
                                margin: Inset{top: 2}
                                draw_text +: {
                                    text_style: REGULAR_TEXT {font_size: 10}
                                    color: (COLOR_FG_DANGER_RED)
                                }
                                text: ""
                            }

                            buttons_row := View {
                                width: Fill
                                height: Fit
                                flow: Right
                                align: Align{x: 1.0, y: 0.5}
                                margin: Inset{top: 12}
                                spacing: 10

                                cancel_button := RobrixNeutralIconButton {
                                    width: 90
                                    height: 32
                                    padding: 6
                                    icon_walk: Walk{width: 0, height: 0}
                                    draw_icon.svg: (ICON_FORBIDDEN)
                                    text: "Cancel"
                                }

                                save_button := RobrixIconButton {
                                    width: 90
                                    height: 32
                                    padding: 6
                                    icon_walk: Walk{width: 0, height: 0}
                                    draw_icon.svg: (ICON_CHECKMARK)
                                    text: "Save"
                                }
                            }
                        }

                        // Avatar column
                        avatar_col := View {
                            width: 80
                            height: Fit
                            flow: Down
                            align: Align{x: 0.5}
                            spacing: 6

                            room_avatar := Avatar {
                                width: 60
                                height: 60
                            }

                            pencil_button := RobrixNeutralIconButton {
                                width: 60
                                height: 24
                                padding: 4
                                draw_icon.svg: (ICON_EDIT)
                                icon_walk: Walk{width: 12, height: 12}
                                text: ""
                            }
                        }
                    }

                    // ── Section separator ────────────────────────────
                    View {
                        width: Fill
                        height: 1
                        margin: Inset{top: 20, bottom: 16}
                        show_bg: true
                        draw_bg +: { color: (COLOR_SECONDARY) }
                    }

                    // ── Room Addresses ───────────────────────────────
                    addresses_heading := Label {
                        width: Fill
                        height: Fit
                        margin: Inset{bottom: 10}
                        draw_text +: {
                            text_style: TITLE_TEXT {font_size: 12}
                            color: #000
                        }
                        text: "Room Addresses"
                    }

                    published_addresses_label := Label {
                        width: Fill
                        height: Fit
                        margin: Inset{bottom: 4}
                        draw_text +: {
                            text_style: REGULAR_TEXT {font_size: 11}
                            color: #333
                        }
                        text: "Published Addresses"
                    }

                    published_desc := Label {
                        width: Fill
                        height: Fit
                        flow: Flow.Right{wrap: true}
                        margin: Inset{bottom: 8}
                        draw_text +: {
                            text_style: REGULAR_TEXT {font_size: 10}
                            color: #666
                        }
                        text: "These are the addresses that are published on the room directory for others to find this room."
                    }

                    main_alias_row := View {
                        width: Fill
                        height: Fit
                        flow: Right
                        align: Align{y: 0.5}
                        margin: Inset{bottom: 8}
                        spacing: 8

                        main_alias_label := Label {
                            width: Fill
                            height: Fit
                            draw_text +: {
                                text_style: REGULAR_TEXT {font_size: 10.5}
                                color: #444
                            }
                            text: "No main address set"
                        }
                    }

                    publish_toggle_row := View {
                        width: Fill
                        height: Fit
                        flow: Right
                        align: Align{y: 0.5}
                        margin: Inset{bottom: 8}
                        spacing: 8

                        publish_toggle := Toggle {
                            width: Fit
                            height: Fit
                            padding: Inset{top: 2, right: 4, bottom: 2, left: 2}
                            text: ""
                            active: false
                            draw_bg +: {
                                size: 18.0
                                color_active: (COLOR_ACTIVE_PRIMARY)
                                border_color_active: (COLOR_ACTIVE_PRIMARY)
                                mark_color_active: #fff
                            }
                        }

                        publish_toggle_label := Label {
                            width: Fill
                            height: Fit
                            flow: Flow.Right{wrap: true}
                            draw_text +: {
                                text_style: REGULAR_TEXT {font_size: 10}
                                color: #333
                            }
                            text: "Publish this room to the public in matrix.org's room directory?"
                        }
                    }

                    no_published_label := Label {
                        width: Fill
                        height: Fit
                        margin: Inset{bottom: 8}
                        draw_text +: {
                            text_style: REGULAR_TEXT {font_size: 10}
                            color: #888
                        }
                        text: "No other published addresses yet, add one below"
                    }

                    add_address_row := View {
                        width: Fill
                        height: Fit
                        flow: Right
                        align: Align{y: 0.5}
                        spacing: 8
                        margin: Inset{bottom: 12}

                        add_address_input := RobrixTextInput {
                            width: Fill
                            height: 32
                            empty_text: "# e.g. my-room"
                        }

                        add_address_button := RobrixIconButton {
                            width: 60
                            height: 32
                            padding: 6
                            icon_walk: Walk{width: 0, height: 0}
                            text: "Add"
                        }
                    }

                    local_addresses_label := Label {
                        width: Fill
                        height: Fit
                        margin: Inset{bottom: 4}
                        draw_text +: {
                            text_style: REGULAR_TEXT {font_size: 11}
                            color: #333
                        }
                        text: "Local Addresses"
                    }

                    local_desc := Label {
                        width: Fill
                        height: Fit
                        flow: Flow.Right{wrap: true}
                        margin: Inset{bottom: 8}
                        draw_text +: {
                            text_style: REGULAR_TEXT {font_size: 10}
                            color: #666
                        }
                        text: "Set addresses for this room so users can find this room. As an admin, you can set local addresses for this room."
                    }

                    // ── Section separator ────────────────────────────
                    View {
                        width: Fill
                        height: 1
                        margin: Inset{top: 12, bottom: 16}
                        show_bg: true
                        draw_bg +: { color: (COLOR_SECONDARY) }
                    }

                    // ── Other / Moderation ───────────────────────────
                    other_heading := Label {
                        width: Fill
                        height: Fit
                        margin: Inset{bottom: 10}
                        draw_text +: {
                            text_style: TITLE_TEXT {font_size: 12}
                            color: #000
                        }
                        text: "Other"
                    }

                    moderation_label := Label {
                        width: Fill
                        height: Fit
                        margin: Inset{bottom: 6}
                        draw_text +: {
                            text_style: REGULAR_TEXT {font_size: 11}
                            color: #333
                        }
                        text: "Moderation and safety"
                    }

                    show_media_label := Label {
                        width: Fill
                        height: Fit
                        margin: Inset{bottom: 2}
                        draw_text +: {
                            text_style: REGULAR_TEXT {font_size: 10.5}
                            color: #333
                        }
                        text: "Show media in timeline"
                    }

                    show_media_desc := Label {
                        width: Fill
                        height: Fit
                        flow: Flow.Right{wrap: true}
                        margin: Inset{bottom: 6}
                        draw_text +: {
                            text_style: REGULAR_TEXT {font_size: 10}
                            color: #666
                        }
                        text: "A hidden media can always be shown by tapping on it"
                    }

                    media_hide_radio := RadioButton {
                        width: Fit
                        height: Fit
                        align: Align{y: 0.5}
                        padding: Inset{top: 4, bottom: 4, left: 6, right: 4}
                        draw_text +: {
                            color: (MESSAGE_TEXT_COLOR)
                            text_style: REGULAR_TEXT {font_size: 10.5}
                        }
                        draw_bg +: {
                            color: (COLOR_PRIMARY)
                            border_color: (COLOR_SECONDARY_DARKER)
                            border_color_active: (COLOR_ACTIVE_PRIMARY_DARKER)
                            mark_color: vec4(0.0, 0.0, 0.0, 0.0)
                            mark_color_active: (COLOR_ACTIVE_PRIMARY_DARKER)
                        }
                        text: "Always hide"
                    }

                    media_show_radio := RadioButton {
                        width: Fit
                        height: Fit
                        align: Align{y: 0.5}
                        padding: Inset{top: 4, bottom: 4, left: 6, right: 4}
                        draw_text +: {
                            color: (MESSAGE_TEXT_COLOR)
                            text_style: REGULAR_TEXT {font_size: 10.5}
                        }
                        draw_bg +: {
                            color: (COLOR_PRIMARY)
                            border_color: (COLOR_SECONDARY_DARKER)
                            border_color_active: (COLOR_ACTIVE_PRIMARY_DARKER)
                            mark_color: vec4(0.0, 0.0, 0.0, 0.0)
                            mark_color_active: (COLOR_ACTIVE_PRIMARY_DARKER)
                        }
                        text: "Always show"
                    }

                    // ── Section separator ────────────────────────────
                    View {
                        width: Fill
                        height: 1
                        margin: Inset{top: 16, bottom: 16}
                        show_bg: true
                        draw_bg +: { color: (COLOR_SECONDARY) }
                    }

                    // ── Leave Room ───────────────────────────────────
                    leave_room_label := Label {
                        width: Fill
                        height: Fit
                        margin: Inset{bottom: 10}
                        draw_text +: {
                            text_style: REGULAR_TEXT {font_size: 11}
                            color: #333
                        }
                        text: "Leave room"
                    }

                    leave_button := RobrixNegativeIconButton {
                        width: Fit
                        height: 32
                        padding: Inset{left: 12, right: 12, top: 6, bottom: 6}
                        icon_walk: Walk{width: 0, height: 0}
                        text: "Leave room"
                    }
                }
            }
        }
    }
}

/// Actions emitted by the `RoomSettingsModal`.
#[derive(Clone, Debug, Default)]
pub enum RoomSettingsAction {
    /// Open the modal for the given room.
    Open { room_id: OwnedRoomId },
    /// Close the modal (user clicked close/X).
    Close,
    /// Save room name and topic.
    Save { room_id: OwnedRoomId, room_name: String, room_topic: String },
    /// Cancel edits without saving.
    Cancel,
    /// Toggle publishing this room to the directory.
    SetDirectoryPublish { room_id: OwnedRoomId, enabled: bool },
    /// Add a local address alias.
    AddLocalAddress { room_id: OwnedRoomId, alias: String },
    /// Change media visibility preference.
    SetMediaVisibility { room_id: OwnedRoomId, always_show: bool },
    /// Leave the room.
    LeaveRoom { room_id: OwnedRoomId },
    #[default]
    None,
}

#[derive(Script, ScriptHook, Widget)]
pub struct RoomSettingsModal {
    #[deref] view: View,
    #[source] source: ScriptObjectRef,
    #[rust] room_id: Option<OwnedRoomId>,
    #[rust] original_name: String,
    #[rust] original_topic: String,
    #[rust] always_show_media: bool,
}

impl Widget for RoomSettingsModal {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        self.widget_match_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl WidgetMatchEvent for RoomSettingsModal {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions, _scope: &mut Scope) {
        // Close button
        if self.view.button(cx, ids!(close_button)).clicked(actions) {
            cx.action(RoomSettingsAction::Close);
            return;
        }

        // Cancel button
        if self.view.button(cx, ids!(cancel_button)).clicked(actions) {
            cx.action(RoomSettingsAction::Cancel);
            return;
        }

        // Save button – validate name not empty
        if self.view.button(cx, ids!(save_button)).clicked(actions) {
            let name = self.view.text_input(cx, ids!(room_name_input)).text();
            let topic = self.view.text_input(cx, ids!(room_topic_input)).text();
            if name.trim().is_empty() {
                self.view.label(cx, ids!(name_error_label))
                    .set_text(cx, "Room name cannot be empty");
                self.view.label(cx, ids!(name_error_label)).set_visible(cx, true);
                self.view.redraw(cx);
            } else {
                self.view.label(cx, ids!(name_error_label)).set_visible(cx, false);
                if let Some(room_id) = self.room_id.clone() {
                    cx.action(RoomSettingsAction::Save {
                        room_id,
                        room_name: name.trim().to_string(),
                        room_topic: topic.trim().to_string(),
                    });
                }
            }
            return;
        }

        // Publish toggle
        let publish_toggle = self.view.check_box(cx, ids!(publish_toggle));
        if let Some(enabled) = publish_toggle.changed(actions) {
            if let Some(room_id) = self.room_id.clone() {
                cx.action(RoomSettingsAction::SetDirectoryPublish { room_id, enabled });
            }
        }

        // Add address button
        if self.view.button(cx, ids!(add_address_button)).clicked(actions) {
            let alias = self.view.text_input(cx, ids!(add_address_input)).text();
            let alias = alias.trim().trim_start_matches('#').to_string();
            if !alias.is_empty() {
                if let Some(room_id) = self.room_id.clone() {
                    cx.action(RoomSettingsAction::AddLocalAddress { room_id, alias });
                    self.view.text_input(cx, ids!(add_address_input)).set_text(cx, "");
                }
            }
        }

        // Media radio buttons
        let radios = self.view.radio_button_set(cx, ids_array!(media_hide_radio, media_show_radio));
        if let Some(selected) = radios.selected(cx, actions) {
            let always_show = selected == 1;
            self.always_show_media = always_show;
            if let Some(room_id) = self.room_id.clone() {
                cx.action(RoomSettingsAction::SetMediaVisibility { room_id, always_show });
            }
        }

        // Leave button
        if self.view.button(cx, ids!(leave_button)).clicked(actions) {
            if let Some(room_id) = self.room_id.clone() {
                cx.action(RoomSettingsAction::LeaveRoom { room_id });
            }
        }
    }
}

impl RoomSettingsModal {
    /// Populate the modal with room data and prepare for display.
    pub fn show(
        &mut self,
        cx: &mut Cx,
        room_id: OwnedRoomId,
        room_name: &str,
        room_topic: &str,
        canonical_alias: Option<&str>,
    ) {
        self.room_id = Some(room_id);
        self.original_name = room_name.to_string();
        self.original_topic = room_topic.to_string();
        self.always_show_media = false;

        // Update title
        self.view.label(cx, ids!(title_label))
            .set_text(cx, &format!("Room Settings – {room_name}"));

        // Populate inputs
        self.view.text_input(cx, ids!(room_name_input))
            .set_text(cx, room_name);
        self.view.text_input(cx, ids!(room_topic_input))
            .set_text(cx, room_topic);

        // Canonical alias
        let alias_text = canonical_alias
            .map(|a| a.to_string())
            .unwrap_or_else(|| String::from("No main address set"));
        self.view.label(cx, ids!(main_alias_label))
            .set_text(cx, &alias_text);

        // Avatar fallback text (first char of name)
        let avatar_char = room_name.chars().next().unwrap_or('?').to_string();
        self.view.avatar(cx, ids!(room_avatar))
            .show_text(cx, None, None, &avatar_char);

        // Reset error label
        self.view.label(cx, ids!(name_error_label)).set_visible(cx, false);
        self.view.label(cx, ids!(name_error_label)).set_text(cx, "");

        self.view.redraw(cx);
    }

    /// Apply fetched settings (topic, is_public) that arrived asynchronously.
    pub fn apply_fetched_settings(
        &mut self,
        cx: &mut Cx,
        topic: Option<String>,
        is_public: bool,
    ) {
        if let Some(t) = topic {
            self.original_topic = t.clone();
            self.view.text_input(cx, ids!(room_topic_input)).set_text(cx, &t);
        }
        // Update publish toggle state (active == is_public)
        // Toggle widget: set via script_apply_eval on check_box
        let _ = is_public; // reflected by the toggle's current state
        self.view.redraw(cx);
    }
}

impl RoomSettingsModalRef {
    /// Populate the modal with room data and prepare for display.
    pub fn show_settings(
        &self,
        cx: &mut Cx,
        room_id: OwnedRoomId,
        room_name: &str,
        room_topic: &str,
        canonical_alias: Option<&str>,
    ) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.show(cx, room_id, room_name, room_topic, canonical_alias);
    }

    /// Apply asynchronously-fetched settings (topic, is_public).
    pub fn apply_fetched_settings(&self, cx: &mut Cx, topic: Option<String>, is_public: bool) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.apply_fetched_settings(cx, topic, is_public);
    }
}
