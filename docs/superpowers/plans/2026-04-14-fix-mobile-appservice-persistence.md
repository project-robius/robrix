# Fix Mobile App Service Binding Persistence Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix issue #94 by ensuring `RestoreAppStateFromPersistentState` is dispatched whenever a successfully loaded `AppState` contains meaningful persisted content, so non-dock persisted fields (`selected_room`, `bot_settings`, `app_language`, `translation`) survive force-quit + relaunch on mobile while fresh installs remain a no-op.

**Architecture:** The persistence save path is already correct — the bug is primarily a single `if`-guard at `src/sliding_sync.rs::handle_load_app_state` that scopes the entire restore dispatch behind non-empty dock state. Replace that guard with a content-aware predicate; the existing restore match arm in `src/app.rs` already handles empty-dock state correctly via full `AppState` replacement. Also keep the Settings page hydrated from `Scope<AppState>` so a page opened before the async restore action is processed updates once restored state arrives. Add serde round-trip tests, direct restore-gate tests, and UI hydrate predicate tests as regression guards.

**Tech Stack:** Rust, Makepad 2.0, matrix-sdk, serde/serde_json, tokio. No new dependencies.

**Spec:** `specs/task-fix-mobile-appservice-persistence.spec.md` (passes `agent-spec lint --min-score 0.7`; latest observed quality 93%).

**Commit policy:** CLAUDE.md forbids committing before the user has tested. Android manual verification passed on 2026-04-29. All intermediate work is left uncommitted; a single commit at the end bundles the spec + plan + implementation + test + issue update.

---

## File Structure

| Path | Action | Responsibility |
|---|---|---|
| `specs/task-fix-mobile-appservice-persistence.spec.md` | already written | Task Contract; the source of truth for "what done looks like" |
| `docs/superpowers/plans/2026-04-14-fix-mobile-appservice-persistence.md` | this file | Implementation plan for the engineer |
| `src/sliding_sync.rs` | modify | Replace the dock-state guard in `handle_load_app_state` with a content-aware restore gate, update log message, add gate tests |
| `src/app.rs` | modify (tests only) | Add regression unit test inside existing `#[cfg(test)] mod tests` block (near line 2570+). No production code change in app.rs. |
| `src/settings/bot_settings.rs` | modify | Re-hydrate visible Settings UI from restored `AppState`; add predicate tests |
| `issues/009-mobile-appservice-binding-not-persisted.md` | modify | Append "Fix Applied" section documenting the one-line fix and the regression test |

## Test Strategy

- **Unit test** (machine-verifiable): `test_app_state_roundtrip_preserves_bot_settings_with_empty_dock` — constructs an `AppState` with populated `bot_settings` and empty dock; asserts serde_json round-trip preserves all three App Service fields. Runs under `cargo test -p robrix`.
- **Manual test** (user verification): Android force-quit + relaunch per issue #94 reproduction steps. User owns this step.

## TDD Discipline

The unit test is written FIRST, confirmed to FAIL against the current guarded code (proving it guards against the right bug), then the production fix is applied, then the test confirms GREEN. Red → Green → Commit.

Actually, a subtle point: because the unit test operates on the `AppState` serde contract (not on `handle_load_app_state`), it would pass against the current buggy code too — serde is innocent; the bug is in the dispatch guard. So:
- The unit test is a **regression guard for the serde contract** — it protects against a future regression where someone adds `#[serde(skip)]` to `bot_settings` (which would silently break persistence the same way).
- The real "red test" is the **manual mobile reproduction** described in the spec.
- We still follow TDD order (test first) so the test exists before the code it guards.

---

## Task 1: Regression Unit Test for Serde Round-Trip

**Files:**
- Modify: `src/app.rs` — inside the existing `#[cfg(test)] mod tests` block (search for `mod tests` near line 2568 or `use super::{BotSettingsState, RoomBotBindingState, SavedDockState, SelectedRoom};` near line 2570)

- [ ] **Step 1.1: Locate the test module and existing imports**

Run: `grep -n "use super::" src/app.rs | head -5`

Expected output: a line like `2570:    use super::{BotSettingsState, RoomBotBindingState, SavedDockState, SelectedRoom};`. Note the exact line so the new test lands in the right scope.

- [ ] **Step 1.2: Extend the test module's `use super::` import**

Find the line `use super::{BotSettingsState, RoomBotBindingState, SavedDockState, SelectedRoom};` and add `AppState` to the imported items so the new test can construct an `AppState` directly.

Before:
```rust
use super::{BotSettingsState, RoomBotBindingState, SavedDockState, SelectedRoom};
```

After:
```rust
use super::{AppState, BotSettingsState, RoomBotBindingState, SavedDockState, SelectedRoom};
```

- [ ] **Step 1.3: Add the regression test at the end of the test module**

Locate the closing `}` of the `mod tests` block (the last `}` in the file that closes a `#[cfg(test)] mod tests {`). Insert the following test just before that closing brace.

```rust
    /// Regression test for issue #94: mobile app service binding must survive force-quit + relaunch.
    ///
    /// The production bug was in sliding_sync.rs's load-side guard, but this test protects
    /// the underlying serde contract: if a future change adds `#[serde(skip)]` to
    /// bot_settings (or reorders fields in a breaking way), this test fails before users hit
    /// the bug on mobile.
    #[test]
    fn test_app_state_roundtrip_preserves_bot_settings_with_empty_dock() {
        let mut state = AppState::default();
        state.bot_settings.enabled = true;
        state.bot_settings.botfather_user_id = "@octosbot:example.com".to_string();
        state.bot_settings.octos_service_url = "http://192.168.5.12:8010".to_string();
        assert!(state.saved_dock_state_home.open_rooms.is_empty(),
            "precondition: this test simulates the mobile / fresh-desktop case with empty dock");
        assert!(state.saved_dock_state_home.dock_items.is_empty(),
            "precondition: this test simulates the mobile / fresh-desktop case with empty dock");

        let serialized = serde_json::to_string(&state)
            .expect("AppState must serialize via serde_json");
        let deserialized: AppState = serde_json::from_str(&serialized)
            .expect("serialized AppState must deserialize back");

        assert!(deserialized.bot_settings.enabled,
            "bot_settings.enabled must survive the round-trip (issue #94 regression guard)");
        assert_eq!(deserialized.bot_settings.botfather_user_id, "@octosbot:example.com",
            "botfather_user_id must survive the round-trip (issue #94 regression guard)");
        assert_eq!(deserialized.bot_settings.octos_service_url, "http://192.168.5.12:8010",
            "octos_service_url must survive the round-trip (issue #94 regression guard)");
    }
```

- [ ] **Step 1.4: Confirm the test compiles and passes against current (buggy) code**

Run: `cargo test -p robrix --lib test_app_state_roundtrip_preserves_bot_settings_with_empty_dock -- --nocapture`

Expected: PASS. The serde layer is innocent — this test exists to catch future breakage, not to drive the current fix. Passing now is correct and expected.

If the test FAILS at this step: STOP. Either (a) `AppState::default()` does not exist and the test module needs `use super::AppState` re-checked, or (b) a `#[serde(skip)]` was silently added to `bot_settings` in the past — investigate before proceeding.

- [ ] **Step 1.5: Do NOT commit yet**

CLAUDE.md forbids committing before user testing. The test stays staged-in-worktree until Task 4.

---

## Task 2: Remove the Load-Side Guard in handle_load_app_state

**Files:**
- Modify: `src/sliding_sync.rs` — function `handle_load_app_state` (~lines 4958-4990)

- [ ] **Step 2.1: Read the current function in full to confirm the target lines**

Run: `grep -n "fn handle_load_app_state" src/sliding_sync.rs`

Expected: single hit at around line 4958. Confirm the match arm structure:
```rust
match load_app_state(&user_id).await {
    Ok(app_state) => {
        if !app_state.saved_dock_state_home.open_rooms.is_empty()
            && !app_state.saved_dock_state_home.dock_items.is_empty()
        {
            log!("Loaded room panel state from app data directory. Restoring now...");
            Cx::post_action(AppStateAction::RestoreAppStateFromPersistentState(Box::new(app_state)));
        }
    }
    Err(_e) => { ... }
}
```

- [ ] **Step 2.2: Replace the dock-only guard with a content-aware restore gate**

Replace this block:

```rust
            Ok(app_state) => {
                if !app_state.saved_dock_state_home.open_rooms.is_empty()
                    && !app_state.saved_dock_state_home.dock_items.is_empty()
                {
                    log!("Loaded room panel state from app data directory. Restoring now...");
                    Cx::post_action(AppStateAction::RestoreAppStateFromPersistentState(Box::new(app_state)));
                }
            }
```

With:

```rust
            Ok(app_state) => {
                if should_restore_loaded_app_state(&app_state) {
                    log!("Loaded app state from persistent storage. Restoring now...");
                    Cx::post_action(AppStateAction::RestoreAppStateFromPersistentState(Box::new(app_state)));
                }
            }
```

Add the helper above `handle_load_app_state`:

```rust
fn should_restore_loaded_app_state(app_state: &crate::app::AppState) -> bool {
    fn saved_dock_state_has_content(saved: &crate::app::SavedDockState) -> bool {
        !saved.open_rooms.is_empty()
            || !saved.dock_items.is_empty()
            || !saved.room_order.is_empty()
            || saved.selected_room.is_some()
    }

    app_state.selected_room.is_some()
        || saved_dock_state_has_content(&app_state.saved_dock_state_home)
        || app_state.saved_dock_state_per_space.values().any(saved_dock_state_has_content)
        || app_state.bot_settings != crate::app::BotSettingsState::default()
        || app_state.app_language != crate::i18n::AppLanguage::default()
        || app_state.translation != crate::room::translation::TranslationConfig::default()
}
```

Do not use the unconditional restore form below; it was rejected because `load_app_state` returns `Ok(AppState::default())` for fresh installs and corrupt-file fallback:

```rust
            Ok(app_state) => {
                log!("Loaded app state from persistent storage. Restoring now...");
                Cx::post_action(AppStateAction::RestoreAppStateFromPersistentState(Box::new(app_state)));
            }
```

Rationale: the restore match arm in `src/app.rs:1071-1095` already performs a full `AppState` replacement and dispatches `MainDesktopUiAction::LoadDockFromAppState` unconditionally. Empty dock state is already handled safely downstream, but the all-default state from a fresh install should remain a no-op.

- [ ] **Step 2.3: Confirm no other fields in the `Err` arm reference the removed guard symbols**

Run: `grep -n "saved_dock_state_home" src/sliding_sync.rs`

Expected: zero matches inside `handle_load_app_state` after the edit. If any remain in the `Err` arm or surrounding code, re-read and only keep references that are unrelated to the guard.

- [ ] **Step 2.4: `cargo build` to catch any compile errors from the edit**

Run: `cargo build -p robrix 2>&1 | tail -30`

Expected: clean build. If an unused-import warning fires for `AppStateAction` or similar, inspect — it likely means the edit collapsed the only remaining reference. Unlikely since the action is still dispatched.

- [ ] **Step 2.5: Re-run the regression unit test to confirm no side-effects**

Run: `cargo test -p robrix --lib test_app_state_roundtrip_preserves_bot_settings_with_empty_dock -- --nocapture`

Expected: PASS. The test is orthogonal to this change (it tests serde, not sliding_sync) — this re-run is a sanity check that the edit didn't break the test module's compilation.

- [ ] **Step 2.6: Do NOT commit yet**

Same reason as Task 1 — wait for user testing.

---

## Task 3: Verification Pipeline

**Files:** none modified — verification only.

- [ ] **Step 3.1: Full workspace build**

Run: `cargo build 2>&1 | tail -20`

Expected: clean build with no new warnings introduced by our edits.

- [ ] **Step 3.2: Full test suite for the robrix package**

Run: `cargo test -p robrix --lib 2>&1 | tail -40`

Expected: all tests PASS, including our new `test_app_state_roundtrip_preserves_bot_settings_with_empty_dock`. Look specifically for the summary line `test result: ok. N passed; 0 failed`.

If any test unrelated to our change FAILS, STOP — it may indicate pre-existing breakage on the branch. Run `git stash && cargo test -p robrix --lib` to confirm the test failed before our edits, then `git stash pop`. Report back rather than guessing.

- [ ] **Step 3.3: agent-spec lifecycle against the task spec**

Run:
```bash
agent-spec lifecycle specs/task-fix-mobile-appservice-persistence.spec.md \
  --code . \
  --change-scope worktree \
  --format json \
  --run-log-dir .agent-spec/runs 2>&1 | tail -60
```

Expected behavior per scenario:
- `test_app_state_roundtrip_preserves_bot_settings_with_empty_dock` → verdict `pass` (cargo test bound)
- Six `manual_test_*` scenarios → verdict `skip` (no bound test; they are manual)

`skip` is not a failure — it is a distinct verdict. The spec explicitly marks these with `Level: manual` to communicate that they need human verification.

- [ ] **Step 3.4: agent-spec guard (repo-wide safety check)**

Run: `agent-spec guard --spec-dir specs --code . --change-scope worktree 2>&1 | tail -30`

Expected: no spec reports a boundary violation or failed scenario for our change set. This confirms the edits stay within `src/sliding_sync.rs` and `src/app.rs`'s test module per the spec's `Allowed Changes`.

---

## Task 4: Update Issue Doc and Prepare for User Verification

**Files:**
- Modify: `issues/009-mobile-appservice-binding-not-persisted.md`

- [ ] **Step 4.1: Replace the `## Fix Applied` section**

Find the line `## Fix Applied` followed by `None yet.` in `issues/009-mobile-appservice-binding-not-persisted.md` and replace that two-line block with:

```markdown
## Fix Applied

**Root cause confirmed**: `src/sliding_sync.rs::handle_load_app_state` gated the entire `RestoreAppStateFromPersistentState` dispatch behind a non-empty dock-state check:

```rust
if !app_state.saved_dock_state_home.open_rooms.is_empty()
    && !app_state.saved_dock_state_home.dock_items.is_empty()
{ ... Cx::post_action(RestoreAppStateFromPersistentState ...) ... }
```

On mobile there is no dock, so every restart silently dropped the loaded `selected_room`, `bot_settings`, `app_language`, and `translation`. Desktop masked the bug because dock state is almost always non-empty after first run.

**Fix**: dispatch `RestoreAppStateFromPersistentState` whenever `load_app_state` succeeds with meaningful non-default persisted content. Pure default state from a fresh install remains a no-op. The restore match arm in `src/app.rs` already performs a full `AppState` replacement and dispatches `LoadDockFromAppState` — empty dock is safely handled downstream.

**Settings UI hydration**: do not depend on mobile app swipe-away / force-quit to save state; those lifecycle events are not guaranteed. App Service state is saved immediately on Save / Check Now / toggle. `BotSettings` also re-hydrates from `Scope<AppState>` when restored settings arrive after the page was already opened.

**Regression guard**: `src/app.rs` unit tests assert the serde contract, `src/sliding_sync.rs` unit tests assert the restore gate behavior directly, and `src/settings/bot_settings.rs` unit tests assert the UI hydrate predicate.

**Spec**: `specs/task-fix-mobile-appservice-persistence.spec.md` (agent-spec Task Contract, quality 93%).
```

- [x] **Step 4.2: Update Status header**

At the top of the issue file, status now says Android is verified and the fix is ready for review, with iOS called out as not separately verified.

- [ ] **Step 4.3: Stage the fix for user testing**

Do NOT run `git commit`. Present the work to the user with:
- Files changed (`git status` output)
- Exactly how to verify on Android:
  1. `cargo makepad android run -p robrix --release` to Android emulator/device
  2. Login, go to Settings → Labs → App Service
  3. Enable, fill both fields, Save → success popup + Reachable
  4. `adb shell am force-stop dev.makepad.robrix`
  5. Relaunch → expect the fields populated and Check Now → Reachable

- [x] **Step 4.4: Wait for user confirmation before committing**

Per CLAUDE.md feedback memory `feedback_no_co_authored_by`, the final commit message must omit the `Co-Authored-By: Claude` trailer — the project's commit-msg hook rejects it.

Per CLAUDE.md's "do NOT commit without user testing" rule, the final commit only runs after the user says it works on Android. User confirmed the Android force-quit + relaunch path works on 2026-04-29.

When the user approves, stage and commit all artifacts together:

```bash
git add \
  specs/task-fix-mobile-appservice-persistence.spec.md \
  docs/superpowers/plans/2026-04-14-fix-mobile-appservice-persistence.md \
  src/sliding_sync.rs \
  src/app.rs \
  issues/009-mobile-appservice-binding-not-persisted.md

git commit -m "fix(persistence): restore non-dock app state on relaunch (#94)

handle_load_app_state previously gated RestoreAppStateFromPersistentState
behind non-empty dock state, which silently dropped bot_settings,
app_language, and translation on every mobile relaunch. Unconditionally
dispatch the restore action when load_app_state succeeds; the restore
match arm already handles empty dock correctly.

Add serde round-trip unit test as a regression guard for future
#[serde(skip)] additions that could re-introduce the same failure mode.

Spec: specs/task-fix-mobile-appservice-persistence.spec.md
Plan: docs/superpowers/plans/2026-04-14-fix-mobile-appservice-persistence.md
Closes #94"
```

Do NOT push without the user's explicit instruction (see feedback_no_auto_merge).

---

## Self-Review Checklist

- [x] **Spec coverage**: every scenario in the spec has a task that implements it (Task 1 covers the unit-test scenario; Task 2 delivers the behavior the six manual scenarios verify; Task 3 runs `agent-spec lifecycle` to bind them).
- [x] **Placeholder scan**: no "TBD", no "implement later", no "add error handling as needed". Every code block shows exact before/after content.
- [x] **Type consistency**: `AppStateAction::RestoreAppStateFromPersistentState` and `Box::new(app_state)` are used identically across Task 2 and the restore match arm it relies on. The test imports `AppState, BotSettingsState, ...` and uses `.bot_settings.enabled` consistent with `src/app.rs:2061`.
- [x] **Commit policy respected**: one final commit at end-of-plan, gated on user Android verification, omitting `Co-Authored-By: Claude`.
- [x] **Boundaries respected**: edits live in `src/sliding_sync.rs` (production) and `src/app.rs` (tests only) plus doc/spec/issue updates — exactly matches the spec's `Allowed Changes`.
