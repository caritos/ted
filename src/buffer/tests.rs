use super::*;
use std::fs;
use std::path::PathBuf;

fn temp_path(name: &str) -> PathBuf {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "ted-test-{}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed),
        name
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir.join("file.txt")
}

fn typed(text: &str) -> Buffer {
    let mut b = Buffer::new();
    for c in text.chars() {
        if c == '\n' {
            b.insert_newline();
        } else {
            b.insert_char(c);
        }
    }
    b
}

#[test]
fn new_buffer_is_empty_and_clean() {
    let b = Buffer::new();
    assert_eq!(b.text(), "");
    assert_eq!(b.cursor(), (0, 0));
    assert_eq!(b.line_count(), 1);
    assert!(!b.is_modified());
}

#[test]
fn typing_inserts_at_cursor_and_marks_modified() {
    let b = typed("hi");
    assert_eq!(b.text(), "hi");
    assert_eq!(b.cursor(), (0, 2));
    assert!(b.is_modified());
}

#[test]
fn newline_splits_line() {
    let mut b = typed("abcd");
    b.move_left();
    b.move_left();
    b.insert_newline();
    assert_eq!(b.text(), "ab\ncd");
    assert_eq!(b.cursor(), (1, 0));
}

#[test]
fn insert_in_middle() {
    let mut b = typed("ac");
    b.move_left();
    b.insert_char('b');
    assert_eq!(b.text(), "abc");
    assert_eq!(b.cursor(), (0, 2));
}

#[test]
fn backspace_removes_previous_char() {
    let mut b = typed("abc");
    b.backspace();
    assert_eq!(b.text(), "ab");
    assert_eq!(b.cursor(), (0, 2));
}

#[test]
fn backspace_at_start_of_file_does_nothing() {
    let mut b = Buffer::new();
    b.backspace();
    assert_eq!(b.text(), "");
    assert!(!b.is_modified());
}

#[test]
fn backspace_at_line_start_joins_with_previous_line() {
    let mut b = Buffer::from_text("ab\ncd");
    b.move_down();
    assert_eq!(b.cursor(), (1, 0));
    b.backspace();
    assert_eq!(b.text(), "abcd");
    assert_eq!(b.cursor(), (0, 2));
}

#[test]
fn backspace_joining_crlf_line_removes_whole_break() {
    let mut b = Buffer::from_text("ab\r\ncd");
    b.move_down();
    assert_eq!(b.cursor(), (1, 0));
    b.backspace();
    assert_eq!(b.text(), "abcd");
    assert_eq!(b.cursor(), (0, 2));
}

#[test]
fn delete_removes_char_under_cursor() {
    let mut b = typed("abc");
    b.move_left();
    b.move_left();
    b.move_left();
    b.delete();
    assert_eq!(b.text(), "bc");
    assert_eq!(b.cursor(), (0, 0));
}

#[test]
fn delete_at_line_end_joins_next_line() {
    let mut b = Buffer::from_text("ab\ncd");
    b.move_right();
    b.move_right();
    b.delete();
    assert_eq!(b.text(), "abcd");
    assert_eq!(b.cursor(), (0, 2));
}

#[test]
fn delete_at_end_of_file_does_nothing() {
    let mut b = typed("ab");
    b.delete();
    assert_eq!(b.text(), "ab");
}

#[test]
fn left_wraps_to_end_of_previous_line() {
    let mut b = Buffer::from_text("abc\nd");
    b.move_down();
    b.move_left();
    assert_eq!(b.cursor(), (0, 3));
}

#[test]
fn right_wraps_to_start_of_next_line() {
    let mut b = Buffer::from_text("abc\nd");
    for _ in 0..3 {
        b.move_right();
    }
    b.move_right();
    assert_eq!(b.cursor(), (1, 0));
}

#[test]
fn movement_stops_at_edges() {
    let mut b = Buffer::from_text("a");
    b.move_left();
    b.move_up();
    assert_eq!(b.cursor(), (0, 0));
    b.move_right();
    b.move_right();
    b.move_down();
    assert_eq!(b.cursor(), (0, 1));
}

#[test]
fn vertical_movement_remembers_goal_column() {
    let mut b = Buffer::from_text("abcdef\nab\nabcdef");
    for _ in 0..5 {
        b.move_right();
    }
    b.move_down();
    assert_eq!(b.cursor(), (1, 2), "clamped to short line");
    b.move_down();
    assert_eq!(b.cursor(), (2, 5), "returns to the remembered column");
}

#[test]
fn trailing_newline_gives_an_empty_last_line() {
    let mut b = Buffer::from_text("a\n");
    assert_eq!(b.line_count(), 2);
    b.move_down();
    assert_eq!(b.cursor(), (1, 0));
}

#[test]
fn line_returns_text_without_line_break() {
    let b = Buffer::from_text("one\r\ntwo\nthree");
    assert_eq!(b.line(0), "one");
    assert_eq!(b.line(1), "two");
    assert_eq!(b.line(2), "three");
}

#[test]
fn unicode_chars_count_as_one_column() {
    let mut b = typed("héllo🌍");
    assert_eq!(b.cursor(), (0, 6));
    b.backspace();
    assert_eq!(b.text(), "héllo");
}

#[test]
fn open_missing_file_gives_empty_buffer_with_path() {
    let path = temp_path("missing");
    let b = Buffer::open(&path).unwrap();
    assert_eq!(b.text(), "");
    assert_eq!(b.path(), Some(path.as_path()));
    assert!(!b.is_modified());
}

#[test]
fn open_loads_file_contents() {
    let path = temp_path("load");
    fs::write(&path, "hello\nworld").unwrap();
    let b = Buffer::open(&path).unwrap();
    assert_eq!(b.text(), "hello\nworld");
    assert!(!b.is_modified());
}

#[test]
fn open_rejects_non_utf8_instead_of_corrupting() {
    let path = temp_path("binary");
    fs::write(&path, [0xFF, 0xFE, 0x00, 0xC0]).unwrap();
    let err = Buffer::open(&path).unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
}

#[test]
fn open_failure_other_than_not_found_is_an_error() {
    let dir = temp_path("isdir");
    let dir = dir.parent().unwrap();
    assert!(Buffer::open(dir).is_err());
}

#[test]
fn save_writes_file_and_clears_modified() {
    let path = temp_path("save");
    fs::write(&path, "old").unwrap();
    let mut b = Buffer::open(&path).unwrap();
    b.insert_char('!');
    assert!(b.is_modified());
    b.save().unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), "!old");
    assert!(!b.is_modified());
}

#[test]
fn save_creates_a_new_file_for_a_missing_path() {
    let path = temp_path("create");
    let mut b = Buffer::open(&path).unwrap();
    b.insert_char('x');
    b.save().unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), "x");
}

#[test]
fn save_without_a_path_is_an_error_and_stays_modified() {
    let mut b = typed("x");
    let err = b.save().unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
    assert!(b.is_modified());
}

#[test]
fn failed_save_leaves_the_buffer_modified() {
    let path = temp_path("nodir").parent().unwrap().join("does-not-exist").join("file.txt");
    let mut b = Buffer::open(&path).unwrap();
    b.insert_char('x');
    assert!(b.save().is_err());
    assert!(b.is_modified());
}

#[test]
fn save_leaves_no_temp_files_behind() {
    let path = temp_path("tmpclean");
    let mut b = Buffer::open(&path).unwrap();
    b.insert_char('x');
    b.save().unwrap();
    let entries = fs::read_dir(path.parent().unwrap()).unwrap().count();
    assert_eq!(entries, 1);
}

/// Buffer of `lines` lines "aaaa", cursor at (`row`, `col`), dirty row drained.
fn at(lines: usize, row: usize, col: usize) -> Buffer {
    let mut b = Buffer::from_text(&vec!["aaaa"; lines].join("\n"));
    for _ in 0..row {
        b.move_down();
    }
    for _ in 0..col {
        b.move_right();
    }
    b.take_dirty_row();
    b
}

#[test]
fn a_new_or_opened_buffer_is_dirty_from_row_zero() {
    let mut b = Buffer::from_text("a\nb");
    assert_eq!(b.take_dirty_row(), Some(0));
    assert_eq!(b.take_dirty_row(), None);
    let path = temp_path("dirty-open");
    fs::write(&path, "x\ny").unwrap();
    let mut b = Buffer::open(&path).unwrap();
    assert_eq!(b.take_dirty_row(), Some(0));
}

#[test]
fn moving_and_saving_do_not_mark_rows_dirty() {
    let path = temp_path("dirty-save");
    fs::write(&path, "ab\ncd").unwrap();
    let mut b = Buffer::open(&path).unwrap();
    b.take_dirty_row();
    b.move_down();
    b.move_right();
    b.move_up();
    b.move_left();
    b.save().unwrap();
    assert_eq!(b.take_dirty_row(), None);
}

#[test]
fn edits_mark_their_row_and_joins_or_splits_the_row_before() {
    let mut b = at(5, 2, 1);
    b.insert_char('x');
    assert_eq!(b.take_dirty_row(), Some(2));
    let mut b = at(5, 2, 1);
    b.backspace();
    assert_eq!(b.take_dirty_row(), Some(2));
    let mut b = at(5, 2, 1);
    b.delete();
    assert_eq!(b.take_dirty_row(), Some(2));
    // Enter splits; Backspace at column 0 and Delete at the end join.
    let mut b = at(5, 2, 1);
    b.insert_newline();
    assert_eq!(b.take_dirty_row(), Some(1));
    let mut b = at(5, 2, 0);
    b.backspace();
    assert_eq!(b.take_dirty_row(), Some(1));
    let mut b = at(5, 2, 4);
    b.delete();
    assert_eq!(b.take_dirty_row(), Some(1));
    // Row 0 saturates.
    let mut b = at(5, 0, 4);
    b.delete();
    assert_eq!(b.take_dirty_row(), Some(0));
    let mut b = at(5, 0, 0);
    b.insert_newline();
    assert_eq!(b.take_dirty_row(), Some(0));
}

#[test]
fn take_dirty_row_returns_the_lowest_row_since_the_last_take() {
    let mut b = at(10, 6, 0);
    b.insert_char('x');
    for _ in 0..3 {
        b.move_up();
    }
    b.insert_char('y');
    b.move_down();
    b.insert_char('z');
    assert_eq!(b.take_dirty_row(), Some(3));
    assert_eq!(b.take_dirty_row(), None);
    b.insert_char('w');
    assert_eq!(b.take_dirty_row(), Some(4));
}

/// Asserts the cursor is on a real row and column, then moves it every way.
fn assert_cursor_valid(b: &mut Buffer) {
    let (row, col) = b.cursor();
    assert!(row < b.line_count(), "row {row} of {}", b.line_count());
    assert!(col <= b.line(row).chars().count(), "col {col} on row {row}");
    b.move_right();
    b.move_left();
    b.move_down();
    b.move_up();
}

#[test]
fn enter_after_a_lone_cr_break_keeps_the_cursor_on_a_real_row() {
    // The new \n joins the \r before it into one \r\n break (issue #2).
    let mut b = Buffer::from_text("a\rb");
    b.move_down();
    b.insert_newline();
    assert_eq!(b.text(), "a\r\nb");
    assert_eq!(b.cursor(), (1, 0));
    assert_cursor_valid(&mut b);
}

#[test]
fn backspace_that_joins_a_cr_and_lf_keeps_the_cursor_valid() {
    let mut b = Buffer::from_text("a\rx\nb");
    b.move_down();
    b.move_right();
    b.backspace(); // removes x: "a\r\nb"
    assert_eq!(b.text(), "a\r\nb");
    assert_cursor_valid(&mut b);
}

#[test]
fn delete_that_joins_a_cr_and_lf_keeps_the_cursor_valid() {
    let mut b = Buffer::from_text("a\rx\nb");
    b.move_down();
    b.delete(); // removes x: "a\r\nb"
    assert_eq!(b.text(), "a\r\nb");
    assert_cursor_valid(&mut b);
}

fn f(row: usize, col: usize, wrapped: bool) -> Found {
    Found { row, col, wrapped }
}

#[test]
fn find_forward_from_cursor_and_include_from() {
    let b = Buffer::from_text("foo bar foo\nfoo");
    assert_eq!(b.find("foo", (0, 0), true, true), Some(f(0, 0, false)));
    assert_eq!(b.find("foo", (0, 0), true, false), Some(f(0, 8, false)));
    assert_eq!(b.find("foo", (0, 1), true, true), Some(f(0, 8, false)));
    assert_eq!(b.find("foo", (0, 9), true, true), Some(f(1, 0, false)));
}

#[test]
fn find_forward_wraps_to_top() {
    let b = Buffer::from_text("foo\nbar\nbaz");
    assert_eq!(b.find("foo", (1, 0), true, true), Some(f(0, 0, true)));
    assert_eq!(b.find("foo", (2, 3), true, false), Some(f(0, 0, true)));
}

#[test]
fn find_single_match_next_lands_on_itself_wrapped() {
    let b = Buffer::from_text("a\nneedle\nb");
    assert_eq!(b.find("needle", (1, 0), true, false), Some(f(1, 0, true)));
    assert_eq!(b.find("needle", (1, 0), false, false), Some(f(1, 0, true)));
}

#[test]
fn find_backward_is_strictly_before_and_wraps() {
    let b = Buffer::from_text("foo bar foo\nfoo");
    assert_eq!(b.find("foo", (1, 0), false, false), Some(f(0, 8, false)));
    assert_eq!(b.find("foo", (0, 8), false, false), Some(f(0, 0, false)));
    assert_eq!(b.find("foo", (0, 9), false, false), Some(f(0, 8, false)));
    assert_eq!(b.find("foo", (0, 0), false, false), Some(f(1, 0, true)));
}

#[test]
fn find_overlapping_matches_report_each_start() {
    let b = Buffer::from_text("aaaa");
    assert_eq!(b.find("aa", (0, 0), true, false), Some(f(0, 1, false)));
    assert_eq!(b.find("aa", (0, 3), false, false), Some(f(0, 2, false)));
}

#[test]
fn find_smart_case() {
    let b = Buffer::from_text("Foo foo FOO");
    // lowercase query ignores case
    assert_eq!(b.find("foo", (0, 0), true, false), Some(f(0, 4, false)));
    assert_eq!(b.find("foo", (0, 4), true, false), Some(f(0, 8, false)));
    // a query with an uppercase char is exact
    assert_eq!(b.find("Foo", (0, 0), true, false), Some(f(0, 0, true)));
    assert_eq!(b.find("FOO", (0, 0), true, true), Some(f(0, 8, false)));
    assert_eq!(b.find("fOo", (0, 0), true, true), None);
}

#[test]
fn find_column_is_a_char_index_for_non_ascii() {
    let b = Buffer::from_text("h\u{e9}llo \u{1F600} w\u{f6}rld\n\u{c9}t\u{e9}");
    assert_eq!(b.find("w\u{f6}rld", (0, 0), true, true), Some(f(0, 8, false)));
    assert_eq!(b.find("rld", (0, 0), true, true), Some(f(0, 10, false)));
    // smart case folds a non-ASCII letter without changing lengths
    assert_eq!(b.find("\u{e9}t", (0, 0), true, true), Some(f(1, 0, false)));
}

#[test]
fn find_with_crlf_lone_cr_and_no_trailing_newline() {
    let b = Buffer::from_text("one\r\ntwo\rthree\nfour");
    assert_eq!(b.find("one", (0, 0), true, true), Some(f(0, 0, false)));
    assert_eq!(b.find("two", (0, 0), true, true), Some(f(1, 0, false)));
    assert_eq!(b.find("three", (0, 0), true, true), Some(f(2, 0, false)));
    assert_eq!(b.find("four", (0, 0), true, true), Some(f(3, 0, false)));
    assert_eq!(b.find("our", (3, 3), false, false), Some(f(3, 1, false)));
    // a break is never part of a match
    assert_eq!(b.find("e\r", (0, 0), true, true), None);
    assert_eq!(b.find("e\n", (0, 0), true, true), None);
    assert_eq!(b.find("ee", (0, 0), true, true), Some(f(2, 3, false)));
}

#[test]
fn find_match_at_line_end_and_last_line() {
    let b = Buffer::from_text("ab\ncd");
    assert_eq!(b.find("b", (0, 0), true, false), Some(f(0, 1, false)));
    assert_eq!(b.find("d", (1, 0), true, false), Some(f(1, 1, false)));
    assert_eq!(b.find("d", (1, 1), true, false), Some(f(1, 1, true)));
}

#[test]
fn find_empty_query_and_too_long_query_and_missing() {
    let b = Buffer::from_text("abc\nde");
    assert_eq!(b.find("", (0, 0), true, true), None);
    assert_eq!(b.find("", (0, 0), false, false), None);
    assert_eq!(b.find("abcd", (0, 0), true, true), None);
    assert_eq!(b.find("zzz", (0, 0), true, true), None);
    assert_eq!(Buffer::new().find("a", (0, 0), true, true), None);
}

#[test]
fn find_does_not_touch_cursor_dirty_or_modified() {
    let mut b = Buffer::from_text("foo\nbar");
    b.take_dirty_row();
    b.move_down();
    let before = b.cursor();
    let _ = b.find("foo", before, true, true);
    assert_eq!(b.cursor(), before);
    assert_eq!(b.take_dirty_row(), None);
    assert!(!b.is_modified());
}

#[test]
fn set_cursor_clamps_and_does_not_modify() {
    let mut b = Buffer::from_text("abc\nde\r\nf");
    b.take_dirty_row();
    b.set_cursor(0, 2);
    assert_eq!(b.cursor(), (0, 2));
    b.set_cursor(1, 99);
    assert_eq!(b.cursor(), (1, 2));
    b.set_cursor(99, 99);
    assert_eq!(b.cursor(), (2, 1));
    b.set_cursor(99, 0);
    assert_eq!(b.cursor(), (2, 0));
    assert!(!b.is_modified());
    assert_eq!(b.take_dirty_row(), None);
    assert_cursor_valid(&mut b);
}

#[test]
fn set_cursor_resets_the_goal_column() {
    let mut b = Buffer::from_text("abcdef\nxy\nabcdef");
    b.set_cursor(0, 5);
    b.move_down(); // (1, 2), goal 5
    b.set_cursor(1, 1);
    b.move_down();
    assert_eq!(b.cursor(), (2, 1));
}

#[test]
fn go_to_line_is_one_based_and_clamped() {
    let mut b = Buffer::from_text("a\nbb\nccc");
    b.move_down();
    b.move_end();
    b.take_dirty_row();
    b.go_to_line(2);
    assert_eq!(b.cursor(), (1, 0));
    b.go_to_line(0);
    assert_eq!(b.cursor(), (0, 0));
    b.go_to_line(999);
    assert_eq!(b.cursor(), (2, 0));
    b.go_to_line(1);
    assert_eq!(b.cursor(), (0, 0));
    assert!(!b.is_modified());
    assert_eq!(b.take_dirty_row(), None);
    let mut e = Buffer::new();
    e.go_to_line(5);
    assert_eq!(e.cursor(), (0, 0));
}

/// Naive reference for `find`: every match listed, then the first or last
/// one that qualifies, wrapping when none does.
fn naive_find(b: &Buffer, query: &str, from: (usize, usize), forward: bool, include_from: bool) -> Option<Found> {
    if query.is_empty() {
        return None;
    }
    let exact = query.chars().any(char::is_uppercase);
    let needle: Vec<char> = query.chars().map(|c| fold(c, exact)).collect();
    let mut all = Vec::new();
    for row in 0..b.line_count() {
        let hay: Vec<char> = b.line(row).chars().map(|c| fold(c, exact)).collect();
        for col in 0..hay.len() {
            if hay[col..].starts_with(&needle) {
                all.push((row, col));
            }
        }
    }
    let hit = if forward {
        all.iter().copied().find(|&p| p > from || (include_from && p == from))
    } else {
        all.iter().copied().rev().find(|&p| p < from)
    };
    match hit {
        Some((row, col)) => Some(Found { row, col, wrapped: false }),
        None if forward => all.first().map(|&(row, col)| Found { row, col, wrapped: true }),
        None => all.last().map(|&(row, col)| Found { row, col, wrapped: true }),
    }
}

#[test]
fn find_matches_a_naive_reference_on_random_text() {
    let alphabet: Vec<char> = "aAbB \n\r\u{e9}\u{c9}x".chars().collect();
    let mut seed = 12345u64;
    let mut next = |n: usize| {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((seed >> 33) as usize) % n
    };
    for _ in 0..300 {
        let len = next(40);
        let text: String = (0..len).map(|_| alphabet[next(alphabet.len())]).collect();
        let b = Buffer::from_text(&text);
        for _ in 0..12 {
            let qlen = 1 + next(3);
            let query: String = (0..qlen).map(|_| alphabet[next(alphabet.len())]).collect();
            if query.contains(['\n', '\r']) {
                continue;
            }
            let from = (next(b.line_count() + 1), next(6));
            for forward in [true, false] {
                for include_from in [true, false] {
                    assert_eq!(
                        b.find(&query, from, forward, include_from),
                        naive_find(&b, &query, from, forward, include_from),
                        "text {text:?} query {query:?} from {from:?} fwd {forward} incl {include_from}"
                    );
                }
            }
        }
    }
}
