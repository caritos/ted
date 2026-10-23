use super::*;

fn assert_cursor_valid(b: &Buffer) {
    let (row, col) = b.cursor();
    assert!(row < b.line_count(), "row {row} of {}", b.line_count());
    assert!(col <= b.line(row).chars().count(), "col {col} on row {row}");
}

fn sel(b: &Buffer) -> Option<((usize, usize), (usize, usize))> {
    b.selection()
}

fn temp_file(name: &str) -> PathBuf {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "ted-sel-test-{}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed),
        name
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir.join("file.txt")
}

// --- selecting -------------------------------------------------------------

#[test]
fn extending_right_selects_one_char() {
    let mut b = Buffer::from_text("abc");
    assert_eq!(sel(&b), None);
    b.move_right_select(true);
    assert_eq!(sel(&b), Some(((0, 0), (0, 1))));
    assert_eq!(b.cursor(), (0, 1));
    b.move_right_select(true);
    assert_eq!(sel(&b), Some(((0, 0), (0, 2))));
}

#[test]
fn extending_in_every_direction_orders_start_and_end() {
    let text = "abc\nde\nfghi";
    let at = |row, col| {
        let mut b = Buffer::from_text(text);
        b.set_cursor(row, col);
        b
    };
    let mut b = at(1, 1);
    b.move_left_select(true);
    assert_eq!(sel(&b), Some(((1, 0), (1, 1))));
    // Left across a line start goes to the end of the line above.
    b.move_left_select(true);
    assert_eq!(sel(&b), Some(((0, 3), (1, 1))));
    let mut b = at(1, 1);
    b.move_right_select(true);
    b.move_right_select(true); // over the line end onto the next row
    assert_eq!(sel(&b), Some(((1, 1), (2, 0))));
    let mut b = at(1, 1);
    b.move_up_select(true);
    assert_eq!(sel(&b), Some(((0, 1), (1, 1))));
    let mut b = at(1, 1);
    b.move_down_select(true);
    assert_eq!(sel(&b), Some(((1, 1), (2, 1))));
    let mut b = at(1, 1);
    b.move_home_select(true);
    assert_eq!(sel(&b), Some(((1, 0), (1, 1))));
    let mut b = at(1, 1);
    b.move_end_select(true);
    assert_eq!(sel(&b), Some(((1, 1), (1, 2))));
    let mut b = at(2, 2);
    b.move_up_by_select(2, true);
    assert_eq!(sel(&b), Some(((0, 2), (2, 2))));
    let mut b = at(0, 2);
    b.move_down_by_select(5, true);
    assert_eq!(sel(&b), Some(((0, 2), (2, 2))));
}

#[test]
fn the_anchor_stays_while_extending_both_ways() {
    let mut b = Buffer::from_text("abcdef");
    b.set_cursor(0, 3);
    b.move_right_select(true);
    b.move_right_select(true);
    assert_eq!(sel(&b), Some(((0, 3), (0, 5))));
    b.move_left_select(true);
    b.move_left_select(true);
    b.move_left_select(true);
    assert_eq!(sel(&b), Some(((0, 2), (0, 3))));
    assert_eq!(b.cursor(), (0, 2));
}

#[test]
fn extending_back_to_the_anchor_leaves_no_selection() {
    let mut b = Buffer::from_text("abc\ndef");
    b.set_cursor(0, 1);
    b.move_down_select(true);
    assert!(sel(&b).is_some());
    b.move_up_select(true);
    assert_eq!(sel(&b), None);
    assert_eq!(b.selected_text(), None);
    // And a fresh anchor is taken afterwards.
    b.move_right_select(true);
    assert_eq!(sel(&b), Some(((0, 1), (0, 2))));
}

#[test]
fn extending_keeps_the_goal_column() {
    let mut b = Buffer::from_text("abcdef\nx\nabcdef");
    b.set_cursor(0, 5);
    b.move_down_select(true);
    b.move_down_select(true);
    assert_eq!(b.cursor(), (2, 5));
    assert_eq!(sel(&b), Some(((0, 5), (2, 5))));
}

#[test]
fn extending_at_the_edges_selects_nothing_new() {
    let mut b = Buffer::from_text("ab");
    b.move_left_select(true);
    b.move_up_select(true);
    assert_eq!(sel(&b), None);
    b.move_end();
    b.move_right_select(true);
    b.move_down_select(true);
    assert_eq!(sel(&b), None);
}

#[test]
fn select_all_selects_the_whole_text_with_the_cursor_at_the_end() {
    let mut b = Buffer::from_text("ab\ncd\n");
    b.select_all();
    assert_eq!(sel(&b), Some(((0, 0), (2, 0))));
    assert_eq!(b.cursor(), (2, 0));
    assert_eq!(b.selected_text().as_deref(), Some("ab\ncd\n"));
    assert!(!b.is_modified());
}

#[test]
fn select_all_on_an_empty_buffer_selects_nothing() {
    let mut b = Buffer::new();
    b.select_all();
    assert_eq!(sel(&b), None);
    assert_eq!(b.selected_text(), None);
    assert!(!b.delete_selection());
}

#[test]
fn clear_selection_keeps_the_cursor() {
    let mut b = Buffer::from_text("abc");
    b.select_all();
    b.clear_selection();
    assert_eq!(sel(&b), None);
    assert_eq!(b.cursor(), (0, 3));
}

#[test]
fn selected_text_across_line_breaks_and_multibyte_chars() {
    for text in ["a\nb", "a\r\nb", "a\rb", "x\r\n\r\ny\n"] {
        let mut b = Buffer::from_text(text);
        b.select_all();
        assert_eq!(b.selected_text().as_deref(), Some(text), "{text:?}");
    }
    let mut b = Buffer::from_text("h\u{e9}llo w\u{f6}rld\n\u{1f600}x");
    b.set_cursor(0, 1);
    b.move_right_select(true);
    assert_eq!(b.selected_text().as_deref(), Some("\u{e9}"));
    b.set_cursor(0, 7);
    b.move_down_select(true);
    b.move_right_select(true);
    assert_eq!(b.selected_text().as_deref(), Some("\u{f6}rld\n\u{1f600}x"));
    assert_eq!(sel(&b), Some(((0, 7), (1, 2))));
}

#[test]
fn unshifted_moves_and_cursor_jumps_clear_the_selection() {
    type Act = fn(&mut Buffer);
    let acts: Vec<(&str, Act)> = vec![
        ("left", |b| b.move_left()),
        ("right", |b| b.move_right()),
        ("up", |b| b.move_up()),
        ("down", |b| b.move_down()),
        ("home", |b| b.move_home()),
        ("end", |b| b.move_end()),
        ("up_by", |b| b.move_up_by(1)),
        ("down_by", |b| b.move_down_by(1)),
        ("set_cursor", |b| b.set_cursor(0, 1)),
        ("go_to_line", |b| b.go_to_line(2)),
        ("left unselected", |b| b.move_left_select(false)),
    ];
    for (name, act) in acts {
        let mut b = Buffer::from_text("abc\ndef");
        b.set_cursor(0, 1);
        b.move_right_select(true);
        b.move_down_select(true);
        assert!(sel(&b).is_some(), "{name}");
        act(&mut b);
        assert_eq!(sel(&b), None, "{name}");
        assert!(!b.is_modified(), "{name}");
    }
}

#[test]
fn undo_redo_and_save_clear_the_selection() {
    let path = temp_file("clear");
    let mut b = Buffer::open(&path).unwrap();
    b.insert_char('a');
    b.move_left();
    b.move_right_select(true);
    b.undo();
    assert_eq!(sel(&b), None);
    b.select_all();
    b.redo();
    assert_eq!(sel(&b), None);
    b.select_all();
    b.save().unwrap();
    assert_eq!(sel(&b), None);
    b.select_all();
    let other = temp_file("clear2");
    b.save_as(other.clone()).unwrap();
    assert_eq!(sel(&b), None);
    let _ = fs::remove_dir_all(path.parent().unwrap());
    let _ = fs::remove_dir_all(other.parent().unwrap());
}

// --- delete_selection ------------------------------------------------------

#[test]
fn delete_selection_is_one_undo_step() {
    let mut b = Buffer::from_text("ab\ncd\nef");
    b.set_cursor(0, 1);
    b.move_down_select(true);
    b.move_down_select(true);
    b.move_right_select(true);
    assert_eq!(sel(&b), Some(((0, 1), (2, 2))));
    assert!(b.delete_selection());
    assert_eq!(b.text(), "a");
    assert_eq!(b.cursor(), (0, 1));
    assert_eq!(sel(&b), None);
    assert!(b.is_modified());
    assert!(b.undo());
    assert_eq!(b.text(), "ab\ncd\nef");
    assert_eq!(b.cursor(), (2, 2));
    assert!(!b.is_modified());
    assert!(!b.undo());
    assert!(b.redo());
    assert_eq!(b.text(), "a");
    assert_eq!(b.cursor(), (0, 1));
    assert!(b.is_modified());
}

#[test]
fn delete_selection_with_the_cursor_at_the_start_restores_it_there() {
    let mut b = Buffer::from_text("abcd");
    b.set_cursor(0, 3);
    b.move_left_select(true);
    b.move_left_select(true);
    assert!(b.delete_selection());
    assert_eq!(b.text(), "ad");
    assert!(b.undo());
    assert_eq!(b.text(), "abcd");
    assert_eq!(b.cursor(), (0, 1));
}

#[test]
fn delete_selection_without_a_selection_does_nothing() {
    let mut b = Buffer::from_text("abc");
    assert!(!b.delete_selection());
    assert!(!b.can_undo());
    assert!(!b.is_modified());
}

#[test]
fn delete_selection_never_merges_with_neighbours() {
    let mut b = Buffer::from_text("abcdef");
    b.set_cursor(0, 6);
    b.backspace(); // run
    b.backspace();
    b.move_left_select(true);
    b.move_left_select(true);
    b.delete_selection();
    b.backspace(); // must not merge into the selection delete
    assert_eq!(b.text(), "a");
    assert!(b.undo());
    assert_eq!(b.text(), "ab");
    assert!(b.undo());
    assert_eq!(b.text(), "abcd");
    assert!(b.undo());
    assert_eq!(b.text(), "abcdef");
    assert!(!b.undo());
}

#[test]
fn delete_selection_marks_the_dirty_row() {
    let mut b = Buffer::from_text("a\nb\nc\nd\ne\nf");
    b.take_dirty_row();
    b.set_cursor(3, 0);
    b.move_down_select(true);
    assert!(b.delete_selection());
    assert!(b.take_dirty_row().is_some_and(|r| r <= 2));
    assert!(b.undo());
    assert!(b.take_dirty_row().is_some_and(|r| r <= 2));
    b.set_cursor(3, 0);
    b.move_right_select(true); // moves to the end... within the row there is 1 char
    assert!(b.delete_selection());
    assert!(b.take_dirty_row().is_some_and(|r| r <= 3));
}

// --- insert_str ------------------------------------------------------------

#[test]
fn insert_str_without_a_selection_is_one_step() {
    let mut b = Buffer::from_text("ab\ncd");
    b.set_cursor(0, 1);
    b.take_dirty_row();
    b.insert_str("XY\nZ\n!");
    assert_eq!(b.text(), "aXY\nZ\n!b\ncd");
    assert_eq!(b.cursor(), (2, 1));
    assert!(b.is_modified());
    assert!(b.take_dirty_row().is_some_and(|r| r == 0));
    assert!(b.undo());
    assert_eq!(b.text(), "ab\ncd");
    assert_eq!(b.cursor(), (0, 1));
    assert!(!b.is_modified());
    assert!(!b.undo());
    assert!(b.redo());
    assert_eq!(b.text(), "aXY\nZ\n!b\ncd");
    assert_eq!(b.cursor(), (2, 1));
}

#[test]
fn insert_str_of_one_line_marks_only_its_row() {
    let mut b = Buffer::from_text("a\nb\nc\nd");
    b.take_dirty_row();
    b.set_cursor(2, 1);
    b.insert_str("xyz");
    assert_eq!(b.take_dirty_row(), Some(2));
    assert_eq!(b.cursor(), (2, 4));
}

#[test]
fn insert_str_with_line_breaks_marks_the_row_above() {
    let mut b = Buffer::from_text("a\nb\nc\nd");
    b.take_dirty_row();
    b.set_cursor(2, 1);
    b.insert_str("x\ny");
    assert_eq!(b.take_dirty_row(), Some(1));
}

#[test]
fn insert_str_replaces_the_selection_in_one_step() {
    let mut b = Buffer::from_text("one two three");
    b.set_cursor(0, 4);
    for _ in 0..3 {
        b.move_right_select(true);
    }
    b.insert_str("2\n2");
    assert_eq!(b.text(), "one 2\n2 three");
    assert_eq!(b.cursor(), (1, 1));
    assert_eq!(sel(&b), None);
    assert!(b.undo());
    assert_eq!(b.text(), "one two three");
    assert_eq!(b.cursor(), (0, 7));
    assert!(!b.is_modified());
    assert!(!b.undo());
    assert!(b.redo());
    assert_eq!(b.text(), "one 2\n2 three");
}

#[test]
fn insert_str_is_not_merged_with_typing_around_it() {
    let mut b = Buffer::new();
    b.insert_char('a');
    b.insert_str("bc");
    b.insert_char('d');
    b.insert_char('e');
    assert_eq!(b.text(), "abcde");
    assert!(b.undo());
    assert_eq!(b.text(), "abc");
    assert!(b.undo());
    assert_eq!(b.text(), "a");
    assert!(b.undo());
    assert_eq!(b.text(), "");
}

#[test]
fn insert_str_of_nothing_does_nothing_but_still_replaces_a_selection() {
    let mut b = Buffer::from_text("abc");
    b.insert_str("");
    assert!(!b.can_undo());
    b.select_all();
    b.insert_str("");
    assert_eq!(b.text(), "");
    assert!(b.undo());
    assert_eq!(b.text(), "abc");
}

#[test]
fn insert_str_keeps_odd_text_as_given() {
    let mut b = Buffer::new();
    b.insert_str("a\r\nb\rc\u{1b}[31m\t\u{e9}");
    assert_eq!(b.text(), "a\r\nb\rc\u{1b}[31m\t\u{e9}");
    assert_cursor_valid(&b);
    assert_eq!(b.cursor(), (2, 8));
}

#[test]
fn insert_str_next_to_a_lone_cr_joins_safely() {
    let mut b = Buffer::from_text("a\rb");
    b.set_cursor(1, 0);
    b.insert_str("\n");
    assert_eq!(b.text(), "a\r\nb");
    assert_cursor_valid(&b);
    assert!(b.undo());
    assert_eq!(b.text(), "a\rb");
    assert_cursor_valid(&b);
    assert!(b.redo());
    assert_eq!(b.text(), "a\r\nb");
    assert_cursor_valid(&b);
    assert!(b.take_dirty_row().is_some_and(|r| r == 0));
}

#[test]
fn a_selection_deleted_next_to_crlf_keeps_the_cursor_valid() {
    for text in ["a\r\nb\r\nc", "a\rb\rc\r", "\r\r\n\n", "x\r\ny"] {
        let mut b = Buffer::from_text(text);
        b.select_all();
        b.move_left_select(true);
        b.move_left_select(true);
        b.delete_selection();
        assert_cursor_valid(&b);
        while b.undo() {
            assert_cursor_valid(&b);
        }
        assert_eq!(b.text(), text);
    }
}

// --- typing over a selection ----------------------------------------------

fn select_bc() -> Buffer {
    let mut b = Buffer::from_text("abcd");
    b.set_cursor(0, 1);
    b.move_right_select(true);
    b.move_right_select(true);
    b
}

#[test]
fn typing_over_a_selection_is_one_step_and_the_next_char_starts_a_new_run() {
    let mut b = select_bc();
    b.insert_char('X');
    assert_eq!(b.text(), "aXd");
    assert_eq!(b.cursor(), (0, 2));
    assert_eq!(sel(&b), None);
    b.insert_char('Y');
    b.insert_char('Z');
    assert_eq!(b.text(), "aXYZd");
    assert!(b.undo()); // YZ
    assert_eq!(b.text(), "aXd");
    assert!(b.undo()); // the replacement alone
    assert_eq!(b.text(), "abcd");
    assert_eq!(b.cursor(), (0, 3));
    assert!(!b.is_modified());
    assert!(!b.undo());
    assert!(b.redo());
    assert_eq!(b.text(), "aXd");
    assert_eq!(b.cursor(), (0, 2));
}

#[test]
fn enter_over_a_selection_is_one_step() {
    let mut b = select_bc();
    b.insert_newline();
    assert_eq!(b.text(), "a\nd");
    assert_eq!(b.cursor(), (1, 0));
    assert!(b.undo());
    assert_eq!(b.text(), "abcd");
    assert!(!b.undo());
}

#[test]
fn backspace_and_delete_over_a_selection_only_delete_it() {
    let mut b = select_bc();
    b.backspace();
    assert_eq!(b.text(), "ad");
    assert_eq!(b.cursor(), (0, 1));
    assert!(b.undo());
    assert_eq!(b.text(), "abcd");
    assert!(!b.undo());
    let mut b = select_bc();
    b.delete();
    assert_eq!(b.text(), "ad");
    assert_eq!(b.cursor(), (0, 1));
    assert!(b.undo());
    assert_eq!(b.text(), "abcd");
    assert!(!b.undo());
}

#[test]
fn backspace_after_deleting_a_selection_starts_its_own_run() {
    let mut b = select_bc();
    b.backspace();
    b.backspace();
    b.backspace();
    assert_eq!(b.text(), "d");
    assert!(b.undo()); // both plain backspaces
    assert_eq!(b.text(), "ad");
    assert!(b.undo());
    assert_eq!(b.text(), "abcd");
}

#[test]
fn replacing_with_the_same_text_is_still_a_step() {
    let mut b = Buffer::from_text("abc");
    b.move_right_select(true);
    b.insert_char('a');
    assert_eq!(b.text(), "abc");
    assert!(b.is_modified());
    assert!(b.undo());
    assert!(!b.is_modified());
}

// --- performance -----------------------------------------------------------

#[test]
fn a_five_megabyte_paste_is_fast() {
    use std::time::Instant;
    let line = "let value = compute(42); // some text\n";
    let text = line.repeat(5_000_000 / line.len());
    assert!(text.len() >= 4_900_000);
    let mut b = Buffer::from_text("head\ntail");
    b.set_cursor(0, 2);
    let start = Instant::now();
    b.insert_str(&text);
    let pasted = start.elapsed();
    assert_eq!(b.rope.len_chars(), 9 + text.chars().count());
    assert!(pasted.as_secs_f64() < 1.0, "pasting {} bytes took {pasted:?}", text.len());
    let start = Instant::now();
    assert!(b.undo());
    assert!(b.redo());
    let round = start.elapsed();
    assert!(round.as_secs_f64() < 2.0, "undo + redo took {round:?}");
    b.select_all();
    let start = Instant::now();
    b.insert_str("x");
    let replaced = start.elapsed();
    assert_eq!(b.text(), "x");
    assert!(replaced.as_secs_f64() < 1.0, "replacing everything took {replaced:?}");
}

// --- randomized ------------------------------------------------------------

struct Rng(u64);

impl Rng {
    fn below(&mut self, n: usize) -> usize {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((self.0 >> 33) as usize) % n
    }
}

/// Char index of (`row`, `col`) in `text`, by an independent line splitter.
fn index_of(text: &str, row: usize, col: usize) -> usize {
    let chars: Vec<char> = text.chars().collect();
    let mut starts = vec![0usize];
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '\r' && chars.get(i + 1) == Some(&'\n') {
            i += 1;
            starts.push(i + 1);
        } else if is_break(c) {
            starts.push(i + 1);
        }
        i += 1;
    }
    starts[row] + col
}

fn slice(text: &str, from: usize, to: usize) -> String {
    text.chars().skip(from).take(to - from).collect()
}

fn splice(text: &str, from: usize, to: usize, with: &str) -> String {
    let total = text.chars().count();
    format!("{}{}{}", slice(text, 0, from), with, slice(text, to, total))
}

fn rows(b: &Buffer) -> Vec<String> {
    (0..b.line_count()).map(|r| b.line(r)).collect()
}

fn random_run(seed: u64) {
    let starts = ["", "abc", "a\rb\r\nc\n", "x\r\ny\rz", "\r\r\n\n", "h\u{e9}llo\nw\u{f6}rld"];
    let pieces = ["a", "b", "\n", "\r", "\r\n", "\u{e9}", "xy\nz", "\n\n", "q\r", "\t"];
    let mut rng = Rng(seed);
    let initial = starts[rng.below(starts.len())];
    let mut b = Buffer::from_text(initial);
    b.take_dirty_row();
    let mut prev = rows(&b);
    let mut hist = vec![initial.to_string()];
    let mut pos = 0usize;
    for step in 0..150 {
        let op = rng.below(16);
        let ctx = format!("seed {seed} step {step} op {op}");
        let before = b.text();
        let had_sel = b.selection();
        let range = had_sel.map(|(s, e)| {
            let (from, to) = (index_of(&before, s.0, s.1), index_of(&before, e.0, e.1));
            assert!(from < to, "{ctx}");
            (from, to)
        });
        assert_eq!(b.selected_text(), range.map(|(f, t)| slice(&before, f, t)), "{ctx}");
        let cur = b.cursor();
        let cur_idx = index_of(&before, cur.0, cur.1);
        // A plain edit with no selection must not merge into the last step.
        let mut expected: Option<String> = None;
        let mut stepped = false;
        match op {
            0..=3 => {
                match rng.below(8) {
                    0 => b.move_left_select(true),
                    1 => b.move_right_select(true),
                    2 => b.move_up_select(true),
                    3 => b.move_down_select(true),
                    4 => b.move_home_select(true),
                    5 => b.move_end_select(true),
                    6 => b.move_up_by_select(1 + rng.below(3), true),
                    _ => b.move_down_by_select(1 + rng.below(3), true),
                }
                assert_eq!(b.text(), before, "{ctx}");
            }
            4 => {
                b.select_all();
                assert_eq!(b.text(), before, "{ctx}");
            }
            5 => match rng.below(3) {
                0 => b.move_left(),
                1 => b.move_down(),
                _ => b.clear_selection(),
            },
            6 => {
                let piece = pieces[rng.below(pieces.len())];
                if had_sel.is_none() {
                    b.set_cursor(cur.0, cur.1);
                }
                let (from, to) = range.unwrap_or((cur_idx, cur_idx));
                b.insert_str(piece);
                expected = Some(splice(&before, from, to, piece));
                stepped = true;
                if piece.is_empty() {
                    unreachable!();
                }
            }
            7 | 8 => {
                let did = b.delete_selection();
                assert_eq!(did, range.is_some(), "{ctx}");
                if let Some((from, to)) = range {
                    expected = Some(splice(&before, from, to, ""));
                    stepped = true;
                    // Cursor goes to the start of the deleted range (modulo CR/LF joins).
                    let (r, c) = b.cursor();
                    assert!(index_of(&expected.clone().unwrap(), r, c) <= from, "{ctx}");
                }
            }
            9 | 10 => {
                if had_sel.is_none() {
                    b.set_cursor(cur.0, cur.1);
                }
                let c = if rng.below(2) == 0 { 'a' } else { '\u{e9}' };
                b.insert_char(c);
                stepped = true;
                let (from, to) = range.unwrap_or((cur_idx, cur_idx));
                expected = Some(splice(&before, from, to, &c.to_string()));
            }
            11 => {
                if had_sel.is_none() {
                    b.set_cursor(cur.0, cur.1);
                }
                b.insert_newline();
                stepped = true;
                let (from, to) = range.unwrap_or((cur_idx, cur_idx));
                expected = Some(splice(&before, from, to, "\n"));
            }
            12 | 13 => {
                if had_sel.is_none() {
                    b.set_cursor(cur.0, cur.1);
                }
                let back = op == 12;
                if back {
                    b.backspace();
                } else {
                    b.delete();
                }
                let total = before.chars().count();
                let expect = match range {
                    Some((f, t)) => Some((f, t)),
                    None if back && cur_idx > 0 => {
                        // Joining a line removes the whole break (CRLF = 2 chars).
                        let prefix = slice(&before, 0, cur_idx);
                        let n = if cur.1 == 0 && prefix.ends_with("\r\n") { 2 } else { 1 };
                        Some((cur_idx - n, cur_idx))
                    }
                    None if !back && cur_idx < total => {
                        let rest = slice(&before, cur_idx, total);
                        let n = if rest.starts_with("\r\n") { 2 } else { 1 };
                        Some((cur_idx, cur_idx + n))
                    }
                    None => None,
                };
                if let Some((f, t)) = expect {
                    expected = Some(splice(&before, f, t, ""));
                    stepped = true;
                }
            }
            14 => {
                let did = b.undo();
                assert_eq!(did, pos > 0, "{ctx}");
                if did {
                    pos -= 1;
                    assert_eq!(b.text(), hist[pos], "{ctx}");
                }
                assert_eq!(b.selection(), None, "{ctx}");
                assert_eq!(b.is_modified(), pos != 0, "{ctx}");
                dirty_ok(&mut b, &mut prev, &ctx);
                assert_cursor_valid(&b);
                continue;
            }
            _ => {
                let did = b.redo();
                assert_eq!(did, pos + 1 < hist.len(), "{ctx}");
                if did {
                    pos += 1;
                    assert_eq!(b.text(), hist[pos], "{ctx}");
                }
                assert_eq!(b.selection(), None, "{ctx}");
                assert_eq!(b.is_modified(), pos != 0, "{ctx}");
                dirty_ok(&mut b, &mut prev, &ctx);
                assert_cursor_valid(&b);
                continue;
            }
        }
        assert_cursor_valid(&b);
        dirty_ok(&mut b, &mut prev, &ctx);
        if let Some(sel) = b.selection() {
            let (from, to) = (index_of(&b.text(), sel.0 .0, sel.0 .1), index_of(&b.text(), sel.1 .0, sel.1 .1));
            assert!(from < to, "{ctx}");
        }
        match expected {
            Some(text) => {
                assert_eq!(b.text(), text, "{ctx}");
                assert_eq!(b.selection(), None, "{ctx}");
            }
            None => assert_eq!(b.text(), before, "{ctx}"),
        }
        if stepped {
            hist.truncate(pos + 1);
            hist.push(b.text());
            pos += 1;
        }
        assert_eq!(b.is_modified(), pos != 0, "{ctx}");
    }
    while b.redo() {}
    let final_text = b.text();
    assert_eq!(final_text, hist[hist.len() - 1], "seed {seed}");
    while b.undo() {
        assert_cursor_valid(&b);
    }
    assert_eq!(b.text(), initial, "undo all, seed {seed}");
    assert!(!b.is_modified(), "seed {seed}");
    while b.redo() {
        assert_cursor_valid(&b);
    }
    assert_eq!(b.text(), final_text, "redo all, seed {seed}");
}

/// Rows above the reported dirty row did not change.
fn dirty_ok(b: &mut Buffer, prev: &mut Vec<String>, context: &str) {
    let now = rows(b);
    let limit = b.take_dirty_row().unwrap_or(usize::MAX);
    for r in 0..limit.min(prev.len()).min(now.len()) {
        assert_eq!(prev[r], now[r], "row {r} changed but dirty row is {limit} ({context})");
    }
    *prev = now;
}

#[test]
fn randomized_selection_edits_match_a_reference() {
    for seed in 0..400 {
        random_run(seed);
    }
}

// --- history size and the cap ----------------------------------------------

#[test]
fn replace_steps_stay_under_the_byte_budget() {
    let big = "0123456789abcdef\n".repeat(12_000); // about 200 KB
    let mut b = Buffer::from_text(&big);
    b.hist_limit = 1 << 20;
    for i in 0..40 {
        b.select_all();
        b.insert_str(&format!("{i}{big}"));
        assert_eq!(b.hist_bytes, b.history_bytes(), "step {i}");
        let newest = b.undo.back().unwrap().bytes();
        assert!(b.history_bytes() <= b.hist_limit + newest, "step {i}: {}", b.history_bytes());
    }
    assert!(b.undo.len() < 40, "{} steps kept", b.undo.len());
    assert!(b.undo.len() >= 2);
    let mut undone = 0;
    while b.undo() {
        undone += 1;
        assert_eq!(b.hist_bytes, b.history_bytes());
    }
    assert_eq!(undone, b.redo.len());
    assert_eq!(b.hist_bytes, 0);
    while b.redo() {}
    assert_eq!(b.hist_bytes, b.history_bytes());
    assert!(b.text().ends_with(&big));
}

#[test]
fn a_single_step_over_the_budget_is_still_kept() {
    let mut b = Buffer::from_text("abc");
    b.hist_limit = 4;
    b.select_all();
    b.insert_str("a long replacement text");
    assert_eq!(b.undo.len(), 1);
    assert!(b.undo());
    assert_eq!(b.text(), "abc");
}

#[test]
fn dropping_the_clean_step_by_budget_makes_it_unreachable() {
    let big = "x".repeat(100_000);
    let mut b = Buffer::from_text("abc");
    b.hist_limit = 450_000;
    b.insert_char('!'); // step 1, then saved
    b.mark_clean();
    assert!(!b.is_modified());
    for _ in 0..6 {
        b.select_all();
        b.insert_str(&big);
    }
    assert!(b.undo.len() < 7, "oldest steps were dropped");
    while b.undo() {}
    assert!(b.is_modified(), "the saved state is gone");
    while b.redo() {}
    assert!(b.is_modified());
}

#[test]
fn dropping_older_steps_by_budget_shifts_the_clean_position() {
    let big = "x".repeat(100_000);
    let mut b = Buffer::from_text("abc");
    b.hist_limit = 450_000;
    for _ in 0..3 {
        b.select_all();
        b.insert_str(&big);
    }
    b.mark_clean();
    for _ in 0..6 {
        b.select_all();
        b.insert_str(&big);
        b.mark_clean();
    }
    assert!(!b.is_modified());
    assert!(b.undo());
    assert!(b.is_modified());
    assert!(b.redo());
    assert!(!b.is_modified());
    b.select_all();
    b.insert_str("zz");
    assert!(b.is_modified());
    assert!(b.undo());
    assert!(!b.is_modified());
}

#[test]
fn a_backspace_run_is_sealed_compactly_and_still_undoes() {
    let mut b = Buffer::from_text(&"ab".repeat(500));
    b.move_end();
    for _ in 0..300 {
        b.backspace();
    }
    assert!(!b.undo.back().unwrap().rev_buf.is_empty());
    b.move_left();
    b.insert_char('x'); // ends the run
    assert!(b.undo.iter().all(|e| e.rev_buf.is_empty()));
    assert_eq!(b.hist_bytes, b.history_bytes());
    b.undo();
    b.undo();
    assert_eq!(b.text(), "ab".repeat(500));
}

#[test]
fn save_replace_undo_is_unmodified_and_redo_is_modified() {
    let path = temp_file("srm");
    let mut b = Buffer::open(&path).unwrap();
    b.insert_str("hello");
    b.save().unwrap();
    assert!(!b.is_modified());
    b.select_all();
    b.insert_char('x');
    assert!(b.is_modified());
    assert!(b.undo());
    assert!(!b.is_modified());
    assert_eq!(b.text(), "hello");
    assert!(b.redo());
    assert!(b.is_modified());
    assert_eq!(b.text(), "x");
    let _ = fs::remove_dir_all(path.parent().unwrap());
}

#[test]
fn the_step_cap_with_replace_steps_drops_the_oldest() {
    let mut b = Buffer::from_text("abc");
    for i in 0..10_001 {
        b.select_all();
        b.insert_char(if i % 2 == 0 { 'a' } else { 'b' });
    }
    assert_eq!(b.text(), "a");
    let mut undone = 0;
    while b.undo() {
        undone += 1;
    }
    assert_eq!(undone, 10_000);
    assert_eq!(b.text(), "a", "the first replacement is the oldest kept state");
    assert!(b.is_modified(), "the clean state was dropped");
    let mut redone = 0;
    while b.redo() {
        redone += 1;
    }
    assert_eq!(redone, 10_000);
    assert_eq!(b.text(), "a");
    assert_eq!(b.hist_bytes, b.history_bytes());
}

// --- randomized, with merging, saves and cursor checks ---------------------

fn random_run_merging(seed: u64) {
    let starts = ["", "abc", "a\rb\r\nc\n", "x\r\ny\rz", "h\u{e9}llo\nw\u{f6}rld\n"];
    let pieces = ["a", "\n", "\r", "\r\n", "\u{e9}", "xy\nz", "q"];
    let mut rng = Rng(seed);
    let initial = starts[rng.below(starts.len())];
    let mut b = Buffer::from_text(initial);
    let path = temp_file("rm");
    b.path = Some(path.clone());
    b.take_dirty_row();
    let mut prev = rows(&b);
    // By history position: the text there; by step: cursor before and after it.
    let mut texts = vec![initial.to_string()];
    let mut befores: Vec<(usize, usize)> = Vec::new();
    let mut afters: Vec<(usize, usize)> = Vec::new();
    let mut clean = Some(0usize);
    for step in 0..200 {
        let op = rng.below(18);
        let ctx = format!("seed {seed} step {step} op {op}");
        let old_n = b.undo.len();
        let before_text = b.text();
        let before_cur = b.cursor();
        match op {
            0..=2 => match rng.below(6) {
                0 => b.move_left_select(true),
                1 => b.move_right_select(true),
                2 => b.move_up_select(true),
                3 => b.move_down_select(true),
                4 => b.move_end_select(true),
                _ => b.move_down_by_select(2, true),
            },
            3 => b.select_all(),
            4 => match rng.below(3) {
                0 => b.move_left(),
                1 => b.move_right(),
                _ => b.move_down(),
            },
            5 => b.insert_str(pieces[rng.below(pieces.len())]),
            6 => {
                b.delete_selection();
            }
            7 | 8 => b.insert_char(if rng.below(2) == 0 { 'a' } else { 'b' }),
            9 => b.insert_newline(),
            10 | 11 => b.backspace(),
            12 => b.delete(),
            13 | 14 => {
                if b.undo() {
                    let n = b.undo.len();
                    assert_eq!(n + 1, old_n, "{ctx}");
                    assert_eq!(b.text(), texts[n], "{ctx}");
                    assert_eq!(b.cursor(), befores[n], "{ctx}");
                } else {
                    assert_eq!(old_n, 0, "{ctx}");
                }
                assert_eq!(b.selection(), None, "{ctx}");
            }
            15 => {
                if b.redo() {
                    let n = b.undo.len();
                    assert_eq!(n, old_n + 1, "{ctx}");
                    assert_eq!(b.text(), texts[n], "{ctx}");
                    assert_eq!(b.cursor(), afters[n - 1], "{ctx}");
                }
                assert_eq!(b.selection(), None, "{ctx}");
            }
            _ => {
                b.save().unwrap();
                clean = Some(old_n);
            }
        }
        assert_cursor_valid(&b);
        dirty_ok(&mut b, &mut prev, &ctx);
        let n = b.undo.len();
        if (5..=12).contains(&op) && !(n == old_n && b.text() == before_text) {
            assert!(n == old_n || n == old_n + 1, "{ctx}");
            if clean.is_some_and(|c| c > old_n) {
                clean = None;
            }
            texts.truncate(old_n + 1);
            befores.truncate(old_n);
            afters.truncate(old_n);
            if n == old_n + 1 {
                texts.push(b.text());
                befores.push(before_cur);
                afters.push(b.cursor());
            } else {
                // Merged into the last step: only its end state changes.
                texts[old_n] = b.text();
                afters[old_n - 1] = b.cursor();
            }
        }
        assert_eq!(b.is_modified(), clean != Some(n), "{ctx}");
        assert_eq!(b.hist_bytes, b.history_bytes(), "{ctx}");
    }
    while b.redo() {}
    let last = b.text();
    assert_eq!(last, *texts.last().unwrap(), "seed {seed}");
    while b.undo() {
        let n = b.undo.len();
        assert_eq!(b.text(), texts[n], "seed {seed}");
        assert_eq!(b.cursor(), befores[n], "seed {seed}");
    }
    assert_eq!(b.text(), initial, "seed {seed}");
    assert_eq!(b.is_modified(), clean != Some(0), "seed {seed}");
    while b.redo() {}
    assert_eq!(b.text(), last, "seed {seed}");
    let _ = fs::remove_dir_all(path.parent().unwrap());
}

#[test]
fn randomized_edits_with_merging_saves_and_cursors() {
    for seed in 0..300 {
        random_run_merging(seed);
    }
}
