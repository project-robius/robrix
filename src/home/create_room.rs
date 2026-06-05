//! "Create room" flow — pure config types + validation helpers + the
//! `CreateRoomScreen` widget that drives them.
//!
//! Validation and ruma-request building live in pure functions
//! (`validate_create_room_config`, `CreateRoomConfig::to_ruma_request`)
//! so the spec's per-rule scenarios can eventually be unit-tested
//! without spinning up a Matrix `Client`.

use makepad_widgets::*;
use ruma::{
    OwnedUserId, UserId,
    api::client::room::{
        create_room::v3::{Request as CreateRoomRequest, RoomPreset},
        Visibility,
    },
    events::{
        AnyInitialStateEvent, InitialStateEvent,
        room::encryption::RoomEncryptionEventContent,
    },
    serde::Raw,
    EventEncryptionAlgorithm,
};

use crate::shared::popup_list::{enqueue_popup_notification, PopupKind};
use crate::sliding_sync::{submit_async_request, MatrixRequest};

// ============================================================================
// Config + validation types
// ============================================================================

#[derive(Clone, Debug)]
pub struct CreateRoomConfig {
    pub name: String,
    pub topic: Option<String>,
    pub avatar_bytes: Option<Vec<u8>>,
    pub avatar_mime: Option<String>,
    pub visibility: RoomVisibilityChoice,
    pub e2ee_enabled: bool,
    pub initial_invitees: Vec<OwnedUserId>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoomVisibilityChoice {
    Public,
    Private,
}

impl Default for CreateRoomConfig {
    fn default() -> Self {
        Self {
            name: String::new(),
            topic: None,
            avatar_bytes: None,
            avatar_mime: None,
            visibility: RoomVisibilityChoice::Private,
            e2ee_enabled: false,
            initial_invitees: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CreateRoomConfigError {
    EmptyName,
    NameTooLong { len: usize, max: usize },
    AvatarTooLarge { bytes: usize, max: usize },
    /// E2EE on a Public room is explicitly refused by robrix this month.
    EncryptedPublicRoom,
}

const NAME_MAX: usize = 255;
const AVATAR_MAX_BYTES: usize = 4 * 1024 * 1024; // 4 MiB

// ============================================================================
// Pure validators
// ============================================================================

/// Returns `Ok(())` iff the given config is well-formed enough to send
/// to the server. The widget's submit button is wired to this — if it
/// returns `Err`, the button stays disabled so we never round-trip an
/// invalid request.
pub fn validate_create_room_config(
    config: &CreateRoomConfig,
) -> Result<(), CreateRoomConfigError> {
    let trimmed = config.name.trim();
    if trimmed.is_empty() {
        return Err(CreateRoomConfigError::EmptyName);
    }
    let name_len = trimmed.chars().count();
    if name_len > NAME_MAX {
        return Err(CreateRoomConfigError::NameTooLong {
            len: name_len,
            max: NAME_MAX,
        });
    }
    if let Some(bytes) = &config.avatar_bytes {
        if bytes.len() > AVATAR_MAX_BYTES {
            return Err(CreateRoomConfigError::AvatarTooLarge {
                bytes: bytes.len(),
                max: AVATAR_MAX_BYTES,
            });
        }
    }
    if config.e2ee_enabled && config.visibility == RoomVisibilityChoice::Public {
        return Err(CreateRoomConfigError::EncryptedPublicRoom);
    }
    Ok(())
}

/// Split a raw textarea string into successfully-parsed `OwnedUserId`s
/// and a parallel list of raw substrings that failed to parse. Order
/// is preserved so the UI can render failed strings inline next to
/// where the user typed them. Whitespace AND commas are delimiters.
pub fn parse_invitee_list(raw: &str) -> (Vec<OwnedUserId>, Vec<String>) {
    let mut parsed = Vec::new();
    let mut failed = Vec::new();
    for token in raw.split(|c: char| c.is_whitespace() || c == ',') {
        let token = token.trim();
        if token.is_empty() {
            continue;
        }
        match UserId::parse(token) {
            Ok(uid) => parsed.push(uid),
            Err(_) => failed.push(token.to_string()),
        }
    }
    (parsed, failed)
}

// ============================================================================
// Ruma request construction
// ============================================================================

impl CreateRoomConfig {
    /// Build the `create_room::v3::Request` from this config. Visibility
    /// and preset are derived from the user's choice; an
    /// `m.room.encryption` initial-state event is added iff
    /// `e2ee_enabled == true`. `initial_invitees` are NOT placed in the
    /// request — they are invited via `Joined::invite_user_by_id` calls
    /// after creation so per-invitee failures don't block the create.
    pub fn to_ruma_request(&self) -> CreateRoomRequest {
        let mut request = CreateRoomRequest::new();
        let name = self.name.trim().to_string();
        if !name.is_empty() {
            request.name = Some(name);
        }
        if let Some(topic) = self.topic.as_ref().filter(|s| !s.trim().is_empty()) {
            request.topic = Some(topic.trim().to_string());
        }
        let (visibility, preset) = match self.visibility {
            RoomVisibilityChoice::Public => (Visibility::Public, Some(RoomPreset::PublicChat)),
            RoomVisibilityChoice::Private => (Visibility::Private, Some(RoomPreset::PrivateChat)),
        };
        request.visibility = visibility;
        request.preset = preset;
        if self.e2ee_enabled {
            // H9: use the typed variant directly, not an `&str` conversion.
            let encryption_content = RoomEncryptionEventContent::new(
                EventEncryptionAlgorithm::MegolmV1AesSha2,
            );
            let initial_state_event =
                InitialStateEvent::new(Default::default(), encryption_content);
            let raw: Raw<AnyInitialStateEvent> = Raw::new(&initial_state_event)
                .expect("RoomEncryptionEventContent serializes")
                .cast_unchecked();
            request.initial_state = vec![raw];
        }
        request
    }
}

// ============================================================================
// Cross-widget actions
// ============================================================================

#[derive(Clone, Debug)]
pub enum CreateRoomFromConfigAction {
    /// Room creation succeeded and all invites (if any) were accepted.
    Created {
        room_id: ruma::OwnedRoomId,
        room_name: String,
    },
    /// Room created but one or more invites failed. The room is usable;
    /// the UI should surface the failed user-ids so the user can retry.
    PartialInvite {
        room_id: ruma::OwnedRoomId,
        room_name: String,
        failed: Vec<OwnedUserId>,
    },
    /// `Client::create_room` itself failed.
    Failed { reason: String },
}

// ============================================================================
// Widget
// ============================================================================

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.CreateRoomScreen = #(CreateRoomScreen::register_widget(vm)) {
        ..mod.widgets.View

        // `height: Fit` is required because this widget is embedded inside
        // `add_room.rs::create_new_view` (which is also `Fit`) which itself
        // sits inside the outer `AddRoomScreen` ScrollYView. Using `Fill`
        // here would collapse the form to zero height.
        width: Fill, height: Fit,
        flow: Down,
        padding: Inset{top: 5, left: 15, right: 15, bottom: 0},
        spacing: 8,

        title := Label {
            width: Fill, height: Fit,
            draw_text +: {
                text_style: theme.font_regular { font_size: 18 },
                color: #000
            }
            text: "Create a new room"
        }

        SubsectionLabel {
            text: "Room name (required)"
        }
        name_input := RobrixTextInput {
            width: Fill,
            empty_text: "Project Alpha"
        }

        SubsectionLabel { text: "Topic (optional)" }
        topic_input := RobrixTextInput {
            width: Fill,
            empty_text: "What's this room about?"
        }

        SubsectionLabel { text: "Avatar (optional)" }
        Label {
            width: Fill, height: Fit
            draw_text +: {
                color: (MESSAGE_TEXT_COLOR)
                text_style: MESSAGE_TEXT_STYLE { font_size: 10 }
            }
            text: "Upload an image to represent the room."
        }
        avatar_row := View {
            width: Fill, height: Fit
            flow: Right, spacing: 12
            align: Align{ y: 0.5 }
            avatar_preview_view := RoundedView {
                width: 80, height: 80
                align: Align{ x: 0.5, y: 0.5 }
                show_bg: true
                draw_bg +: {
                    color: (COLOR_PRIMARY)
                    border_size: 1.0
                    border_color: (COLOR_SECONDARY_DARKER)
                    border_radius: 6.0
                }
                avatar_preview_image := Image {
                    visible: false
                    fit: ImageFit.Stretch
                    width: 72, height: 72
                }
                avatar_preview_caption := Label {
                    width: Fit, height: Fit
                    draw_text +: {
                        color: (MESSAGE_TEXT_COLOR)
                        text_style: MESSAGE_TEXT_STYLE { font_size: 9 }
                    }
                    text: "No image"
                }
            }
            avatar_actions := View {
                width: Fit, height: Fit
                flow: Down, spacing: 4
                upload_image_button := RobrixIconButton {
                    draw_icon.svg: (ICON_UPLOAD)
                    icon_walk: Walk{ width: 14, height: 14 }
                    text: "Upload image"
                }
                Label {
                    width: Fit, height: Fit
                    draw_text +: {
                        color: (MESSAGE_TEXT_COLOR)
                        text_style: MESSAGE_TEXT_STYLE { font_size: 9 }
                    }
                    text: "JPG, PNG or GIF. Max 4 MB."
                }
            }
        }

        SubsectionLabel { text: "Visibility" }
        visibility_row := View {
            width: Fill, height: Fit
            flow: Right, spacing: 12
            visibility_public_card := RoundedView {
                width: Fill, height: Fit
                flow: Down
                padding: 12, spacing: 6
                show_bg: true
                draw_bg +: {
                    color: (COLOR_PRIMARY)
                    border_size: 1.0
                    border_color: (COLOR_SECONDARY_DARKER)
                    border_radius: 6.0
                }
                visibility_public_header := View {
                    width: Fill, height: Fit
                    flow: Right, spacing: 8
                    align: Align{ y: 0.5 }
                    Icon {
                        draw_icon +: {
                            svg: (ICON_GLOBE)
                            color: (COLOR_TEXT)
                        }
                        icon_walk: Walk{ width: 20, height: 20 }
                    }
                    visibility_public := RadioButtonFlat {
                        text: "Public"
                        draw_text +: {
                            color: (COLOR_TEXT)
                            color_hover: (COLOR_TEXT)
                            color_focus: (COLOR_TEXT)
                            color_down: (COLOR_TEXT)
                            color_active: (COLOR_TEXT)
                            color_disabled: (COLOR_TEXT)
                            text_style: MESSAGE_TEXT_STYLE { font_size: 13 }
                        }
                    }
                }
                Label {
                    width: Fill, height: Fit
                    draw_text +: {
                        color: (MESSAGE_TEXT_COLOR)
                        text_style: MESSAGE_TEXT_STYLE { font_size: 10 }
                    }
                    text: "Anyone can discover and join this room."
                }
            }
            visibility_private_card := RoundedView {
                width: Fill, height: Fit
                flow: Down
                padding: 12, spacing: 6
                show_bg: true
                draw_bg +: {
                    color: (COLOR_PRIMARY)
                    border_size: 2.0
                    border_color: (COLOR_FG_ACCEPT_GREEN)
                    border_radius: 6.0
                }
                visibility_private_header := View {
                    width: Fill, height: Fit
                    flow: Right, spacing: 8
                    align: Align{ y: 0.5 }
                    Icon {
                        draw_icon +: {
                            svg: (ICON_LOCK)
                            color: (COLOR_TEXT)
                        }
                        icon_walk: Walk{ width: 20, height: 20 }
                    }
                    visibility_private := RadioButtonFlat {
                        text: "Private"
                        draw_text +: {
                            color: (COLOR_TEXT)
                            color_hover: (COLOR_TEXT)
                            color_focus: (COLOR_TEXT)
                            color_down: (COLOR_TEXT)
                            color_active: (COLOR_TEXT)
                            color_disabled: (COLOR_TEXT)
                            text_style: MESSAGE_TEXT_STYLE { font_size: 13 }
                        }
                    }
                }
                Label {
                    width: Fill, height: Fit
                    draw_text +: {
                        color: (MESSAGE_TEXT_COLOR)
                        text_style: MESSAGE_TEXT_STYLE { font_size: 10 }
                    }
                    text: "Only invited people can join this room."
                }
            }
        }

        SubsectionLabel { text: "End-to-end encryption" }
        e2ee_toggle := CheckBoxFlat {
            text: "Enable E2EE for this room"
            active: false
            draw_text +: {
                color: (COLOR_TEXT)
                color_hover: (COLOR_TEXT)
                color_focus: (COLOR_TEXT)
                color_down: (COLOR_TEXT)
            }
        }

        SubsectionLabel { text: "Invite people (matrix IDs, space- or comma-separated)" }
        invitees_input := RobrixTextInput {
            width: Fill,
            empty_text: "@alice:matrix.org, @bob:matrix.org"
        }

        validation_label := Label {
            width: Fill, height: Fit,
            draw_text +: { color: #xa10000 }
            text: ""
        }

        create_button := RobrixPositiveIconButton {
            width: Fit,
            text: "Create room"
        }
    }
}

#[derive(Script, Widget, ScriptHook)]
pub struct CreateRoomScreen {
    #[deref] view: View,
    #[rust] avatar_bytes: Option<Vec<u8>>,
    #[rust] avatar_mime: Option<String>,
}

impl Widget for CreateRoomScreen {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        self.widget_match_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl WidgetMatchEvent for CreateRoomScreen {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions, _scope: &mut Scope) {
        let visibility_set = self.view.radio_button_set(cx, ids_array!(
            visibility_public,
            visibility_private,
        ));
        if visibility_set.selected(cx, actions).is_some() {
            self.sync_visibility_card_highlight(cx);
            self.redraw(cx);
        }

        if self.view.button(cx, ids!(upload_image_button)).clicked(actions) {
            self.handle_avatar_upload(cx);
        }

        let create_button = self.view.button(cx, ids!(create_button));
        if create_button.clicked(actions) {
            let (config, failed_invitees) = self.collect_config(cx);
            if !failed_invitees.is_empty() {
                self.view.label(cx, ids!(validation_label)).set_text(
                    cx,
                    &format!(
                        "Invalid Matrix IDs (please fix or remove): {}",
                        failed_invitees.join(", "),
                    ),
                );
                return;
            }
            match validate_create_room_config(&config) {
                Ok(()) => {
                    self.view
                        .label(cx, ids!(validation_label))
                        .set_text(cx, "Creating room…");
                    submit_async_request(MatrixRequest::CreateRoomFromConfig { config });
                }
                Err(error) => {
                    self.view
                        .label(cx, ids!(validation_label))
                        .set_text(cx, &describe_error(&error));
                }
            }
        }

        for action in actions {
            match action.downcast_ref::<CreateRoomFromConfigAction>() {
                Some(CreateRoomFromConfigAction::Created { room_id: _, room_name }) => {
                    self.view
                        .label(cx, ids!(validation_label))
                        .set_text(cx, &format!("Room created: {room_name}"));
                    self.reset_inputs(cx);
                    enqueue_popup_notification(
                        format!("Room created: {room_name}"),
                        PopupKind::Success,
                        Some(4.0),
                    );
                    self.redraw(cx);
                }
                Some(CreateRoomFromConfigAction::PartialInvite { room_id: _, room_name, failed }) => {
                    let failed_list = failed
                        .iter()
                        .map(|u| u.as_str())
                        .collect::<Vec<_>>()
                        .join(", ");
                    let msg = format!(
                        "Room created ({room_name}), but invites failed for: {failed_list}"
                    );
                    self.view
                        .label(cx, ids!(validation_label))
                        .set_text(cx, &msg);
                    self.reset_inputs(cx);
                    enqueue_popup_notification(msg, PopupKind::Warning, None);
                    self.redraw(cx);
                }
                Some(CreateRoomFromConfigAction::Failed { reason }) => {
                    let msg = format!("Failed to create room: {reason}");
                    self.view
                        .label(cx, ids!(validation_label))
                        .set_text(cx, &msg);
                    enqueue_popup_notification(msg, PopupKind::Error, None);
                    self.redraw(cx);
                }
                None => {}
            }
        }
    }
}

impl CreateRoomScreen {
    fn collect_config(&mut self, cx: &mut Cx) -> (CreateRoomConfig, Vec<String>) {
        let name = self.view.text_input(cx, ids!(name_input)).text();
        let topic_raw = self.view.text_input(cx, ids!(topic_input)).text();
        let topic = (!topic_raw.trim().is_empty()).then(|| topic_raw.clone());
        let invitees_raw = self.view.text_input(cx, ids!(invitees_input)).text();
        let (parsed_invitees, failed_invitees) = parse_invitee_list(&invitees_raw);
        let public = self
            .view
            .radio_button(cx, ids!(visibility_public))
            .active(cx);
        let visibility = if public {
            RoomVisibilityChoice::Public
        } else {
            RoomVisibilityChoice::Private
        };
        let e2ee_enabled = self.view.check_box(cx, ids!(e2ee_toggle)).active(cx);
        let config = CreateRoomConfig {
            name,
            topic,
            avatar_bytes: self.avatar_bytes.clone(),
            avatar_mime: self.avatar_mime.clone(),
            visibility,
            e2ee_enabled,
            initial_invitees: parsed_invitees,
        };
        (config, failed_invitees)
    }

    fn reset_inputs(&mut self, cx: &mut Cx) {
        self.view.text_input(cx, ids!(name_input)).set_text(cx, "");
        self.view.text_input(cx, ids!(topic_input)).set_text(cx, "");
        self.view.text_input(cx, ids!(invitees_input)).set_text(cx, "");
        self.view.radio_button(cx, ids!(visibility_public)).set_active(cx, false, Animate::No);
        self.view.radio_button(cx, ids!(visibility_private)).set_active(cx, true, Animate::No);
        self.view.check_box(cx, ids!(e2ee_toggle)).set_active(cx, false, Animate::No);
        self.avatar_bytes = None;
        self.avatar_mime = None;
        self.view.image(cx, ids!(avatar_preview_image)).set_visible(cx, false);
        let caption = self.view.label(cx, ids!(avatar_preview_caption));
        caption.set_visible(cx, true);
        caption.set_text(cx, "No image");
        self.sync_visibility_card_highlight(cx);
    }

    fn handle_avatar_upload(&mut self, cx: &mut Cx) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Image", &["png", "jpg", "jpeg", "gif"])
            .pick_file()
        else {
            return;
        };

        match std::fs::metadata(&path) {
            Ok(meta) if meta.len() as usize > AVATAR_MAX_BYTES => {
                enqueue_popup_notification(
                    format!(
                        "Avatar is too large ({} bytes, max {} bytes).",
                        meta.len(),
                        AVATAR_MAX_BYTES,
                    ),
                    PopupKind::Error,
                    None,
                );
                return;
            }
            Err(e) => {
                enqueue_popup_notification(
                    format!("Could not stat selected file: {e}"),
                    PopupKind::Error,
                    None,
                );
                return;
            }
            _ => {}
        }

        let bytes = match std::fs::read(&path) {
            Ok(b) => b,
            Err(e) => {
                enqueue_popup_notification(
                    format!("Could not read selected file: {e}"),
                    PopupKind::Error,
                    None,
                );
                return;
            }
        };

        let mime = crate::image_utils::detect_mime_type(&bytes)
            .map(str::to_string)
            .or_else(|| {
                path.extension()
                    .and_then(|e| e.to_str())
                    .map(|e| match e.to_ascii_lowercase().as_str() {
                        "png" => "image/png".to_string(),
                        "jpg" | "jpeg" => "image/jpeg".to_string(),
                        "gif" => "image/gif".to_string(),
                        other => format!("image/{other}"),
                    })
            });

        let preview = self.view.image(cx, ids!(avatar_preview_image));
        let caption = self.view.label(cx, ids!(avatar_preview_caption));
        match crate::utils::load_png_or_jpg(&preview, cx, &bytes) {
            Ok(()) => {
                preview.set_visible(cx, true);
                caption.set_visible(cx, false);
            }
            Err(_) => {
                preview.set_visible(cx, false);
                caption.set_visible(cx, true);
                caption.set_text(cx, "Image ready");
            }
        }

        self.avatar_bytes = Some(bytes);
        self.avatar_mime = mime;
        self.redraw(cx);
    }

    fn sync_visibility_card_highlight(&mut self, cx: &mut Cx) {
        let private_active = self.view.radio_button(cx, ids!(visibility_private)).active(cx);
        let mut public_card = self.view.view(cx, ids!(visibility_public_card));
        let mut private_card = self.view.view(cx, ids!(visibility_private_card));
        if private_active {
            script_apply_eval!(cx, public_card, {
                draw_bg.border_color: mod.widgets.COLOR_SECONDARY_DARKER,
                draw_bg.border_size: 1.0,
            });
            script_apply_eval!(cx, private_card, {
                draw_bg.border_color: mod.widgets.COLOR_FG_ACCEPT_GREEN,
                draw_bg.border_size: 2.0,
            });
        } else {
            script_apply_eval!(cx, public_card, {
                draw_bg.border_color: mod.widgets.COLOR_FG_ACCEPT_GREEN,
                draw_bg.border_size: 2.0,
            });
            script_apply_eval!(cx, private_card, {
                draw_bg.border_color: mod.widgets.COLOR_SECONDARY_DARKER,
                draw_bg.border_size: 1.0,
            });
        }
    }
}

fn describe_error(error: &CreateRoomConfigError) -> String {
    match error {
        CreateRoomConfigError::EmptyName => "Please enter a room name.".to_string(),
        CreateRoomConfigError::NameTooLong { len, max } => {
            format!("Name is too long ({len} chars, max {max}).")
        }
        CreateRoomConfigError::AvatarTooLarge { bytes, max } => {
            format!("Avatar is too large ({bytes} bytes, max {max}).")
        }
        CreateRoomConfigError::EncryptedPublicRoom => {
            "Public rooms can't be end-to-end encrypted.".to_string()
        }
    }
}
