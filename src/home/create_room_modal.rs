//! A modal dialog for creating a new room.
//!
//! The user fills out a form with the room's details, settings, and people to invite,
//! and then reviews a summary of the new room before creating it.

use std::cmp::Reverse;

use makepad_widgets::*;
use matrix_sdk::{Client, ruma::{
    OwnedUserId, RoomAliasId, RoomVersionId, UInt, UserId, int,
    api::{
        client::{
            discovery::get_capabilities::v3::{RoomVersionStability, RoomVersionsCapability},
            room::{Visibility, create_room::{self, RoomPowerLevelsContentOverride, v3::{CreationContent, RoomPreset}}},
        },
        error::ErrorKind,
    },
    events::{InitialStateEvent, room::{
        avatar::{ImageInfo, RoomAvatarEventContent},
        encryption::RoomEncryptionEventContent,
        guest_access::{GuestAccess, RoomGuestAccessEventContent},
        history_visibility::{HistoryVisibility, RoomHistoryVisibilityEventContent},
        join_rules::{JoinRule, RoomJoinRulesEventContent},
    }},
    serde::Raw,
}};
use rustc_hash::FxHashSet;

use crate::{
    app::AppStateAction,
    directory_search::{DirectorySearchAction, SEARCH_PAGE_SIZE, Search, SearchQuery, UserSearchResult},
    home::room_creation_item::{RoomIntroCardWidgetExt, RoomSettings, format_room_version},
    profile::user_profile::UserProfile,
    room::BasicRoomDetails,
    shared::{
        avatar::{AvatarRef, AvatarState, AvatarWidgetExt, AvatarWidgetRefExt},
        collapsible_header::{CollapsibleHeaderAction, CollapsibleHeaderWidgetExt, HeaderCategory},
        file_upload_modal::{FilePreview, FileUploadMetadata, PreviewPayload, handle_picker_launch_errors, load_picked_file, validate_avatar_image},
        list_rows::{draw_flat_list_status_row, handle_row_actions},
        popup_list::{PopupKind, enqueue_popup_notification},
        styles::set_view_bg_color,
    },
    sliding_sync::{MatrixRequest, current_user_id, submit_async_request, upload_avatar_image, wait_for_create_event},
    utils::{self, RoomNameId},
};

/// The join rules that the user can choose from, each with its label and description.
const JOIN_RULE_OPTIONS: [(JoinRule, &str, &str); 3] = [
    (JoinRule::Invite, "Invite only", "Only people who are invited can join."),
    (JoinRule::Knock, "Ask to join", "Anyone can ask to join, and members can let them in."),
    (JoinRule::Public, "Anyone", "Anyone can join without being invited."),
];

/// The history visibilities that the user can choose from, each with its label and description.
const HISTORY_VISIBILITY_OPTIONS: [(HistoryVisibility, &str, &str); 4] = [
    (HistoryVisibility::Shared, "Members (all history)", "Members can read all of its messages, even ones from before they joined."),
    (HistoryVisibility::Invited, "Members (since invited)", "Members can only read messages from after they were invited."),
    (HistoryVisibility::Joined, "Members (since joining)", "Members can only read messages from after they joined."),
    (HistoryVisibility::WorldReadable, "Anyone", "Anyone can read its messages, even without joining."),
];

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*


    // A label above one of the form's fields.
    let FieldLabel = Label {
        width: Fill, height: Fit
        flow: Flow.Right{wrap: true}
        padding: 0
        margin: Inset{top: 18, bottom: 8}
        draw_text +: {
            text_style: mod.widgets.SETTINGS_BOLD_TEXT_STYLE {}
            color: (MESSAGE_TEXT_COLOR)
        }
    }

    // A short explanation beneath one of the form's fields.
    let FieldDescription = Label {
        width: Fill, height: Fit
        flow: Flow.Right{wrap: true}
        padding: 0
        margin: Inset{top: 6}
        draw_text +: {
            text_style: REGULAR_TEXT {font_size: 9.5}
            color: (COLOR_TEXT_MUTED)
        }
        text: ""
    }

    let FormTextInput = RobrixTextInput {
        draw_text +: {
            text_style: REGULAR_TEXT {font_size: 11}
            color: (COLOR_MODAL_INPUT_TEXT)
        }
    }

    let FormToggle = mod.widgets.RobrixSettingsToggle {
        margin: Inset{left: 0.5, top: 18, bottom: 0}
    }

    let FormDropDown = mod.widgets.RobrixSettingsDropDown {
        width: Fill {max: 320}
        margin: 0
        popup_min_width: 260
    }

    // The `#` and `:server` around the address input.
    let AddressPart = Label {
        width: Fit, height: Fit
        draw_text +: {
            text_style: REGULAR_TEXT {font_size: 12}
            color: (COLOR_TEXT_MUTED)
        }
    }

    mod.widgets.CreateRoomModal = set_type_default() do #(CreateRoomModal::register_widget(vm)) {
        ..mod.widgets.LargeModal

        width: Fill {max: 700}
        color: (COLOR_TRANSPARENT)
        color_hover: (COLOR_LIST_ITEM_BG_HOVER)

        title := ModalTitle { text: "Create a New Room" }

        error_label := ModalBody {
            visible: false
            margin: Inset{bottom: 15}
            draw_text +: { color: (COLOR_FG_DANGER_RED) }
        }

        // The form where the user chooses the new room's details and settings.
        form := View {
            width: Fill, height: Fit
            flow: Down

            View {
                width: Fill, height: Fit
                flow: Right, spacing: 15
                align: Align{y: 0.5}

                avatar := Avatar {
                    width: 64, height: 64
                    text_view +: {
                        text +: {
                            draw_text +: { text_style: TITLE_TEXT {font_size: 22} }
                        }
                    }
                }

                View {
                    width: Fill, height: Fit
                    flow: Flow.Right{wrap: true, row_align: RowAlign.Center}
                    spacing: 10, wrap_spacing: 10

                    choose_avatar_button := RobrixNeutralIconButton {
                        padding: Inset{top: 10, bottom: 10, left: 12, right: 15}
                        draw_icon.svg: (ICON_ADD_PHOTO)
                        icon_walk: Walk{width: 16, height: 16}
                        text: "Choose Avatar"
                    }
                    remove_avatar_button := RobrixNegativeIconButton {
                        visible: false
                        padding: Inset{top: 10, bottom: 10, left: 12, right: 15}
                        draw_icon.svg: (ICON_TRASH)
                        icon_walk: Walk{width: 16, height: 16}
                        text: "Remove"
                    }
                }
            }

            FieldLabel { text: "Name" }
            name_input := FormTextInput { empty_text: "Enter a name for the room..." }

            FieldLabel { text: "Topic (optional)" }
            topic_input := FormTextInput { empty_text: "What is this room about?" }

            FieldLabel { text: "Address (optional)" }
            View {
                width: Fill, height: Fit
                flow: Right, spacing: 5
                align: Align{y: 0.5}

                AddressPart { text: "#" }
                alias_input := FormTextInput {
                    empty_text: "my-room"
                    autocapitalize: None,
                    autocorrect: Disabled,
                }
                alias_server_label := AddressPart {}
            }
            FieldDescription { text: "People can use this address to find, join, and link to the room." }

            FieldLabel { text: "Who can join" }
            join_rule_dropdown := FormDropDown {}
            join_rule_description := FieldDescription {}

            room_directory_toggle := FormToggle {}
            FieldDescription { text: "People can find it by searching the room directory." }

            guest_access_toggle := FormToggle { text: "Allow guests to join" }
            FieldDescription { text: "Guests can join without a full account." }

            FieldLabel { text: "Who can read its history" }
            history_dropdown := FormDropDown {}
            history_description := FieldDescription {}

            encryption_toggle := FormToggle { text: "End-to-end encryption" }
            FieldDescription { text: "Only its members can read its messages. This can't be turned off later." }

            advanced_header := CollapsibleHeader {
                width: Fit, height: 30
                margin: Inset{top: 20}
                padding: Inset{left: 5, right: 12, top: 5, bottom: 5}
                starts_expanded: false
                draw_bg +: { color: (COLOR_BG_LAVENDER) }
                collapse_icon +: { draw_bg +: { color: (ROOM_NAME_TEXT_COLOR) } }
                label +: {
                    width: Fit
                    text: "Advanced options"
                    draw_text +: { color: (ROOM_NAME_TEXT_COLOR) }
                }
            }

            advanced_view := View {
                visible: false
                width: Fill, height: Fit
                flow: Down

                limited_posting_toggle := FormToggle { text: "Only moderators can post" }
                FieldDescription { text: "Everyone else can only read its messages." }

                local_only_toggle := FormToggle {}
                FieldDescription { text: "People on other servers can never join it. This can't be changed later." }

                FieldLabel { text: "Room version" }
                room_version_dropdown := FormDropDown { labels: ["Server default"] }
                FieldDescription { text: "Most rooms should use your server's default version." }
            }

            FieldLabel { text: "Invite people (optional)" }

            invitee_list := FlatList {
                width: Fill, height: Fit
                scroll_bars: ScrollBars { show_scroll_x: false, show_scroll_y: false }

                invitee_row := mod.widgets.AvatarListRow {
                    cursor: MouseCursor.Default
                    remove_invitee_button := RobrixNeutralIconButton {
                        spacing: 0
                        padding: 9
                        draw_icon.svg: (ICON_CLOSE)
                        icon_walk: Walk{width: 12, height: 12}
                        text: ""
                    }
                }
            }

            invite_input := FormTextInput {
                margin: Inset{top: 4}
                empty_text: "Search by name or user ID..."
                autocapitalize: None,
                autocorrect: Disabled,
            }

            invite_results := FlatList {
                width: Fill, height: Fit
                margin: Inset{top: 4}
                scroll_bars: ScrollBars { show_scroll_x: false, show_scroll_y: false }

                result_row := mod.widgets.AvatarListRow {}
                // The rows below are left-aligned with the results' avatars.
                loading_row := mod.widgets.ListLoadingRow {
                    align: Align{x: 0.0, y: 0.5}
                    padding: Inset{left: 10, right: 10, top: 13, bottom: 13}
                }
                empty_row := mod.widgets.ListEmptyRow {
                    padding: Inset{left: 10, right: 10, top: 13, bottom: 13}
                    empty_label +: { padding: theme.mspace_1{left: 0.}, align: Align{x: 0.0, y: 0.0} }
                }
            }
        }

        // The summary of the new room, which the user reviews before creating it.
        review := View {
            visible: false
            width: Fill, height: Fit
            flow: Down

            summary_card := mod.widgets.RoomIntroCard {
                margin: 0
                description +: { visible: false }
            }

            invitees_summary := ModalBody {
                visible: false
                margin: Inset{top: 15}
            }
        }

        creating_view := View {
            visible: false
            width: Fill, height: Fit
            flow: Right, spacing: 8
            align: Align{x: 1.0, y: 0.5}
            margin: Inset{top: 15}

            LoadingSpinner {
                width: 18, height: 18
                draw_bg +: { color: (COLOR_ACTIVE_PRIMARY), stroke_width: 2.5 }
            }
            Label {
                width: Fit, height: Fit
                draw_text +: { color: (COLOR_TEXT_BODY), text_style: REGULAR_TEXT {font_size: 11} }
                text: "Creating the room..."
            }
        }

        buttons_view := ModalButtonsRow {
            cancel_button := RobrixNeutralIconButton {
                width: Fit{min: FitBound.Abs(120.0)}
                align: Align{x: 0.5, y: 0.5}
                padding: 15,
                draw_icon.svg: (ICON_FORBIDDEN)
                icon_walk: Walk{width: 16, height: 16, margin: Inset{left: -2, right: -1} }
                text: "Cancel"
            }

            back_button := RobrixNeutralIconButton {
                visible: false
                width: Fit{min: FitBound.Abs(120.0)}
                align: Align{x: 0.5, y: 0.5}
                padding: 15,
                draw_icon.svg: (ICON_ARROW_LEFT)
                icon_walk: Walk{width: 16, height: 16, margin: Inset{left: -2, right: -1} }
                text: "Go Back"
            }

            review_button := RobrixIconButton {
                width: Fit{min: FitBound.Abs(120.0)}
                align: Align{x: 0.5, y: 0.5}
                padding: 15,
                draw_icon.svg: (ICON_ARROW_RIGHT)
                icon_walk: Walk{width: 16, height: 16, margin: Inset{left: -2, right: -1} }
                text: "Review"
            }

            create_button := RobrixPositiveIconButton {
                visible: false
                width: Fit{min: FitBound.Abs(120.0)}
                align: Align{x: 0.5, y: 0.5}
                padding: 15,
                draw_icon.svg: (ICON_CHECKMARK)
                icon_walk: Walk{width: 16, height: 16, margin: Inset{left: -2, right: -1} }
                text: "Create Room"
            }
        }
    }
}

/// Actions emitted by other widgets to show or hide the `CreateRoomModal`.
#[derive(Clone, Debug)]
pub enum CreateRoomModalAction {
    /// Open the modal with an empty form for a new room.
    Open,
    /// Close the modal.
    Close,
}

/// Actions posted to the `CreateRoomModal` from background tasks.
#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum CreateRoomAction {
    /// The user picked this image for the new room's avatar.
    AvatarPicked {
        file_data: FileUploadMetadata,
        preview: PreviewPayload,
    },
    /// The room versions that our homeserver supports.
    RoomVersions(RoomVersionsCapability),
    /// We created the new room and are now showing it.
    Created,
    /// We failed to create the new room, which we've already told the user about.
    Failed,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum CreateRoomModalState {
    /// The user is choosing the new room's details and settings.
    #[default]
    EditingForm,
    /// The user is reviewing the summary of the new room before creating it.
    Reviewing,
    /// We're waiting for our homeserver to create the room.
    CreatingRoom,
}

/// The details of a new room that the user chose in the `CreateRoomModal`.
#[derive(Clone)]
pub struct NewRoomDetails {
    name: String,
    topic: Option<String>,
    /// The part of the room's address (alias) between its `#` and `:`.
    alias_localpart: Option<String>,
    avatar: Option<FileUploadMetadata>,
    join_rule: JoinRule,
    is_in_room_directory: bool,
    guest_access: GuestAccess,
    history_visibility: HistoryVisibility,
    is_encrypted: bool,
    is_posting_limited: bool,
    is_federated: bool,
    /// If `None`, our homeserver chooses the room version.
    room_version: Option<RoomVersionId>,
    invitees: Vec<OwnedUserId>,
}

/// The people that we found in a search for people to invite.
#[derive(Default)]
struct InviteResults {
    /// The people that we found, in the order that we found them.
    users: Vec<UserProfile>,
    has_exact_matches: bool,
    has_directory_matches: bool,
    error: Option<String>,
}

#[derive(Script, ScriptHook, Widget)]
pub struct CreateRoomModal {
    #[deref] view: View,

    #[live] color: Vec4f,
    #[live] color_hover: Vec4f,

    #[rust] state: CreateRoomModalState,
    /// The details that the user is reviewing, which we create the room with.
    #[rust] reviewed_room: Option<NewRoomDetails>,
    #[rust] avatar_file: Option<FileUploadMetadata>,
    /// The picked avatar image, if we could decode it, which we show in both the form and the summary.
    #[rust] avatar_texture: Option<Texture>,
    /// The room versions that the user can choose instead of our homeserver's default version.
    #[rust] room_versions: Vec<RoomVersionId>,
    #[rust] default_room_version: Option<RoomVersionId>,
    #[rust] invite_search: Search<InviteResults>,
    #[rust] invitees: Vec<UserProfile>,
    #[rust] hovered_result: Option<LiveId>,
    /// The keys of the rows whose content is up to date, which needn't be set again when drawn.
    #[rust] populated_invitee_rows: FxHashSet<LiveId>,
    #[rust] populated_result_rows: FxHashSet<LiveId>,
}

impl Widget for CreateRoomModal {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        self.widget_match_event(cx, event, scope);
        if self.invite_search.timer.is_event(event).is_some() {
            let query = self.view.text_input(cx, ids!(invite_input)).text();
            self.search_people(cx, query.trim());
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        let invitee_list_uid = self.view.child_by_path(ids!(invitee_list)).widget_uid();
        while let Some(step) = self.view.draw_walk(cx, scope, walk).step() {
            // A borrowed widget's uid reads as 0, so we get it before borrowing the list.
            let step_uid = step.widget_uid();
            let list_ref = step.as_flat_list();
            let Some(mut list) = list_ref.borrow_mut() else { continue };
            if step_uid == invitee_list_uid {
                for invitee in &self.invitees {
                    let key = LiveId::from_str(invitee.user_id.as_str());
                    draw_user_row(cx, scope, &mut list, key, id!(invitee_row), invitee, &mut self.populated_invitee_rows, None);
                }
            } else {
                match &self.invite_search.results {
                    Some(results) => {
                        let users = results.users.iter()
                            .filter(|user| !self.invitees.iter().any(|invitee| invitee.user_id == user.user_id));
                        let mut num_users = 0;
                        for user in users {
                            num_users += 1;
                            let key = LiveId::from_str(user.user_id.as_str());
                            let color = if self.hovered_result == Some(key) { self.color_hover } else { self.color };
                            draw_user_row(cx, scope, &mut list, key, id!(result_row), user, &mut self.populated_result_rows, Some(color));
                        }
                        if !results.has_exact_matches || !results.has_directory_matches {
                            draw_flat_list_status_row(cx, scope, &mut list, id!(loading), true, "Searching for people...");
                        } else if let Some(error) = &results.error {
                            draw_flat_list_status_row(cx, scope, &mut list, id!(error), false, error);
                        } else if num_users == 0 {
                            draw_flat_list_status_row(cx, scope, &mut list, id!(no_results), false, "No people found.");
                        }
                    }
                    None if !self.invite_search.query.is_empty() => {
                        draw_flat_list_status_row(cx, scope, &mut list, id!(hint), false, "Search for a person's name or user ID.");
                    }
                    None => {}
                }
            }
            // Drop the rows that we didn't draw this time.
            list.items.retain_visible();
        }
        DrawStep::done()
    }
}

impl WidgetMatchEvent for CreateRoomModal {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions, _scope: &mut Scope) {
        // The form starts over the next time it opens, so we let go of the avatar image as soon as it closes.
        if actions.iter().any(|action| matches!(action.downcast_ref(), Some(ModalAction::Dismissed))) {
            self.remove_avatar(cx);
            return;
        }

        for action in actions {
            match action.downcast_ref() {
                Some(CreateRoomAction::AvatarPicked { file_data, preview }) => {
                    self.avatar_file = Some(file_data.clone());
                    self.avatar_texture = match preview.take() {
                        Some(FilePreview::Image(image_buffer)) => Some(image_buffer.into_new_texture(cx)),
                        _ => None,
                    };
                    self.show_form_avatar(cx);
                }
                Some(CreateRoomAction::RoomVersions(room_versions)) => self.show_room_versions(cx, room_versions),
                Some(CreateRoomAction::Created) if self.state == CreateRoomModalState::CreatingRoom => {
                    cx.action(CreateRoomModalAction::Close);
                }
                Some(CreateRoomAction::Failed) if self.state == CreateRoomModalState::CreatingRoom => {
                    self.state = CreateRoomModalState::Reviewing;
                    self.show_state(cx);
                }
                _ => {}
            }

            match action.downcast_ref() {
                Some(DirectorySearchAction::ExactUsers { query, users }) if *query == self.invite_search.query => {
                    self.add_invite_results(users);
                    if let Some(results) = self.invite_search.results.as_mut() {
                        results.has_exact_matches = true;
                    }
                    self.redraw(cx);
                }
                Some(DirectorySearchAction::DirectoryUsers { query, users, error, .. }) if *query == self.invite_search.query => {
                    self.add_invite_results(users);
                    if let Some(results) = self.invite_search.results.as_mut() {
                        results.has_directory_matches = true;
                        results.error = error.clone();
                    }
                    self.redraw(cx);
                }
                _ => {}
            }
        }

        if self.view.button(cx, ids!(cancel_button)).clicked(actions) {
            cx.action(CreateRoomModalAction::Close);
            return;
        }

        if self.view.button(cx, ids!(choose_avatar_button)).clicked(actions) {
            handle_picker_launch_errors(
                robius_file_picker::FileDialog::new()
                    .add_filter("Images", utils::AVATAR_IMAGE_EXTENSIONS)
                    .set_media_representation(robius_file_picker::MediaRepresentation::Compatible)
                    .pick_image(|result| {
                        let Some((file_data, preview_source)) = load_picked_file(result, validate_avatar_image) else { return };
                        // We decode the image off of the main thread, so the form can show it right away.
                        let preview = PreviewPayload::new(preview_source.build());
                        Cx::post_action(CreateRoomAction::AvatarPicked { file_data, preview });
                    })
            );
        }
        if self.view.button(cx, ids!(remove_avatar_button)).clicked(actions) {
            self.remove_avatar(cx);
        }
        if self.view.text_input(cx, ids!(name_input)).changed(actions).is_some() {
            self.show_form_avatar(cx);
        }

        let advanced_header = self.view.collapsible_header(cx, ids!(advanced_header));
        if let Some(action) = actions.find_widget_action(advanced_header.widget_uid())
            && let CollapsibleHeaderAction::Toggled { .. } = action.cast()
        {
            self.view.view(cx, ids!(advanced_view)).set_visible(cx, advanced_header.is_expanded());
            self.redraw(cx);
        }
        if let Some(index) = self.view.drop_down2(cx, ids!(join_rule_dropdown)).changed(actions)
            && let Some((_, _, description)) = JOIN_RULE_OPTIONS.get(index)
        {
            self.view.label(cx, ids!(join_rule_description)).set_text(cx, description);
        }
        if let Some(index) = self.view.drop_down2(cx, ids!(history_dropdown)).changed(actions)
            && let Some((_, _, description)) = HISTORY_VISIBILITY_OPTIONS.get(index)
        {
            self.view.label(cx, ids!(history_description)).set_text(cx, description);
        }

        let invite_input = self.view.text_input(cx, ids!(invite_input));
        if let Some(text) = invite_input.changed(actions) {
            self.invite_search.handle_text_changed(cx, &text);
            self.redraw(cx);
        }
        // Pressing Enter after a full user ID invites that person, even if we couldn't find their profile.
        if let Some((text, _)) = invite_input.returned(actions)
            && let Ok(user_id) = UserId::parse(text.trim())
        {
            let user_profile = self.invite_search.results.iter()
                .flat_map(|results| &results.users)
                .find(|user| user.user_id == user_id)
                .cloned()
                .unwrap_or(UserProfile { user_id, username: None, avatar_state: AvatarState::Unknown });
            self.add_invitee(cx, user_profile);
            cx.stop_timer(self.invite_search.timer);
            self.invite_search = Search::default();
            invite_input.set_text(cx, "");
        }

        let result_items = self.view.flat_list(cx, ids!(invite_results)).items_with_actions(actions);
        let (tapped_result, is_hover_changed) = handle_row_actions(result_items, false, actions, &mut self.hovered_result);
        if let Some(key) = tapped_result
            && let Some(user_profile) = self.invite_search.results.iter()
                .flat_map(|results| &results.users)
                .find(|user| LiveId::from_str(user.user_id.as_str()) == key)
                .cloned()
        {
            self.add_invitee(cx, user_profile);
        }
        if is_hover_changed {
            self.redraw(cx);
        }
        for (key, item) in self.view.flat_list(cx, ids!(invitee_list)).items_with_actions(actions) {
            if item.button(cx, ids!(remove_invitee_button)).clicked(actions) {
                self.invitees.retain(|invitee| LiveId::from_str(invitee.user_id.as_str()) != key);
                self.redraw(cx);
            }
        }

        // Pressing Enter in one of the inputs above the invite search box is like clicking Review.
        let submitted_input = [ids!(name_input), ids!(topic_input), ids!(alias_input)].into_iter()
            .any(|input| self.view.text_input(cx, input).returned(actions).is_some());
        if self.state == CreateRoomModalState::EditingForm
            && (self.view.button(cx, ids!(review_button)).clicked(actions) || submitted_input)
        {
            self.show_review(cx);
        }
        if self.view.button(cx, ids!(back_button)).clicked(actions) {
            self.reviewed_room = None;
            self.state = CreateRoomModalState::EditingForm;
            self.show_state(cx);
        }
        if self.state == CreateRoomModalState::Reviewing
            && self.view.button(cx, ids!(create_button)).clicked(actions)
            && let Some(details) = self.reviewed_room.clone()
        {
            submit_async_request(MatrixRequest::CreateRoom { details });
            self.state = CreateRoomModalState::CreatingRoom;
            self.show_state(cx);
        }
    }
}

impl CreateRoomModal {
    /// Resets this modal to an empty form for a new room.
    fn show(&mut self, cx: &mut Cx) {
        self.state = CreateRoomModalState::EditingForm;
        self.reviewed_room = None;
        self.invitees.clear();
        cx.stop_timer(self.invite_search.timer);
        self.invite_search = Search::default();
        self.hovered_result = None;
        self.populated_invitee_rows.clear();
        self.populated_result_rows.clear();

        for input in [ids!(name_input), ids!(topic_input), ids!(alias_input), ids!(invite_input)] {
            self.view.text_input(cx, input).set_text(cx, "");
        }
        let join_rule_dropdown = self.view.drop_down2(cx, ids!(join_rule_dropdown));
        join_rule_dropdown.set_labels(cx, JOIN_RULE_OPTIONS.iter().map(|(_, label, _)| label.to_string()).collect());
        join_rule_dropdown.set_selected_item(cx, 0);
        self.view.label(cx, ids!(join_rule_description)).set_text(cx, JOIN_RULE_OPTIONS[0].2);
        let history_dropdown = self.view.drop_down2(cx, ids!(history_dropdown));
        history_dropdown.set_labels(cx, HISTORY_VISIBILITY_OPTIONS.iter().map(|(_, label, _)| label.to_string()).collect());
        history_dropdown.set_selected_item(cx, 0);
        self.view.label(cx, ids!(history_description)).set_text(cx, HISTORY_VISIBILITY_OPTIONS[0].2);
        self.view.drop_down2(cx, ids!(room_version_dropdown)).set_selected_item(cx, 0);
        for toggle in [
            ids!(room_directory_toggle),
            ids!(guest_access_toggle),
            ids!(encryption_toggle),
            ids!(limited_posting_toggle),
            ids!(local_only_toggle),
        ] {
            self.view.check_box(cx, toggle).set_active(cx, false, Animate::No);
        }
        let own_server = current_user_id().map(|user_id| user_id.server_name().to_string()).unwrap_or_default();
        self.view.label(cx, ids!(alias_server_label)).set_text(cx, &format!(":{own_server}"));
        self.view.check_box(cx, ids!(room_directory_toggle)).set_text(&format!("List in {own_server}'s room directory"));
        self.view.check_box(cx, ids!(local_only_toggle)).set_text(&format!("Only allow {own_server} users"));
        self.view.collapsible_header(cx, ids!(advanced_header)).set_details(cx, false, HeaderCategory::None, 0, 0);
        self.view.view(cx, ids!(advanced_view)).set_visible(cx, false);

        self.remove_avatar(cx);
        self.show_state(cx);
        submit_async_request(MatrixRequest::GetRoomVersions);
        // On a touchscreen, focusing the name input would pop up a keyboard that covers most of the form.
        if !cfg!(any(target_os = "android", target_os = "ios")) || cx.keyboard.has_physical_keyboard() {
            self.view.text_input(cx, ids!(name_input)).set_key_focus(cx);
        }
    }

    /// Shows the form or the new room's summary, and the buttons for our current state.
    fn show_state(&mut self, cx: &mut Cx) {
        let is_editing = self.state == CreateRoomModalState::EditingForm;
        let is_creating = self.state == CreateRoomModalState::CreatingRoom;
        // Hidden text inputs still get key presses, so we take key focus away from the form's inputs.
        if !is_editing {
            cx.set_key_focus(Area::Empty);
        }
        self.view.label(cx, ids!(title)).set_text(cx, if is_editing { "Create a New Room" } else { "Review Your New Room" });
        self.view.view(cx, ids!(form)).set_visible(cx, is_editing);
        self.view.view(cx, ids!(review)).set_visible(cx, !is_editing);
        self.view.view(cx, ids!(creating_view)).set_visible(cx, is_creating);
        self.view.label(cx, ids!(error_label)).set_visible(cx, false);
        for (button, is_shown) in [
            (ids!(cancel_button), is_editing),
            (ids!(review_button), is_editing),
            (ids!(back_button), !is_editing),
            (ids!(create_button), !is_editing),
        ] {
            let button = self.view.button(cx, button);
            button.set_visible(cx, is_shown);
            button.set_enabled(cx, !is_creating);
            button.reset_hover(cx);
        }
        self.redraw(cx);
    }

    /// Shows the given error above the form, which the user must fix before going on.
    fn show_error(&mut self, cx: &mut Cx, error: &str) {
        let error_label = self.view.label(cx, ids!(error_label));
        error_label.set_text(cx, error);
        error_label.set_visible(cx, true);
        self.view.set_scroll_pos(cx, Vec2d::default());
        self.redraw(cx);
    }

    /// Shows the picked avatar image in the form, or the first letter of the room's name if there isn't one.
    fn show_form_avatar(&mut self, cx: &mut Cx) {
        let name = self.view.text_input(cx, ids!(name_input)).text();
        let avatar = self.view.avatar(cx, ids!(form.avatar));
        self.show_avatar(cx, &avatar, name.trim());
        self.view.button(cx, ids!(remove_avatar_button)).set_visible(cx, self.avatar_file.is_some());
        self.redraw(cx);
    }

    /// Forgets the picked avatar image, so the form and the summary show the first letter of the room's name instead.
    fn remove_avatar(&mut self, cx: &mut Cx) {
        self.avatar_file = None;
        self.avatar_texture = None;
        self.show_form_avatar(cx);
        let summary_avatar = self.view.avatar(cx, ids!(summary_card.room_avatar));
        self.show_avatar(cx, &summary_avatar, "");
    }

    /// Shows the picked avatar image in the given avatar, or else the first letter of the room's name.
    fn show_avatar(&self, cx: &mut Cx, avatar: &AvatarRef, name: &str) {
        match &self.avatar_texture {
            Some(texture) => {
                let _ = avatar.show_image(cx, None, |cx, img| {
                    img.set_texture(cx, Some(texture.clone()));
                    Ok::<_, ()>(())
                });
            }
            None => {
                avatar.show_text(cx, None, None, name);
                // The hidden image would otherwise keep the whole decoded picture in memory.
                avatar.image(cx, ids!(img_view.img)).set_texture(cx, None);
            }
        }
    }

    /// Shows the room versions that our homeserver supports in the dropdown, keeping the user's choice.
    fn show_room_versions(&mut self, cx: &mut Cx, room_versions: &RoomVersionsCapability) {
        let dropdown = self.view.drop_down2(cx, ids!(room_version_dropdown));
        let chosen_version = dropdown.selected_item().checked_sub(1).and_then(|index| self.room_versions.get(index).cloned());
        let mut versions: Vec<(&RoomVersionId, bool)> = room_versions.available.iter()
            .map(|(version, stability)| (version, matches!(stability, RoomVersionStability::Stable)))
            .collect();
        // The newest stable versions go first, then the unstable ones.
        versions.sort_by_key(|(version, is_stable)| (!is_stable, Reverse(version.as_str().parse::<u32>().ok())));
        let labels = std::iter::once(format!("Server default ({})", format_room_version(&room_versions.default)))
            .chain(versions.iter().map(|(version, is_stable)| match is_stable {
                true => format_room_version(version),
                false => format!("{} (unstable)", format_room_version(version)),
            }))
            .collect();
        self.room_versions = versions.into_iter().map(|(version, _)| version.clone()).collect();
        self.default_room_version = Some(room_versions.default.clone());
        dropdown.set_labels(cx, labels);
        let index = chosen_version
            .and_then(|chosen| self.room_versions.iter().position(|version| *version == chosen))
            .map_or(0, |index| index + 1);
        dropdown.set_selected_item(cx, index);
    }

    /// Starts searching for people to invite who match the given query.
    fn search_people(&mut self, cx: &mut Cx, query: &str) {
        cx.stop_timer(self.invite_search.timer);
        let can_match_users = SearchQuery::parse(query).is_some_and(|search_query| search_query.can_match_users());
        if can_match_users {
            submit_async_request(MatrixRequest::ResolveUserSearchQuery { query: query.to_owned() });
            submit_async_request(MatrixRequest::SearchUserDirectory { query: query.to_owned(), limit: SEARCH_PAGE_SIZE });
        }
        self.invite_search = Search {
            query: query.to_owned(),
            results: can_match_users.then(InviteResults::default),
            ..Default::default()
        };
        self.hovered_result = None;
        self.redraw(cx);
    }

    /// Adds the given people that we found to the invite search results, except for ourselves and people we already have.
    ///
    /// An exact match replaces the person we already have in place, since it has their full profile.
    fn add_invite_results(&mut self, users: &[UserSearchResult]) {
        let Some(results) = self.invite_search.results.as_mut() else { return };
        let own_user_id = current_user_id();
        for user in users {
            let user_profile = &user.user_profile;
            if own_user_id.as_ref() == Some(&user_profile.user_id) {
                continue;
            }
            match results.users.iter_mut().find(|existing| existing.user_id == user_profile.user_id) {
                Some(existing) if user.is_exact_match => {
                    *existing = user_profile.clone();
                    self.populated_result_rows.remove(&LiveId::from_str(user_profile.user_id.as_str()));
                }
                Some(_) => {}
                None => results.users.push(user_profile.clone()),
            }
        }
    }

    /// Adds the given person to the people that we'll invite, if they aren't already.
    fn add_invitee(&mut self, cx: &mut Cx, user_profile: UserProfile) {
        if !self.invitees.iter().any(|invitee| invitee.user_id == user_profile.user_id) {
            self.invitees.push(user_profile);
        }
        self.hovered_result = None;
        self.redraw(cx);
    }

    /// Checks the form, and if it's valid, shows the summary of the new room for the user to review.
    fn show_review(&mut self, cx: &mut Cx) {
        let Some(own_server) = current_user_id().map(|user_id| user_id.server_name().to_owned()) else { return };
        let name = self.view.text_input(cx, ids!(name_input)).text().trim().to_owned();
        if name.is_empty() {
            self.show_error(cx, "Please enter a name for the room.");
            self.view.text_input(cx, ids!(name_input)).set_key_focus(cx);
            return;
        }
        let topic = self.view.text_input(cx, ids!(topic_input)).text().trim().to_owned();
        // The user may have entered the whole address, not just the part between its `#` and `:`.
        let alias_text = self.view.text_input(cx, ids!(alias_input)).text();
        let alias_localpart = alias_text.trim().trim_start_matches('#');
        let alias_localpart = alias_localpart.strip_suffix(&format!(":{own_server}")).unwrap_or(alias_localpart);
        let alias = (!alias_localpart.is_empty()).then(|| format!("#{alias_localpart}:{own_server}"));
        if let Some(alias) = &alias
            && (alias_localpart.contains(|c: char| c == ':' || c.is_whitespace()) || RoomAliasId::parse(alias).is_err())
        {
            self.show_error(cx, "That address isn't valid. It can't have any spaces or colons.");
            self.view.text_input(cx, ids!(alias_input)).set_key_focus(cx);
            return;
        }

        let join_rule = JOIN_RULE_OPTIONS.get(self.view.drop_down2(cx, ids!(join_rule_dropdown)).selected_item())
            .map_or(JoinRule::Invite, |(join_rule, ..)| join_rule.clone());
        let history_visibility = HISTORY_VISIBILITY_OPTIONS.get(self.view.drop_down2(cx, ids!(history_dropdown)).selected_item())
            .map_or(HistoryVisibility::Shared, |(history_visibility, ..)| history_visibility.clone());
        let room_version = self.view.drop_down2(cx, ids!(room_version_dropdown)).selected_item()
            .checked_sub(1)
            .and_then(|index| self.room_versions.get(index).cloned());
        let shown_room_version = room_version.clone().or_else(|| self.default_room_version.clone());
        if matches!(join_rule, JoinRule::Knock)
            && let Some(version) = &shown_room_version
            && version.rules().is_some_and(|rules| !rules.authorization.knocking)
        {
            self.show_error(cx, &format!("Room version {version} doesn't let people ask to join. Please choose a newer version."));
            return;
        }

        let details = NewRoomDetails {
            name,
            topic: (!topic.is_empty()).then_some(topic),
            alias_localpart: (!alias_localpart.is_empty()).then(|| alias_localpart.to_owned()),
            avatar: self.avatar_file.clone(),
            join_rule,
            is_in_room_directory: self.view.check_box(cx, ids!(room_directory_toggle)).active(cx),
            guest_access: match self.view.check_box(cx, ids!(guest_access_toggle)).active(cx) {
                true => GuestAccess::CanJoin,
                false => GuestAccess::Forbidden,
            },
            history_visibility,
            is_encrypted: self.view.check_box(cx, ids!(encryption_toggle)).active(cx),
            is_posting_limited: self.view.check_box(cx, ids!(limited_posting_toggle)).active(cx),
            is_federated: !self.view.check_box(cx, ids!(local_only_toggle)).active(cx),
            room_version,
            invitees: self.invitees.iter().map(|invitee| invitee.user_id.clone()).collect(),
        };

        let summary_card = self.view.room_intro_card(cx, ids!(summary_card));
        let summary_avatar = summary_card.avatar(cx, ids!(room_avatar));
        self.show_avatar(cx, &summary_avatar, &details.name);
        summary_card.label(cx, ids!(room_name)).set_text(cx, &details.name);
        summary_card.show_details(cx, alias.as_deref(), details.topic.as_deref(), &RoomSettings {
            is_encrypted: details.is_encrypted,
            join_rule: Some(details.join_rule.clone()),
            history_visibility: Some(details.history_visibility.clone()),
            guest_access: Some(details.guest_access.clone()),
            is_posting_limited: details.is_posting_limited,
            is_in_room_directory: details.is_in_room_directory,
            local_only_server: (!details.is_federated).then_some(own_server),
            room_version: shown_room_version,
        });
        let invitees_summary = self.view.label(cx, ids!(invitees_summary));
        invitees_summary.set_visible(cx, !self.invitees.is_empty());
        let invitee_names: Vec<&str> = self.invitees.iter().map(UserProfile::displayable_name).collect();
        invitees_summary.set_text(cx, &format!("You'll invite: {}", invitee_names.join(", ")));

        self.reviewed_room = Some(details);
        self.state = CreateRoomModalState::Reviewing;
        self.show_state(cx);
        // The summary replaces the form, which the user may have scrolled down.
        self.view.set_scroll_pos(cx, Vec2d::default());
    }
}

impl CreateRoomModalRef {
    /// See [`CreateRoomModal::show()`].
    pub fn show(&self, cx: &mut Cx) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.show(cx);
    }
}

/// Draws the row with the given key that shows the given person, setting its content if it isn't up to date yet.
#[allow(clippy::too_many_arguments)]
fn draw_user_row(
    cx: &mut Cx2d,
    scope: &mut Scope,
    list: &mut FlatList,
    key: LiveId,
    template: LiveId,
    user_profile: &UserProfile,
    populated_rows: &mut FxHashSet<LiveId>,
    bg_color: Option<Vec4f>,
) {
    // The list drops rows that it didn't draw last time, so a row that it creates again starts out empty.
    if !list.items.contains_key(&key) {
        populated_rows.remove(&key);
    }
    let Some(row) = list.item(cx, key, template) else { return };
    if !populated_rows.contains(&key) {
        let name = user_profile.displayable_name();
        row.label(cx, ids!(info.title)).set_text(cx, name);
        row.label(cx, ids!(info.subtitle)).set_text(cx, user_profile.user_id.as_str());
        if row.avatar(cx, ids!(avatar)).show_user(cx, user_profile.avatar_state.uri(), name) {
            populated_rows.insert(key);
        }
    }
    if let Some(bg_color) = bg_color {
        set_view_bg_color(cx, &row.as_view(), bg_color);
    }
    row.draw_all(cx, scope);
}

/// Creates the given new room and shows it, and then invites the people that the user chose.
///
/// This posts a [`CreateRoomAction::Created`] once the room is ready, or a [`CreateRoomAction::Failed`].
pub async fn create_room(client: Client, details: NewRoomDetails) {
    let NewRoomDetails {
        name,
        topic,
        alias_localpart,
        avatar,
        join_rule,
        is_in_room_directory,
        guest_access,
        history_visibility,
        is_encrypted,
        is_posting_limited,
        is_federated,
        room_version,
        invitees,
    } = details;
    let is_public = matches!(join_rule, JoinRule::Public);
    let mut initial_state = vec![
        InitialStateEvent::with_empty_state_key(RoomJoinRulesEventContent::new(join_rule)).to_raw_any(),
        InitialStateEvent::with_empty_state_key(RoomGuestAccessEventContent::new(guest_access)).to_raw_any(),
        InitialStateEvent::with_empty_state_key(RoomHistoryVisibilityEventContent::new(history_visibility)).to_raw_any(),
    ];
    if is_encrypted {
        initial_state.push(InitialStateEvent::with_empty_state_key(RoomEncryptionEventContent::with_recommended_defaults()).to_raw_any());
    }
    if let Some(file_data) = avatar {
        match upload_avatar_image(&client, &file_data).await {
            Ok(avatar_url) => {
                let mut info = ImageInfo::new();
                info.mimetype = Some(file_data.mime_type.clone());
                info.size = UInt::new(file_data.size);
                let mut content = RoomAvatarEventContent::new();
                content.url = Some(avatar_url);
                content.info = Some(Box::new(info));
                initial_state.push(InitialStateEvent::with_empty_state_key(content).to_raw_any());
            }
            Err(error) => {
                enqueue_popup_notification(format!("Couldn't create the room.\n\n{error}"), PopupKind::Error, None);
                Cx::post_action(CreateRoomAction::Failed);
                return;
            }
        }
    }

    let mut request = create_room::v3::Request::new();
    request.name = Some(name.clone());
    request.topic = topic;
    request.room_alias_name = alias_localpart.clone();
    request.room_version = room_version;
    request.initial_state = initial_state;
    // Our initial state overrides the preset's, but a homeserver may also use the preset to choose other defaults.
    request.preset = Some(if is_public { RoomPreset::PublicChat } else { RoomPreset::PrivateChat });
    if is_in_room_directory {
        request.visibility = Visibility::Public;
    }
    if !is_federated {
        let mut creation_content = CreationContent::new();
        creation_content.federate = false;
        request.creation_content = Raw::new(&creation_content).ok();
    }
    if is_posting_limited {
        let mut power_levels = RoomPowerLevelsContentOverride::new();
        // This is the power level of moderators, so only they and admins can send messages.
        power_levels.events_default = Some(int!(50));
        request.power_level_content_override = Raw::new(&power_levels).ok();
    }

    log!("Creating new room \"{name}\"...");
    let room = match client.create_room(request).await {
        Ok(room) => room,
        Err(e) => {
            error!("Failed to create new room \"{name}\": {e:?}");
            let message = match (e.client_api_error_kind(), &alias_localpart, client.user_id()) {
                (Some(ErrorKind::RoomInUse), Some(localpart), Some(user_id)) => format!(
                    "The address #{localpart}:{} is already taken. Please go back and choose another one.",
                    user_id.server_name(),
                ),
                _ => format!("Couldn't create the room.\n\nError: {e}"),
            };
            enqueue_popup_notification(message, PopupKind::Error, None);
            Cx::post_action(CreateRoomAction::Failed);
            return;
        }
    };
    log!("Successfully created new room \"{name}\" ({}).", room.room_id());
    // The rooms list only gets a new room once its state arrives, so we wait for that too.
    wait_for_create_event(&room).await;
    Cx::post_action(CreateRoomAction::Created);
    Cx::post_action(AppStateAction::NavigateToRoom {
        room_to_close: None,
        destination_room: BasicRoomDetails::RoomId(RoomNameId::from_room(&room).await),
    });

    let mut failed_invitees = Vec::new();
    for user_id in invitees {
        if let Err(e) = room.invite_user_by_id(&user_id).await {
            error!("Failed to invite {user_id} to new room {}: {e:?}", room.room_id());
            failed_invitees.push(user_id.to_string());
        }
    }
    if !failed_invitees.is_empty() {
        enqueue_popup_notification(
            format!("Created \"{name}\", but couldn't invite {}.", failed_invitees.join(", ")),
            PopupKind::Warning,
            None,
        );
    }
}
