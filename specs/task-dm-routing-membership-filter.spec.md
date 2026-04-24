---
spec: task
name: "DM Reopen After Leave — Hide Stale Empty DMs And Reuse Only Active Candidates"
inherits: project
tags: [bugfix, dm, sliding-sync, room-list]
estimate: 0.5d
---

## Intent

Fix the DM reopen flow after leaving a direct room with user X.

Before the fix, Robrix could:

- restore the old DM as `Empty Room` on startup, and
- reopen that stale room when the user searched X again from People.

The desired behavior is:

- stale empty direct rooms are not shown in the joined room list, and
- DM reopen only reuses a room when the target user is still an active member of
  that room.

## Constraints

- Keep the public shape of `MatrixRequest::OpenOrCreateDirectMessage` unchanged
- Keep the `DirectMessageRoomAction` enum variants unchanged
- Do not modify People-tab click handling in `src/profile/user_profile.rs`
- Do not modify matrix-sdk behavior or `m.direct` account-data semantics
- Do not introduce new dependencies
- Do not change Makepad DSL files

## Decisions

- Add a pure predicate `should_display_joined_room_entry(...)` that treats
  `Joined + direct + Empty/EmptyWas(...)` rooms as hidden
- Use the same stale-room classification in both:
  - room-list ingestion/update, and
  - DM reuse selection
- Replace direct trust in `client.get_dm_room()` with
  `find_reusable_direct_message_room(...)`, which scans joined rooms and only
  returns a candidate when:
  - the room is a direct room for the requested user,
  - the requested user's membership is still `Join` or `Invite`,
  - the room is not a stale empty direct room
- If multiple reusable candidates exist, pick the newest by latest event
  timestamp
- Keep the implementation local to `src/sliding_sync.rs`

## Boundaries

### Allowed Changes
- `src/sliding_sync.rs`
- `specs/task-dm-routing-membership-filter.spec.md`
- `issues/009-dm-routing-ignores-membership.md`

### Forbidden
- Do not modify matrix-sdk or `client.get_dm_room()` upstream behavior
- Do not modify unrelated `MatrixRequest::*` handlers
- Do not modify composer/send-path membership gating
- Do not modify dock-tab cleanup on leave
- Do not run `cargo fmt`
- Do not change Makepad DSL files

## Acceptance Criteria

Scenario: Startup does not restore a stale empty DM
  Test: manual_test_stale_empty_dm_hidden_on_startup
  Given the user previously left a DM with peer X
  And the local client still knows that room as a direct room candidate
  When Robrix starts and ingests joined rooms
  Then that stale empty direct room is not shown in the joined room list

Scenario: Reopening the same user after leave does not route back to the stale room
  Test: manual_test_reopen_dm_after_leave_avoids_stale_room
  Given the user previously chatted with peer X in a DM
  And the user left that DM
  When the user opens People, searches X, and starts a DM again
  Then Robrix does not route into the abandoned empty room
  And Robrix opens or creates a valid DM instead

Scenario: An active DM is still reused directly
  Test: manual_test_active_dm_reused_normally
  Given the user has an active DM with peer Y
  When the user opens People and starts a DM with Y
  Then Robrix navigates directly to the existing valid DM room

Scenario: Reuse candidate selection prefers a room where the target user is still active
  Test: choose_reusable_dm_candidate_prefers_room_where_target_is_still_active
  Given multiple joined direct-room candidates for the same target user
  And one candidate has target membership `Leave`
  And another candidate has target membership `Join`
  When the reusable candidate is selected
  Then the room with active target membership is chosen

Scenario: Reuse candidate selection rejects stale empty direct rooms
  Test: choose_reusable_dm_candidate_rejects_empty_direct_room
  Given a joined direct-room candidate whose display name is `EmptyWas("X")`
  And the target membership is still cached as `Join`
  When the reusable candidate is selected
  Then no reusable candidate is returned

Scenario: Joined-room visibility hides stale empty direct rooms
  Test: should_display_joined_room_entry_hides_empty_direct_dm
  Given a joined direct room whose display name is `Empty` or `EmptyWas(...)`
  When joined-room visibility is evaluated
  Then the room is treated as hidden

Scenario: Non-empty or non-direct rooms remain displayable
  Test: should_display_joined_room_entry_keeps_non_empty_or_non_direct_rooms
  Given a room that is either non-empty, non-direct, or non-joined
  When joined-room visibility is evaluated
  Then the room remains displayable

Scenario: Sending in the newly reopened DM succeeds
  Test: manual_test_reopened_dm_send_succeeds
  Given the user left a DM with peer X
  And the user reopened DM with X after the fix
  When the user sends a message in the reopened flow's resulting room
  Then messaging succeeds in that room
  And the stale empty room is not reused

## Out Of Scope

- Composer gating on membership inside already-open left/banned rooms
- Auto-closing dock tabs on local `/leave`
- Server-side cleanup of stale `m.direct` entries
- Unrelated Makepad startup/runtime property errors
