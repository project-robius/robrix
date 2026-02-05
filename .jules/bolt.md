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
