use super::*;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn shift(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::SHIFT)
}

fn ctrl(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
}

fn type_str(editor: &mut Editor, text: &str) {
    for c in text.chars() {
        editor.handle_key(key(KeyCode::Char(c)));
    }
}

fn ed(text: &str) -> Editor {
    Editor::new(Buffer::from_text(text))
}

/// Cells of `row` drawn as a match.
fn matched(v: &View, row: usize) -> Vec<usize> {
    v.styles[row].iter().enumerate().filter(|(_, s)| **s == Style::Match).map(|(i, _)| i).collect()
}

fn any_match(v: &View) -> bool {
    v.styles.iter().flatten().any(|s| *s == Style::Match)
}

#[test]
fn find_highlights_every_visible_match() {
    let mut e = ed("foo x foo\nnothing\nFOO foofoo");
    e.handle_key(ctrl('f'));
    type_str(&mut e, "foo");
    let v = e.view(40, 8);
    assert_eq!(matched(&v, 0), vec![0, 1, 2, 6, 7, 8]);
    assert!(matched(&v, 1).is_empty());
    // Smart case: all lower case matches any case; adjacent matches both drawn.
    assert_eq!(matched(&v, 2), vec![0, 1, 2, 4, 5, 6, 7, 8, 9]);
    assert!(matched(&v, 3).is_empty(), "the ~ row");
}

#[test]
fn an_uppercase_term_matches_exactly() {
    let mut e = ed("foo Foo");
    e.handle_key(ctrl('f'));
    type_str(&mut e, "Foo");
    assert_eq!(matched(&e.view(40, 5), 0), vec![4, 5, 6]);
}

#[test]
fn highlight_ends_with_the_prompt() {
    for closing in [KeyCode::Enter, KeyCode::Esc] {
        let mut e = ed("foo foo");
        e.handle_key(ctrl('f'));
        type_str(&mut e, "foo");
        assert!(any_match(&e.view(40, 5)));
        e.handle_key(key(closing));
        assert!(!any_match(&e.view(40, 5)), "{closing:?}");
    }
}

#[test]
fn no_highlight_without_a_term_or_in_other_prompts() {
    let mut e = ed("foo foo");
    e.handle_key(ctrl('f'));
    assert!(!any_match(&e.view(40, 5)));
    type_str(&mut e, "foo");
    for _ in 0..3 {
        e.handle_key(key(KeyCode::Backspace));
    }
    assert!(!any_match(&e.view(40, 5)));

    let mut e = ed("foo foo");
    e.set_last_search("foo");
    e.handle_key(ctrl('l'));
    assert!(!any_match(&e.view(40, 5)));
    e.handle_key(key(KeyCode::Esc));
    e.handle_key(ctrl('x'));
    type_str(&mut e, "foo");
    assert!(!any_match(&e.view(40, 5)));
    e.handle_key(key(KeyCode::Esc));
    e.handle_key(ctrl('r'));
    assert!(!any_match(&e.view(40, 5)));
}

#[test]
fn stepping_keeps_the_highlight() {
    let mut e = ed("foo\nfoo\nfoo");
    e.handle_key(ctrl('f'));
    type_str(&mut e, "foo");
    e.handle_key(key(KeyCode::Down));
    let v = e.view(40, 6);
    for r in 0..3 {
        assert_eq!(matched(&v, r), vec![0, 1, 2]);
    }
}

#[test]
fn a_tab_before_the_match_shifts_the_cells() {
    let mut e = ed("\tfoo\nab\tfoo");
    e.handle_key(ctrl('f'));
    type_str(&mut e, "foo");
    let v = e.view(40, 6);
    assert_eq!(matched(&v, 0), vec![4, 5, 6]);
    assert_eq!(matched(&v, 1), vec![4, 5, 6]);
}

#[test]
fn a_match_that_is_a_tab() {
    let mut e = ed("a\tb");
    e.handle_key(ctrl('f'));
    type_str(&mut e, "\t");
    assert_eq!(matched(&e.view(40, 5), 0), vec![1, 2, 3]);
}

#[test]
fn horizontal_scroll_clips_the_highlight() {
    let line = format!("{}foo{}foo", "x".repeat(30), "y".repeat(10));
    let mut e = ed(&line);
    e.handle_key(ctrl('f'));
    type_str(&mut e, "foo");
    // The cursor is at the first match (col 30): scrolled so it shows.
    let v = e.view(12, 5);
    let cols = matched(&v, 0);
    assert!(!cols.is_empty());
    let first = v.lines[0].chars().position(|c| c == 'f').unwrap();
    assert!(cols.contains(&first));
    for c in &cols {
        assert!(matches!(v.lines[0].chars().nth(*c), Some('f' | 'o')), "{c}: {:?}", v.lines[0]);
    }
}

#[test]
fn a_match_cut_by_the_left_edge_shows_its_visible_part() {
    let mut e = ed("xxfooyyy");
    e.buffer.set_cursor(0, 7);
    e.view(5, 5);
    e.handle_key(ctrl('f'));
    // Set the term without searching, so the scroll stays put.
    e.prompt.as_mut().unwrap().text = "foo".to_string();
    let v = e.view(5, 5);
    assert_eq!(v.lines[0], "ooyyy");
    assert_eq!(matched(&v, 0), vec![0, 1]);
}

#[test]
fn the_gutter_is_not_highlighted_and_columns_shift() {
    let mut e = ed("foo\nxfoo");
    e.set_line_numbers(true);
    e.handle_key(ctrl('f'));
    type_str(&mut e, "foo");
    let v = e.view(30, 6);
    assert_eq!(&v.styles[0][..4], &[Style::CurrentLineNo; 4]);
    assert_eq!(matched(&v, 0), vec![4, 5, 6]);
    assert_eq!(matched(&v, 1), vec![5, 6, 7]);
}

#[test]
fn selected_wins_over_match() {
    let mut e = ed("foo foo");
    for _ in 0..3 {
        e.handle_key(shift(KeyCode::Right));
    }
    e.handle_key(ctrl('f'));
    // Put the term in without moving the cursor (a found match would clear
    // the selection), so Selected and Match overlap.
    e.prompt.as_mut().unwrap().text = "foo".to_string();
    let v = e.view(30, 5);
    assert_eq!(&v.styles[0][..3], &[Style::Selected; 3]);
    assert_eq!(matched(&v, 0), vec![4, 5, 6]);
}

#[test]
fn match_replaces_the_syntax_colour_on_matched_cells_only() {
    let dir = std::env::temp_dir().join(format!("ted-match-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("m.rs");
    std::fs::write(&path, "fn main() {}\n").unwrap();
    let mut e = Editor::new(Buffer::open(&path).unwrap());
    e.handle_key(ctrl('f'));
    type_str(&mut e, "fn");
    let v = e.view(30, 5);
    assert_eq!(&v.styles[0][..3], &[Style::Match, Style::Match, Style::Normal]);
    e.handle_key(key(KeyCode::Esc));
    assert_eq!(e.view(30, 5).styles[0][0], Style::Keyword);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn not_found_inside_a_selection_says_so() {
    let mut e = ed("foo\nbar baz");
    e.buffer.set_cursor(1, 0);
    for _ in 0..3 {
        e.handle_key(shift(KeyCode::Right));
    }
    e.handle_key(ctrl('r'));
    type_str(&mut e, "foo");
    e.handle_key(key(KeyCode::Enter));
    e.handle_key(key(KeyCode::Enter));
    assert_eq!(e.view(40, 5).message, "Not found in selection: foo");
    assert_eq!(e.buffer().text(), "foo\nbar baz");
    assert_eq!(e.buffer().selection(), Some(((1, 0), (1, 3))));
}

#[test]
fn a_hundred_thousand_lines_with_thousands_of_matches_stay_fast() {
    let big = format!("{}\n{}", "ab ".repeat(100_000), "ab\n".repeat(100_000));
    let mut e = ed(&big);
    e.handle_key(ctrl('f'));
    type_str(&mut e, "ab");
    let start = std::time::Instant::now();
    for _ in 0..5 {
        let v = e.view(120, 40);
        assert!(any_match(&v));
    }
    assert!(start.elapsed() < std::time::Duration::from_secs(3), "{:?}", start.elapsed());
}

// Replace flow.

#[test]
fn ctrl_r_opens_the_replace_prompt_prefilled_with_the_search_term() {
    let mut e = ed("foo foo");
    e.handle_key(ctrl('r'));
    assert!(e.in_prompt());
    assert_eq!(e.view(40, 5).message, "Replace: _");
    e.handle_key(key(KeyCode::Esc));
    e.set_last_search("foo");
    e.handle_key(ctrl('r'));
    assert_eq!(e.view(40, 5).message, "Replace: foo_");
    type_str(&mut e, "x");
    assert_eq!(e.view(40, 5).message, "Replace: foox_");
    e.handle_key(key(KeyCode::Backspace));
    e.handle_key(key(KeyCode::Backspace));
    assert_eq!(e.view(40, 5).message, "Replace: fo_");
    // Opening the prompt moves nothing.
    assert_eq!(e.buffer().cursor(), (0, 0));
}

#[test]
fn enter_moves_to_the_with_step_and_enter_again_replaces() {
    let mut e = ed("foo bar foo\nfoo");
    e.handle_key(ctrl('r'));
    type_str(&mut e, "foo");
    e.handle_key(key(KeyCode::Enter));
    assert!(e.in_prompt());
    assert_eq!(e.view(40, 5).message, "With (foo): _");
    type_str(&mut e, "baz");
    assert_eq!(e.view(40, 5).message, "With (foo): baz_");
    e.handle_key(key(KeyCode::Enter));
    assert!(!e.in_prompt());
    assert_eq!(e.buffer().text(), "baz bar baz\nbaz");
    assert_eq!(e.view(40, 5).message, "Replaced 3 occurrences");
    assert_eq!(e.buffer().cursor(), (0, 0));
    assert!(e.buffer().is_modified());
    assert_eq!(e.take_search().as_deref(), Some("foo"));
    // One undo restores everything.
    e.handle_key(ctrl('z'));
    assert_eq!(e.buffer().text(), "foo bar foo\nfoo");
    assert!(!e.buffer().is_modified());
}

#[test]
fn one_occurrence_is_singular() {
    let mut e = ed("a b");
    e.handle_key(ctrl('r'));
    type_str(&mut e, "b");
    e.handle_key(key(KeyCode::Enter));
    type_str(&mut e, "c");
    e.handle_key(key(KeyCode::Enter));
    assert_eq!(e.view(40, 5).message, "Replaced 1 occurrence");
}

#[test]
fn an_empty_replacement_deletes_the_matches() {
    let mut e = ed("a-b-c");
    e.handle_key(ctrl('r'));
    type_str(&mut e, "-");
    e.handle_key(key(KeyCode::Enter));
    e.handle_key(key(KeyCode::Enter));
    assert_eq!(e.buffer().text(), "abc");
    assert_eq!(e.view(40, 5).message, "Replaced 2 occurrences");
}

#[test]
fn the_with_step_falls_back_when_the_term_does_not_fit() {
    let mut e = ed("x");
    e.handle_key(ctrl('r'));
    type_str(&mut e, "0123456789");
    e.handle_key(key(KeyCode::Enter));
    type_str(&mut e, "ab");
    assert_eq!(e.view(40, 5).message, "With (0123456789): ab_");
    assert_eq!(e.view(18, 5).message, "With: ab_");
    // Even the short form shows only its tail when the row is tiny.
    assert_eq!(e.view(4, 5).message, "…ab_");
}

#[test]
fn an_empty_term_cancels_quietly() {
    let mut e = ed("foo");
    e.handle_key(ctrl('r'));
    e.handle_key(key(KeyCode::Enter));
    assert!(!e.in_prompt());
    assert_eq!(e.view(40, 5).message, "");
    assert_eq!(e.buffer().text(), "foo");
    assert!(!e.buffer().is_modified());
    assert_eq!(e.take_search(), None);
}

#[test]
fn not_found_says_so_and_changes_nothing() {
    let mut e = ed("foo\nbar");
    e.handle_key(key(KeyCode::Down));
    e.handle_key(ctrl('r'));
    type_str(&mut e, "zzz");
    e.handle_key(key(KeyCode::Enter));
    type_str(&mut e, "q");
    e.handle_key(key(KeyCode::Enter));
    assert!(!e.in_prompt());
    assert_eq!(e.view(40, 5).message, "Not found: zzz");
    assert_eq!(e.buffer().text(), "foo\nbar");
    assert!(!e.buffer().is_modified());
    assert_eq!(e.buffer().cursor(), (1, 0));
    // Nothing to undo.
    e.handle_key(ctrl('z'));
    assert_eq!(e.view(40, 5).message, "Nothing to undo");
}

#[test]
fn esc_at_either_step_restores_cursor_scroll_and_selection() {
    for steps in [0, 1] {
        let text = (0..60).map(|i| format!("line {i} foo")).collect::<Vec<_>>().join("\n");
        let mut e = ed(&text);
        e.view(30, 10);
        for _ in 0..3 {
            e.handle_key(key(KeyCode::Down));
        }
        e.handle_key(shift(KeyCode::Right));
        e.handle_key(shift(KeyCode::Right));
        let (cursor, selection) = (e.buffer().cursor(), e.buffer().selection());
        let before = e.view(30, 10);
        e.handle_key(ctrl('r'));
        type_str(&mut e, "line 5");
        if steps == 1 {
            e.handle_key(key(KeyCode::Enter));
            type_str(&mut e, "zz");
        }
        e.handle_key(key(KeyCode::Esc));
        assert!(!e.in_prompt());
        assert_eq!(e.buffer().cursor(), cursor);
        assert_eq!(e.buffer().selection(), selection);
        assert_eq!(e.buffer().text(), text);
        assert!(!e.buffer().is_modified());
        let after = e.view(30, 10);
        assert_eq!(after.lines, before.lines);
        assert_eq!(after.styles, before.styles);
        assert_eq!(e.take_search(), None);
    }
}

#[test]
fn the_selection_at_ctrl_r_is_the_scope() {
    let mut e = ed("foo foo foo\nfoo");
    // Select the second and third "foo" of line 0 (cols 4..11).
    e.buffer.set_cursor(0, 4);
    for _ in 0..7 {
        e.handle_key(shift(KeyCode::Right));
    }
    e.handle_key(ctrl('r'));
    type_str(&mut e, "foo");
    e.handle_key(key(KeyCode::Enter));
    type_str(&mut e, "X");
    e.handle_key(key(KeyCode::Enter));
    assert_eq!(e.buffer().text(), "foo X X\nfoo");
    assert_eq!(e.view(40, 5).message, "Replaced 2 occurrences");
    assert_eq!(e.buffer().selection(), None);
    assert_eq!(e.buffer().cursor(), (0, 4));
}

#[test]
fn partial_matches_at_the_selection_edges_are_left_alone() {
    let mut e = ed("foo foo foo");
    e.buffer.set_cursor(0, 1);
    for _ in 0..9 {
        e.handle_key(shift(KeyCode::Right));
    }
    e.handle_key(ctrl('r'));
    type_str(&mut e, "foo");
    e.handle_key(key(KeyCode::Enter));
    type_str(&mut e, "X");
    e.handle_key(key(KeyCode::Enter));
    assert_eq!(e.buffer().text(), "foo X foo");
}

#[test]
fn a_replacement_containing_the_term_is_not_rescanned() {
    let mut e = ed("a a");
    e.handle_key(ctrl('r'));
    type_str(&mut e, "a");
    e.handle_key(key(KeyCode::Enter));
    type_str(&mut e, "aa");
    e.handle_key(key(KeyCode::Enter));
    assert_eq!(e.buffer().text(), "aa aa");
}

#[test]
fn ctrl_r_inside_a_prompt_is_not_a_new_replace() {
    let mut e = ed("foo");
    e.handle_key(ctrl('f'));
    e.handle_key(ctrl('r'));
    assert_eq!(e.view(40, 5).message, "Find: _");
    e.handle_key(key(KeyCode::Esc));
    e.handle_key(ctrl('r'));
    type_str(&mut e, "foo");
    e.handle_key(ctrl('r'));
    assert_eq!(e.view(40, 5).message, "Replace: foo_");
    e.handle_key(key(KeyCode::Enter));
    e.handle_key(ctrl('r'));
    assert_eq!(e.view(40, 5).message, "With (foo): _");
    assert_eq!(e.buffer().text(), "foo");
}

#[test]
fn cancel_prompt_closes_either_step_and_restores() {
    let mut e = ed("foo\nfoo");
    e.handle_key(key(KeyCode::Down));
    e.handle_key(ctrl('r'));
    type_str(&mut e, "foo");
    e.handle_key(key(KeyCode::Enter));
    e.cancel_prompt();
    assert!(!e.in_prompt());
    assert_eq!(e.buffer().cursor(), (1, 0));
    assert_eq!(e.buffer().text(), "foo\nfoo");
}

#[test]
fn a_paste_goes_into_the_replace_prompt_without_line_breaks() {
    let mut e = ed("foo");
    e.handle_key(ctrl('r'));
    assert!(e.paste_into_prompt("fo\no"));
    assert_eq!(e.view(40, 5).message, "Replace: foo_");
    e.handle_key(key(KeyCode::Enter));
    assert!(e.paste_into_prompt("a\r\nb"));
    assert_eq!(e.view(40, 5).message, "With (foo): ab_");
}

#[test]
fn open_replace_is_the_same_prompt() {
    let mut e = ed("foo");
    e.set_last_search("foo");
    e.open_replace();
    assert_eq!(e.view(40, 5).message, "Replace: foo_");
}

#[test]
fn hostile_text_in_the_prompt_is_sanitized() {
    let mut e = ed("x");
    e.handle_key(ctrl('r'));
    e.paste_into_prompt("a\u{1b}[2Jb");
    let m = e.view(40, 5).message;
    assert!(!m.contains('\u{1b}'), "{m:?}");
    assert_eq!(m, "Replace: a?[2Jb_");
}

#[test]
fn tiny_sizes_with_highlights_and_both_prompt_steps_never_panic() {
    for w in 0..=16 {
        for h in 0..=6 {
            let mut e = ed("foo\tfoo\nxfoo foo\n\u{e9}foo");
            e.set_line_numbers(true);
            e.handle_key(ctrl('f'));
            type_str(&mut e, "foo");
            e.view(w, h);
            e.handle_key(key(KeyCode::Down));
            e.view(w, h);
            e.handle_key(key(KeyCode::Esc));
            e.handle_key(shift(KeyCode::Right));
            e.handle_key(ctrl('r'));
            type_str(&mut e, "foo");
            let v = e.view(w, h);
            assert!(v.message.chars().count() <= w);
            e.handle_key(key(KeyCode::Enter));
            type_str(&mut e, "replacement text");
            let v = e.view(w, h);
            assert!(v.message.chars().count() <= w);
            e.handle_key(key(KeyCode::Enter));
            e.view(w, h);
        }
    }
}
