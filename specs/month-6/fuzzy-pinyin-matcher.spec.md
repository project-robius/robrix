<!-- Month 6: Search & Navigation — Deliverable D (cross-cutting matcher).
     This spec ships the pinyin-aware substring matcher that future Month-6
     specs (in-room search "A", global search "B") will reuse without changes. -->
spec: task
name: "Fuzzy Pinyin Matcher (Substring + Pinyin)"
inherits: project
tags: [month-6, search, i18n, chinese-input]
---

## Intent

Robrix's current name filtering (`room_display_filter::matches_room_name`, `member_search::match_member_with_priority`) uses ASCII case-insensitive substring matching. Users typing Chinese names by their pinyin (`beijing`, `bj`) cannot find rooms or members whose display names are written in Hanzi (`北京`, `张三`).

This task adds a small, reusable pinyin-aware substring matcher and wires it into the room-list filter and member-search call sites. It does NOT add subsequence matching, typo tolerance, relevance scoring, message search, or other romanizations (Japanese, Korean, Cyrillic) — those are tracked separately for Month 6.

Tracks robius issue project-robius/robrix#564.

## Decisions

- **New module:** `src/shared/pinyin_match.rs`. Pure logic; no Makepad widgets; not registered in `script_mod`.
- **Public API:** single free function
  ```rust
  pub fn pinyin_substring_match(candidate: &str, query: &str) -> bool;
  ```
  Caller passes the candidate name (display string) and the user's query. Function is pure, holds no state, performs no I/O.
- **New dependency:** `pinyin = "0.10"` added to `Cargo.toml`. This is the only new crate this spec introduces. Explicit per `project.spec.md` constraint against adding deps without task-level approval. License: MIT/Apache-2.0.
- **Matching algorithm** (in order, short-circuit on first hit):
  1. Case-insensitive literal substring (`candidate.to_lowercase().contains(&query.to_lowercase())`).
  2. If candidate contains any CJK character: pinyin syllables substring (`"北京" → "beijing"`).
  3. If candidate contains any CJK character: pinyin initials substring (`"北京" → "bj"`).
- **Pinyin coverage** (in scope):
  - Full pinyin without tones (`"beijing"` matches `"北京"`).
  - Pinyin initials (`"bj"` matches `"北京"`).
  - Single canonical pronunciation per character (the `pinyin` crate's default).
  - Non-CJK characters in the candidate pass through unchanged in both the syllable and initials forms (so `"北京 Cafe"` → syllables `"beijing cafe"`, initials `"bj cafe"`).
- **Query normalization:** matcher lowercases the query internally; callers do not need to pre-lowercase. Empty query returns `true` (consistent with existing `RoomDisplayFilterBuilder` short-circuit).
- **No CJK ⇒ no pinyin work:** internal helpers return `Option<String>`; `None` when candidate contains zero CJK characters, so purely-ASCII names skip pinyin allocations entirely on the hot path.
- **No caching:** pinyin is recomputed per query per candidate. Lazy strategy chosen explicitly; revisited only if profiling shows a regression.
- **Room-list integration:** `room_display_filter::matches_room_name` (`src/room/room_display_filter.rs`) replaces its `to_lowercase().contains` body with `pinyin_substring_match(&room.room_name(), keywords)`. Other matchers in that file (`matches_room_id`, `matches_room_alias`, `matches_room_tags`) are not changed — pinyin applies to human-readable names only.
- **Member-search integration:** `match_member_with_priority` (`src/room/member_search.rs`) gains two new priority tiers below all existing literal tiers (0–9) and above the empty-search sentinel:
  - **New tier (display name, full pinyin syllables):** ranks below the existing priority 9 contains-tier.
  - **New tier (display name, pinyin initials):** ranks below the syllables tier.
  - The empty-search return value moves to a higher numeric value than these new tiers to preserve the "matches but ranks last" semantics.
  - User IDs and localparts are ASCII Matrix identifiers; pinyin matching does not apply to them.
- **Behavior preserved on ASCII inputs:** every test currently passing for ASCII-only candidates and queries must continue to pass. Pinyin matching is additive.

## Boundaries

### Allowed to Modify

- `Cargo.toml` — add `pinyin = "0.10"` (or current stable equivalent) to `[dependencies]`.
- `src/shared/mod.rs` — declare `pub mod pinyin_match;`.
- `src/room/room_display_filter.rs` — replace `matches_room_name` body to call the new matcher.
- `src/room/member_search.rs` — add pinyin matcher tiers in `MATCHERS` (or adjacent fallthrough), adjust empty-search sentinel.

### Must Create

- `src/shared/pinyin_match.rs` — module containing `pinyin_substring_match`, private helpers `cjk_to_pinyin_syllables`, `cjk_to_pinyin_initials`, and a `#[cfg(test)] mod tests` block.
- `specs/month-6/` directory (this spec's parent).

### Forbidden

- Do NOT add subsequence matching, edit-distance, typo tolerance, or relevance scoring in this task.
- Do NOT add other romanizations (Japanese romaji, Korean RR, Cyrillic transliteration) in this task.
- Do NOT add tone-marked input handling (`běijīng`) as an accepted query form.
- Do NOT add heteronym / polyphone support (`贾→gu`); only the `pinyin` crate's primary pronunciation is used.
- Do NOT precompute or cache pinyin across the codebase; the lazy per-query strategy is the explicit design choice.
- Do NOT apply pinyin to non-display fields: room IDs, room aliases, room tags, user IDs, or localparts.
- Do NOT wire the matcher into `src/home/search_messages.rs` in this task; that file remains a TODO stub. (Message search lands in deliverable A.)
- Do NOT add new cargo dependencies beyond `pinyin`.
- Do NOT run `cargo fmt`.

## Acceptance Criteria

Scenario: Literal ASCII substring still matches
  Test:
    package: robrix
    filter: test_pinyin_match_literal_ascii_substring
  Given candidate `"Beijing"` and query `"beij"`
  Then `pinyin_substring_match` returns `true`

Scenario: Full pinyin matches Hanzi candidate
  Test:
    package: robrix
    filter: test_pinyin_match_full_pinyin_cjk
  Given candidate `"北京"` and query `"beijing"`
  Then `pinyin_substring_match` returns `true`

Scenario: Pinyin initials match Hanzi candidate
  Test:
    package: robrix
    filter: test_pinyin_match_initials_cjk
  Given candidate `"北京"` and query `"bj"`
  Then `pinyin_substring_match` returns `true`

Scenario: Literal CJK substring matches
  Test:
    package: robrix
    filter: test_pinyin_match_literal_cjk_substring
  Given candidate `"北京"` and query `"北"`
  Then `pinyin_substring_match` returns `true`

Scenario: Mixed CJK + ASCII candidate
  Test:
    package: robrix
    filter: test_pinyin_match_mixed_passthrough
  Given candidate `"北京 Cafe"` and query `"beijing cafe"`
  Then `pinyin_substring_match` returns `true`

Scenario: Heteronyms are not matched by secondary pronunciation
  Test:
    package: robrix
    filter: test_pinyin_match_heteronym_primary_only
  Given candidate `"贾"` and query `"gu"`
  Then `pinyin_substring_match` returns `false`
  And the canonical pronunciation `"jia"` would still match

Scenario: Pure ASCII candidate skips pinyin work
  Test:
    package: robrix
    filter: test_pinyin_match_skips_pinyin_for_pure_ascii
  Given candidate `"Alice"`
  Then `cjk_to_pinyin_syllables` returns `None`
  And `cjk_to_pinyin_initials` returns `None`

Scenario: Empty query returns true
  Test:
    package: robrix
    filter: test_pinyin_match_empty_query_returns_true
  Given any candidate and query `""`
  Then `pinyin_substring_match` returns `true`

Scenario: Case-insensitive on both sides
  Test:
    package: robrix
    filter: test_pinyin_match_case_insensitive
  Given candidate `"Beijing"` and query `"BEIJ"`
  Then `pinyin_substring_match` returns `true`

Scenario: Room filter finds Hanzi room by pinyin
  Test:
    package: robrix
    filter: test_room_filter_matches_room_name_by_pinyin
  Given a `JoinedRoomInfo` with display name `"北京"`
  And a `RoomDisplayFilterBuilder` configured with keywords `"beijing"`
  When the built filter is applied
  Then the room is included

Scenario: Room filter finds Hanzi room by initials
  Test:
    package: robrix
    filter: test_room_filter_matches_room_name_by_initials
  Given a `JoinedRoomInfo` with display name `"北京"`
  And a `RoomDisplayFilterBuilder` configured with keywords `"bj"`
  When the built filter is applied
  Then the room is included

Scenario: Room filter backwards compatibility on ASCII
  Test:
    package: robrix
    filter: test_room_filter_backwards_compat_ascii
  Given a `JoinedRoomInfo` with display name `"Robrix Team"`
  And keywords `"team"`
  Then the room is included
  And behavior is unchanged from before this task

Scenario: Member search finds Hanzi display name by full pinyin
  Test:
    package: robrix
    filter: test_member_search_matches_by_pinyin
  Given a room member with display name `"张三"`
  And a search query `"zhangsan"`
  Then `match_member_with_priority` returns `Some(priority)` where `priority` is the new pinyin-syllables tier
  And the member appears in `search_room_members_streaming_with_sort` results

Scenario: Member search finds Hanzi display name by initials
  Test:
    package: robrix
    filter: test_member_search_matches_by_initials
  Given a room member with display name `"张三"`
  And a search query `"zs"`
  Then `match_member_with_priority` returns `Some(priority)` where `priority` is the new pinyin-initials tier

Scenario: Member search priority order preserves literal-match wins
  Test:
    package: robrix
    filter: test_member_search_literal_outranks_pinyin
  Given a member `"zhangsan"` (ASCII display name) and a member `"张三"` (Hanzi display name)
  And a search query `"zhangsan"`
  Then both members match
  And `"zhangsan"` ranks higher (lower priority value) than `"张三"`

## Out of Scope

- Subsequence ("fzf-style") matching, edit-distance / typo tolerance, relevance scoring beyond the existing priority tiers.
- Romanization for languages other than Chinese (Japanese romaji, Korean Revised Romanization, Cyrillic transliteration, etc.).
- Heteronym / polyphone handling (`贾→gu`, `行→hang/xing`).
- Tone-marked input (`běijīng`); we strip tones from the candidate side only.
- Precomputing or caching pinyin on `JoinedRoomInfo` / `RoomMember` structs.
- Wiring into message search (`src/home/search_messages.rs`) — that arrives in Month 6 deliverable A.
- Global cross-room search, date navigation, jump-to-message — Month 6 deliverables B and C.
- Performance benchmarking and optimization beyond the no-CJK short-circuit.
- UI affordance changes (placeholder text, hint that pinyin works) — text changes are tracked separately if needed.
