## 2024-05-18 - Optimized space sync async N+1 fetching
**Learning:** In space service synchronization, sequential async execution (awaiting each avatar fetch in a loop for `VectorDiff::Append`, `VectorDiff::Reset`, or initial synchronization) caused a significant N+1 query problem that blocked rendering the list until all were complete.
**Action:** Use `futures_util::future::join_all` on an iterator of futures to run asynchronous fetches (like avatar requests) concurrently while resolving sequentially to preserve the correct UI order.
