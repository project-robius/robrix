# Issue #001: Dock.load_state() causes DrawList corruption and blank main page

**Date:** 2026-04-04
**Severity:** Critical (blocks all UI rendering)
**Status:** Fixed (workaround applied)
**Affected component:** `src/home/main_desktop_ui.rs` — `load_dock_state_from()`

## Summary

Restoring the Dock layout from persisted state via `Dock.load_state()` corrupts Makepad's internal DrawList references, causing the entire main content area (rooms list + room tabs) to render as a blank grey page.

## Symptoms

- Left navigation bar (NavigationTabBar) renders correctly
- Main content area (Dock with RoomsSideBar + room tabs) is completely blank/grey
- Console shows massive `Drawlist id generation wrong` errors:
  ```
  [E] draw_list.rs:324: Drawlist id generation wrong index: 21 current gen:1 in pointer:0
  ```
- Errors repeat continuously for draw list indices 21 and 22

## Root Cause

`Dock.load_state()` in Makepad's `widgets/src/dock.rs:1310` destroys DrawList references during event handling:

```rust
pub fn load_state(&mut self, cx: &mut Cx, dock_items: HashMap<LiveId, DockItem>) {
    self.dock_items = dock_items;
    self.items.clear();
    self.tab_bars.clear();     // Drops TabBarWrap, freeing DrawList2d
    self.splitters.clear();
    self.area.redraw(cx);      // Marks redraw, but stale refs remain
    self.create_all_items(cx);
}
```

The lifecycle issue:

1. `tab_bars.clear()` drops `TabBarWrap` instances containing `contents_draw_list: DrawList2d`
2. Drop increments the DrawList pool entry generation (0 → 1)
3. Makepad's rendering pipeline still holds cached `DrawListId(index, gen=0)` from the previous frame
4. Next frame accesses stale references → generation mismatch → rendering failure

This only triggers when the Dock already has live tab_bars (created during the first draw pass) and `load_state()` replaces them. On first startup with empty tab_bars, `clear()` is a no-op and causes no issue.

## Reproduction

1. Run the app, log in, open some room tabs
2. Close the app (state is persisted to `latest_app_state.json`)
3. Restart the app → blank main page

**Verification:** Deleting `latest_app_state.json` before restart → UI renders correctly with 0 DrawList errors.

## Fix Applied

Modified `load_dock_state_from()` in `src/home/main_desktop_ui.rs` to avoid calling `dock.load_state()`. Instead, tabs are recreated programmatically:

```rust
fn load_dock_state_from(&mut self, cx: &mut Cx, app_state: &mut AppState) {
    // ... resolve which state to restore ...

    let room_order = to_restore.room_order.clone();
    let selected_room = to_restore.selected_room.clone();

    // Close existing tabs using the Dock's normal API (safe)
    self.close_all_tabs(cx);

    // Recreate each room tab in saved order (safe)
    for room in &room_order {
        self.focus_or_create_tab(cx, room.clone());
    }

    // Re-select the previously-selected room
    let final_selected = selected_room.or_else(|| room_order.last().cloned());
    if let Some(selected) = final_selected.clone() {
        self.focus_or_create_tab(cx, selected);
    }
    app_state.selected_room = final_selected;
    self.redraw(cx);
}
```

This uses `close_all_tabs()` + `focus_or_create_tab()` which operate through the Dock's normal widget API, avoiding direct destruction of DrawList2d objects.

## Remaining Issues

1. **Splitter position not restored:** Custom sidebar width (if user dragged the splitter) resets to default 300px on restart.

2. **Multi-pane layout not restored:** If the user created split-view arrangements by dragging tabs, those layouts are lost on restart. All tabs return to the single default tab bar.

3. **Same issue exists in space switching:** `NavigationBarAction::TabSelected` also calls `load_dock_state_from()`, which previously used `dock.load_state()`. The fix applies to this path as well, but the same layout-loss trade-off exists.

4. **Upstream Makepad bug:** `Dock.load_state()` should be fixed in Makepad to properly handle DrawList lifecycle when called during event handling. The fix should either:
   - Defer the actual destruction to the next draw pass
   - Properly invalidate cached DrawList references in the rendering pipeline
   - Or use a two-phase approach: mark old DrawLists for cleanup, create new ones, then clean up

5. **`SETTINGS_BUTTON_HEIGHT` undefined:** Unrelated but observed during debugging — `account_settings.rs:63,86` references `mod.widgets.SETTINGS_BUTTON_HEIGHT` which is never defined, causing DSL parse warnings at startup.

## Files Changed

- `src/home/main_desktop_ui.rs` — `load_dock_state_from()` rewritten

## Test Verification

| Scenario | Before Fix | After Fix |
|----------|-----------|-----------|
| Start with persisted state | Blank page, ~50+ DrawList errors | Rooms render, 0 DrawList errors |
| Start without persisted state | Works | Works |
| Room tabs restored | N/A (blank) | All saved tabs recreated correctly |
| Selected room restored | N/A (blank) | Correct room selected and loaded |
