use super::*;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::fs;

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn shift(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::SHIFT)
}

fn ctrl_a() -> KeyEvent {
    KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL)
}

fn ctrl(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
}

fn plain(text: &str) -> Editor {
    Editor::new(Buffer::from_text(text))
}

fn rs_editor(text: &str) -> Editor {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!("ted-sel-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("x.rs");
    fs::write(&path, text).unwrap();
    Editor::new(Buffer::open(&path).unwrap())
}

fn press(e: &mut Editor, ev: KeyEvent, times: usize) {
    for _ in 0..times {
        e.handle_key(ev);
    }
}

/// Columns of `row` drawn Selected.
fn selected(v: &View, row: usize) -> Vec<usize> {
    v.styles[row].iter().enumerate().filter(|(_, s)| **s == Style::Selected).map(|(i, _)| i).collect()
}

#[test]
fn shift_right_selects_exactly_one_cell() {
    let mut e = plain("abc\ndef");
    e.handle_key(shift(KeyCode::Right));
    let v = e.view(20, 6);
    assert_eq!(selected(&v, 0), vec![0]);
    assert!(selected(&v, 1).is_empty());
    assert_eq!(v.lines[0], "abc");
}

#[test]
fn a_multi_line_selection_styles_each_lines_cells() {
    let mut e = plain("abc\n\ndef\nghi");
    e.handle_key(key(KeyCode::Right));
    press(&mut e, shift(KeyCode::Down), 2);
    let v = e.view(20, 8);
    assert_eq!(selected(&v, 0), vec![1, 2]);
    assert!(selected(&v, 1).is_empty());
    assert_eq!(selected(&v, 2), vec![0]);
    assert!(selected(&v, 3).is_empty());
}

#[test]
fn a_selection_with_the_cursor_at_either_end_looks_the_same() {
    let mut a = plain("abcdef");
    a.handle_key(key(KeyCode::Right));
    press(&mut a, shift(KeyCode::Right), 3);
    let mut b = plain("abcdef");
    press(&mut b, key(KeyCode::Right), 4);
    press(&mut b, shift(KeyCode::Left), 3);
    assert_eq!(selected(&a.view(20, 4), 0), vec![1, 2, 3]);
    assert_eq!(selected(&b.view(20, 4), 0), vec![1, 2, 3]);
}

#[test]
fn a_selection_that_starts_off_screen_styles_the_visible_part() {
    let text: String = (0..30).map(|i| format!("line {i}\n")).collect();
    let mut e = plain(&text);
    press(&mut e, shift(KeyCode::Down), 20);
    // Cursor on row 20, window of 4 text rows: rows 17..=20 show.
    let v = e.view(20, 6);
    assert_eq!(e.scroll_row, 17);
    assert_eq!(selected(&v, 0).len(), 7); // all of "line 17"
    assert_eq!(selected(&v, 2).len(), 7);
    assert!(selected(&v, 3).is_empty()); // cursor at column 0 of row 20
}

#[test]
fn a_selection_whose_other_end_is_below_the_window_styles_the_visible_part() {
    let text: String = (0..30).map(|i| format!("line {i}\n")).collect();
    let mut e = plain(&text);
    e.handle_key(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::CONTROL));
    for c in "21".chars() {
        e.handle_key(key(KeyCode::Char(c)));
    }
    e.handle_key(key(KeyCode::Enter));
    e.view(20, 6);
    press(&mut e, shift(KeyCode::Up), 10);
    // Anchor row 20 (below), cursor row 10: window rows 10..=13.
    let v = e.view(20, 6);
    assert_eq!(e.scroll_row, 10);
    for row in 0..4 {
        assert_eq!(selected(&v, row).len(), 7, "row {row}");
    }
}

#[test]
fn tabs_in_the_selection_are_styled_cell_by_cell() {
    let mut e = plain("a\tb");
    e.handle_key(key(KeyCode::Right));
    e.handle_key(shift(KeyCode::Right));
    let v = e.view(20, 4);
    assert_eq!(v.lines[0], "a   b");
    assert_eq!(selected(&v, 0), vec![1, 2, 3]);
}

#[test]
fn selection_wins_over_syntax_colors() {
    let mut e = rs_editor("fn main() {}\n");
    let before = e.view(30, 4);
    assert_eq!(before.styles[0][0], Style::Keyword);
    press(&mut e, shift(KeyCode::Right), 2);
    let v = e.view(30, 4);
    assert_eq!(&v.styles[0][..3], &[Style::Selected, Style::Selected, before.styles[0][2]]);
}

#[test]
fn the_gutter_is_never_selected() {
    let mut e = plain("abc\ndef");
    e.set_line_numbers(true);
    e.handle_key(ctrl_a());
    let v = e.view(30, 6);
    assert_eq!(v.lines[0], "  1 abc");
    assert!(v.styles[0][..4].iter().all(|s| *s == Style::LineNo));
    assert_eq!(selected(&v, 0), vec![4, 5, 6]);
    assert!(v.styles[1][..4].iter().all(|s| *s == Style::LineNo));
    assert_eq!(selected(&v, 1), vec![4, 5, 6]);
}

#[test]
fn columns_line_up_with_horizontal_scroll() {
    let mut e = plain("0123456789abcdefghij");
    e.handle_key(key(KeyCode::End));
    e.view(10, 4);
    press(&mut e, shift(KeyCode::Left), 3);
    let v = e.view(10, 4);
    assert!(e.scroll_col > 0);
    let drawn: String = v.lines[0].chars().zip(&v.styles[0]).filter(|(_, s)| **s == Style::Selected).map(|(c, _)| c).collect();
    assert_eq!(drawn, "hij");
}

#[test]
fn ctrl_a_then_typing_replaces_everything_and_undo_restores() {
    let mut e = plain("one\ntwo");
    e.handle_key(ctrl_a());
    e.handle_key(key(KeyCode::Char('x')));
    assert_eq!(e.buffer().text(), "x");
    assert!(e.buffer().selection().is_none());
    e.handle_key(ctrl('z'));
    assert_eq!(e.buffer().text(), "one\ntwo");
}

#[test]
fn enter_delete_and_backspace_act_on_the_selection() {
    let mut e = plain("abcd");
    press(&mut e, shift(KeyCode::Right), 2);
    e.handle_key(key(KeyCode::Backspace));
    assert_eq!(e.buffer().text(), "cd");
    press(&mut e, shift(KeyCode::Right), 1);
    e.handle_key(key(KeyCode::Delete));
    assert_eq!(e.buffer().text(), "d");
    press(&mut e, shift(KeyCode::Right), 1);
    e.handle_key(key(KeyCode::Enter));
    assert_eq!(e.buffer().text(), "\n");
}

#[test]
fn tab_over_a_selection_replaces_it_in_one_undo_step() {
    let mut e = plain("abcd");
    press(&mut e, shift(KeyCode::Right), 2);
    e.handle_key(key(KeyCode::Tab));
    assert_eq!(e.buffer().text(), "    cd");
    assert_eq!(e.buffer().cursor(), (0, 4));
    e.handle_key(ctrl('z'));
    assert_eq!(e.buffer().text(), "abcd");
}

#[test]
fn unshifted_keys_clear_the_selection() {
    for code in [KeyCode::Left, KeyCode::Right, KeyCode::Up, KeyCode::Down, KeyCode::Home, KeyCode::End, KeyCode::PageUp, KeyCode::PageDown] {
        let mut e = plain("abc\ndef\nghi");
        e.handle_key(key(KeyCode::Right));
        e.handle_key(shift(KeyCode::Right));
        assert!(e.buffer().selection().is_some());
        e.handle_key(key(code));
        assert!(e.buffer().selection().is_none(), "{code:?}");
        assert!(selected(&e.view(20, 6), 0).is_empty());
    }
}

#[test]
fn all_shifted_moves_extend_the_selection() {
    for code in [KeyCode::Left, KeyCode::Right, KeyCode::Up, KeyCode::Down, KeyCode::Home, KeyCode::End, KeyCode::PageUp, KeyCode::PageDown] {
        let mut e = plain("abc\ndef\nghi");
        e.handle_key(key(KeyCode::Down));
        e.handle_key(key(KeyCode::Right));
        e.handle_key(shift(code));
        assert!(e.buffer().selection().is_some(), "{code:?}");
    }
}

#[test]
fn esc_clears_the_selection() {
    let mut e = plain("abc");
    e.handle_key(ctrl_a());
    e.handle_key(key(KeyCode::Esc));
    assert!(e.buffer().selection().is_none());
    assert_eq!(e.buffer().text(), "abc");
}

#[test]
fn esc_in_a_prompt_cancels_the_prompt_only() {
    let mut e = plain("abc");
    e.handle_key(ctrl_a());
    e.handle_key(ctrl('f'));
    assert!(e.in_prompt());
    e.handle_key(key(KeyCode::Esc));
    // The prompt took the Esc (the selection is kept); nothing was edited.
    assert!(!e.in_prompt());
    assert!(e.buffer().selection().is_some());
    assert_eq!(e.buffer().text(), "abc");
}

#[test]
fn selecting_leaves_the_status_line_unchanged_but_the_position() {
    let mut e = plain("abc");
    let before = e.view(30, 4).status;
    e.handle_key(shift(KeyCode::Right));
    let after = e.view(30, 4).status;
    assert_eq!(before.replace("1:1", "1:2"), after);
    assert!(!after.contains("[+]"));
}

#[test]
fn clear_selection_drops_it() {
    let mut e = plain("abc");
    e.handle_key(ctrl_a());
    e.clear_selection();
    assert!(e.buffer().selection().is_none());
}

#[test]
fn tiny_sizes_with_a_selection_never_panic() {
    for gutter in [false, true] {
        for w in 0..=16 {
            for h in 0..=6 {
                let mut e = plain("a\tb\n\nlonger line here\nx");
                e.set_line_numbers(gutter);
                e.view(w, h);
                e.handle_key(key(KeyCode::Right));
                press(&mut e, shift(KeyCode::Down), 2);
                e.view(w, h);
                e.handle_key(ctrl_a());
                e.view(w, h);
                press(&mut e, shift(KeyCode::Left), 5);
                e.view(w, h);
            }
        }
    }
}

// --- prompts keep the selection on Esc -------------------------------------

fn selected_text(e: &Editor) -> Option<String> {
    e.buffer().selected_text()
}

fn type_chars(e: &mut Editor, text: &str) {
    for c in text.chars() {
        e.handle_key(key(KeyCode::Char(c)));
    }
}

#[test]
fn esc_in_find_goto_and_command_prompts_keeps_the_selection() {
    for open in ['f', 'l', 'x'] {
        let mut e = plain("one two\nthree two\nfour");
        e.handle_key(key(KeyCode::Right));
        press(&mut e, shift(KeyCode::Right), 4);
        let want = selected_text(&e);
        assert_eq!(want.as_deref(), Some("ne t"));
        let cursor = e.buffer().cursor();
        e.handle_key(ctrl(open));
        assert!(e.in_prompt());
        if open == 'f' {
            type_chars(&mut e, "two");
        } else if open == 'l' {
            type_chars(&mut e, "3");
        } else {
            type_chars(&mut e, "undo");
        }
        e.handle_key(key(KeyCode::Esc));
        assert!(!e.in_prompt());
        assert_eq!(selected_text(&e), want, "prompt {open}");
        assert_eq!(e.buffer().cursor(), cursor);
    }
}

#[test]
fn esc_keeps_a_backwards_selection_too() {
    let mut e = plain("abcdef");
    e.handle_key(key(KeyCode::End));
    press(&mut e, shift(KeyCode::Left), 3);
    let want = selected_text(&e);
    e.handle_key(ctrl('f'));
    type_chars(&mut e, "b");
    e.handle_key(key(KeyCode::Esc));
    assert_eq!(selected_text(&e), want);
    assert_eq!(want.as_deref(), Some("def"));
}

#[test]
fn cancel_prompt_keeps_the_selection() {
    let mut e = plain("abcdef");
    press(&mut e, shift(KeyCode::Right), 2);
    e.handle_key(ctrl('f'));
    type_chars(&mut e, "d");
    e.cancel_prompt();
    assert_eq!(selected_text(&e).as_deref(), Some("ab"));
}

#[test]
fn an_accepted_find_or_goto_clears_the_selection() {
    for (open, typed) in [('f', "def"), ('l', "2"), ('f', "zzz")] {
        let mut e = plain("abc def\nsecond");
        press(&mut e, shift(KeyCode::Right), 2);
        e.handle_key(ctrl(open));
        type_chars(&mut e, typed);
        e.handle_key(key(KeyCode::Enter));
        assert!(e.buffer().selection().is_none(), "{open} {typed}");
    }
}

#[test]
fn an_accepted_command_prompt_keeps_the_selection_for_the_command() {
    let mut e = plain("abc def");
    press(&mut e, shift(KeyCode::Right), 2);
    e.handle_key(ctrl('x'));
    type_chars(&mut e, "copy");
    e.handle_key(key(KeyCode::Enter));
    assert_eq!(selected_text(&e).as_deref(), Some("ab"));
    assert_eq!(e.take_command().as_deref(), Some("copy"));
}
