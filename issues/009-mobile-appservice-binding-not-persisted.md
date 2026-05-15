# Issue #009: Mobile — App Service binding is lost after force-quit + relaunch

**Date:** 2026-04-14
**Severity:** High (blocks any practical mobile usage of the App Service / BotFather feature)
**Status:** Fix verified on Android — ready for review; iOS not separately verified
**Affected component:** `src/sliding_sync.rs` (`handle_load_app_state`), mobile platforms

## Summary
On Android (and likely iOS), after the user fills the App Service settings (BotFather User ID, Octos Service URL) and clicks Save, the binding works during the current app session. However, once the user force-quits robrix2 and relaunches it, the App Service settings page comes up empty — both fields are blank and the Octos Service connection shows "Unreachable". The bot binding is not persisted across app restarts on mobile.

## Symptoms
- Open robrix2 on Android emulator or device
- Navigate to Settings → Labs → App Service
- Toggle "Enabled" on, fill BotFather User ID (e.g. `@octosbot:192.168.5.12:8128`) and Octos Service (e.g. `http://192.168.5.12:8010`), click **Save** — saves with success popup, "Check Now" shows Reachable ✅
- Force-quit robrix2 (swipe from recent apps / kill the process)
- Relaunch robrix2, go back to the same settings page
- Both fields are empty; Check Now shows Unreachable ❌

## Root Cause (hypothesis, needs verification)
`bot_settings.rs` currently calls `persist_bot_settings(app_state)` → `persistence::save_app_state(...)` after Save (line 331 / 427 / 578-581 in `src/settings/bot_settings.rs`). So the persistence CODE is wired. Candidate failure points that a fixer should audit:

1. **App-state hydrate on startup doesn't include App Service fields.** `load_app_state` may succeed but the App Service subsection is silently dropped (missing serde field, or a default that overwrites on deserialization).
2. **`app_data_dir()` on Android resolves to a path that doesn't survive app restart.** Android apps have multiple storage locations; cache and some internal dirs can be wiped by OS. If the persistence file ends up under `cacheDir` instead of `filesDir`, the OS can reclaim it at any time.
3. **App Service state is stored separately from the rest of `AppState` and only the main branch is loaded on startup.** Mobile code path may bypass the App Service load.
4. **Permission / path issue**: on Android, the app may succeed `save_app_state` to the Rust-side path but that path is inside a container the next process can't read (multi-process / scoped storage).

Desktop (macOS/Linux/Windows) likely works because `app_data_dir()` there resolves to a user-writable persistent location. Android/iOS have more constrained storage layers.

## Reproduction
1. Start local palpo + octos backend (see issue #005 and [clipboard pitfall doc from 2026-04-14] for correct setup)
2. Build & install robrix2 on Android emulator: `cargo makepad android run -p robrix --release`
3. Register/login; go to Settings → Labs → App Service
4. Enable and fill both fields; click Save
5. Verify "Check Now" reports Reachable
6. `adb shell am force-stop dev.makepad.robrix` (or swipe-kill from recent apps)
7. Relaunch robrix2
8. Navigate back to App Service settings — observe empty fields

## Fix Applied

**Root cause confirmed**: `src/sliding_sync.rs::handle_load_app_state` gated the entire `RestoreAppStateFromPersistentState` dispatch behind a non-empty dock-state check:

```rust
if !app_state.saved_dock_state_home.open_rooms.is_empty()
    && !app_state.saved_dock_state_home.dock_items.is_empty()
{
    Cx::post_action(AppStateAction::RestoreAppStateFromPersistentState(Box::new(app_state)));
}
```

Mobile has no dock, so every relaunch silently dropped the loaded non-dock state: `selected_room`, `bot_settings`, `app_language`, and `translation` config. Desktop masked the bug because dock state is almost always non-empty after first run. The save path itself was always correct.

**Fix**: replace the old "dock must be non-empty" gate with a broader "persisted state is meaningfully non-default" check. `handle_load_app_state` now restores when the loaded `AppState` contains any real persisted content (`selected_room`, dock state, bot settings, language, translation), while keeping the fresh-install / no-file path as a no-op. The restore match arm in `src/app.rs:1071-1095` already performs a full `AppState` replacement and dispatches `LoadDockFromAppState`, so empty-dock-but-configured-mobile state is handled correctly downstream. Log and popup messages inside `handle_load_app_state` were also reworded away from "dock layout" language to reflect the broader scope.

**UI hydration fix**: mobile force-quit / app swipe-away must not be the save trigger; Android/iOS do not guarantee `Shutdown` delivery. App Service settings are persisted immediately on Save / Check Now / toggle. The missing piece was that an already-visible Settings page could stay populated from the pre-restore default `AppState`, so `BotSettings` now re-hydrates from `Scope<AppState>` when restored `bot_settings` arrive.

**Regression guards**: `src/app.rs` unit tests pin the serde contract for `bot_settings` and `selected_room`; `src/sliding_sync.rs` unit tests pin the actual restore gate so empty dock + bot settings restores, empty dock + selected room restores, and pure default state remains a no-op; `src/settings/bot_settings.rs` unit tests pin the UI hydrate predicate.

**Manual verification**: Android force-quit + relaunch was verified on 2026-04-29 after rebuilding/reinstalling the current branch. App Service settings remained populated after relaunch.

**Spec + Plan**:
- Contract: `specs/task-fix-mobile-appservice-persistence.spec.md` (agent-spec Task Contract, quality 93%; lifecycle command passes, but the `manual_test_*` scenarios still require human execution)
- Plan: `docs/superpowers/plans/2026-04-14-fix-mobile-appservice-persistence.md`

## Follow-ups
1. Check iOS with the same force-quit + relaunch flow. The code path is shared, but iOS still needs platform verification.
2. Consider adding a UI-level smoke test or a small "Last saved" indicator in the settings page so future regressions are easier to diagnose.

## Files Likely Involved
- `src/settings/bot_settings.rs` — Save path (calls `persist_bot_settings`)
- `src/persistence/*` — `save_app_state` / `load_app_state` implementation
- `src/app.rs` — where `load_app_state` is called on startup and fields are restored to `AppState`
- `src/sliding_sync.rs` — where `app_data_dir()` is resolved per platform

## Test Verification
| Before fix | After fix |
|---|---|
| Mobile: App Service binding cleared after force-quit + relaunch | Mobile: binding restored on relaunch; Check Now succeeds without re-entering fields |

## Related
- Blocking real-world mobile testing of PR [octos-org/octos#345](https://github.com/octos-org/octos/pull/345) (bidirectional Matrix media + bot routing) — every restart forces a re-bind, which makes iterative testing painful
