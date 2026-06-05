spec: task
name: "Mobile App Lifecycle Handling"
inherits: project
tags: [makepad, mobile, lifecycle, matrix, persistence]
---

## Intent

Bring Robrix2's app lifecycle handling in line with platform expectations on mobile and desktop. The app must persist runtime state at lifecycle boundaries, stop Matrix sync while backgrounded, resume Matrix sync when foregrounded, route graceful quit requests through Makepad's lifecycle events, and give macOS the Robrix application menu identity.

This task is a lifecycle infrastructure migration from Robrix-style behavior into Robrix2's Makepad 2.0 codebase. It does not redesign settings, login, logout, persistence restore, or business features beyond the lifecycle hooks named here.

## Constraints

- Use Makepad 2.0 APIs and syntax only: `script_mod!`, `#[derive(Script, ScriptHook, Widget)]`, `:=` named children, and `+:` property merges.
- Do not run `cargo fmt` or `rustfmt`.
- Do not add new Cargo dependencies.
- Async Matrix operations must continue to go through `submit_async_request(MatrixRequest::*)`; this task must not introduce raw UI-side tokio Matrix calls.
- Preserve Robrix2-local persistence behavior: `load_app_state` keeps returning `anyhow::Result<AppState>`, `skip_app_state_restore_once` / `take_skip_app_state_restore_once` stay intact, and `RestoreAppStateFromPersistentState(Box<AppState>)` remains boxed.

## Decisions

- Makepad runtime source: switch `makepad-widgets` and `makepad-code-editor` from `kevinaboos/makepad` branch `cargo_makepad_ndk_fix` to canonical `makepad/makepad` branch `dev`.
- macOS identity: keep `MAKEPAD_BUNDLE_IDENTIFIER = "rs.robius.robrix"` and add `MAKEPAD_BUNDLE_NAME = "Robrix"` using the existing `{ value, force = true }` style.
- App lifecycle subsystem: add private `AppLifecycle` state to `App`, move the old inline `Event::Shutdown` persistence logic into `handle_shutdown`, and route lifecycle events through `handle_lifecycle_event`.
- Lifecycle persistence chokepoint: `persist_runtime_state(reason)` writes window geometry, serializes current `AppState`, fingerprints user id + byte length + hash, skips identical consecutive lifecycle app-state writes, and writes bytes via `save_app_state_bytes` only when changed.
- Lifecycle app-state dedup scope: `App::persist_runtime_state` owns dedup for lifecycle-triggered saves. Direct `persistence::save_app_state(...)` call sites for user preference changes remain unchanged.
- Mobile background behavior: `Event::Background` persists runtime state and marks Matrix sync as not desired; `Event::Foreground` marks Matrix sync as desired again.
- Mobile active behavior: `Event::Pause` persists runtime state; `Event::Resume` marks Matrix sync as desired.
- Desktop close/quit behavior: `Event::WindowCloseRequested` for the main window persists runtime state; `Event::QuitRequested` persists runtime state before Makepad proceeds to shutdown; `Event::Shutdown` persists runtime state, stops sync with a 3-second timeout, and preserves the TSP wallet shutdown path behind the existing `tsp` feature.
- Sync lifecycle state machine: keep `SYNC_SERVICE_DESIRED_RUNNING`, `SYNC_SERVICE_ASSUMED_RUNNING`, and `SYNC_SERVICE_LIFECYCLE_LOCK` next to `SYNC_SERVICE`; lifecycle requests update desired state and reconcile under the async lock.
- Sync startup: initial login and account-switch startup store the new `Arc<SyncService>` in `SYNC_SERVICE` before reconciling desired state, instead of directly calling `sync_service.start().await`.
- Sync error handling: sync-service error restart clears `ASSUMED_RUNNING`, does not restart when desired state is stopped, and otherwise routes restart through lifecycle reconciliation.
- Persistence split: keep public `save_app_state(AppState, OwnedUserId)` signature, and add `serialize_app_state(&AppState)` plus `save_app_state_bytes(&[u8], &UserId)` for lifecycle dedup.
- Menu quit path: add a `WindowMenu` with a `quit` item named "Quit Robrix" bound to Cmd+Q, and change unrecoverable logout restart from `cx.quit()` to `cx.request_quit(QuitReason::App)`.

## Boundaries

### Allowed Changes
- .cargo/config.toml
- Cargo.toml
- Cargo.lock
- **/docs/superpowers/specs/2026-05-28-mobile-lifecycle-design.md
- **/specs/task-mobile-lifecycle.spec.md
- **/src/app.rs
- src/logout/logout_confirm_modal.rs
- src/persistence/app_state.rs
- src/sliding_sync.rs
- src/home/add_room.rs
- src/login/login_screen.rs
- src/settings/app_settings.rs
- src/settings/bot_settings.rs
- src/settings/settings_screen.rs
- src/settings/translation_settings.rs

### Forbidden
- Do not change `load_app_state` signature or rewrite `handle_load_app_state`.
- Do not remove `skip_app_state_restore_once` or `take_skip_app_state_restore_once`.
- Do not change `RestoreAppStateFromPersistentState(Box<AppState>)`.
- Do not change `MAKEPAD_BUNDLE_IDENTIFIER` away from `rs.robius.robrix`.
- Do not add new unit or integration tests for this ported lifecycle code.
- Do not run `cargo fmt` or `rustfmt`.

## Completion Criteria

Scenario: Makepad runtime is aligned with lifecycle APIs
  Test: cargo_build
  Level: integration
  Test Double: none; compile the real crate dependency graph
  Targets: Cargo.toml, Cargo.lock
  Given Robrix2 depends on Makepad widgets and code editor
  When the lifecycle migration is built
  Then `makepad-widgets` uses `https://github.com/makepad/makepad` branch `dev`
  And `makepad-code-editor` uses `https://github.com/makepad/makepad` branch `dev`
  And `cargo build` completes successfully

Scenario: macOS app menu identity is Robrix
  Test: manual_test_macos_menu_identity
  Level: manual
  Test Double: none; requires real macOS menu inspection
  Targets: .cargo/config.toml, src/app.rs
  Given the app named "Robrix" is launched on macOS
  When the application menu is shown
  Then the app menu identity is "Robrix"
  And the menu contains a "Quit Robrix" item bound to Cmd+Q
  And `MAKEPAD_BUNDLE_IDENTIFIER` remains `rs.robius.robrix`

Scenario: mobile background persists and stops Matrix sync
  Test: manual_test_mobile_background_persists_and_stops_sync
  Level: manual
  Test Double: none; requires real Makepad lifecycle event observation
  Targets: src/app.rs, src/sliding_sync.rs, src/persistence/app_state.rs
  Given a logged-in user has unsaved app state
  When Makepad emits `Event::Background`
  Then Robrix writes window geometry
  And Robrix writes the current app state through `persist_runtime_state`
  And Robrix sets Matrix sync desired state to stopped
  And lifecycle reconciliation stops the current `SyncService`

Scenario: mobile foreground resumes Matrix sync
  Test: manual_test_mobile_foreground_resumes_sync
  Level: manual
  Test Double: none; requires real Makepad lifecycle event observation
  Targets: src/app.rs, src/sliding_sync.rs
  Given Matrix sync desired state was stopped by a background lifecycle event
  When Makepad emits `Event::Foreground`
  Then Robrix sets Matrix sync desired state to running
  And lifecycle reconciliation starts the current `SyncService`

Scenario: pause persists state without requiring shutdown
  Test: manual_test_mobile_pause_persists_state
  Level: manual
  Test Double: none; requires real Makepad lifecycle event observation
  Targets: src/app.rs, src/persistence/app_state.rs
  Given a logged-in user has unsaved app state
  When Makepad emits `Event::Pause`
  Then Robrix writes window geometry
  And Robrix writes the current app state through `persist_runtime_state`
  And the app does not rely on a later `Event::Shutdown` to preserve that state

Scenario: main window close persists before the window goes away
  Test: manual_test_main_window_close_persists_state
  Level: manual
  Test Double: none; requires real desktop window close event
  Targets: src/app.rs, src/persistence/app_state.rs
  Given a logged-in desktop user has unsaved app state
  When Makepad emits `Event::WindowCloseRequested` for `main_window`
  Then Robrix writes window geometry
  And Robrix writes the current app state through `persist_runtime_state`

Scenario: graceful quit routes through lifecycle persistence
  Test: manual_test_quit_request_persists_before_exit
  Level: manual
  Test Double: none; requires real OS/menu/signal quit paths
  Targets: src/app.rs, src/logout/logout_confirm_modal.rs
  Given a logged-in user has unsaved app state
  When the OS, Cmd+Q, terminal Ctrl+C, or unrecoverable logout restart requests a graceful quit
  Then Robrix handles `Event::QuitRequested`
  And Robrix writes runtime state before process exit continues
  And the logout restart path calls `cx.request_quit(QuitReason::App)` rather than direct `cx.quit()`

Scenario: shutdown stops sync and preserves TSP wallet path
  Test: manual_test_shutdown_stops_sync_with_timeout
  Level: manual
  Test Double: none; requires real Makepad shutdown event and Matrix sync service
  Targets: src/app.rs, src/sliding_sync.rs
  Given Robrix is shutting down
  When Makepad emits `Event::Shutdown`
  Then `handle_shutdown` runs at most once
  And Robrix persists runtime state
  And Robrix calls `stop_sync_service_for_shutdown` with a 3-second timeout
  And the existing TSP close-and-serialize path remains guarded by the `tsp` feature

Scenario: identical lifecycle app-state saves are deduplicated
  Test: manual_test_lifecycle_app_state_save_dedup
  Level: manual
  Test Double: filesystem observation of `latest_app_state.json` and lifecycle logs
  Targets: src/app.rs, src/persistence/app_state.rs
  Given a logged-in user triggers two lifecycle save events in one session without changing `AppState`
  When both events call `persist_runtime_state`
  Then the first lifecycle save writes `latest_app_state.json`
  And the second lifecycle save fingerprints identical user id, byte length, and hash
  And the second lifecycle save skips writing app-state JSON again
  And this does not change direct `save_app_state` behavior for preference save paths

Scenario: lifecycle dedup does not replace explicit preference saves
  Test: manual_test_lifecycle_dedup_scope_excludes_preference_saves
  Level: manual/code review
  Test Double: source inspection plus manual preference save smoke
  Targets: src/app.rs, src/persistence/app_state.rs, src/settings/app_settings.rs, src/settings/bot_settings.rs, src/settings/translation_settings.rs
  Given lifecycle dedup state is stored in `AppLifecycle.last_app_state_save`
  When a lifecycle event calls `persist_runtime_state`
  Then identical consecutive lifecycle app-state bytes are skipped
  When a settings, bot-settings, language, or translation action explicitly calls `persistence::save_app_state`
  Then that explicit preference save still writes through `save_app_state`
  And no global dedup cache in `src/persistence/app_state.rs` changes non-lifecycle save behavior

Scenario: sync error does not restart while lifecycle wants stopped
  Test: manual_test_sync_error_respects_lifecycle_stopped_state
  Level: manual
  Test Double: Matrix sync error observation or source inspection of subscriber branch
  Targets: src/sliding_sync.rs
  Given lifecycle desired state is stopped because the app is backgrounded
  When the sync-service state subscriber receives a non-token sync error
  Then it clears `SYNC_SERVICE_ASSUMED_RUNNING`
  And it does not restart the sync service
  And it leaves restart to a later foreground or resume lifecycle event

Scenario: Robrix2-local persistence restore behavior is preserved
  Test: manual_test_persistence_restore_local_behavior_preserved
  Level: manual
  Test Double: source inspection of local restore APIs and action payload shape
  Targets: src/persistence/app_state.rs, src/sliding_sync.rs, src/app.rs
  Given Robrix2 has existing local persistence restore behavior
  When the lifecycle migration is applied
  Then `load_app_state` still returns `anyhow::Result<AppState>`
  And `handle_load_app_state` keeps its broad restore decision behavior
  And `skip_app_state_restore_once` / `take_skip_app_state_restore_once` remain available
  And `RestoreAppStateFromPersistentState(Box<AppState>)` remains boxed

Scenario: source diff has no whitespace errors
  Test: git_diff_check
  Level: integration
  Test Double: none; run git whitespace checker on real diff
  Targets: all changed files
  Given lifecycle migration changes are ready for review
  When `git diff --check origin/main...HEAD` is run
  Then no trailing whitespace or whitespace error is reported

## Out of Scope

- Notification handoff while the app is backgrounded.
- Mobile push notification integration.
- Network-class change handling on resume.
- Redesigning settings, login, logout, or Matrix session restore.
- Migrating every pre-existing direct `sync_service.start()` / `sync_service.stop()` call outside the lifecycle path.
- Android and iOS manual smoke execution by this agent; platform smoke testing is reviewer-owned.
- Adding automated tests for lifecycle OS events.
- Expanding app-state deduplication beyond lifecycle-triggered `persist_runtime_state` saves.
