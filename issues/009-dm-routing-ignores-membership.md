# Stale Empty DM Room Is Displayed And Reused

## Summary

After leaving a DM with user X, Robrix could keep showing the old DM as an
`Empty Room`, and opening X again from People could route back into that stale
room instead of using a valid DM. In that state the user sees one of two bad
behaviors:

- On startup, the old DM can reappear in the room list even though Element does
  not show it.
- Searching the same user again can reopen that stale empty room, or otherwise
  reuse the wrong DM candidate.

This is the primary defect tracked under issue #98.

## Reproduction

1. Create or open a DM with an appservice peer X.
2. Exchange at least one message so the room is clearly established.
3. Leave the DM.
4. Restart Robrix or search for X again from People.

Observed before the fix:

- Robrix may restore the old DM as `Empty Room (was "X")`.
- Clicking X again can reopen that stale room.
- The flow diverges from Element, which hides that room.

## Root Cause

The bug turned out to be a compound client-side classification problem, not just
one stale `m.direct` lookup.

1. A stale direct room whose display name had become `Empty` or
   `EmptyWas("X")` could still enter Robrix's joined room list and stay visible.
2. DM reuse logic was still willing to select a joined direct room for X without
   first proving that X was still an active member of that room.

That combination let a self-only stale DM survive in local state and then be
selected again as the "existing DM" for the same target user.

## Fix Applied

The fix has two coordinated parts in `src/sliding_sync.rs`:

1. Introduce a joined-room visibility predicate that hides stale direct rooms
   whose display name is `Empty` or `EmptyWas(...)`.
2. Replace direct reuse of a single `get_dm_room()` result with a joined-room
   scan that:
   - keeps only direct rooms targeting the requested user,
   - requires that target user's membership in the room to still be `Join` or
     `Invite`,
   - rejects stale empty direct rooms,
   - chooses the newest valid candidate by latest event timestamp.

This keeps stale empty DMs out of both:

- the room list / startup restoration path, and
- the "open or create DM" routing path.

## Code References

- `src/sliding_sync.rs`:
  - `should_display_joined_room_entry(...)`
  - `find_reusable_direct_message_room(...)`
  - `update_room(...)`
  - `add_new_room(...)`

## Verification

Manual verification completed against the fixed binary:

- Startup no longer restores the stale `Empty Room` DM.
- Open DM with `@octosbot`, send a message, leave the room, search the same user
  again, and reopen DM: the client no longer routes back to the stale empty room.
- A fresh DM room is created and messaging succeeds.

Relevant log evidence from `/tmp/robrix_debug.log` after the fix:

- the stale room is no longer restored into the visible room list,
- the first DM after startup is newly created,
- after leaving, reopening the same user creates a fresh DM instead of reusing
  the abandoned one,
- sending in the new room succeeds.

## Out Of Scope

- Composer gating on membership in any already-open left/banned room
- Auto-closing dock tabs on local `/leave`
- Cleaning stale `m.direct` entries server-side
- Unrelated Makepad runtime/property errors seen during app startup

## Residual Follow-up

Post-leave logs still show a small amount of async cleanup noise for rooms that
have just been removed, for example late updates racing with room removal. That
did not reproduce the user-facing DM bug after this fix, so it is treated as a
follow-up cleanup task rather than part of this defect.
