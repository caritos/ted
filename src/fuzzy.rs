//! Fuzzy matching of a short query against file paths. Pure logic.

/// How well `query` matches `path`: `None` when its chars do not all appear
/// in `path` in order (case-insensitive), otherwise a score where higher is
/// better. An empty query matches everything with the same score.
pub fn score(query: &str, path: &str) -> Option<i64> {
    let query: Vec<char> = query.chars().flat_map(char::to_lowercase).collect();
    if query.is_empty() {
        return Some(0);
    }
    let chars: Vec<char> = path.chars().collect();
    let lower: Vec<char> = chars.iter().flat_map(|c| c.to_lowercase()).collect();
    // Lower-casing can change the length of an odd char; fall back to the
    // original chars then so indexes stay aligned.
    let lower = if lower.len() == chars.len() { lower } else { chars.clone() };

    let mut total = 0i64;
    let mut q = 0;
    let mut previous: Option<usize> = None;
    for (i, &c) in lower.iter().enumerate() {
        if q < query.len() && c == query[q] {
            let boundary = i == 0 || matches!(chars[i - 1], '/' | '_' | '-' | '.' | ' ');
            total += 1;
            if previous.is_some_and(|p| p + 1 == i) {
                total += 3;
            }
            if boundary {
                total += 4;
            }
            previous = Some(i);
            q += 1;
        }
    }
    if q < query.len() {
        return None;
    }

    // A match inside the file name counts far more than one in the folders.
    let name_start = chars.iter().rposition(|&c| c == '/').map_or(0, |p| p + 1);
    let name = &lower[name_start..];
    let needle: String = query.iter().collect();
    let name_text: String = name.iter().collect();
    if name_text.starts_with(&needle) {
        total += 40;
    } else if name_text.contains(&needle) {
        total += 30;
    } else if is_subsequence(&query, name) {
        total += 10;
    }
    // Shorter paths first among equals.
    Some(total * 4 - chars.len() as i64)
}

fn is_subsequence(query: &[char], text: &[char]) -> bool {
    let mut q = 0;
    for &c in text {
        if q < query.len() && c == query[q] {
            q += 1;
        }
    }
    q == query.len()
}

/// Indexes into `paths` of the matches, best first. Equal scores keep the
/// order of `paths`. An empty query returns every index in order.
pub fn rank(query: &str, paths: &[String]) -> Vec<usize> {
    let mut scored: Vec<(i64, usize)> =
        paths.iter().enumerate().filter_map(|(i, p)| score(query, p).map(|s| (s, i))).collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    scored.into_iter().map(|(_, i)| i).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    fn ranked(query: &str, list: &[&str]) -> Vec<String> {
        let all = paths(list);
        rank(query, &all).into_iter().map(|i| all[i].clone()).collect()
    }

    const FILES: &[&str] = &["README.md", "src/app.rs", "src/editor.rs", "src/editor/tests.rs", "scripts/pty_smoke.py"];

    #[test]
    fn a_subsequence_matches_and_anything_else_does_not() {
        assert!(score("edr", "src/editor.rs").is_some());
        assert!(score("rde", "src/editor.rs").is_none());
        assert!(score("zzz", "src/editor.rs").is_none());
        assert!(score("", "anything").is_some());
    }

    #[test]
    fn matching_ignores_case_in_both_directions() {
        assert!(score("readme", "README.md").is_some());
        assert!(score("README", "readme.md").is_some());
        assert!(score("\u{c9}", "caf\u{e9}.txt").is_some());
    }

    #[test]
    fn the_file_name_beats_the_folders() {
        assert_eq!(ranked("app", FILES)[0], "src/app.rs");
        assert_eq!(ranked("readme", FILES), vec!["README.md"]);
        // `tests` is in a file name and in no folder name here.
        assert_eq!(ranked("tests", FILES)[0], "src/editor/tests.rs");
    }

    #[test]
    fn a_shorter_path_wins_between_equal_matches() {
        assert_eq!(ranked("editor", FILES)[..2], ["src/editor.rs", "src/editor/tests.rs"]);
    }

    #[test]
    fn a_prefix_of_the_name_beats_a_scattered_match() {
        let list = ["lib/very_nice_stuff.rs", "src/ns.rs", "notes/summary.txt"];
        assert_eq!(ranked("ns", &list)[0], "src/ns.rs");
    }

    #[test]
    fn folders_can_be_part_of_the_query() {
        assert_eq!(ranked("src/ap", FILES), vec!["src/app.rs"]);
        assert_eq!(ranked("sc/pty", FILES), vec!["scripts/pty_smoke.py"]);
    }

    #[test]
    fn no_match_is_empty_and_an_empty_query_keeps_the_order() {
        assert!(ranked("qqq", FILES).is_empty());
        assert_eq!(ranked("", FILES), FILES.to_vec());
    }

    #[test]
    fn equal_scores_keep_the_input_order() {
        let list = ["b/x.rs", "a/x.rs"];
        assert_eq!(ranked("x", &list), vec!["b/x.rs", "a/x.rs"]);
    }

    #[test]
    fn a_long_query_and_odd_chars_do_not_panic() {
        let long = "a".repeat(10_000);
        assert!(score(&long, "a").is_none());
        // Lower-casing U+0130 gives two chars; the lengths then disagree.
        let _ = score("i", "\u{130}stanbul");
        assert!(score("\u{1f600}", "x\u{1f600}y").is_some());
    }
}
