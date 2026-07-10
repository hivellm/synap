//! Minimal Redis-style glob matching for SCAN `MATCH` and pattern pub/sub.
//!
//! Supports `*` (any run, including empty), `?` (exactly one char), `[...]`
//! character classes (with `^` negation and `a-z` ranges), and `\` escaping.
//! Matching is over bytes so it works for arbitrary member/field values.

/// Returns true when `text` matches the glob `pattern`.
pub fn glob_match(pattern: &str, text: &str) -> bool {
    glob_match_bytes(pattern.as_bytes(), text.as_bytes())
}

/// Byte-level glob match (iterative, backtracking on `*`).
pub fn glob_match_bytes(pattern: &[u8], text: &[u8]) -> bool {
    let (mut p, mut t) = (0usize, 0usize);
    // Backtrack points for the most recent `*`.
    let (mut star_p, mut star_t): (Option<usize>, usize) = (None, 0);

    while t < text.len() {
        if p < pattern.len() {
            match pattern[p] {
                b'*' => {
                    star_p = Some(p);
                    star_t = t;
                    p += 1;
                    continue;
                }
                b'?' => {
                    p += 1;
                    t += 1;
                    continue;
                }
                b'[' => {
                    if let Some((matched, next_p)) = match_class(pattern, p, text[t]) {
                        if matched {
                            p = next_p;
                            t += 1;
                            continue;
                        }
                    }
                    // Class didn't match → try backtracking.
                }
                b'\\' if p + 1 < pattern.len() && pattern[p + 1] == text[t] => {
                    p += 2;
                    t += 1;
                    continue;
                }
                c if c == text[t] => {
                    p += 1;
                    t += 1;
                    continue;
                }
                _ => {}
            }
        }

        // Mismatch (or ran out of pattern): backtrack to the last `*`, if any.
        if let Some(sp) = star_p {
            p = sp + 1;
            star_t += 1;
            t = star_t;
        } else {
            return false;
        }
    }

    // Consume any trailing `*`s.
    while p < pattern.len() && pattern[p] == b'*' {
        p += 1;
    }
    p == pattern.len()
}

/// Match a `[...]` class at `pattern[start]` against byte `ch`.
/// Returns `(matched, index just past the class)` or `None` if unterminated.
fn match_class(pattern: &[u8], start: usize, ch: u8) -> Option<(bool, usize)> {
    let mut i = start + 1; // past '['
    let mut negate = false;
    if i < pattern.len() && pattern[i] == b'^' {
        negate = true;
        i += 1;
    }
    let mut matched = false;
    let mut any = false;
    while i < pattern.len() && pattern[i] != b']' {
        any = true;
        // Range a-z
        if i + 2 < pattern.len() && pattern[i + 1] == b'-' && pattern[i + 2] != b']' {
            let (lo, hi) = (pattern[i], pattern[i + 2]);
            if lo <= ch && ch <= hi {
                matched = true;
            }
            i += 3;
        } else {
            if pattern[i] == ch {
                matched = true;
            }
            i += 1;
        }
    }
    if i >= pattern.len() || !any {
        return None; // unterminated or empty class → treat as literal elsewhere
    }
    // i is at ']'
    Some((matched ^ negate, i + 1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literal_and_wildcards() {
        assert!(glob_match("hello", "hello"));
        assert!(!glob_match("hello", "hell"));
        assert!(glob_match("*", "anything"));
        assert!(glob_match("*", ""));
        assert!(glob_match("h*o", "hello"));
        assert!(glob_match("h*o", "ho"));
        assert!(!glob_match("h*o", "hi"));
        assert!(glob_match("h?llo", "hello"));
        assert!(!glob_match("h?llo", "hllo"));
        assert!(glob_match("user:*", "user:1001"));
        assert!(!glob_match("user:*", "admin:1"));
    }

    #[test]
    fn star_backtracking() {
        assert!(glob_match("a*b*c", "axxbyyc"));
        assert!(glob_match("*abc", "zzabc"));
        assert!(glob_match("abc*", "abczz"));
        assert!(!glob_match("a*c", "ab"));
    }

    #[test]
    fn char_classes() {
        assert!(glob_match("h[ae]llo", "hello"));
        assert!(glob_match("h[ae]llo", "hallo"));
        assert!(!glob_match("h[ae]llo", "hillo"));
        assert!(glob_match("key[0-9]", "key5"));
        assert!(!glob_match("key[0-9]", "keyx"));
        assert!(glob_match("h[^x]llo", "hello"));
        assert!(!glob_match("h[^e]llo", "hello"));
    }

    #[test]
    fn escaping() {
        assert!(glob_match(r"a\*b", "a*b"));
        assert!(!glob_match(r"a\*b", "axb"));
    }
}
