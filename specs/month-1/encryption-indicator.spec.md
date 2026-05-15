spec: task
name: "Encryption Indicator"
inherits: project
tags: [month-1, security, ui]
---

<!-- changed: full rewrite of this spec. reason: original badge/header/popup design was redirected by user-supplied mockup (2026-05-13 screenshot) to a top-of-timeline banner notice. All sections below reflect the post-mockup design captured in the brainstorming session on 2026-05-13. -->

## Intent

<!-- changed: rewrote Intent. reason: scope narrowed to a single timeline-prefix notice; dropped avatar badges, header indicator, and verification rollup. -->
<!-- changed: appended scope justification for index-math refactor. reason: reviewer flagged tl_idx_from_item_id refactor (Decisions) as potential scope creep against the original Intent; index translation is necessary for correctness when a synthetic item is injected. -->
<!-- changed: simplified placement to "always at item_id = 0". reason: user directive — the notice is always the first PortalList item; no special-casing around date dividers. -->
<!-- changed: extended Intent to cover the room-list lock indicator added as surface 2. reason: user approved the room-list extension; now two coordinated surfaces share the same is_encrypted data source. -->
Surface this signal in two coordinated places:

1. **In-room timeline notice.** Render a clear, non-interactive notice at the very top of every room's timeline indicating whether the conversation is end-to-end encrypted. The notice is always the first PortalList item (`item_id == 0`), scrolls with timeline content, and helps users understand the room's encryption posture at a glance — including a prompt to verify the other party (for encrypted rooms). Because injecting a synthetic PortalList item shifts indices by exactly one, the change includes a small, scoped refactor to centralize timeline index translation so existing message-action and scroll-to-message behavior remains correct.

2. **Room-list lock indicator.** Render a small, non-interactive lock icon in each joined-room entry in the rooms list, positioned alongside the existing tombstone icon in all three adaptive variants of `RoomsListEntry`. The icon is shown only when the room is end-to-end encrypted and not tombstoned; absence of the icon means "not encrypted" (or unknown). The two surfaces read from the same `JoinedRoomInfo.is_encrypted` field and respond to the same `RoomsListUpdate::UpdateIsEncrypted` live-update channel.

## Decisions

<!-- changed: replaced all prior Decisions with screenshot-driven ones. reason: design shifted from multi-surface indicators (badge + header + popup) to a single PortalList banner. -->
- Single banner widget (`EncryptionNotice`) rendered inside `room_screen.rs`'s PortalList — no avatar badges, no room-header lock icon, no popup/modal.
<!-- changed: collapsed two placement rules into one. reason: user directive — the notice is always at PortalList index 0 regardless of date-divider presence. Earlier "after first date divider" rule (drawn from the screenshot crop) is superseded by the user's text instruction "first timeline Item in the portallist." -->
- Placement: the notice is **always** at PortalList index `0`. It is rendered as the very first item, before any `tl_items` entry (including date dividers). All `tl_items` are shifted by exactly one position in the PortalList: `tl_items[i]` renders at PortalList index `i + 1`.
<!-- changed: pinned the body string verbatim and named the dash character. reason: reviewer flagged that an unspecified hyphen/dash codepoint makes exact-match tests fragile. -->
- Encrypted state copy: title is the literal string `Encryption enabled`; body is the literal string `Messages here are end-to-end encrypted. Verify {first_other_member} in their profile - tap on their profile picture.` The dash between `profile` and `tap` is ASCII hyphen-minus (U+002D) surrounded by single spaces.
- Not-encrypted state copy: title is the literal string `Encryption not enabled`; body is the literal string `Messages here are not end-to-end encrypted.` (title-only swap; body has no verify sentence.)
<!-- changed: clarified back-fill semantics, named exact placeholder char, fixed truncation length to an exact value. reason: reviewer flagged ambiguity over what `…` replaces, the codepoint, and untestable `~30 chars`. -->
<!-- changed: dropped the user-visible "…" placeholder. reason: user reported that rendering "Verify … in their profile" looks broken/dotted to end users; the placeholder is now an internal "no name yet" sentinel only — never rendered. While no name is resolvable, the verify sentence is suppressed entirely and the body falls back to "Messages here are end-to-end encrypted." -->
- `{first_other_member}` is the first non-self member returned by `Room::members()` iteration. The verify sentence is rendered **only** when this name has resolved to a non-empty display name (or, as fallback, a non-empty user-id string).
- While the member list has not yet been fetched, OR the room contains no non-self members, OR no display name is available, the verify sentence is suppressed and the body renders as the literal string `Messages here are end-to-end encrypted.`. No "…" placeholder is ever rendered into user-visible copy.
- When the member-list load completes (or membership changes) and a non-self display name becomes available, the body re-renders to include the verify sentence with the resolved name.
- Long display names are truncated to exactly 30 characters and suffixed with `…` (U+2026) before substitution into the body — the only place "…" is allowed to appear in rendered copy.
<!-- changed: replaced approximations with exact values. reason: reviewer flagged untestable `~` dimensions. -->
- Visual: light-gray rounded rectangle, background color `#F0F2F5`, corner radius `6`, padding `12`, horizontal margin `16`, vertical margin `8`. Lock icon (filled for encrypted, open for not-encrypted) on the left, size `16`, color `#888888`. Bold title; regular-weight body. (Units are Makepad logical pixels.)
<!-- added: mandated implementation pattern for the dual lock icons. reason: bare `Icon { visible: false }` does not gate Icon rendering in Makepad 2.0 — both icons would render simultaneously regardless of set_visible() calls. Wrap each Icon in a named View (mirroring the working TombstoneIcon and verification_badge.rs IconYes/IconNo/IconUnk patterns); toggle the View's visibility. -->
- Both state icons (`lock_filled_icon`, `lock_open_icon`) are declared as named `View` siblings inside the `EncryptionNotice` `script_mod!` block, each wrapping a single `Icon` with the appropriate SVG. Both Views are always present in the widget tree. Runtime visibility is controlled by calling `self.view.view(cx, ids!(lock_filled_icon)).set_visible(cx, is_encrypted)` and the inverse for `lock_open_icon`. Do **not** declare the Icons directly (without a wrapping View) and do **not** call `set_visible` on the Icon directly — Makepad 2.0's `Icon` widget does not gate its draw on the `visible` field, so a bare `Icon { visible: false }` still renders. The working reference for this pattern in the codebase is `TombstoneIcon` in `src/home/rooms_list_entry.rs:20` and the `IconYes`/`IconNo`/`IconUnk` Views in `src/shared/verification_badge.rs`.
- Icon assets: `resources/icons/lock_filled.svg` and `resources/icons/lock_open.svg` (added if not already present).
- The notice is informational and non-interactive: no tap handler, no action emitted, no popup.
- Encryption state is sourced from a new `is_encrypted: Option<bool>` field on `JoinedRoomInfo`. The field is initialized during sliding-sync room registration and updated live via a new `RoomsListUpdate::UpdateIsEncrypted { room_id, is_encrypted }` variant (parallels the existing `UpdateIsDirect`).
- Live updates: when a room transitions off→on encrypted mid-session, the notice swaps from "not encrypted" to "encrypted" without requiring a room re-open. (Matrix encryption is monotonic; off→on is the only transition handled.)
<!-- added: monotonicity of cached value. reason: reviewer asked how `Some(_) -> None` is handled; explicitly forbid demotion of cached state. -->
- The cached `is_encrypted` value is monotonic in the upgrade direction only. Allowed transitions: `None → Some(false)`, `None → Some(true)`, `Some(false) → Some(true)`. The system never demotes a known state back to `None`, and never demotes `Some(true)` back to `Some(false)`.
- When `is_encrypted` is `None` (initial state before sliding-sync registration completes, or fetch failed), the notice does not render. No false-positive encryption claim is ever shown.
<!-- added: explicit render-while-loading invariant. reason: reviewer flagged unclear ordering between is_encrypted resolution and member-list load. -->
<!-- changed: removed placeholder from render-while-loading invariant. reason: the placeholder is no longer rendered; instead the verify sentence is suppressed until a real name resolves. -->
- The notice renders as soon as `is_encrypted` is `Some(_)`, regardless of whether the member list has loaded. While no name has resolved, the body omits the verify sentence; encryption-state resolution is the sole gate on visibility (not member-list load).
<!-- changed: simplified index-math description to match the always-at-0 placement. reason: with notice_position fixed at 0, the offset is a constant 1 — helper logic reduces to a saturating subtract. -->
<!-- changed: enumerated all index-translation sites including the items_with_actions loop. reason: regression hit during implementation — the action-loop handlers (small-state-group toggle, invite button) used `index` directly as a tl_idx, breaking fold/unfold and invite when the encryption notice was rendered. The spec now explicitly enumerates every site that must convert. -->
- PortalList index math is centralized in a helper (`tl_idx_from_item_id`) that maps PortalList `item_id` → `tl_items` index using the constant rule: `item_id == 0` is the encryption notice (no `tl_items` lookup); `item_id >= 1` maps to `tl_items[item_id - 1]`. Every site that receives a PortalList `item_id` and then either (a) indexes into `tl_items`, (b) compares against a `tl_idx`-keyed structure (e.g., `SmallStateEventGroup::start`), or (c) calls a function expecting `tl_idx` (e.g., `toggle_small_state_event_group`) must route through this helper. Sites that must convert include: the render loop in `draw_walk`; `MessageAction::HighlightMessage` and `JumpToRelated` handlers; jump-to-bottom and scroll-to-message; `handle_image_click`; the body of `portal_list.items_with_actions(actions)` in `handle_event` (specifically the `state_group_toggle_button` click branch and the `invite_user_button` click branch). The helper is applied **exactly once** per `item_id` value — never apply the offset by hand or pass an `item_id` to a function that internally calls the helper again.

<!-- added: room-list indicator decisions (surface 2). reason: user approved the extension to render a sibling lock icon in rooms_list_entry.rs mirroring the TombstoneIcon pattern. -->
### Room-list indicator (surface 2)

- Add a new `EncryptionIcon` widget in `rooms_list_entry.rs`'s `script_mod!` block, sibling to the existing `TombstoneIcon` declared at the same location. The widget structure mirrors `TombstoneIcon` exactly: outer `View { width: Fit, height: Fit, visible: false }` wrapping an `Icon { width: 19, height: 19, icon_walk: Walk{ width: 15, height: 15 } }`.
- Icon SVG: reuses `resources/icons/lock_filled.svg` already declared by the in-room notice. The open-lock variant is **not** rendered in the room list (indicator is encrypted-only).
- Icon color: `#888888` (matches the in-room banner icon color for visual consistency).
- The `encryption_icon` instance is added to all three adaptive variants of `RoomsListEntry` (`OnlyIcon`, `IconAndName`, `FullPreview`), placed in the DSL tree immediately before each existing `tombstone_icon` so the two siblings stay co-located across layouts.
- Visibility rule (applied in the joined-room arm of `set_entry`): `encryption_icon` is visible iff `matches!(room_info.is_encrypted, Some(true)) && !room_info.is_tombstoned`. Tombstone takes visual priority; encrypted-but-tombstoned rooms show only the tombstone icon.
- Show only for joined rooms. `InvitedRoomInfo` entries do not render an encryption icon (invited rooms lack reliable encryption metadata pre-join).
- The indicator is informational and non-interactive: no tap handler, no action emitted. Tapping the row continues to open the room as today.
- Live updates: the same `RoomsListUpdate::UpdateIsEncrypted` channel that drives the in-room notice also drives the room-list icon. When an update arrives, the affected row redraws and visibility recomputes from the new `is_encrypted` value.
- Avatar overlay (badge on the avatar itself) is **not** the chosen surface — the icon is a sibling element in the row, exactly as `tombstone_icon` is.

## Boundaries

### Allowed to Modify
<!-- changed: replaced file list. reason: scope dropped rooms_list avatar surface; added sliding_sync for state subscription. -->
<!-- changed: added rooms_list_entry.rs. reason: room-list indicator (surface 2) is a sibling icon in RoomsListEntry, so the file is in scope; not an avatar overlay so the prior "no avatar overlay" exclusion is preserved. -->
- `src/home/room_screen.rs` — inject `EncryptionNotice` into the PortalList render loop; apply index offset; refresh on encryption-state updates.
- `src/home/rooms_list.rs` — add `is_encrypted: Option<bool>` to `JoinedRoomInfo`; handle the new `UpdateIsEncrypted` variant.
- `src/home/rooms_list_entry.rs` — declare `EncryptionIcon` widget; place `encryption_icon` instance in all three adaptive variants beside each existing `tombstone_icon`; toggle visibility in the joined-room arm of `set_entry`.
- `src/sliding_sync.rs` — populate initial `is_encrypted` during room registration; subscribe to room state and emit `UpdateIsEncrypted` on encryption-enabled transitions.

### Must Create
<!-- changed: dropped encryption_info.rs (popup) and renamed/relocated badge. reason: design no longer uses room-list badge or popup; single banner lives in home/. -->
- `src/home/encryption_notice.rs` — the banner widget (Makepad 2.0 `script_mod!` widget, View with lock icon + title + body labels).
- `resources/icons/lock_filled.svg` and `resources/icons/lock_open.svg` if not already present.

### Forbidden
<!-- changed: expanded Forbidden list. reason: user explicitly required no get_client() in UI path; existing project rules carried forward. -->
<!-- changed: broadened then scoped get_client() forbid. reason: reviewer flagged that the prior wording could be argued to exempt room_screen.rs since it doesn't directly "handle the notice"; verification step then found 5 pre-existing get_client() callsites under src/home/ and src/shared/, so the rule is scoped to "no NEW callsites added by this task" rather than a blanket ban that would mark existing code as violating. -->
- Do NOT introduce any new call to `crate::sliding_sync::get_client()` in any file created or modified by this task. This applies to `src/home/encryption_notice.rs` (new), `src/home/room_screen.rs`, `src/home/rooms_list.rs`, `src/sliding_sync.rs`, and any helper modules added in support. The rule also forbids introducing intermediate helpers anywhere in `src/home/` or `src/shared/` that proxy to `get_client()`. Pre-existing callsites in the repository at the time of this spec — `src/home/main_desktop_ui.rs`, `src/home/room_screen.rs` (lines outside the new injection block), and `src/shared/verification_badge.rs` — are out of scope for this task and must not be removed or relocated as part of it.
- All Matrix queries needed by this task (`Room::is_encrypted()`, room-state subscription, member fetches) must run inside `sliding_sync.rs` async tasks. UI code reads only from cached `JoinedRoomInfo` fields and from actions posted via `Cx::post_action()`.
- Do NOT spawn raw tokio tasks for encryption-state subscription — register the watcher inside the existing room-registration flow in `sliding_sync.rs`.
- Do NOT add an encryption badge on room-list avatars (`rooms_list.rs` / `rooms_list_entry.rs` UI is unchanged for this purpose).
- Do NOT add a room-header lock icon or "Encrypted" text label outside the PortalList notice.
- Do NOT add a popup, modal, or sliding pane for encryption details (`src/shared/encryption_info.rs` from the prior spec draft is explicitly dropped).
- Do NOT change existing E2EE decryption logic.
- Do NOT modify the device verification flow.
- Do NOT alter room encryption settings or expose a toggle.
- Do NOT make the notice tappable or emit actions from it.

## Completion Criteria

<!-- changed: replaced all BDD scenarios. reason: prior scenarios targeted badge/header/popup; new scenarios target the banner placement, copy, back-fill, live updates, and the no-get_client() code constraint. -->

<!-- changed: scenario renamed and Then clause simplified. reason: user directive — placement is now always index 0; no special-casing for date dividers. -->
Scenario: Encrypted room shows notice at PortalList index 0
  Test:
    package: robrix
    filter: test_notice_encrypted
  Given user opens a room with end-to-end encryption enabled
  When the room screen renders
  Then the EncryptionNotice appears at PortalList index 0
    And its title is "Encryption enabled"
    And its body is "Messages here are end-to-end encrypted. Verify {first_other_member} in their profile - tap on their profile picture."

<!-- changed: scenario renamed and Then clause simplified. reason: user directive — placement is now always index 0. -->
Scenario: Unencrypted room shows notice at index 0 with title swap
  Test:
    package: robrix
    filter: test_notice_unencrypted
  Given user opens a room with no encryption
  When the room screen renders
  Then the EncryptionNotice appears at PortalList index 0
    And its title is "Encryption not enabled"
    And its body is "Messages here are not end-to-end encrypted."

<!-- changed: kept as a guard against regressions to a date-divider-coupled placement; explicitly verifies behavior is identical to the populated-room case. reason: with the always-at-0 rule the empty-room case no longer differs, but a regression that ties placement to dividers would re-introduce a divergence — this scenario locks the invariant. -->
Scenario: Empty room still places notice at index 0
  Test:
    package: robrix
    filter: test_notice_empty_room
  Given user opens a room with no timeline items (tl_items.len() == 0)
  When the room screen renders
  Then the EncryptionNotice appears at PortalList index 0
    And no other PortalList item is rendered before it

<!-- changed: split into two single-trigger scenarios. reason: reviewer flagged non-standard Given/Then/When/Then form and missing concrete trigger for member load. -->
<!-- changed: scenario rewritten to assert no-placeholder behavior. reason: user redirected the design — when members are not loaded the body is the generic encrypted sentence (no verify prompt, no "…"). -->
Scenario: Notice suppresses verify sentence when members are not loaded
  Test:
    package: robrix
    filter: test_notice_member_placeholder
  Level: integration
  Given JoinedRoomInfo.is_encrypted is Some(true) for the open room
    And the room's member list has not yet been fetched
  When the room screen renders
  Then the notice body is "Messages here are end-to-end encrypted."
    And no verify sentence is included
    And the body does not contain the character "…"

<!-- changed: back-fill Given reflects the new "generic body" pre-state instead of a placeholder. reason: matches the redirected behavior. -->
Scenario: Notice adds verify sentence when member name resolves
  Test:
    package: robrix
    filter: test_notice_member_backfill
  Level: integration
  Given the notice is rendered with the generic body "Messages here are end-to-end encrypted."
    And the room's member-loading completion action subsequently posts loaded members to the timeline state with at least one non-self member that has a display name
  When the next render runs
  Then the notice body becomes "Messages here are end-to-end encrypted. Verify {first_other_member} in their profile - tap on their profile picture."
    And no other body text changes

Scenario: Room with no other members drops verify sentence
  Test:
    package: robrix
    filter: test_notice_lonely_room
  Given user opens an encrypted room where they are the only member
  When the room screen renders
  Then the notice body is "Messages here are end-to-end encrypted."
    And the verify sentence is absent

Scenario: Encryption enabled mid-session updates notice live
  Test:
    package: robrix
    filter: test_notice_live_update
  Level: integration
  Test Double: mock matrix-sdk room state subscription
  Given user is viewing an unencrypted room with the notice visible
  When the room transitions to encrypted (RoomsListUpdate::UpdateIsEncrypted arrives)
  Then the notice title swaps to "Encryption enabled" without re-opening the room
    And the body updates to include the verify sentence

<!-- added: explicit error-path scenario for is_encrypted fetch failure. reason: linter warned no error-path scenarios; this scenario tests the side-effect side (logging) that the "unknown state hides notice" scenario does not. -->
Scenario: Encryption-state fetch failure during room registration is logged and surfaces as None
  Test:
    package: robrix
    filter: test_notice_fetch_failure_logs
  Level: integration
  Test Double: mock matrix-sdk Room::is_encrypted() to return an error
  Given a room registration is in progress
    And the underlying Room::is_encrypted() call returns an error
  When sliding_sync completes registration for the room
  Then JoinedRoomInfo.is_encrypted for that room is None
    And an error is logged via the project's standard logging facility describing the failed encryption-state fetch
    And no UpdateIsEncrypted action is posted for that room until a subsequent successful fetch

Scenario: Unknown encryption state hides notice
  Test:
    package: robrix
    filter: test_notice_unknown_hidden
  Given JoinedRoomInfo.is_encrypted is None for the open room
  When the room screen renders
  Then no EncryptionNotice is rendered
    And no false positive encryption claim appears

<!-- added: room-switching scenario. reason: reviewer flagged absence. -->
Scenario: Switching rooms updates notice to the new room's state
  Test:
    package: robrix
    filter: test_notice_room_switch
  Level: integration
  Given user is viewing encrypted room A with the notice showing "Encryption enabled"
  When user opens unencrypted room B
  Then the notice in the room screen shows title "Encryption not enabled"
    And the body is "Messages here are not end-to-end encrypted."
    And no stale state from room A persists in the rendered notice

<!-- changed: rewrote Given to reflect the constant offset of 1 (notice always at index 0). reason: simpler invariant after the placement change. -->
Scenario: Constant +1 PortalList offset preserves message actions
  Test:
    package: robrix
    filter: test_notice_offset_actions
  Given the EncryptionNotice renders at PortalList index 0 (the constant invariant)
    And tl_items[i] renders at PortalList index i + 1 for all i in 0..tl_items.len()
  When the user triggers a message highlight on the timeline item at tl_items index k
  Then MessageAction::HighlightMessage targets PortalList item_id k + 1
    And jump-to-bottom and scroll-to-message land on the intended tl_items entry after applying the same offset

<!-- added: regression coverage for the items_with_actions loop after the toggle/invite bug. reason: the prior offset scenario only covered MessageAction-style calls; clicking the small-state-group toggle button (a portal_list.items_with_actions(actions) branch) failed silently because `index` was used as a tl_idx without conversion. -->
Scenario: Small-state-group fold/unfold works in encrypted rooms
  Test:
    package: robrix
    filter: test_notice_offset_preserves_state_group_toggle
  Level: integration
  Given user opens an encrypted room where the encryption notice renders at item_id 0
    And the timeline contains a small-state-event group starting at tl_items index k (i.e. PortalList item_id k + 1)
    And the group is currently expanded
  When the user clicks the state_group_toggle_button on the first event of the group
  Then the portal_list.items_with_actions handler converts item_id (k + 1) to tl_idx k via tl_idx_from_item_id
    And toggle_small_state_event_group is invoked with tl_idx k
    And the group transitions from expanded to collapsed
    And the same flow works in reverse (collapsed → expanded) on a subsequent click

<!-- added: regression coverage for the invite button's tl_items lookup. reason: same loop body used `tl.items.get(index)` where index was item_id; in encrypted rooms this read the wrong tl_item or read nothing. -->
Scenario: Invite-user button on a small-state event reads the correct tl_item in encrypted rooms
  Test:
    package: robrix
    filter: test_notice_offset_preserves_invite_button
  Level: integration
  Given user opens an encrypted room where the encryption notice renders at item_id 0
    And a small-state event at tl_items index k displays an invite_user_button
  When the user clicks the invite_user_button
  Then the handler converts item_id (k + 1) to tl_idx k via tl_idx_from_item_id
    And tl.items.get(k) returns the intended event_tl_item
    And the invite confirmation modal is populated with the correct user_id and username from that event

<!-- added: room-list indicator scenarios (surface 2). reason: user approved the room-list extension; six scenarios cover the visibility matrix (encrypted/unencrypted/unknown × tombstoned/not), live updates, and invited-room exclusion. -->
Scenario: Encrypted joined room shows lock icon in all three adaptive variants
  Test:
    package: robrix
    filter: test_room_list_icon_visible_when_encrypted
  Given JoinedRoomInfo.is_encrypted is Some(true) for a room
    And the room is not tombstoned (is_tombstoned == false)
  When the room's RoomsListEntry renders in each of the three adaptive variants (OnlyIcon, IconAndName, FullPreview)
  Then the encryption_icon View has visible == true in all three variants
    And the icon uses resources/icons/lock_filled.svg with color #888888

Scenario: Unencrypted joined room hides lock icon
  Test:
    package: robrix
    filter: test_room_list_icon_hidden_when_unencrypted
  Given JoinedRoomInfo.is_encrypted is Some(false) for a room
  When the room's RoomsListEntry renders
  Then the encryption_icon View has visible == false in all three adaptive variants

Scenario: Unknown encryption state hides lock icon
  Test:
    package: robrix
    filter: test_room_list_icon_hidden_when_unknown
  Given JoinedRoomInfo.is_encrypted is None for a room
  When the room's RoomsListEntry renders
  Then the encryption_icon View has visible == false in all three adaptive variants
    And no false-positive encryption claim is rendered

Scenario: Tombstoned encrypted room hides encryption icon and shows tombstone
  Test:
    package: robrix
    filter: test_room_list_icon_yields_to_tombstone
  Given JoinedRoomInfo.is_encrypted is Some(true) and is_tombstoned is true for a room
  When the room's RoomsListEntry renders
  Then the encryption_icon View has visible == false
    And the tombstone_icon View has visible == true

Scenario: UpdateIsEncrypted live-updates the row icon
  Test:
    package: robrix
    filter: test_room_list_icon_live_update
  Level: integration
  Given a visible RoomsListEntry with is_encrypted == Some(false) and the encryption_icon hidden
  When a RoomsListUpdate::UpdateIsEncrypted { is_encrypted: true } for that room is processed
  Then the row redraws and encryption_icon becomes visible
    And no full room-list rebuild is required

<!-- added: covers None -> Some(true) live transition for the row icon. reason: reviewer noted a coverage nit — monotonicity allows this transition (line in Decisions) but no scenario exercised it for the row icon specifically. -->
Scenario: UpdateIsEncrypted resolves None to Some(true) and reveals the row icon
  Test:
    package: robrix
    filter: test_room_list_icon_resolve_from_unknown
  Level: integration
  Given a visible RoomsListEntry with is_encrypted == None and the encryption_icon hidden
  When a RoomsListUpdate::UpdateIsEncrypted { is_encrypted: true } for that room is processed (the room transitions from unknown to known-encrypted)
  Then the row redraws and encryption_icon becomes visible
    And the underlying JoinedRoomInfo.is_encrypted is now Some(true)

Scenario: Invited rooms render no encryption icon
  Test:
    package: robrix
    filter: test_room_list_icon_invited_room_unaffected
  Given a RoomsListEntry rendering an InvitedRoomInfo (not a JoinedRoomInfo)
  When the entry renders
  Then no encryption_icon is rendered for that entry regardless of any encryption metadata on the underlying room

<!-- changed: kept agent-spec test selector format but converted enforcement to PR-diff grep. reason: reviewer flagged that a Rust unit test cannot grep source; verification then revealed 5 pre-existing get_client() callsites, so the check is a delta check (no NEW occurrences in the PR diff vs. main), not an absolute-zero whole-repo check. -->
Scenario: This task does not add new direct matrix-client queries from the UI tree
  Test:
    package: robrix
    filter: ci_check_no_new_get_client_in_ui
  Level: code review / CI
  Given the encryption_notice module, room_screen integration, rooms_list integration, and any helpers added by this task
    And the enforcement mechanism is a PR-diff grep — pattern `get_client\s*\(`, paths `src/home/**` and `src/shared/**`, comparing the PR head against main, expecting zero net-new matching lines
  When CI runs the named grep over PR-diff additions against `src/home/` and `src/shared/`
  Then no net-new lines containing `get_client(` are added by this task
    And all is_encrypted reads in net-new code come from JoinedRoomInfo
    And all is_encrypted writes added by this task originate inside sliding_sync.rs async tasks

## Out of Scope

<!-- changed: updated out-of-scope list. reason: prior verification rollup, avatar badge, room-header indicator, and popup are now explicitly excluded from this spec; verification rollup deferred. -->
<!-- changed: clarified avatar-overlay exclusion. reason: room-list indicator is now in scope as a sibling icon (not an avatar overlay), so the prior blanket "avatar badge" exclusion was too broad. -->
- Per-room "all devices verified" rollup (green checkmark / orange warning indicators) — deferred to a future spec.
- Encryption indicator drawn as an overlay on the room avatar (badge over avatar image) — explicitly out of scope. The room-list indicator added by this spec is a sibling icon in the row, mirroring `tombstone_icon`'s position, not an avatar overlay.
- Encryption details popup or sliding pane — dropped.
- Room header lock icon or "Encrypted" text label inside `room_screen.rs` — dropped (the in-room signal lives in the PortalList notice, not the header).
- Device verification flow — existing feature, untouched.
- Key backup indicators — Month 8.
- Per-message encryption status — out of scope.
- Encryption settings toggle — out of scope.
- Reaction to encryption being disabled mid-session — impossible per Matrix monotonicity; no handler needed.
- Encryption indicator on `InvitedRoomInfo` entries — out of scope (joined-room only).
