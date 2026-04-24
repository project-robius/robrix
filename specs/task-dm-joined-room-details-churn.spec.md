spec: task
name: "DM Rejoin — Stop JoinedRoomDetails Churn On Display Flip"
inherits: project
tags: [bugfix, dm, sliding-sync, timeline, room-list]
estimate: 0.5d
---

## Intent

Fix the bug where a freshly-created DM (after a prior leave) lands on a blank "Loading earlier messages..." timeline with the Dock tab stuck showing `Room ID !...`. The code had already shipped two upstream fixes — stale `m.direct` filtering (`is_active_dm_room_state`) and DM auto-binding — but the timeline was still dead because `src/sliding_sync.rs::update_room` destroyed the `JoinedRoomDetails` the open `RoomScreen` was still holding a singleton receiver for. This happens during the transient `(Joined, is_direct=true, display_name=Empty)` window between DM creation and the invited bot joining.

## Constraints

- The open `RoomScreen`'s singleton timeline endpoints, taken at `navigate_to_room`, must survive every same-room `update_room` call while the room stays `RoomState::Joined`
- The "empty direct DM should not clutter the sidebar" policy already enforced by `should_display_joined_room_entry(...)` must remain observable — empty Joined direct rooms are hidden from `displayed_direct_rooms` / `displayed_regular_rooms`
- Do not change the public shape of `MatrixRequest::*`, `DirectMessageRoomAction::*`, or `RoomsListUpdate::AddJoinedRoom` / `RoomsListUpdate::RemoveRoom`
- Do not add new cargo dependencies
- Do not run `cargo fmt`
- Do not touch Makepad DSL files

## Decisions

- Introduce a pure helper `classify_joined_room_display_flip(old_should_display: bool, new_should_display: bool) -> JoinedRoomDisplayFlip` returning one of `{ BecameDisplayable, BecameHidden, NoDisplayChange }`, and unit-test it
- Introduce a new `RoomsListUpdate::UnhideRoom { room_id: OwnedRoomId }` variant as the semantic dual of `RoomsListUpdate::HideRoom`
- Rewrite `src/sliding_sync.rs::update_room` lines 4819–4825 so that:
  - When `BecameHidden`, emit `RoomsListUpdate::HideRoom { room_id }` and **fall through** to the remaining update logic (do NOT `return Ok(())`)
  - When `BecameDisplayable`, emit `RoomsListUpdate::UnhideRoom { room_id }` and **fall through**
  - When `NoDisplayChange`, skip
- Wire `RoomsListUpdate::UnhideRoom` in `src/home/rooms_list.rs` to: remove `room_id` from `self.hidden_rooms`, then re-check `should_display_room!(self, &room_id, room)` and push the room into `displayed_direct_rooms` / `displayed_regular_rooms` if it is newly eligible and not already present
- Keep the existing state-transition branches (`Left`, `Banned`, `Joined`, `Invited`, `Knocked`) at `src/sliding_sync.rs:4831–4861` — they are still the only lawful producers of `add_new_room` / `remove_room` for a same-room update
- Retire the `should_reuse_existing_joined_room_details` predicate added to guard `add_new_room`: once `update_room` no longer tears the `JoinedRoomDetails` down on display flips, the guard is dead code. Delete the predicate, its two unit tests, and the reuse branch inside `add_new_room`

## Boundaries

### Allowed Changes
- `src/sliding_sync.rs`
- `src/home/rooms_list.rs`
- `specs/task-dm-joined-room-details-churn.spec.md`
- `docs/superpowers/plans/2026-04-16-dm-joined-room-details-churn-plan.md`

### Forbidden
- Do NOT modify `src/home/main_desktop_ui.rs` Dock tab rename plumbing in this task (tracked separately — see Out of Scope)
- Do NOT change `focus_or_create_tab` behavior
- Do NOT change `add_new_room` / `remove_room` call sites outside the single block at `src/sliding_sync.rs:4819–4825`
- Do NOT modify matrix-sdk upstream behavior
- Do NOT change `DirectMessageRoomAction::*` variants or handlers in `src/app.rs`
- Do NOT run `cargo fmt`

## Out of Scope

- Propagating `RoomsListUpdate::UpdateRoomName` to the Dock tab label so the tab text changes from `Room ID !...` to `octosbot` without a re-navigate. The room-name-to-tab propagation is an independent pre-existing TODO at `src/home/rooms_list.rs:716–718` and will be handled in a follow-up task spec
- Changing the moment `DirectMessageRoomAction::NewlyCreated` is posted (it is still emitted with whatever `display_name` the server has at creation time)
- Widening `display_filter` / `should_display_room!` to bake in the `is_empty_direct_room_display_name` rule
- Server-side `m.direct` semantics, SSO flow, and mobile layout

## Completion Criteria

Scenario: display-flip predicate returns BecameHidden when a Joined room drops display eligibility
  Test: classify_joined_room_display_flip_becomes_hidden
  Given `old_should_display = true`
  And `new_should_display = false`
  When `classify_joined_room_display_flip` is called
  Then the returned value equals `JoinedRoomDisplayFlip::BecameHidden`

Scenario: display-flip predicate returns BecameDisplayable when a Joined room regains display eligibility
  Test: classify_joined_room_display_flip_becomes_displayable
  Given `old_should_display = false`
  And `new_should_display = true`
  When `classify_joined_room_display_flip` is called
  Then the returned value equals `JoinedRoomDisplayFlip::BecameDisplayable`

Scenario: display-flip predicate returns NoDisplayChange when eligibility is stable
  Test: classify_joined_room_display_flip_no_change_when_stable
  Given `old_should_display` and `new_should_display` are equal
  When `classify_joined_room_display_flip` is called with the following inputs:
    | old   | new   |
    | true  | true  |
    | false | false |
  Then the returned value equals `JoinedRoomDisplayFlip::NoDisplayChange`

Scenario: RoomsListUpdate::UnhideRoom clears hidden flag and restores a displayable direct room
  Test: manual_test_rooms_list_unhide_room_restores_direct_room
  Given a `RoomsList` whose `hidden_rooms` contains `room_id R`
  And `all_joined_rooms[R].is_direct == true`
  And `all_joined_rooms[R]` would satisfy `should_display_room!` if not hidden
  When the handler processes `RoomsListUpdate::UnhideRoom { room_id: R }`
  Then `self.hidden_rooms` does not contain `R`
  And `self.displayed_direct_rooms` contains `R`
  And `self.displayed_regular_rooms` does not contain `R`

Scenario: RoomsListUpdate::UnhideRoom on an unknown room is a no-op
  Test: manual_test_rooms_list_unhide_room_unknown_room_is_noop
  Given a `RoomsList` whose `all_joined_rooms` does not contain `room_id R`
  When the handler processes `RoomsListUpdate::UnhideRoom { room_id: R }`
  Then no panic occurs
  And `self.displayed_direct_rooms` is unchanged
  And `self.displayed_regular_rooms` is unchanged

Scenario: A first-sync stale-empty-direct DM is tracked and hidden, then restored when its name resolves
  Test: manual_test_first_sync_stale_empty_direct_dm_is_restorable
  Given the first sliding-sync snapshot for room `R` is `(state=Joined, is_direct=true, display_name=Empty)`
  When `add_new_room` handles `R`
  Then `ALL_JOINED_ROOMS` contains `R`
  And `rooms_list.all_joined_rooms` contains `R`
  And `rooms_list.hidden_rooms` contains `R`
  And neither `displayed_direct_rooms` nor `displayed_regular_rooms` contains `R`
  When a subsequent sync resolves `R` to `display_name=Calculated("peer")`
  Then `update_room` emits `RoomsListUpdate::UnhideRoom { room_id: R }`
  And `rooms_list.displayed_direct_rooms` contains `R`

Scenario: A freshly-created DM with an Empty display name no longer churns JoinedRoomDetails on is_direct flip
  Test: manual_test_dm_rejoin_timeline_not_blank
  Given the user has previously left a DM with `@octosbot`
  And the user creates a new DM with `@octosbot` through the People tab
  And the created room lands locally with `is_direct=false, display_name=Empty` on the first sync
  When the next sync update flips the room to `is_direct=true, display_name=Empty`
  Then `/tmp/robrix_debug.log` contains exactly one `Adding new joined room !<id>, name:` line for that room id
  And `/tmp/robrix_debug.log` contains zero `Dropping JoinedRoomDetails for room !<id>` lines for that room id
  And the open RoomScreen's timeline receives subsequent `TimelineUpdate::*` without reconnecting

Scenario: cargo build and existing sliding_sync unit tests still pass
  Test: cargo_build_and_matrix_request_tests_green
  When the developer runs `cargo build`
  And the developer runs `cargo test matrix_request_tests -- --nocapture`
  Then both complete with exit code `0`
