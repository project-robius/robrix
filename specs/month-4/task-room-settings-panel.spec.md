spec: task
name: "Room Settings Panel"
inherits: project
tags: [month-4, ui, room-management, settings]
---

## Intent

Implement a Room Settings modal panel that allows room administrators to view and
modify configuration for a Matrix room. The panel is opened from the room header and
presents settings in a two-column layout: a fixed left sidebar with section navigation
and a scrollable right content area. This covers the four visible sections from the
reference screenshots: General, Room Addresses (Published + Local), Other
(Moderation and safety), and Leave Room.

## Decisions

- Modal container: `RoomSettingsModal` widget, rendered as a floating panel with a
  close (×) button in the top-right corner. Title bar text is
  `"Room Settings - {room_name}"`.
- Layout: two-column `View { flow: Right }`. Left column is a fixed-width sidebar
  (`width: 130`); right column is a `ScrollView` containing all section content.
- Sidebar navigation items (top to bottom): General, Voice & Video,
  Security & Privacy, Roles & Permissions, Notifications, Polls, Advanced.
  Active item is highlighted with a filled background. Only **General** is
  implemented in this task; the remaining six items render as inactive labels.
- **General section** — four widgets stacked vertically:
  - `room_name_input`: single-line `TextInput`, pre-filled with current room name,
    label `"Room Name"` floated above.
  - `room_topic_input`: multi-line `TextInput` (min height 72 px), label
    `"Room Topic"` floated above.
  - Action row: `cancel_button` (label `"Cancel"`) and `save_button`
    (label `"Save"`, primary style). Buttons are right-aligned.
  - Room avatar: right-aligned `View` showing a circular avatar (initials fallback
    `"R"` for no avatar set). A pencil icon overlay in the bottom-right corner of
    the avatar opens avatar-change flow (out of scope; overlay is present but
    non-functional for this task).
- **Room Addresses section** — two subsections:
  - *Published Addresses*: explanatory label; `main_address_dropdown` showing the
    canonical alias (e.g. `#robix_test3:matrix.org`); `publish_directory_toggle`
    (`Toggle` widget, label `"Publish this room to the public in {homeserver}'s
    room directory?"`); empty-state label `"No other published addresses yet, add
    one below"`; `room_address_input` (placeholder `# e.g. my-room`); `add_address_button`
    (label `"Add"`).
  - *Local Addresses*: explanatory label; `show_more_button` (label `"Show more"`,
    link style).
- **Other section** — one subsection *Moderation and safety*:
  - `show_media_radio_group`: two `RadioButton` items — `"Always hide"` and
    `"Always show"`. Default selection: `"Always show"`.
  - Sub-label: `"A hidden media can always be shown by tapping on it"`.
- **Leave Room section**: `leave_room_button` (label `"Leave room"`, destructive /
  red background). Pressing the button shows a confirmation dialog (separate widget,
  out of scope for this task — button is rendered but non-functional in M4).
- Color tokens: primary action `#4A90D9`, destructive action `#E53935`,
  sidebar active background `#E8EEF5`, sidebar text `#1A1A1A`, section
  label `#888888`.
- Scroll: right content area uses `ScrollView { flow: Down }`. The sidebar does
  not scroll (all seven items fit in the viewport).
- Save action: on `save_button` click, emit `RoomSettingsAction::Save { room_name,
  room_topic }` via `Cx::post_action()`. Actual Matrix state-event write is handled
  in `sliding_sync.rs` (async); the modal stays open until the action completes
  (not implemented in M4 — save button posts the action only).
- Cancel action: on `cancel_button` click, emit
  `RoomSettingsAction::Cancel`; the modal closes without saving.
- Modal open trigger: `RoomSettingsAction::Open { room_id }` posted from the
  room header widget closes any previously open modal and opens a fresh one for
  the given room.

## Boundaries

### Allowed Changes
- `src/home/room_settings_modal.rs` — new file; modal widget implementation
- `src/home/main_desktop_ui.rs` — wire `RoomSettingsAction::Open` to show the modal
- `src/home/room_screen.rs` — add open-settings trigger in room header
- `src/app.rs` — register `RoomSettingsModal` in the global widget tree if needed

### Must Create
- `src/home/room_settings_modal.rs` — `RoomSettingsModal` widget
  (`#[derive(Script, ScriptHook, Widget)]`, `script_mod!` DSL)

### Forbidden
- Do NOT use `live_design!` (Makepad 1.x)
- Do NOT call `get_client()` from any UI file added by this task
- Do NOT implement actual Matrix state-event writes for Room Name or Room Topic
  in M4 — post the action only
- Do NOT implement avatar change, Voice & Video, Security & Privacy,
  Roles & Permissions, Notifications, Polls, or Advanced sections
- Do NOT add new cargo dependencies

## Completion Criteria

Scenario: Modal opens with correct title and room name pre-filled
  Test:
    package: robrix
    filter: test_room_settings_opens_with_correct_data
  Given a joined room with display name "robix_test3"
  When RoomSettingsAction::Open { room_id } is posted
  Then the RoomSettingsModal becomes visible
    And the title bar reads "Room Settings - robix_test3"
    And room_name_input contains the text "robix_test3"
    And room_topic_input is empty

Scenario: General tab is active by default on open
  Test:
    package: robrix
    filter: test_room_settings_general_tab_default
  Given the modal opens for any room
  When the modal first renders
  Then the "General" sidebar item has the active highlight style
    And all other sidebar items render with the inactive style

Scenario: Save button posts RoomSettingsAction::Save with edited values
  Test:
    package: robrix
    filter: test_room_settings_save_posts_action
  Given the modal is open
    And the user changes room_name_input to "new-room-name"
    And the user changes room_topic_input to "A test topic"
  When the user clicks save_button
  Then RoomSettingsAction::Save { room_name: "new-room-name", room_topic: "A test topic" } is posted via Cx::post_action()
    And the modal remains open (no close until async confirm)

Scenario: Cancel button closes modal without posting Save
  Test:
    package: robrix
    filter: test_room_settings_cancel_closes_modal
  Given the modal is open
    And the user has edited room_name_input to "dirty-name"
  When the user clicks cancel_button
  Then RoomSettingsAction::Cancel is posted
    And the modal becomes invisible
    And no RoomSettingsAction::Save is posted

Scenario: Close (×) button dismisses modal
  Test:
    package: robrix
    filter: test_room_settings_close_button_dismisses
  Given the modal is open
  When the user clicks the × close button
  Then the modal becomes invisible
    And no save action is posted

Scenario: Publish directory toggle reflects initial state
  Test:
    package: robrix
    filter: test_room_settings_publish_toggle_initial
  Given a room where the directory publish flag is false
  When the Room Addresses section renders
  Then publish_directory_toggle is in the OFF (false) state
    And the label reads "Publish this room to the public in matrix.org's room directory?"

Scenario: Toggling publish_directory_toggle emits correct action
  Test:
    package: robrix
    filter: test_room_settings_publish_toggle_emits
  Given publish_directory_toggle is currently OFF
  When the user clicks publish_directory_toggle
  Then RoomSettingsAction::SetDirectoryPublish { enabled: true } is posted

Scenario: Show media radio group defaults to "Always show"
  Test:
    package: robrix
    filter: test_room_settings_media_radio_default
  Given the modal opens for a room with no stored media-visibility preference
  When the Moderation and safety subsection renders
  Then the "Always show" radio button is selected
    And "Always hide" is not selected

Scenario: Selecting "Always hide" radio button changes selection
  Test:
    package: robrix
    filter: test_room_settings_media_radio_hide_selected
  Given the "Always show" radio button is currently selected
  When the user clicks the "Always hide" radio button
  Then "Always hide" becomes the selected state
    And "Always show" is deselected

Scenario: Leave room button is visible and styled destructively
  Test:
    package: robrix
    filter: test_room_settings_leave_room_button_visible
  Given the modal is open and the user has scrolled to the Leave Room section
  When the section renders
  Then leave_room_button is visible
    And its background color is #E53935
    And its label is "Leave room"

Scenario: Add address button with empty input does not post action
  Test:
    package: robrix
    filter: test_room_settings_add_address_empty_noop
  Given room_address_input is empty
  When the user clicks add_address_button
  Then no RoomSettingsAction::AddLocalAddress is posted

Scenario: Add address button with valid input posts action
  Test:
    package: robrix
    filter: test_room_settings_add_address_valid
  Given the user has typed "my-room" into room_address_input
  When the user clicks add_address_button
  Then RoomSettingsAction::AddLocalAddress { alias: "my-room" } is posted
    And room_address_input is cleared

Scenario: General section renders all four widgets stacked vertically
  Test:
    package: robrix
    filter: test_room_settings_general_section_layout
  Given the modal is open with the General tab active
  When the General section renders
  Then room_name_input is visible with a "Room Name" label above it
    And room_topic_input is visible with a "Room Topic" label above it
    And cancel_button and save_button are visible in a right-aligned row below the inputs
    And the room avatar view with a circular initials fallback is visible to the right

Scenario: Room avatar falls back to initials when no avatar URL is set
  Test:
    package: robrix
    filter: test_room_settings_avatar_initials_fallback
  Given a room with no avatar URL and display name "robix_test3"
  When the avatar widget renders in the General section
  Then the avatar view displays the initial "R"
    And no image fetch is attempted

Scenario: Published Addresses section renders main alias and add-address row
  Test:
    package: robrix
    filter: test_room_settings_published_addresses_layout
  Given the modal is open and the room has canonical alias "#robix_test3:matrix.org"
  When the Room Addresses section renders
  Then main_address_dropdown shows "#robix_test3:matrix.org"
    And the explanatory label containing "Published addresses can be used by anyone" is visible
    And the empty-state label "No other published addresses yet, add one below" is visible
    And room_address_input shows placeholder "# e.g. my-room"
    And add_address_button is visible

Scenario: Local Addresses section renders description and show-more link
  Test:
    package: robrix
    filter: test_room_settings_local_addresses_layout
  Given the modal is open
  When the Local Addresses section renders
  Then the section label "Local Addresses" is visible
    And the description contains the text "matrix.org"
    And show_more_button is visible with label "Show more"

Scenario: Save button disabled and no action posted when room name is blank
  Test:
    package: robrix
    filter: test_room_settings_save_blocked_on_blank_name
  Given the modal is open
    And the user has cleared room_name_input to an empty string
  When the user clicks save_button
  Then no RoomSettingsAction::Save is posted
    And an inline validation error "Room name cannot be empty" is displayed near room_name_input

## Out of Scope

- Voice & Video, Security & Privacy, Roles & Permissions, Notifications, Polls,
  Advanced sidebar sections — future tasks
- Avatar change flow (pencil overlay is rendered non-functional in M4)
- Actual Matrix state-event write for room name / topic (async write deferred)
- Leave room confirmation dialog — future task
- Show more / pagination for Local Addresses
- Keyboard navigation within the sidebar
- Mobile layout (single-column) — future task
