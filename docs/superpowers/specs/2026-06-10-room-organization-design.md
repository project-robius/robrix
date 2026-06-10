# Room Organization: Favorites & Low Priority Sections

**Date:** 2026-06-10
**Branch:** feat/room_organization
**File scope:** `src/home/rooms_list.rs` (primary), minor touch to incremental update handlers

---

## Overview

Add two new collapsible sections to the `RoomsList` widget:

- **Favorites** — non-DM rooms the user has tagged with `m.favourite`, shown at the top
- **Low Priority** — non-DM rooms the user has tagged with `m.lowpriority`, shown at the bottom

The context menu buttons that set/unset these tags already exist and work. The Matrix server already sends tag updates via `RoomsListUpdate::Tags`. This spec covers only the display-side changes needed to separate rooms into the new sections.

---

## Section Order

Top to bottom in the `PortalList`. Defaults for existing sections are unchanged from current code (`#[rust]` initializers).

| # | Section | Default state |
|---|---------|--------------|
| 1 | **Favorites** (non-DM, `m.favourite` tag) | **expanded** (new) |
| 2 | **Invites** (existing, unchanged) | collapsed (existing) |
| 3 | **People** (DM rooms, existing, unchanged) | collapsed (existing) |
| 4 | **Rooms** (non-DM, non-favorite, non-low-priority, existing) | expanded (existing) |
| 5 | **Low Priority** (non-DM, `m.lowpriority` tag) | **collapsed** (new) |
| 6 | Status label (existing, unchanged) | — |

The field initializers in the Data Model section below reflect the defaults for the two new sections only.

---

## Classification Rules

For every joined non-invited room, the following rules are evaluated in order and the first match wins. A room appears in exactly one section.

| Priority | Condition | Section |
|----------|-----------|---------|
| 1 | `is_direct == true` | People (overrides all tags) |
| 2 | `is_direct == false` AND `tags` contains `m.favourite` | Favorites |
| 3 | `is_direct == false` AND `tags` contains `m.lowpriority` | Low Priority |
| 4 | `is_direct == false`, no relevant tags | Rooms |

DMs are **never** reclassified by tags. A room with both `m.favourite` and `m.lowpriority` goes to Favorites (rule 2 matches first). Every non-invited joined room matches exactly one row.

---

## Data Model Changes

### New state fields on `RoomsList`

Six new fields, mirroring the existing 3-section pattern. Only these two sections have non-default initial values; existing sections are unchanged.

```rust
#[rust] displayed_favorite_rooms: Vec<OwnedRoomId>,
#[rust(true)] is_favorite_rooms_header_expanded: bool,
#[rust] favorite_rooms_indexes: RoomCategoryIndexes,

#[rust] displayed_low_priority_rooms: Vec<OwnedRoomId>,
#[rust(false)] is_low_priority_rooms_header_expanded: bool,
#[rust] low_priority_rooms_indexes: RoomCategoryIndexes,
```

---

## Function-Level Changes

### `generate_displayed_rooms` — return 5-tuple

**Important:** the existing return order is `(invited, regular, direct)`. The new order reorders this and adds two elements: `(invited, favorites, direct, regular, low_priority)`. The `update_displayed_rooms` destructuring must match this exact new order.

Change signature from `-> (Vec, Vec, Vec)` to:

```rust
fn generate_displayed_rooms(&self)
    -> (Vec<OwnedRoomId>, Vec<OwnedRoomId>, Vec<OwnedRoomId>, Vec<OwnedRoomId>, Vec<OwnedRoomId>)
    // order: (invited, favorites, direct, regular, low_priority)
```

The inner `push_joined_room` closure applies the classification rules in priority order:

```rust
let mut push_joined_room = |room_id: &OwnedRoomId, jr: &JoinedRoomInfo| {
    let room_id = room_id.clone();
    if jr.is_direct {
        new_displayed_direct_rooms.push(room_id);
    } else if jr.tags.contains_key(&TagName::Favorite) {
        new_displayed_favorite_rooms.push(room_id);
    } else if jr.tags.contains_key(&TagName::LowPriority) {
        new_displayed_low_priority_rooms.push(room_id);
    } else {
        new_displayed_regular_rooms.push(room_id);
    }
};
```

The existing `should_display_room!` guard that wraps all calls to `push_joined_room` must be preserved as-is.

### `update_displayed_rooms`

Destructure all 5 values (in the new order) and assign all 5 lists. The empty-filter fallback must also unpack all 5 values from the second `generate_displayed_rooms()` call — not just 3 — otherwise favorites and low_priority are silently dropped on fallback:

```rust
let (mut invited, mut favorites, mut direct, mut regular, mut low_priority) =
    self.generate_displayed_rooms();
if self.display_filter.is_some()
    && invited.is_empty() && favorites.is_empty()
    && direct.is_empty() && regular.is_empty() && low_priority.is_empty()
{
    self.display_filter = RoomDisplayFilter::default();
    self.sort_fn = None;
    // Must unpack all 5 on the fallback call too:
    (invited, favorites, direct, regular, low_priority) = self.generate_displayed_rooms();
}
self.displayed_invited_rooms = invited;
self.displayed_favorite_rooms = favorites;
self.displayed_direct_rooms = direct;
self.displayed_regular_rooms = regular;
self.displayed_low_priority_rooms = low_priority;
```

### `recalculate_indexes`

Extend the index chain. New order:

```
Favorites → Invited → Direct → Regular → LowPriority → StatusLabel
```

**Favorites anchors the chain at index 0.** The existing `index_of_invited_rooms_header = should_show_invited_rooms_header.then_some(0)` line must be updated to start after Favorites — i.e., `then_some(index_after_favorite_rooms)`. Every subsequent section follows the same arithmetic, pushed right by the Favorites block.

Each block follows the identical arithmetic pattern as the existing 3 blocks, including the existing `should_show_*_header = !self.displayed_*_rooms.is_empty()` guard that suppresses the header when the section is empty. Apply this guard to both new sections.

### `update_status`

Include new lists in total room count:

```rust
let num_rooms = self.displayed_invited_rooms.len()
    + self.displayed_favorite_rooms.len()
    + self.displayed_direct_rooms.len()
    + self.displayed_regular_rooms.len()
    + self.displayed_low_priority_rooms.len();
```

### `draw_walk`

Three changes to `draw_walk`:

1. **Add two new `get_*_room_id` closures** in the preamble, mirroring the existing three:
   ```rust
   let get_favorite_room_id = |portal_list_index: usize| {
       portal_list_index.checked_sub(self.favorite_rooms_indexes.first_room_index)
           .and_then(|index| self.is_favorite_rooms_header_expanded
               .then(|| self.displayed_favorite_rooms.get(index))
           )
           .flatten()
   };
   let get_low_priority_room_id = |portal_list_index: usize| {
       portal_list_index.checked_sub(self.low_priority_rooms_indexes.first_room_index)
           .and_then(|index| self.is_low_priority_rooms_header_expanded
               .then(|| self.displayed_low_priority_rooms.get(index))
           )
           .flatten()
   };
   ```

2. **Update `status_label_id`** — change `self.regular_rooms_indexes.after_rooms_index` to `self.low_priority_rooms_indexes.after_rooms_index`. This assignment appears in `draw_walk` separately from `recalculate_indexes` and must be updated in both places.

3. **Add 2 pairs of new `if/else` branches** in the draw loop (header + room entry), following the same pattern as direct/regular, using `HeaderCategory::Favorites` and `HeaderCategory::LowPriority`. Within each pair, the header-index check must come before the room-entry check in the `if/else if` chain, matching the existing pattern — this ensures `checked_sub` arithmetic in the closures does not accidentally match the header slot. When a section's header index is `None` (because the section is empty), the branch never matches and the section is naturally hidden.

### Collapsible header toggle (`handle_event`)

Replace the current `_todo => todo!(...)` wildcard with:

```rust
HeaderCategory::Favorites => {
    self.is_favorite_rooms_header_expanded = !self.is_favorite_rooms_header_expanded;
}
HeaderCategory::LowPriority => {
    self.is_low_priority_rooms_header_expanded = !self.is_low_priority_rooms_header_expanded;
}
```

### Incremental update handlers

All handlers that previously hard-coded lookups into 2–3 lists must be extended. "Apply classification rules" means: inspect `room.is_direct` and `room.tags` using the same priority-ordered logic as `push_joined_room` (not a sequential search across all lists).

| Handler | Change |
|---------|--------|
| `AddJoinedRoom` | Apply classification rules to route non-DM rooms to favorites / regular / low_priority. The `contains` duplicate-suppression guard must check whichever destination list applies (not only `displayed_regular_rooms`). Preserve the existing `should_display_room!` guard. |
| `HideRoom` | Check and remove from favorites and low_priority lists in addition to direct / regular / invited |
| `UnhideRoom` | After un-hiding, apply classification rules to route restored non-DM rooms to the correct list |
| `TombstonedRoom` | For non-DM rooms, apply classification rules (inspect `room.tags`) to determine the destination list (favorites / low_priority / regular), then check `should_display_room!` and update the identified list. Do not scan all three lists sequentially; use the same `if/else-if/else` logic as `push_joined_room`. |
| `RemoveRoom` | Check and remove from favorites and low_priority lists in addition to direct / regular |
| `UpdateIsDirect` | **Removal side:** when the room was previously non-DM, use `else-if` chaining to find and remove from exactly one of the three lists (favorites, low_priority, regular), consistent with the invariant that each room is in at most one list. **Insertion side:** apply classification rules (inspect `room.tags`) to insert into favorites / low_priority / regular rather than blindly inserting into `displayed_regular_rooms`. |
| `ClearRooms` | Clear `displayed_favorite_rooms` and `displayed_low_priority_rooms` in addition to the existing 3 lists |
| `ScrollToRoom` | Extend the portal-list-index lookup to cover `favorite_rooms_indexes` and `low_priority_rooms_indexes`, using the same offset arithmetic as the existing direct/regular/invited lookups, so that scroll-to-room from notifications works for rooms in these sections. |
| `UpdateRoomName` | The existing handler uses a two-branch `if is_direct { direct } else { regular }` pattern to determine which displayed list to add/remove from when `should_display` changes. Extend the non-DM branch to a three-way `else-if` split (favorites / low_priority / regular) by inspecting `room.tags`, matching the same classification logic as `push_joined_room`. Without this fix, renamed rooms in Favorites or Low Priority will silently fail to be removed when their display eligibility changes. |

### `RoomsListUpdate::Tags` handler

After updating `room.tags`, call `update_displayed_rooms(cx, false)`. This full regeneration correctly re-classifies the room into its new section. Do **not** also write a manual remove-then-re-insert routine in this handler — `update_displayed_rooms` replaces any incremental removal. Note: `update_displayed_rooms` already sets `indexes_dirty = true` and calls `redraw` internally; no separate calls are needed after it.

### Logout clear (`LogoutAction::ClearAppState`)

The following additions extend (not replace) the existing logout block. Add clears for the 2 new fields:

```rust
self.displayed_favorite_rooms.clear();
self.is_favorite_rooms_header_expanded = true;
self.favorite_rooms_indexes = RoomCategoryIndexes::default();
self.displayed_low_priority_rooms.clear();
self.is_low_priority_rooms_header_expanded = false;
self.low_priority_rooms_indexes = RoomCategoryIndexes::default();
```

---

## Out of Scope

- Custom drag-to-reorder within sections
- DMs appearing in Favorites (DMs always stay in People)
- Any changes to `room_context_menu.rs`, `collapsible_header.rs`, or i18n files
- Visual indicators (e.g., star icon) on favorited room entries

---

## Acceptance Criteria

1. Right-clicking a non-DM room and selecting "Favorite" immediately moves it from Rooms to the Favorites section; selecting "Unfavorite" moves it back to Rooms.
2. Right-clicking a non-DM room and selecting "Set Low Priority" immediately moves it to the Low Priority section; selecting "Unset Low Priority" moves it back to Rooms.
3. DM rooms are unaffected by favorite/low-priority tags — they remain in People regardless.
4. Favorites and Low Priority sections are collapsible via their section headers.
5. Low Priority section is collapsed by default; Favorites section is expanded by default.
6. Room counts in the status label include rooms from all 5 sections.
7. Logging out and back in resets all section state correctly (empty lists, correct expansion defaults).
8. When a search filter is active: a favorited room matching the filter keyword appears in the Favorites section; a low-priority room matching the filter appears in the Low Priority section; a section header is hidden when no rooms in that section match the filter. (This relies on two mechanisms working together: `generate_displayed_rooms` only populates rooms passing the filter, so empty sections naturally occur; and `recalculate_indexes` uses the `should_show_*_header` guard to suppress headers for empty sections. Both must be correctly applied to the new sections.)
9. Scrolling to a room (e.g., from a notification) works for rooms in the Favorites and Low Priority sections.
10. Leaving or removing a favorited or low-priority room removes it from its section without leaving a stale entry.
