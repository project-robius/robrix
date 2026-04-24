# DM Rejoin — Stop JoinedRoomDetails Churn On Display Flip — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Stop `src/sliding_sync.rs::update_room` from tearing down `JoinedRoomDetails` when a still-Joined room's display eligibility flips, so the open `RoomScreen`'s singleton timeline receiver survives the `(is_direct=true, display_name=Empty)` transient state of a freshly-created DM.

**Architecture:** Replace the two early-return branches that call `remove_room` / `add_new_room` on display flips with `RoomsListUpdate::HideRoom` / new `RoomsListUpdate::UnhideRoom` signals that only toggle visibility in the sidebar, and let the normal `UpdateRoomName` / `UpdateIsDirect` / avatar / power-level updates flow through in the same `update_room` invocation.

**Tech Stack:** Rust, matrix-sdk sliding-sync, crossbeam_channel, Makepad 2.0 (unchanged in this task).

---

## File Structure

- **Modify** `src/sliding_sync.rs`
  - Add `JoinedRoomDisplayFlip` enum + `classify_joined_room_display_flip` helper (near the existing `should_display_joined_room_entry` at line 1141).
  - Delete `should_reuse_existing_joined_room_details` (line 1151) and its two unit tests (lines 1267–1282).
  - Rewrite `update_room` lines 4819–4825 to emit `HideRoom` / `UnhideRoom` and fall through.
  - Revert `add_new_room` (lines 5081–5142) back to a single unconditional timeline-build path (remove the reuse branch).
- **Modify** `src/home/rooms_list.rs`
  - Add `UnhideRoom { room_id: OwnedRoomId }` variant to `RoomsListUpdate` (near `HideRoom`, line ~180).
  - Add a handler arm for `UnhideRoom` that clears the hidden flag and re-evaluates displayed lists (next to the existing `HideRoom` arm at line 886).
- **Create** `specs/task-dm-joined-room-details-churn.spec.md` — already written.
- **Test:** unit tests live in the existing `#[cfg(test)] mod matrix_request_tests` block at `src/sliding_sync.rs` and a fresh `#[cfg(test)] mod` in `src/home/rooms_list.rs` (or a separate module beside it if needed).

---

### Task 1: Add UnhideRoom variant + handler in rooms_list (TDD)

**Files:**
- Modify: `src/home/rooms_list.rs` (add variant near line 180, add handler arm near line 886)

- [ ] **Step 1: Read the current `RoomsListUpdate` variant list and the `HideRoom` handler**

Run: inspect `src/home/rooms_list.rs` around lines 148–250 (enum) and lines 886–935 (HideRoom arm). Note the exact field layout of `HideRoom` and how it removes from `displayed_direct_rooms` / `displayed_regular_rooms`.

- [ ] **Step 2: Write a failing test that asserts UnhideRoom restores a hidden direct room**

Add at the end of `src/home/rooms_list.rs` (after the last `impl` block):

```rust
#[cfg(test)]
mod unhide_room_tests {
    use super::*;

    // Sketch: we cannot instantiate RoomsList fully (it's a Makepad widget),
    // so we test the pure decision: after receiving UnhideRoom, hidden_rooms
    // no longer contains the id, and a room that satisfies the default
    // display_filter is pushed onto displayed_direct_rooms.
    //
    // If exercising the Widget handler requires a Cx, split the pure logic
    // into a free function `apply_unhide_room(&mut RoomsListState, room_id)`
    // during Task 2 and test that instead.

    #[test]
    fn unhide_room_enum_variant_exists() {
        // Compile-only smoke test: ensures we can construct the new variant.
        let _ = RoomsListUpdate::UnhideRoom {
            room_id: "!x:example.org".parse().unwrap(),
        };
    }
}
```

- [ ] **Step 3: Run the test — expect it to FAIL with "no variant named UnhideRoom"**

Run: `cargo test -p robrix unhide_room_enum_variant_exists -- --nocapture`
Expected: build error — `no variant named "UnhideRoom" on enum "RoomsListUpdate"`

- [ ] **Step 4: Add the `UnhideRoom` variant next to `HideRoom`**

Edit `src/home/rooms_list.rs` — inside `pub enum RoomsListUpdate { ... }` (just before or after the existing `HideRoom { room_id: OwnedRoomId },` line). Add:

```rust
    /// Clear an entry from `hidden_rooms` for `room_id`, then re-check
    /// `should_display_room!` and restore the room into `displayed_direct_rooms`
    /// or `displayed_regular_rooms` if it is newly eligible.
    ///
    /// Semantic dual of [`RoomsListUpdate::HideRoom`]. Used by
    /// `update_room` when a Joined room's display eligibility flips from
    /// hidden back to displayable without changing `RoomState`.
    UnhideRoom {
        room_id: OwnedRoomId,
    },
```

- [ ] **Step 5: Re-run the compile-smoke test — expect PASS**

Run: `cargo test -p robrix unhide_room_enum_variant_exists -- --nocapture`
Expected: `test result: ok. 1 passed`

- [ ] **Step 6: Add the match arm for `UnhideRoom`**

Edit the `handle_event` dispatch block (same file, search for `RoomsListUpdate::HideRoom { room_id } =>`). Insert a new arm immediately after it:

```rust
                RoomsListUpdate::UnhideRoom { room_id } => {
                    let was_hidden = self.hidden_rooms.remove(&room_id);
                    if !was_hidden {
                        // Already not hidden — nothing to do.
                        continue;
                    }
                    if let Some(room) = self.all_joined_rooms.get(&room_id) {
                        let is_direct = room.is_direct;
                        let should_display = should_display_room!(self, &room_id, room);
                        if should_display {
                            let displayed_list = if is_direct {
                                &mut self.displayed_direct_rooms
                            } else {
                                &mut self.displayed_regular_rooms
                            };
                            if !displayed_list.contains(&room_id) {
                                displayed_list.push(room_id);
                            }
                        }
                    }
                    // Invited-room unhide is not required: HideRoom currently
                    // never fires for invited rooms from sliding_sync.
                }
```

- [ ] **Step 7: `cargo build` — expect clean build**

Run: `cargo build`
Expected: compiles with zero new warnings related to `UnhideRoom`.

- [ ] **Step 8: Commit**

Run:
```
git add src/home/rooms_list.rs
git commit -m "rooms_list: add UnhideRoom update variant and handler"
```

---

### Task 2: Extract `classify_joined_room_display_flip` pure helper (TDD)

**Files:**
- Modify: `src/sliding_sync.rs` — add helper near line 1141, add unit tests in the existing `mod matrix_request_tests` block near line 1225.

- [ ] **Step 1: Write the three failing tests**

Inside `mod matrix_request_tests` in `src/sliding_sync.rs`, append:

```rust
    #[test]
    fn classify_joined_room_display_flip_becomes_hidden() {
        assert_eq!(
            classify_joined_room_display_flip(true, false),
            JoinedRoomDisplayFlip::BecameHidden
        );
    }

    #[test]
    fn classify_joined_room_display_flip_becomes_displayable() {
        assert_eq!(
            classify_joined_room_display_flip(false, true),
            JoinedRoomDisplayFlip::BecameDisplayable
        );
    }

    #[test]
    fn classify_joined_room_display_flip_no_change_when_stable() {
        assert_eq!(
            classify_joined_room_display_flip(true, true),
            JoinedRoomDisplayFlip::NoDisplayChange
        );
        assert_eq!(
            classify_joined_room_display_flip(false, false),
            JoinedRoomDisplayFlip::NoDisplayChange
        );
    }
```

- [ ] **Step 2: Run the tests — expect FAIL with "not found"**

Run: `cargo test classify_joined_room_display_flip -- --nocapture`
Expected: compile error — `cannot find function "classify_joined_room_display_flip"` and `cannot find type "JoinedRoomDisplayFlip"`.

- [ ] **Step 3: Implement the helper next to `should_display_joined_room_entry`**

Edit `src/sliding_sync.rs` immediately after `should_display_joined_room_entry` (current line 1149):

```rust
/// Semantic result of comparing a Joined room's display eligibility between
/// two successive sliding-sync snapshots, while the room stays `RoomState::Joined`.
///
/// Used by `update_room` to decide whether the visibility flip should hide
/// or restore the room in the sidebar *without* destroying its `JoinedRoomDetails`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum JoinedRoomDisplayFlip {
    /// The room became eligible for display (e.g., Empty direct DM finally got
    /// a calculated name).
    BecameDisplayable,
    /// The room lost display eligibility (e.g., `is_direct` flipped true while
    /// display_name was still `Empty`).
    BecameHidden,
    /// No change in display eligibility; the caller should perform no
    /// visibility-only side effect.
    NoDisplayChange,
}

fn classify_joined_room_display_flip(
    old_should_display: bool,
    new_should_display: bool,
) -> JoinedRoomDisplayFlip {
    match (old_should_display, new_should_display) {
        (true, false) => JoinedRoomDisplayFlip::BecameHidden,
        (false, true) => JoinedRoomDisplayFlip::BecameDisplayable,
        _ => JoinedRoomDisplayFlip::NoDisplayChange,
    }
}
```

- [ ] **Step 4: Re-run — expect all three PASS**

Run: `cargo test classify_joined_room_display_flip -- --nocapture`
Expected:
```
test matrix_request_tests::classify_joined_room_display_flip_becomes_hidden ... ok
test matrix_request_tests::classify_joined_room_display_flip_becomes_displayable ... ok
test matrix_request_tests::classify_joined_room_display_flip_no_change_when_stable ... ok
```

- [ ] **Step 5: Commit**

Run:
```
git add src/sliding_sync.rs
git commit -m "sliding_sync: add classify_joined_room_display_flip helper + tests"
```

---

### Task 3: Rewrite `update_room` display-flip branches to use Hide/Unhide signals

**Files:**
- Modify: `src/sliding_sync.rs:4807–4825` (`update_room` top)

- [ ] **Step 1: Read the current code at `src/sliding_sync.rs:4807–4825`**

You will be replacing this block:
```rust
    let new_room_id = new_room.room_id.clone();
    if old_room.room_id == new_room_id {
        let old_should_display = should_display_joined_room_entry(
            old_room.state,
            old_room.is_direct,
            old_room.display_name.as_ref(),
        );
        let new_should_display = should_display_joined_room_entry(
            new_room.state,
            new_room.is_direct,
            new_room.display_name.as_ref(),
        );
        if old_should_display && !new_should_display {
            remove_room(new_room);
            return Ok(());
        }
        if !old_should_display && new_should_display {
            return add_new_room(new_room, room_list_service, true).await;
        }
```

- [ ] **Step 2: Apply the replacement**

Use `Edit` with the old block above and the following new block:
```rust
    let new_room_id = new_room.room_id.clone();
    if old_room.room_id == new_room_id {
        // Same-room update. Only treat hard state transitions (Left, Banned)
        // as producers of remove_room/add_new_room — they are handled below
        // in the explicit state-transition block. A pure display-eligibility
        // flip while the room stays Joined must NOT destroy its
        // JoinedRoomDetails (otherwise an open RoomScreen's singleton
        // timeline receiver is orphaned and the pane goes blank forever).
        //
        // See specs/task-dm-joined-room-details-churn.spec.md.
        let old_should_display = should_display_joined_room_entry(
            old_room.state,
            old_room.is_direct,
            old_room.display_name.as_ref(),
        );
        let new_should_display = should_display_joined_room_entry(
            new_room.state,
            new_room.is_direct,
            new_room.display_name.as_ref(),
        );
        match classify_joined_room_display_flip(old_should_display, new_should_display) {
            JoinedRoomDisplayFlip::BecameHidden => {
                log!(
                    "[dm-debug] Hiding joined room {} from rooms list on display flip (is_direct={}, display_name={:?})",
                    new_room_id, new_room.is_direct, new_room.display_name,
                );
                enqueue_rooms_list_update(RoomsListUpdate::HideRoom {
                    room_id: new_room_id.clone(),
                });
            }
            JoinedRoomDisplayFlip::BecameDisplayable => {
                log!(
                    "[dm-debug] Unhiding joined room {} in rooms list on display flip (is_direct={}, display_name={:?})",
                    new_room_id, new_room.is_direct, new_room.display_name,
                );
                enqueue_rooms_list_update(RoomsListUpdate::UnhideRoom {
                    room_id: new_room_id.clone(),
                });
            }
            JoinedRoomDisplayFlip::NoDisplayChange => {}
        }
```

Note: **do not** add a `return Ok(());` — processing must continue into the existing state-transition block and the `UpdateRoomName` / `UpdateIsDirect` / avatar / power-level branches further down.

- [ ] **Step 3: `cargo build` — expect clean build**

Run: `cargo build`
Expected: compiles. If there's a warning about `classify_joined_room_display_flip` being unused in non-test config, it is now used → warning should not appear.

- [ ] **Step 4: Run all existing sliding_sync tests**

Run: `cargo test -p robrix --lib matrix_request_tests -- --nocapture`
Expected: all green, including the prior:
- `should_display_joined_room_entry_hides_empty_direct_dm`
- `should_display_joined_room_entry_keeps_non_empty_or_non_direct_rooms`
- `is_active_dm_room_state_only_joined_is_reusable`
- `dm_target_matching_configured_bot_auto_binds_new_room`
- `ordinary_dm_target_does_not_auto_bind_new_room`
- `choose_reusable_dm_candidate_*`
- plus the three new `classify_joined_room_display_flip_*` tests from Task 2.

- [ ] **Step 5: Commit**

Run:
```
git add src/sliding_sync.rs
git commit -m "sliding_sync: keep JoinedRoomDetails alive across display-flip updates"
```

---

### Task 4: Retire the now-dead `should_reuse_existing_joined_room_details` guard inside `add_new_room`

**Files:**
- Modify: `src/sliding_sync.rs` — remove dead predicate at lines 1151–1156, delete its two unit tests at lines 1267–1282, restore `add_new_room` lines 5084–5142 to its pre-Codex-reuse-fix single-branch form.

- [ ] **Step 1: Read `src/sliding_sync.rs:5081–5142`**

Confirm the current if/else structure introduced by the previous attempt: the `if should_reuse_existing_joined_room_details(...)` branch + matching `else`.

- [ ] **Step 2: Replace the if/else with the unconditional timeline-build body**

Use `Edit` to replace:

```rust
    let has_existing_joined_room_details = ALL_JOINED_ROOMS.lock().unwrap().contains_key(&new_room.room_id);
    if should_reuse_existing_joined_room_details(new_room.state, has_existing_joined_room_details) {
        // Newly-created DMs often get a second joined-room snapshot moments later while
        // the homeserver finishes resolving `is_direct` and the calculated room name.
        // Replacing the JoinedRoomDetails here would orphan the open RoomScreen's
        // singleton timeline receiver and leave the timeline pane blank.
        log!(
            "[dm-debug] add_new_room reusing existing JoinedRoomDetails for room {} while refreshing metadata: name={:?} is_direct={}",
            new_room.room_id,
            new_room.display_name,
            new_room.is_direct,
        );
    } else {
        let timeline = Arc::new(
            new_room.room.timeline_builder()
                .with_focus(TimelineFocus::Live {
                    // we show threads as separate timelines in their own RoomScreen
                    hide_threaded_events: true,
                })
                .track_read_marker_and_receipts(TimelineReadReceiptTracking::AllEvents)
                .build()
                .await
                .map_err(|e| anyhow::anyhow!("BUG: Failed to build timeline for room {}: {e}", new_room.room_id))?,
        );
        let (timeline_update_sender, timeline_update_receiver) = crossbeam_channel::unbounded();

        let (request_sender, request_receiver) = watch::channel(Vec::new());
        let timeline_subscriber_handler_task = Handle::current().spawn(timeline_subscriber_handler(
            new_room.room.clone(),
            timeline.clone(),
            timeline_update_sender.clone(),
            request_receiver,
            None,
        ));

        // We need to add the room to the `ALL_JOINED_ROOMS` list before we can send
        // an `AddJoinedRoom` update to the RoomsList widget, because that widget might
        // immediately issue a `MatrixRequest` that relies on that room being in `ALL_JOINED_ROOMS`.
        log!("Adding new joined room {}, name: {:?}", new_room.room_id, new_room.display_name);
        ALL_JOINED_ROOMS.lock().unwrap().insert(
            new_room.room_id.clone(),
            JoinedRoomDetails {
                room_id: new_room.room_id.clone(),
                main_timeline: PerTimelineDetails {
                    timeline,
                    timeline_singleton_endpoints: Some((timeline_update_receiver, request_sender)),
                    timeline_update_sender,
                    timeline_subscriber_handler_task,
                },
                thread_timelines: HashMap::new(),
                pending_thread_timelines: HashSet::new(),
                typing_notice_subscriber: None,
                pinned_events_subscriber: None,
            },
        );
    }
```

with:

```rust
    let timeline = Arc::new(
        new_room.room.timeline_builder()
            .with_focus(TimelineFocus::Live {
                // we show threads as separate timelines in their own RoomScreen
                hide_threaded_events: true,
            })
            .track_read_marker_and_receipts(TimelineReadReceiptTracking::AllEvents)
            .build()
            .await
            .map_err(|e| anyhow::anyhow!("BUG: Failed to build timeline for room {}: {e}", new_room.room_id))?,
    );
    let (timeline_update_sender, timeline_update_receiver) = crossbeam_channel::unbounded();

    let (request_sender, request_receiver) = watch::channel(Vec::new());
    let timeline_subscriber_handler_task = Handle::current().spawn(timeline_subscriber_handler(
        new_room.room.clone(),
        timeline.clone(),
        timeline_update_sender.clone(),
        request_receiver,
        None,
    ));

    // We need to add the room to the `ALL_JOINED_ROOMS` list before we can send
    // an `AddJoinedRoom` update to the RoomsList widget, because that widget might
    // immediately issue a `MatrixRequest` that relies on that room being in `ALL_JOINED_ROOMS`.
    log!("Adding new joined room {}, name: {:?}", new_room.room_id, new_room.display_name);
    ALL_JOINED_ROOMS.lock().unwrap().insert(
        new_room.room_id.clone(),
        JoinedRoomDetails {
            room_id: new_room.room_id.clone(),
            main_timeline: PerTimelineDetails {
                timeline,
                timeline_singleton_endpoints: Some((timeline_update_receiver, request_sender)),
                timeline_update_sender,
                timeline_subscriber_handler_task,
            },
            thread_timelines: HashMap::new(),
            pending_thread_timelines: HashSet::new(),
            typing_notice_subscriber: None,
            pinned_events_subscriber: None,
        },
    );
```

- [ ] **Step 3: Delete the dead predicate**

Use `Edit` to remove the block at lines 1151–1156:
```rust
fn should_reuse_existing_joined_room_details(
    room_state: RoomState,
    has_existing_joined_room_details: bool,
) -> bool {
    room_state == RoomState::Joined && has_existing_joined_room_details
}
```

- [ ] **Step 4: Delete the two dead unit tests**

Use `Edit` to remove the test pair (inside `mod matrix_request_tests`):

```rust
    #[test]
    fn should_reuse_existing_joined_room_details_for_joined_room_readd() {
        assert!(should_reuse_existing_joined_room_details(
            RoomState::Joined,
            true,
        ));
    }

    #[test]
    fn should_not_reuse_existing_joined_room_details_for_first_add() {
        assert!(!should_reuse_existing_joined_room_details(
            RoomState::Joined,
            false,
        ));
    }
```

- [ ] **Step 5: `cargo build` — expect clean build**

Run: `cargo build`
Expected: no "unused" warnings for `should_reuse_existing_joined_room_details` (it is fully removed).

- [ ] **Step 6: Run full matrix_request_tests suite**

Run: `cargo test -p robrix --lib matrix_request_tests -- --nocapture`
Expected: green; the two removed tests are no longer listed.

- [ ] **Step 7: Commit**

Run:
```
git add src/sliding_sync.rs
git commit -m "sliding_sync: drop dead reuse-guard now that update_room preserves JoinedRoomDetails"
```

---

### Task 5: Lint the spec and verify plan coverage

**Files:**
- Read: `specs/task-dm-joined-room-details-churn.spec.md`

- [ ] **Step 1: Confirm `agent-spec` CLI is available**

Run: `command -v agent-spec || cargo install agent-spec`

- [ ] **Step 2: Parse the spec**

Run: `agent-spec parse specs/task-dm-joined-room-details-churn.spec.md`
Expected: non-zero scenarios listed under Completion Criteria.

- [ ] **Step 3: Lint the spec**

Run: `agent-spec lint specs/task-dm-joined-room-details-churn.spec.md --min-score 0.7`
Expected: score >= 0.7. If below, fix the reported warnings in the spec file and re-run.

- [ ] **Step 4: Commit**

Run:
```
git add specs/task-dm-joined-room-details-churn.spec.md docs/superpowers/plans/2026-04-16-dm-joined-room-details-churn-plan.md
git commit -m "spec: record DM rejoin JoinedRoomDetails churn fix"
```

---

### Task 6: Manual verification — do NOT commit or push until the user has tested

**Files:**
- Read: `/tmp/robrix_debug.log` after the test run.

- [ ] **Step 1: Clean previous log**

Run: `: > /tmp/robrix_debug.log`

- [ ] **Step 2: Run the app**

Run: `cargo run`

- [ ] **Step 3: Reproduce the exact flow**

1. Log in as the test user.
2. Open an existing DM with `@octosbot`, `Leave` it.
3. Re-open People, search `octosbot`, confirm the "Create New Direct Message" modal, confirm create.
4. Send "hello" in the new DM.

- [ ] **Step 4: Verify log assertions**

Run: `grep -cE "Adding new joined room !<new-id>" /tmp/robrix_debug.log`
Expected: `1` (exactly one add, no re-add).

Run: `grep -cE "Dropping JoinedRoomDetails for room !<new-id>" /tmp/robrix_debug.log`
Expected: `0` until the user actually Leaves the room again.

Run: `grep -cE "\[dm-debug\] (Hiding|Unhiding) joined room !<new-id>" /tmp/robrix_debug.log`
Expected: `>= 1` for at least one `Hiding` during the Empty-direct window, followed by an `Unhiding` once the server reports the calculated name.

- [ ] **Step 5: Verify visual behavior in the app**

- Right pane timeline of the new DM shows the user's "hello" plus any octosbot replies (no longer stuck on "Loading earlier messages…").
- Left sidebar briefly omits the Empty direct DM, then shows it as `octosbot` once the homeserver resolves the name. It does not show a duplicate.

- [ ] **Step 6: Hand off to user**

Post a short report to the user including:
- Exact new room id from the reproduction.
- The four log assertion counts from Step 4.
- A note that the Dock tab label may still show `Room ID !...` (out-of-scope follow-up — see `Out of Scope` in the spec).
- Do NOT commit `Co-Authored-By: Claude` lines; do NOT auto-merge. Wait for explicit user approval before creating a PR.

---

## Self-Review Checklist

- [x] Every spec Completion Criteria scenario maps to a task:
  - `classify_joined_room_display_flip_*` × 3 → Task 2
  - `rooms_list_unhide_room_restores_direct_room` → Task 1 (handler body)
  - `rooms_list_unhide_room_unknown_room_is_noop` → Task 1 (handler body, early-continue)
  - `manual_test_dm_rejoin_timeline_not_blank` → Task 6
  - `cargo_build_and_matrix_request_tests_green` → Tasks 3 & 4
- [x] No `TBD` / `TODO` / placeholder steps.
- [x] Every step that changes code has the exact code block inline.
- [x] Type names are consistent across tasks: `JoinedRoomDisplayFlip` and its variants `BecameDisplayable` / `BecameHidden` / `NoDisplayChange` appear identically in the spec, tests, helper, and `update_room` match arm.
- [x] `RoomsListUpdate::UnhideRoom { room_id: OwnedRoomId }` field shape is identical in Task 1 Step 4, Task 1 Step 6, and Task 3 Step 2.

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-04-16-dm-joined-room-details-churn-plan.md`.

Execution choices:
1. **Inline Execution (recommended for this scope)** — 6 focused tasks, each < 10 minutes, easier to roll back the single-repo change.
2. **Subagent-Driven** — one subagent per task, fresh context. Good if the human wants a second-eye review between Tasks 3 and 4 before touching `add_new_room`.
