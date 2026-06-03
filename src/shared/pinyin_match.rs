//! Pinyin-aware substring matcher.
//!
//! Public API: [`pinyin_substring_match`]. Returns `true` if `query` matches
//! `candidate` either as a case-insensitive literal substring, or as a
//! substring of the candidate's pinyin syllables, or as a substring of the
//! candidate's pinyin initials. Pure function; no state; no I/O.
//!
//! Spec: `specs/month-6/fuzzy-pinyin-matcher.spec.md`.

/// Returns `true` if `query` matches `candidate`.
///
/// Match order, short-circuiting on first hit:
///   1. Case-insensitive literal substring.
///   2. If candidate contains CJK: pinyin syllables substring.
///   3. If candidate contains CJK: pinyin initials substring.
///
/// `query` is lowercased internally; the caller need not pre-lowercase.
/// Empty `query` returns `true`.
pub fn pinyin_substring_match(candidate: &str, query: &str) -> bool {
    if query.is_empty() {
        return true;
    }
    let query_lc = query.to_lowercase();
    if candidate.to_lowercase().contains(&query_lc) {
        return true;
    }
    if let Some(syllables) = cjk_to_pinyin_syllables(candidate) {
        if syllables.contains(&query_lc) {
            return true;
        }
    }
    if let Some(initials) = cjk_to_pinyin_initials(candidate) {
        if initials.contains(&query_lc) {
            return true;
        }
    }
    false
}

/// Returns the candidate's pinyin syllables (tone-less, lowercased), with
/// non-CJK characters passed through unchanged and lowercased. Returns
/// `None` if the candidate contains zero CJK characters, so callers can
/// skip pinyin work on the hot path.
fn cjk_to_pinyin_syllables(s: &str) -> Option<String> {
    use pinyin::ToPinyin;
    let mut has_cjk = false;
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c.to_pinyin() {
            Some(py) => {
                has_cjk = true;
                out.push_str(py.plain());
            }
            None => {
                for lc in c.to_lowercase() {
                    out.push(lc);
                }
            }
        }
    }
    if has_cjk { Some(out) } else { None }
}

/// Returns the candidate's pinyin initials (lowercased), with non-CJK
/// characters passed through unchanged and lowercased. Returns `None` if
/// the candidate contains zero CJK characters.
fn cjk_to_pinyin_initials(s: &str) -> Option<String> {
    use pinyin::ToPinyin;
    let mut has_cjk = false;
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c.to_pinyin() {
            Some(py) => {
                has_cjk = true;
                let plain = py.plain();
                if let Some(first) = plain.chars().next() {
                    out.push(first);
                }
            }
            None => {
                for lc in c.to_lowercase() {
                    out.push(lc);
                }
            }
        }
    }
    if has_cjk { Some(out) } else { None }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pinyin_match_empty_query_returns_true() {
        assert!(pinyin_substring_match("anything", ""));
        assert!(pinyin_substring_match("", ""));
        assert!(pinyin_substring_match("北京", ""));
    }

    #[test]
    fn test_pinyin_match_literal_ascii_substring() {
        assert!(pinyin_substring_match("Beijing", "beij"));
        assert!(pinyin_substring_match("Robrix Team", "team"));
        assert!(!pinyin_substring_match("Beijing", "shanghai"));
    }

    #[test]
    fn test_pinyin_match_case_insensitive() {
        assert!(pinyin_substring_match("Beijing", "BEIJ"));
        assert!(pinyin_substring_match("beijing", "BEIJING"));
        assert!(pinyin_substring_match("ROBRIX TEAM", "team"));
    }

    #[test]
    fn test_pinyin_match_literal_cjk_substring() {
        // Direct Hanzi-in-query matches the same Hanzi in candidate.
        assert!(pinyin_substring_match("北京", "北"));
        assert!(pinyin_substring_match("北京", "京"));
        assert!(pinyin_substring_match("张三李四", "三李"));
    }

    #[test]
    fn test_pinyin_match_full_pinyin_cjk() {
        assert!(pinyin_substring_match("北京", "beijing"));
        assert!(pinyin_substring_match("北京", "bei"));
        assert!(pinyin_substring_match("张三", "zhangsan"));
        // No match against unrelated pinyin.
        assert!(!pinyin_substring_match("北京", "shanghai"));
    }

    #[test]
    fn test_pinyin_match_initials_cjk() {
        assert!(pinyin_substring_match("北京", "bj"));
        assert!(pinyin_substring_match("张三", "zs"));
        assert!(pinyin_substring_match("张三李四", "zsls"));
        // Initials don't accidentally match unrelated letters.
        assert!(!pinyin_substring_match("北京", "xy"));
    }
}
