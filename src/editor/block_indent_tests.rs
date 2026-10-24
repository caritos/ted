use super::*;

fn ed(text: &str) -> Editor {
    Editor::new(Buffer::from_text(text))
}

fn key(e: &mut Editor, code: KeyCode, m: KeyModifiers) {
    e.handle_key(KeyEvent::new(code, m));
}

fn select_rows(e: &mut Editor) {
    key(e, KeyCode::Down, KeyModifiers::SHIFT);
    key(e, KeyCode::Down, KeyModifiers::SHIFT);
}

#[test]
fn tab_on_a_multi_row_selection_indents_the_rows() {
    let mut e = ed("a\nb\nc\n");
    select_rows(&mut e);
    key(&mut e, KeyCode::Tab, KeyModifiers::NONE);
    assert_eq!(e.buffer().text(), "    a\n    b\nc\n");
    assert!(e.buffer().selection().is_some());
}

#[test]
fn tab_on_a_selection_within_one_row_still_replaces_it() {
    let mut e = ed("abc\n");
    key(&mut e, KeyCode::Right, KeyModifiers::SHIFT);
    key(&mut e, KeyCode::Tab, KeyModifiers::NONE);
    assert_eq!(e.buffer().text(), "    bc\n");
}

#[test]
fn tab_without_a_selection_still_inserts_spaces() {
    let mut e = ed("a\n");
    key(&mut e, KeyCode::Tab, KeyModifiers::NONE);
    assert_eq!(e.buffer().text(), "    a\n");
}

#[test]
fn shift_tab_dedents_the_selected_rows_or_the_cursor_row() {
    let mut e = ed("    a\n    b\n    c\n");
    select_rows(&mut e);
    key(&mut e, KeyCode::BackTab, KeyModifiers::SHIFT);
    assert_eq!(e.buffer().text(), "a\nb\n    c\n");
    let mut e = ed("        a\n");
    key(&mut e, KeyCode::BackTab, KeyModifiers::SHIFT);
    assert_eq!(e.buffer().text(), "    a\n");
}

#[test]
fn shift_tab_with_nothing_to_remove_is_silent() {
    let mut e = ed("a\n");
    key(&mut e, KeyCode::BackTab, KeyModifiers::SHIFT);
    assert_eq!(e.buffer().text(), "a\n");
    assert!(!e.buffer().can_undo());
}

#[test]
fn a_prompt_swallows_shift_tab() {
    let mut e = ed("    a\n");
    key(&mut e, KeyCode::Char('l'), KeyModifiers::CONTROL);
    key(&mut e, KeyCode::BackTab, KeyModifiers::SHIFT);
    assert_eq!(e.buffer().text(), "    a\n");
}
