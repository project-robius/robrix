# Bolt's Journal

## 2024-05-22 - [Project Initialization]
**Learning:** This is a fresh start for Bolt in this codebase.
**Action:** Proceed with identifying performance bottlenecks in the Rust/Makepad application.

## 2026-02-04 - [Type Coercion & Sys Deps]
**Learning:** `Vec<OwnedType>` does not coerce to `&[BorrowedType]` even if `OwnedType` derefs to `BorrowedType`. Explicit `map` is required. Also, `wayland-sys` issues block local `cargo check`.
**Action:** Use `.map(|x| x.as_ref())` for slice coercion. Rely on CI for full build verification when system deps are missing.

## 2026-02-04 - [Batching Room Subscriptions]
**Learning:** Matrix SDK `subscribe_to_rooms` incurs overhead. Batching subscriptions during bulk updates (`Append`/`Reset`) is significantly more efficient than individual calls. Also, caller responsibility for subscription allows cleaner separation of concerns than burying it in helper functions like `add_new_room`.
**Action:** Always look for loops calling async methods that take a list; refactor to batch calls where possible.

## 2026-02-04 - [Parallelizing Async Tasks]
**Learning:** When processing a list of items where each item requires async work (like `add_new_room`), using `join_all` to run them concurrently is much faster than sequential iteration. This is safe as long as the critical sections (like shared map insertion) are brief or thread-safe.
**Action:** Identify sequential `await` loops in async functions and refactor to `join_all` if the operations are independent.
