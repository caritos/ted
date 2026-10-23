use super::*;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn ctrl(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
}

fn type_str(editor: &mut Editor, text: &str) {
    for c in text.chars() {
        editor.handle_key(key(KeyCode::Char(c)));
    }
}

/// An editor whose ask-replace of `term` -> `with` is waiting for the first answer.
fn asking(text: &str, term: &str, with: &str) -> Editor {
    let mut e = Editor::new(Buffer::from_text(text));
    e.open_replace_ask();
    type_str(&mut e, term);
    e.handle_key(key(KeyCode::Enter));
    type_str(&mut e, with);
    e.handle_key(key(KeyCode::Enter));
    e
}

fn text(e: &Editor) -> String {
    e.buffer().text()
}

fn message(e: &mut Editor) -> String {
    e.view(60, 6).message
}

#[test]
fn the_prompt_asks_with_a_counter_and_selects_the_first_match() {
    let mut e = asking("foo x foo\nfoo", "foo", "bar");
    assert!(e.in_prompt());
    assert_eq!(message(&mut e), "Replace? (y)es (n)o (a)ll (q)uit [1/3]");
    assert_eq!(e.buffer().selection(), Some(((0, 0), (0, 3))));
    assert_eq!(text(&e), "foo x foo\nfoo", "nothing is replaced yet");
}

#[test]
fn y_and_n_choose_per_match_and_the_result_is_one_undo_step() {
    let mut e = asking("foo x foo\nfoo", "foo", "bar");
    e.handle_key(key(KeyCode::Char('y')));
    assert_eq!(message(&mut e), "Replace? (y)es (n)o (a)ll (q)uit [2/3]");
    assert_eq!(e.buffer().selection(), Some(((0, 6), (0, 9))));
    e.handle_key(key(KeyCode::Char('n')));
    e.handle_key(key(KeyCode::Char('y')));
    assert!(!e.in_prompt());
    assert_eq!(text(&e), "bar x foo\nbar");
    assert_eq!(message(&mut e), "Replaced 2 of 3 occurrences");
    assert!(e.buffer().selection().is_none());
    e.handle_key(ctrl('z'));
    assert_eq!(text(&e), "foo x foo\nfoo");
    e.handle_key(ctrl('z'));
    assert_eq!(message(&mut e), "Nothing to undo");
}

#[test]
fn a_replaces_the_rest_but_not_the_skipped() {
    let mut e = asking("foo foo foo foo", "foo", "X");
    e.handle_key(key(KeyCode::Char('n')));
    e.handle_key(key(KeyCode::Char('a')));
    assert_eq!(text(&e), "foo X X X");
    assert_eq!(message(&mut e), "Replaced 3 of 4 occurrences");
}

#[test]
fn q_and_esc_stop_but_keep_what_was_chosen() {
    for stop in [KeyCode::Char('q'), KeyCode::Esc] {
        let mut e = asking("foo foo foo", "foo", "X");
        e.handle_key(key(KeyCode::Char('y')));
        e.handle_key(key(stop));
        assert!(!e.in_prompt(), "{stop:?}");
        assert_eq!(text(&e), "X foo foo", "{stop:?}");
        assert_eq!(message(&mut e), "Replaced 1 of 3 occurrences");
    }
}

#[test]
fn stopping_with_nothing_chosen_changes_nothing_and_restores_the_cursor() {
    let mut e = Editor::new(Buffer::from_text("a\nb foo foo"));
    e.open_replace_ask();
    type_str(&mut e, "foo");
    e.handle_key(key(KeyCode::Enter));
    type_str(&mut e, "X");
    e.handle_key(key(KeyCode::Enter));
    e.handle_key(key(KeyCode::Char('n')));
    e.handle_key(key(KeyCode::Char('q')));
    assert_eq!(text(&e), "a\nb foo foo");
    assert!(!e.buffer().is_modified());
    assert_eq!(e.buffer().cursor(), (0, 0));
    assert!(e.buffer().selection().is_none());
}

#[test]
fn skipping_every_match_ends_the_session() {
    let mut e = asking("foo foo", "foo", "X");
    e.handle_key(key(KeyCode::Char('n')));
    e.handle_key(key(KeyCode::Char('n')));
    assert!(!e.in_prompt());
    assert_eq!(text(&e), "foo foo");
    assert!(!e.buffer().is_modified());
}

#[test]
fn other_keys_are_ignored_while_asking() {
    let mut e = asking("foo foo", "foo", "X");
    for code in [KeyCode::Char('x'), KeyCode::Enter, KeyCode::Down, KeyCode::Backspace, KeyCode::Tab] {
        e.handle_key(key(code));
    }
    e.handle_key(ctrl('s'));
    assert!(e.in_prompt());
    assert_eq!(text(&e), "foo foo");
    assert_eq!(message(&mut e), "Replace? (y)es (n)o (a)ll (q)uit [1/2]");
}

#[test]
fn upper_case_answers_work_too() {
    let mut e = asking("foo foo", "foo", "X");
    e.handle_key(key(KeyCode::Char('Y')));
    e.handle_key(key(KeyCode::Char('N')));
    assert_eq!(text(&e), "X foo");
}

#[test]
fn not_found_says_so_and_never_asks() {
    let mut e = asking("abc", "zzz", "X");
    assert!(!e.in_prompt());
    assert_eq!(message(&mut e), "Not found: zzz");
}

#[test]
fn the_scope_is_the_selection() {
    let mut e = Editor::new(Buffer::from_text("foo foo\nfoo foo"));
    e.handle_key(key(KeyCode::Down));
    e.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::SHIFT));
    for _ in 0..6 {
        e.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::SHIFT));
    }
    e.open_replace_ask();
    type_str(&mut e, "foo");
    e.handle_key(key(KeyCode::Enter));
    type_str(&mut e, "X");
    e.handle_key(key(KeyCode::Enter));
    assert_eq!(message(&mut e), "Replace? (y)es (n)o (a)ll (q)uit [1/2]");
    e.handle_key(key(KeyCode::Char('a')));
    assert_eq!(text(&e), "foo foo\nX X");
}

#[test]
fn the_term_is_remembered_and_the_empty_replacement_deletes() {
    let mut e = asking("foo bar foo", "foo", "");
    assert_eq!(e.take_search().as_deref(), Some("foo"));
    e.handle_key(key(KeyCode::Char('a')));
    assert_eq!(text(&e), " bar ");
}

#[test]
fn cancelling_the_prompt_from_outside_changes_nothing() {
    let mut e = asking("foo foo", "foo", "X");
    e.handle_key(key(KeyCode::Char('y')));
    e.cancel_prompt();
    assert!(!e.in_prompt());
    assert_eq!(text(&e), "foo foo");
    assert_eq!(e.buffer().cursor(), (0, 0));
}

#[test]
fn the_current_match_is_drawn_selected_at_every_size() {
    let mut e = asking("a\tfoo\n\nfoo", "foo", "X");
    for w in 0..=14 {
        for h in 0..=6 {
            let v = e.view(w, h);
            assert!(v.lines.len() == h.saturating_sub(2));
        }
    }
    let v = asking("a\tfoo\n\nfoo", "foo", "X").view(40, 6);
    assert_eq!(v.styles[0].iter().filter(|s| **s == Style::Selected).count(), 3);
}
