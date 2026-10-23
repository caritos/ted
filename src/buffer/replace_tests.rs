use super::*;
use std::time::Instant;

fn replaced(text: &str, query: &str, with: &str) -> (String, usize) {
    let mut b = Buffer::from_text(text);
    let n = b.replace_all(query, with, None);
    (b.text(), n)
}

// --- matches_in_line -------------------------------------------------------

#[test]
fn matches_smart_case_both_ways() {
    let b = Buffer::from_text("Foo foo FOO");
    assert_eq!(b.matches_in_line(0, "foo"), vec![(0, 3), (4, 3), (8, 3)]);
    assert_eq!(b.matches_in_line(0, "Foo"), vec![(0, 3)]);
    assert_eq!(b.matches_in_line(0, "fOO"), vec![]);
}

#[test]
fn matches_are_non_overlapping() {
    let b = Buffer::from_text("aaaa\naaa");
    assert_eq!(b.matches_in_line(0, "aa"), vec![(0, 2), (2, 2)]);
    assert_eq!(b.matches_in_line(1, "aa"), vec![(0, 2)]);
}

#[test]
fn matches_count_chars_with_multibyte_and_tab() {
    let b = Buffer::from_text("\u{e9}\ta\u{e9}\u{e9}\r\nx");
    assert_eq!(b.matches_in_line(0, "\u{e9}"), vec![(0, 1), (3, 1), (4, 1)]);
    assert_eq!(b.matches_in_line(0, "\ta"), vec![(1, 2)]);
    assert_eq!(b.matches_in_line(1, "x"), vec![(0, 1)]);
}

#[test]
fn matches_empty_query_and_out_of_range() {
    let b = Buffer::from_text("abc");
    assert_eq!(b.matches_in_line(0, ""), vec![]);
    assert_eq!(b.matches_in_line(1, "a"), vec![]);
    assert_eq!(b.matches_in_line(99, "a"), vec![]);
}

// --- replace_all -----------------------------------------------------------

#[test]
fn replaces_simple_and_adjacent() {
    assert_eq!(replaced("a foo b foo", "foo", "X"), ("a X b X".into(), 2));
    assert_eq!(replaced("aaaaa", "aa", "b"), ("bba".into(), 2));
}

#[test]
fn replacement_containing_the_term_is_not_rescanned() {
    assert_eq!(replaced("ab ab", "ab", "abab"), ("abab abab".into(), 2));
    assert_eq!(replaced("a", "a", "aa"), ("aa".into(), 1));
}

#[test]
fn empty_replacement_deletes() {
    assert_eq!(replaced("x-foo-y-foo", "foo", ""), ("x--y-".into(), 2));
    assert_eq!(replaced("foo", "foo", ""), ("".into(), 1));
}

#[test]
fn start_and_end_of_line_and_several_lines() {
    assert_eq!(replaced("foo a\nb foo\nfoo\nzzz", "foo", "Q"), ("Q a\nb Q\nQ\nzzz".into(), 3));
}

#[test]
fn crlf_text_keeps_its_breaks() {
    assert_eq!(replaced("foo\r\nbar foo\r\nfoo", "foo", "x"), ("x\r\nbar x\r\nx".into(), 3));
    assert_eq!(replaced("a\rfoo\r", "foo", "x"), ("a\rx\r".into(), 1));
}

#[test]
fn replacement_with_line_breaks_re_places_the_cursor() {
    let mut b = Buffer::from_text("a foo\nfoo b");
    assert_eq!(b.replace_all("foo", "1\n2", None), 2);
    assert_eq!(b.text(), "a 1\n2\n1\n2 b");
    assert_eq!(b.cursor(), (0, 2));
}

#[test]
fn smart_case_applies() {
    assert_eq!(replaced("Foo foo", "foo", "x"), ("x x".into(), 2));
    assert_eq!(replaced("Foo foo", "Foo", "x"), ("x foo".into(), 1));
}

#[test]
fn within_replaces_only_matches_fully_inside() {
    let mut b = Buffer::from_text("foo foo foo\nfoo foo foo");
    // From (0,2) (inside the first foo) to (1,7) (the end of the second foo
    // on row 1): the first foo and the last on row 1 are partial or outside.
    assert_eq!(b.replace_all("foo", "X", Some(((0, 2), (1, 7)))), 4);
    assert_eq!(b.text(), "foo X X\nX X foo");
    let mut b = Buffer::from_text("foo foo");
    assert_eq!(b.replace_all("foo", "X", Some(((0, 0), (0, 3)))), 1);
    assert_eq!(b.text(), "X foo");
    let mut b = Buffer::from_text("foo foo");
    assert_eq!(b.replace_all("foo", "X", Some(((0, 1), (0, 6)))), 0);
    assert_eq!(b.text(), "foo foo");
}

#[test]
fn cursor_ends_at_first_replacement_and_selection_is_cleared() {
    let mut b = Buffer::from_text("ab\nxx foo yy foo");
    b.select_all();
    assert_eq!(b.replace_all("foo", "BAR", Some(((0, 0), (1, 16)))), 2);
    assert_eq!(b.cursor(), (1, 3));
    assert_eq!(b.selection(), None);
    assert_eq!(b.text(), "ab\nxx BAR yy BAR");
}

#[test]
fn zero_matches_change_nothing() {
    let mut b = Buffer::from_text("abc\ndef");
    b.set_cursor(1, 1);
    b.move_right_select(true);
    let _ = b.take_dirty_row();
    assert_eq!(b.replace_all("zzz", "x", None), 0);
    assert_eq!(b.replace_all("", "x", None), 0);
    assert!(!b.can_undo());
    assert!(!b.is_modified());
    assert_eq!(b.selection(), Some(((1, 1), (1, 2))));
    assert_eq!(b.take_dirty_row(), None);
    assert_eq!(b.replace_all("abc", "x", Some(((1, 0), (1, 3)))), 0);
    assert!(!b.can_undo());
    assert_eq!(b.selection(), Some(((1, 1), (1, 2))));
}

#[test]
fn undo_restores_in_one_step_and_redo_reapplies() {
    let mut b = Buffer::from_text("a foo\nfoo b\nfoo");
    b.set_cursor(1, 2);
    assert_eq!(b.replace_all("foo", "X", None), 3);
    assert!(b.is_modified());
    assert_eq!(b.cursor(), (0, 2));
    assert!(b.undo());
    assert_eq!(b.text(), "a foo\nfoo b\nfoo");
    assert_eq!(b.cursor(), (1, 2));
    assert!(!b.is_modified());
    assert!(!b.can_undo());
    assert!(b.redo());
    assert_eq!(b.text(), "a X\nX b\nX");
    assert_eq!(b.cursor(), (0, 2));
    assert!(b.is_modified());
}

#[test]
fn never_merges_with_neighbouring_typing() {
    let mut b = Buffer::from_text("foo");
    b.set_cursor(0, 3);
    b.insert_char('a');
    b.replace_all("foo", "X", None);
    b.insert_char('b');
    assert!(b.undo());
    assert_eq!(b.text(), "Xa");
    assert!(b.undo());
    assert_eq!(b.text(), "fooa");
    assert!(b.undo());
    assert_eq!(b.text(), "foo");
}

#[test]
fn dirty_row_starts_at_the_first_replaced_row() {
    let mut b = Buffer::from_text("a\nb\nfoo x foo\nc");
    let _ = b.take_dirty_row();
    b.replace_all("foo", "X", None);
    assert_eq!(b.take_dirty_row(), Some(2));
    b.replace_all("X", "1\n2", None);
    assert_eq!(b.take_dirty_row(), Some(1));
}

// --- randomized ------------------------------------------------------------

struct Rng(u64);

impl Rng {
    fn below(&mut self, n: usize) -> usize {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((self.0 >> 33) as usize) % n
    }
}

/// Reference: (row, col, len) of non-overlapping smart-case matches.
fn reference(text: &str, query: &str) -> Vec<(usize, usize, usize)> {
    let exact = query.chars().any(char::is_uppercase);
    let norm = |s: &str| -> Vec<char> { s.chars().map(|c| if exact { c } else { c.to_ascii_lowercase() }).collect() };
    let q = norm(query);
    let mut out = Vec::new();
    for (row, line) in text.split('\n').enumerate() {
        let line = line.strip_suffix('\r').unwrap_or(line);
        let h = norm(line);
        let mut i = 0;
        while i + q.len() <= h.len() {
            if h[i..i + q.len()] == q[..] {
                out.push((row, i, q.len()));
                i += q.len();
            } else {
                i += 1;
            }
        }
    }
    out
}

/// Char index of (row, col) when every row ends in "\n" or "\r\n".
fn index_at(text: &str, row: usize, col: usize) -> usize {
    let mut index = 0;
    for line in text.split('\n').take(row) {
        index += line.chars().count() + 1;
    }
    index + col
}

fn run(seed: u64) {
    let mut rng = Rng(seed);
    let alphabet = ['a', 'A', 'b', ' ', '\n', '\n', '\u{e9}'];
    let mut text = String::new();
    for _ in 0..rng.below(40) {
        let c = alphabet[rng.below(alphabet.len())];
        if c == '\n' && rng.below(3) == 0 {
            text.push('\r');
        }
        text.push(c);
    }
    let qalpha = ['a', 'A', 'b', 'a', '\u{e9}'];
    let query: String = (0..1 + rng.below(3)).map(|_| qalpha[rng.below(qalpha.len())]).collect();
    let with = ["", "x", "aa", "a\nb", "\r\n"][rng.below(5)];
    let all = reference(&text, &query);
    let within = if rng.below(2) == 0 {
        None
    } else {
        let rows = text.split('\n').count();
        let mut p = [(rng.below(rows), rng.below(6)), (rng.below(rows), rng.below(6))];
        let len = |r: usize| text.split('\n').nth(r).unwrap().trim_end_matches('\r').chars().count();
        for q in &mut p {
            q.1 = q.1.min(len(q.0));
        }
        p.sort();
        Some((p[0], p[1]))
    };
    let chosen: Vec<_> = all
        .iter()
        .filter(|&&(r, c, l)| within.is_none_or(|(f, t)| (r, c) >= f && (r, c + l) <= t))
        .collect();
    let mut expected = String::new();
    let chars: Vec<char> = text.chars().collect();
    let mut at = 0;
    for &&(r, c, l) in &chosen {
        let s = index_at(&text, r, c);
        expected.extend(&chars[at..s]);
        expected.push_str(with);
        at = s + l;
    }
    expected.extend(&chars[at..]);
    let mut b = Buffer::from_text(&text);
    let n = b.replace_all(&query, with, within);
    assert_eq!(n, chosen.len(), "seed {seed} {text:?} {query:?} {within:?}");
    assert_eq!(b.text(), expected, "seed {seed} {text:?} {query:?} {with:?} {within:?}");
    let (row, col) = b.cursor();
    assert!(row < b.line_count() && col <= b.line(row).chars().count(), "seed {seed}");
    if n == 0 {
        assert!(!b.can_undo(), "seed {seed}");
    } else {
        assert!(b.undo(), "seed {seed}");
        assert_eq!(b.text(), text, "seed {seed}");
        assert!(!b.is_modified(), "seed {seed}");
        assert!(!b.can_undo(), "seed {seed}");
        assert!(b.redo());
        assert_eq!(b.text(), expected, "seed {seed}");
    }
}

#[test]
fn randomized_replace_matches_a_reference() {
    for seed in 0..3000 {
        run(seed);
    }
}

// --- performance -----------------------------------------------------------

#[test]
fn a_huge_replace_is_fast_and_one_step() {
    let text = "let foo = foo + 1; // foo\n".repeat(100_000);
    let mut b = Buffer::from_text(&text);
    let start = Instant::now();
    assert_eq!(b.replace_all("foo", "barbaz", None), 300_000);
    // About 0.6 s in a debug build on its own; a quadratic walk takes minutes.
    assert!(start.elapsed().as_secs_f64() < 3.0, "took {:?}", start.elapsed());
    assert_eq!(b.undo.len(), 1);
    assert_eq!(b.history_bytes(), b.hist_bytes);
    assert_eq!(b.line(99_999), "let barbaz = barbaz + 1; // barbaz");
    assert!(b.undo());
    assert_eq!(b.text().len(), text.len());
}

// --- find_matches / replace_matches ------------------------------------------

#[test]
fn find_matches_lists_every_match_in_order_with_its_length() {
    let b = Buffer::from_text("foo x foo\nnone\nFOOfoo");
    assert_eq!(b.find_matches("foo", None), (vec![(0, 0), (0, 6), (2, 0), (2, 3)], 3));
    assert!(b.find_matches("Foo", None).0.is_empty());
    assert!(b.find_matches("", None).0.is_empty());
}

#[test]
fn find_matches_respects_the_scope() {
    let b = Buffer::from_text("foo foo\nfoo foo");
    assert_eq!(b.find_matches("foo", Some(((0, 4), (1, 3)))).0, vec![(0, 4), (1, 0)]);
}

#[test]
fn replace_matches_replaces_only_the_given_ones_as_one_undo_step() {
    let mut b = Buffer::from_text("foo a foo\nb foo\nfoo");
    let (all, len) = b.find_matches("foo", None);
    assert_eq!(b.replace_matches(&[all[0], all[2]], len, "XY"), 2);
    assert_eq!(b.text(), "XY a foo\nb XY\nfoo");
    assert!(b.is_modified());
    assert!(b.undo());
    assert_eq!(b.text(), "foo a foo\nb foo\nfoo");
    assert!(!b.undo(), "one step only");
}

#[test]
fn replace_matches_with_nothing_changes_nothing() {
    let mut b = Buffer::from_text("foo");
    assert_eq!(b.replace_matches(&[], 3, "x"), 0);
    assert!(!b.is_modified());
}

#[test]
fn replace_matches_may_delete_and_may_add_line_breaks() {
    let mut b = Buffer::from_text("a foo b foo");
    let (all, len) = b.find_matches("foo", None);
    b.replace_matches(&all, len, "");
    assert_eq!(b.text(), "a  b ");
    b.undo();
    b.replace_matches(&all, len, "1\n2");
    assert_eq!(b.text(), "a 1\n2 b 1\n2");
}
