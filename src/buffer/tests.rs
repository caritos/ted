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
