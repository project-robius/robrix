## 2024-05-15 - Replace `String::push_str(&format!(...))` with `writeln!(...)`
**Learning:** In Rust loops, repeatedly concatenating formatted strings via `string.push_str(&format!(...))` creates unnecessary intermediate string allocations on every iteration.
**Action:** Use `std::fmt::Write` and `writeln!(&mut string, ...)` instead to format directly into the destination string buffer, avoiding allocations and improving performance.
