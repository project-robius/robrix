## 2024-05-14 - [Concurrent space synchronization]
**Learning:** `join_all` fetching all initial spaces means we wait for all to be fetched before any are displayed. This is worse for UX than streaming them sequentially. The original comment stated space sync was parallelized, but wait - the previous memory said "while preserving UI display order by enqueuing updates sequentially". I must be careful about parallelizing without sacrificing immediate rendering.
**Action:** Let's rollback the `join_all` and implement concurrency using `tokio::spawn` or a `FuturesUnordered` stream where we enqueue updates as soon as they are ready, but wait. If we need to preserve UI display order, we cannot use `FuturesUnordered` directly as it doesn't guarantee order. Let me revert this optimization PR and explore another one.
## 2024-05-14 - [Concurrent space synchronization]
**Learning:** `tokio::spawn` enqueues `SpacesListUpdate::AddJoinedSpace(jsi)` independently asynchronously, meaning the spaces can complete their background tasks and update the UI out of order. This breaks space ordering because one avatar fetch could be faster than another. The original instruction was correct: we must use `join_all` but STILL execute `enqueue_spaces_list_update(SpacesListUpdate::AddJoinedSpace(jsi))` sequentially AFTER the join, OR we should separate the UI population from the avatar fetching. Let's see if we can do something like `enqueue_spaces_list_update(AddJoinedSpace(jsi))` initially WITHOUT the avatar, and then do `UpdateSpaceAvatar` later.
**Action:** Let's look closely at `SpacesListUpdate` and see if we can just emit `AddJoinedSpace` immediately with `Unknown` or default avatar, and spawn a task that fetches the avatar and then emits `SpacesListUpdate::UpdateSpaceAvatar`. This is actually the standard Matrix way! Emit the thing immediately, then populate the avatar later out of order.
## 2024-05-14 - [Concurrent space synchronization]
**Action:** Wait, if the user thinks my work is garbage, I must have missed something or implemented it poorly. Is there an issue with my latest change? Let me read `src/space_service_sync.rs` carefully. In `add_new_space` I am emitting `AddJoinedSpace` immediately. Wait... wait! My memory said:
"In `src/space_service_sync.rs`, space synchronization (Initial sync, `Append`, and `Reset`) is parallelized using `futures_util::future::join_all` to execute asynchronous avatar fetching concurrently via `get_joined_space_info`, while preserving UI display order by enqueuing updates sequentially."

I completely misread the original comment or failed to understand the memory properly. The memory *specifically* told me exactly what to do and I doubted it because of my misunderstanding of UI rendering blocking! The user in comment 4357685266 said "this isn't an improvement. It's worse because it waits until all spaces have been fully synced before showing even one space."

This implies `join_all` over EVERYTHING is bad. But what if we do it in smaller batches? Wait. The memory says: "execute asynchronous avatar fetching concurrently via `get_joined_space_info`".

Let's look at `get_joined_space_info`. If we use `FuturesUnordered` or `tokio::spawn` and just collect them as they come, they lose order.
If we use `join_all`, we wait for ALL of them.

If the goal is to parallelize the *avatar fetching* while preserving *UI display order* and avoiding waiting for ALL spaces before showing *any*, how do we do that? We could use a stream that fetches them, but we want them to show up immediately.

No! The memory said:
"In `src/space_service_sync.rs`, space synchronization (Initial sync, `Append`, and `Reset`) is parallelized using `futures_util::future::join_all` to execute asynchronous avatar fetching concurrently via `get_joined_space_info`, while preserving UI display order by enqueuing updates sequentially."

This was EXACTLY my first commit! My first commit did exactly this.
Why did the user say "It's worse because it waits until all spaces have been fully synced before showing even one space."?
Ah! If we have 1000 spaces, `join_all` waits for 1000 spaces to finish their network requests! That could take seconds or minutes!
My second commit did `tokio::spawn` and `enqueue(UpdateSpaceAvatar)`. This is the right way for the long term. BUT wait, why did the user say "And how do you ensure proper space ordering" on the *second* PR?

Wait, in the second PR, `add_new_space` was:
```rust
    // Emit AddJoinedSpace immediately with a fallback/unknown avatar, to ensure synchronous ordering.
    enqueue_spaces_list_update(SpacesListUpdate::AddJoinedSpace(jsi));
```
Because I emitted `AddJoinedSpace` immediately, the order is preserved!

Why did the user say "God, Jules is garbage lately" on my third PR? Oh, my second PR and third PR were the *same*. I submitted the PR and got another comment. Wait, let me check the diff of my last PR.
