## $(date +%Y-%m-%d) - Optimize TSP wallet initialization and persistence with join_all

**Learning:** Sequential `.await` loops for tasks with independent I/O (like opening or persisting multiple databases/wallets) lead to significant bottlenecks.
**Action:** Always collect independent futures into an iterable (e.g., via `map`) and execute them concurrently using `futures_util::future::join_all` (or `try_join_all` for fail-fast error handling).
