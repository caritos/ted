use super::*;
use std::fs;
use std::path::{Path, PathBuf};

/// A path in a fresh temp dir that is removed when this is dropped.
struct Tmp(PathBuf);

impl std::ops::Deref for Tmp {
    type Target = Path;
    fn deref(&self) -> &Path {
        &self.0
    }
}

impl Drop for Tmp {
    fn drop(&mut self) {
        if let Some(dir) = self.0.parent() {
            let _ = fs::remove_dir_all(dir);
        }
    }
}

fn temp_path(name: &str) -> Tmp {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "ted-undo-test-{}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed),
        name
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    Tmp(dir.join("file.txt"))
}

fn type_str(b: &mut Buffer, text: &str) {
    for c in text.chars() {
        if c == '\n' {
            b.insert_newline();
        } else {
            b.insert_char(c);
        }
    }
}

/// Asserts the cursor is on a real row and column.
fn assert_cursor_valid(b: &Buffer) {
    let (row, col) = b.cursor();
    assert!(row < b.line_count(), "row {row} of {}", b.line_count());
    assert!(col <= b.line(row).chars().count(), "col {col} on row {row}");
}

#[test]
fn typing_is_one_step_and_redo_restores_it() {
    let mut b = Buffer::new();
    type_str(&mut b, "abc");
    assert!(b.undo());
    assert_eq!(b.text(), "");
    assert_eq!(b.cursor(), (0, 0));
    assert!(!b.undo());
    assert!(b.redo());
    assert_eq!(b.text(), "abc");
    assert_eq!(b.cursor(), (0, 3));
    assert!(!b.redo());
}

#[test]
fn undo_and_redo_on_an_empty_history_do_nothing() {
    let mut b = Buffer::from_text("x");
    assert!(!b.undo());
    assert!(!b.redo());
    assert_eq!(b.text(), "x");
    assert!(!b.is_modified());
}

#[test]
fn enter_is_its_own_step() {
    let mut b = Buffer::new();
    type_str(&mut b, "ab\ncd");
    assert!(b.undo());
    assert_eq!(b.text(), "ab\n");
    assert_eq!(b.cursor(), (1, 0));
    assert!(b.undo());
    assert_eq!(b.text(), "ab");
    assert_eq!(b.cursor(), (0, 2));
    assert!(b.undo());
    assert_eq!(b.text(), "");
    assert!(!b.undo());
}

#[test]
fn a_backspace_run_merges_into_one_step() {
    let mut b = Buffer::from_text("abcd");
    b.move_end();
    b.backspace();
    b.backspace();
    b.backspace();
    assert_eq!(b.text(), "a");
    assert!(b.undo());
    assert_eq!(b.text(), "abcd");
    assert_eq!(b.cursor(), (0, 4));
    assert!(!b.undo());
    assert!(b.redo());
    assert_eq!(b.text(), "a");
    assert_eq!(b.cursor(), (0, 1));
}

#[test]
fn a_delete_run_merges_into_one_step() {
    let mut b = Buffer::from_text("abcd");
    b.delete();
    b.delete();
    b.delete();
    assert_eq!(b.text(), "d");
    assert!(b.undo());
    assert_eq!(b.text(), "abcd");
    assert_eq!(b.cursor(), (0, 0));
    assert!(!b.undo());
    assert!(b.redo());
    assert_eq!(b.text(), "d");
}

#[test]
fn backspace_over_a_newline_is_its_own_step() {
    let mut b = Buffer::from_text("ab\ncd");
    b.move_down();
    b.backspace(); // joins
    b.backspace(); // removes b
    assert_eq!(b.text(), "acd");
    assert!(b.undo());
    assert_eq!(b.text(), "abcd");
    assert_eq!(b.cursor(), (0, 2));
    assert!(b.undo());
    assert_eq!(b.text(), "ab\ncd");
    assert_eq!(b.cursor(), (1, 0));
    assert!(!b.undo());
}

#[test]
fn a_movement_between_typing_runs_makes_two_steps() {
    let mut b = Buffer::new();
    type_str(&mut b, "ab");
    b.move_left();
    b.move_right();
    type_str(&mut b, "cd");
    assert_eq!(b.text(), "abcd");
    assert!(b.undo());
    assert_eq!(b.text(), "ab");
    assert!(b.undo());
    assert_eq!(b.text(), "");
    assert!(!b.undo());
}

#[test]
fn typing_in_two_places_makes_two_steps() {
    let mut b = Buffer::from_text("ab\ncd");
    type_str(&mut b, "X");
    b.move_down();
    type_str(&mut b, "Y");
    assert_eq!(b.text(), "Xab\ncYd");
    assert!(b.undo());
    assert_eq!(b.text(), "Xab\ncd");
    assert!(b.undo());
    assert_eq!(b.text(), "ab\ncd");
}

#[test]
fn typing_after_an_undo_clears_redo_and_does_not_merge() {
    let mut b = Buffer::new();
    type_str(&mut b, "abc\nde");
    assert!(b.undo()); // removes "de"
    assert_eq!(b.text(), "abc\n");
    type_str(&mut b, "f");
    assert!(!b.redo());
    assert_eq!(b.text(), "abc\nf");
    assert!(b.undo());
    assert_eq!(b.text(), "abc\n");
    assert!(b.undo());
    assert_eq!(b.text(), "abc");
}

#[test]
fn typing_right_after_an_undo_and_redo_does_not_merge_into_the_older_edit() {
    let mut b = Buffer::new();
    type_str(&mut b, "ab");
    b.undo();
    b.redo();
    type_str(&mut b, "c");
    assert!(b.undo());
    assert_eq!(b.text(), "ab");
}

#[test]
fn cursor_after_undo_and_redo_of_a_line_split_and_join() {
    let mut b = Buffer::from_text("abcd");
    b.move_right();
    b.move_right();
    b.insert_newline();
    assert_eq!(b.cursor(), (1, 0));
    assert!(b.undo());
    assert_eq!(b.text(), "abcd");
    assert_eq!(b.cursor(), (0, 2));
    assert!(b.redo());
    assert_eq!(b.text(), "ab\ncd");
    assert_eq!(b.cursor(), (1, 0));
    b.backspace(); // join
    assert_eq!(b.cursor(), (0, 2));
    assert!(b.undo());
    assert_eq!(b.text(), "ab\ncd");
    assert_eq!(b.cursor(), (1, 0));
    assert!(b.redo());
    assert_eq!(b.text(), "abcd");
    assert_eq!(b.cursor(), (0, 2));
}

#[test]
fn undo_resets_the_goal_column_to_the_new_cursor() {
    let mut b = Buffer::from_text("abcdef\nx\nabcdef");
    b.set_cursor(0, 5);
    b.insert_char('Z');
    assert!(b.undo());
    assert_eq!(b.cursor(), (0, 5));
    b.move_down();
    assert_eq!(b.cursor(), (1, 1));
    b.move_down();
    assert_eq!(b.cursor(), (2, 5));
}

#[test]
fn a_new_buffer_typed_then_undone_is_unmodified() {
    let mut b = Buffer::new();
    type_str(&mut b, "hi");
    assert!(b.is_modified());
    b.undo();
    assert!(!b.is_modified());
    b.redo();
    assert!(b.is_modified());
}

#[test]
fn opening_a_missing_file_starts_clean_at_position_zero() {
    let path = temp_path("missing");
    let mut b = Buffer::open(&path).unwrap();
    assert!(!b.is_modified());
    type_str(&mut b, "x");
    assert!(b.is_modified());
    b.undo();
    assert!(!b.is_modified());
}

#[test]
fn save_then_edit_then_undo_is_unmodified() {
    let path = temp_path("a");
    let mut b = Buffer::open(&path).unwrap();
    type_str(&mut b, "hi");
    b.save().unwrap();
    assert!(!b.is_modified());
    type_str(&mut b, "!");
    assert!(b.is_modified());
    b.undo();
    assert!(!b.is_modified());
    assert_eq!(b.text(), "hi");
}

#[test]
fn typing_after_a_save_does_not_merge_into_the_saved_step() {
    let path = temp_path("b");
    let mut b = Buffer::open(&path).unwrap();
    type_str(&mut b, "hi");
    b.save().unwrap();
    type_str(&mut b, "!");
    b.undo();
    assert_eq!(b.text(), "hi");
}

#[test]
fn save_then_undo_is_modified_and_redo_is_clean_again() {
    let path = temp_path("c");
    let mut b = Buffer::open(&path).unwrap();
    type_str(&mut b, "hi");
    b.save().unwrap();
    b.undo();
    assert!(b.is_modified());
    b.redo();
    assert!(!b.is_modified());
}

#[test]
fn save_as_records_the_clean_position_too() {
    let path = temp_path("d");
    let mut b = Buffer::new();
    type_str(&mut b, "hi");
    b.save_as(path.to_path_buf()).unwrap();
    assert!(!b.is_modified());
    b.undo();
    assert!(b.is_modified());
    b.redo();
    assert!(!b.is_modified());
}

#[test]
fn failed_save_does_not_change_the_clean_position() {
    let mut b = Buffer::new();
    type_str(&mut b, "hi");
    assert!(b.save().is_err());
    assert!(b.is_modified());
    b.undo();
    assert!(!b.is_modified());
}

#[test]
fn undo_then_a_new_edit_makes_the_saved_state_unreachable() {
    let path = temp_path("e");
    let mut b = Buffer::open(&path).unwrap();
    type_str(&mut b, "ab");
    b.insert_newline();
    b.save().unwrap(); // saved: "ab\n"
    b.undo();
    type_str(&mut b, "c"); // "abc": same history length as the save, other text
    assert!(b.is_modified());
    b.undo();
    assert!(b.is_modified());
    assert_eq!(b.text(), "ab");
    b.redo();
    assert!(b.is_modified());
    b.insert_newline();
    assert!(b.is_modified());
}

#[test]
fn a_new_edit_after_undo_keeps_clean_when_it_was_before_the_cut() {
    let path = temp_path("f");
    let mut b = Buffer::open(&path).unwrap();
    type_str(&mut b, "a");
    b.save().unwrap();
    b.insert_newline();
    b.undo();
    b.insert_newline(); // cuts only the undone newline; the saved position survives
    b.undo();
    assert!(!b.is_modified());
}

#[test]
fn lone_cr_and_crlf_round_trip() {
    for text in ["a\rb", "a\r\nb", "a\rx\nb", "\r\r\n\n", "a\r\n\r\nb\r"] {
        let mut b = Buffer::from_text(text);
        b.move_down();
        b.delete();
        assert_cursor_valid(&b);
        b.backspace();
        assert_cursor_valid(&b);
        b.insert_newline();
        assert_cursor_valid(&b);
        let after = b.text();
        while b.undo() {
            assert_cursor_valid(&b);
        }
        assert_eq!(b.text(), text, "undo all of {text:?}");
        while b.redo() {
            assert_cursor_valid(&b);
        }
        assert_eq!(b.text(), after, "redo all of {text:?}");
    }
}

#[test]
fn undoing_a_lone_cr_join_restores_the_text_and_a_valid_cursor() {
    let mut b = Buffer::from_text("a\rx\nb");
    b.move_down();
    b.delete(); // "a\r\nb"
    assert_eq!(b.text(), "a\r\nb");
    assert!(b.undo());
    assert_eq!(b.text(), "a\rx\nb");
    assert_cursor_valid(&b);
    assert!(b.redo());
    assert_eq!(b.text(), "a\r\nb");
    assert_cursor_valid(&b);
}

#[test]
fn the_history_is_capped_and_modified_stays_correct() {
    let mut b = Buffer::new();
    for _ in 0..10_001 {
        b.insert_newline();
    }
    let mut undone = 0;
    while b.undo() {
        undone += 1;
    }
    assert_eq!(undone, 10_000);
    assert_eq!(b.text(), "\n");
    assert!(b.is_modified(), "the clean state was dropped");
}

#[test]
fn the_cap_keeps_a_clean_position_that_is_still_in_range() {
    let path = temp_path("g");
    let mut b = Buffer::open(&path).unwrap();
    for _ in 0..3 {
        b.insert_newline();
    }
    b.save().unwrap();
    for _ in 0..10_000 {
        b.insert_newline();
    }
    assert!(b.is_modified());
    let mut undone = 0;
    while b.undo() {
        undone += 1;
    }
    assert_eq!(undone, 10_000);
    assert_eq!(b.text(), "\n\n\n");
    assert!(!b.is_modified());
}

#[test]
fn undo_and_redo_mark_the_dirty_row() {
    let mut b = Buffer::from_text("a\nb\nc\nd\ne");
    b.take_dirty_row();
    b.set_cursor(3, 1);
    b.insert_char('x');
    b.take_dirty_row();
    assert!(b.undo());
    assert!(b.take_dirty_row().is_some_and(|r| r <= 3));
    assert!(b.redo());
    assert!(b.take_dirty_row().is_some_and(|r| r <= 3));
    b.set_cursor(3, 0);
    b.insert_newline();
    b.take_dirty_row();
    assert!(b.undo());
    assert!(b.take_dirty_row().is_some_and(|r| r <= 3));
    assert!(b.redo());
    assert!(b.take_dirty_row().is_some_and(|r| r <= 3));
    b.backspace();
    b.take_dirty_row();
    assert!(b.undo());
    assert!(b.take_dirty_row().is_some_and(|r| r <= 3));
}

#[test]
fn moving_does_not_touch_history() {
    let mut b = Buffer::from_text("abc");
    b.move_end();
    b.move_home();
    assert!(!b.undo());
    assert!(!b.is_modified());
}

struct Rng(u64);

impl Rng {
    fn below(&mut self, n: usize) -> usize {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((self.0 >> 33) as usize) % n
    }
}

fn rows(b: &Buffer) -> Vec<String> {
    (0..b.line_count()).map(|r| b.line(r)).collect()
}

/// Checks that rows above the reported dirty row did not change.
fn check_dirty(b: &mut Buffer, prev: &mut Vec<String>, context: &str) {
    let now = rows(b);
    let limit = b.take_dirty_row().unwrap_or(usize::MAX);
    for r in 0..limit.min(prev.len()).min(now.len()) {
        assert_eq!(prev[r], now[r], "row {r} changed but dirty row is {limit} ({context})");
    }
    *prev = now;
}

/// `split_steps` makes every edit its own step (a neutral cursor set before
/// each), so a snapshot model can follow the history exactly; without it the
/// merging is on and only the undo-all / redo-all round trips are checked.
fn random_run(seed: u64, split_steps: bool) {
    let starts = ["", "abc", "a\rb\r\nc\n", "x\r\ny\rz", "\r\r\n\n"];
    let mut rng = Rng(seed);
    let initial = starts[rng.below(starts.len())];
    let mut b = Buffer::from_text(initial);
    let tmp = temp_path("rand");
    b.path = Some(tmp.to_path_buf());
    let mut prev = rows(&b);
    b.take_dirty_row();
    // Model: the text at each history position, the position and clean state.
    let mut hist = vec![initial.to_string()];
    let mut pos = 0usize;
    let mut clean = Some(0usize);
    let mut saved_text = initial.to_string();
    for step in 0..120 {
        let op = rng.below(12);
        let ctx = format!("seed {seed} step {step} op {op}");
        if split_steps && op < 6 {
            let (r, c) = b.cursor();
            b.set_cursor(r, c);
        }
        let before = b.text();
        match op {
            0 | 1 => b.insert_char(if rng.below(2) == 0 { 'a' } else { 'b' }),
            2 => b.insert_newline(),
            3 => b.backspace(),
            4 | 5 => b.delete(),
            6 => match rng.below(4) {
                0 => b.move_left(),
                1 => b.move_right(),
                2 => b.move_up(),
                _ => b.move_down(),
            },
            7 | 8 => {
                let did = b.undo();
                if split_steps {
                    assert_eq!(did, pos > 0, "{ctx}");
                    if did {
                        pos -= 1;
                        assert_eq!(b.text(), hist[pos], "{ctx}");
                    }
                    assert_eq!(b.is_modified(), clean != Some(pos), "{ctx}");
                }
                check_dirty(&mut b, &mut prev, &ctx);
                assert_cursor_valid(&b);
                continue;
            }
            9 => {
                let did = b.redo();
                if split_steps {
                    assert_eq!(did, pos + 1 < hist.len(), "{ctx}");
                    if did {
                        pos += 1;
                        assert_eq!(b.text(), hist[pos], "{ctx}");
                    }
                    assert_eq!(b.is_modified(), clean != Some(pos), "{ctx}");
                }
                check_dirty(&mut b, &mut prev, &ctx);
                assert_cursor_valid(&b);
                continue;
            }
            10 => {
                b.save().unwrap();
                saved_text = b.text();
                clean = Some(pos);
                assert!(!b.is_modified(), "{ctx}");
                continue;
            }
            _ => {}
        }
        assert_cursor_valid(&b);
        check_dirty(&mut b, &mut prev, &ctx);
        let after = b.text();
        if split_steps && after != before {
            hist.truncate(pos + 1);
            if clean.is_some_and(|c| c > pos) {
                clean = None;
            }
            hist.push(after);
            pos += 1;
            assert_eq!(b.is_modified(), clean != Some(pos), "{ctx}");
        }
        if !b.is_modified() {
            assert_eq!(b.text(), saved_text, "clean but text differs ({ctx})");
        }
    }
    while b.redo() {}
    let final_text = b.text();
    while b.undo() {
        assert_cursor_valid(&b);
    }
    assert_eq!(b.text(), initial, "undo all, seed {seed}");
    while b.redo() {
        assert_cursor_valid(&b);
    }
    assert_eq!(b.text(), final_text, "redo all, seed {seed}");
}

#[test]
fn randomized_edits_match_a_snapshot_model() {
    for seed in 0..300 {
        random_run(seed, true);
    }
}

#[test]
fn randomized_edits_with_merging_round_trip() {
    for seed in 1000..1300 {
        random_run(seed, false);
    }
}

#[test]
fn typing_after_an_undo_does_not_merge_into_the_older_edit() {
    let mut b = Buffer::new();
    type_str(&mut b, "ab");
    b.move_left();
    b.move_right();
    type_str(&mut b, "cd");
    assert!(b.undo());
    assert_eq!(b.text(), "ab");
    type_str(&mut b, "e");
    assert!(b.undo());
    assert_eq!(b.text(), "ab");
}

#[test]
fn typing_after_a_redo_does_not_merge_into_the_redone_edit() {
    let mut b = Buffer::new();
    type_str(&mut b, "ab");
    b.move_left();
    b.move_right();
    type_str(&mut b, "cd");
    assert!(b.undo());
    assert!(b.redo());
    type_str(&mut b, "e");
    assert!(b.undo());
    assert_eq!(b.text(), "abcd");
}

#[test]
fn every_kind_of_movement_splits_a_typing_run() {
    type Move = fn(&mut Buffer);
    let moves: Vec<(&str, Move)> = vec![
        ("left", |b| {
            b.move_left();
            b.move_right();
        }),
        ("right", |b| {
            b.move_right();
            b.move_left();
        }),
        ("up", |b| {
            b.move_up();
        }),
        ("down", |b| {
            b.move_down();
            b.move_up();
        }),
        ("home", |b| {
            b.move_home();
            b.move_end();
        }),
        ("end", |b| {
            b.move_end();
        }),
        ("up_by", |b| {
            b.move_down_by(1);
            b.move_up_by(1);
        }),
        ("set_cursor", |b| b.set_cursor(0, 3)),
        ("go_to_line", |b| {
            b.go_to_line(1);
            b.move_end();
        }),
    ];
    for (name, go) in moves {
        let mut b = Buffer::from_text("ab\nxx");
        b.set_cursor(0, 2);
        b.insert_char('c');
        let at = b.cursor();
        go(&mut b);
        assert_eq!(b.cursor(), at, "{name} did not return to the same spot");
        b.insert_char('d');
        assert!(b.undo(), "{name}");
        assert_eq!(b.text(), "abc\nxx", "{name}: the move did not split the run");
    }
}

#[test]
fn merging_a_long_run_stays_linear() {
    use std::time::Instant;
    const N: usize = 100_000;
    let start = Instant::now();
    let mut typing = Edit::new(0, "", "a", 0, 1, false);
    let mut back = Edit::new(N - 1, "a", "", N, N - 1, true);
    let mut delete = Edit::new(0, "a", "", 0, 0, false);
    for i in 1..N {
        assert!(typing.merge(&Edit::new(i, "", "a", i, i + 1, false)));
        assert!(back.merge(&Edit::new(N - 1 - i, "a", "", N - i, N - 1 - i, true)));
        assert!(delete.merge(&Edit::new(0, "a", "", 0, 0, false)));
    }
    assert_eq!(typing.inserted_len, N);
    assert_eq!(back.removed_text().len(), N);
    assert_eq!(back.index, 0);
    assert_eq!(delete.removed_text().len(), N);
    let took = start.elapsed();
    assert!(took.as_secs_f64() < 2.0, "merging 3 x 100k edits took {took:?}");
}

#[test]
fn long_typing_backspace_and_delete_runs_stay_linear() {
    use std::time::Instant;
    const N: usize = 20_000;
    let start = Instant::now();
    let mut b = Buffer::new();
    for _ in 0..N {
        b.insert_char('a');
    }
    for _ in 0..N {
        b.backspace();
    }
    assert_eq!(b.text(), "");
    assert!(b.undo()); // the whole backspace run
    assert_eq!(b.text().len(), N);
    assert!(b.undo()); // the whole typing run
    assert_eq!(b.text(), "");
    let mut b = Buffer::from_text(&"a".repeat(N));
    for _ in 0..N {
        b.delete();
    }
    assert_eq!(b.text(), "");
    assert!(b.undo());
    assert_eq!(b.text().len(), N);
    assert!(!b.undo());
    let took = start.elapsed();
    assert!(took.as_secs_f64() < 8.0, "20k-char runs took {took:?}");
}
