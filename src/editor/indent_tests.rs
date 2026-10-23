use super::*;

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn type_str(e: &mut Editor, text: &str) {
    for c in text.chars() {
        e.handle_key(key(KeyCode::Char(c)));
    }
}

#[test]
fn enter_keeps_the_indent_and_typing_continues_there() {
    let mut e = Editor::new(Buffer::from_text(""));
    e.handle_key(key(KeyCode::Tab));
    type_str(&mut e, "a");
    e.handle_key(key(KeyCode::Enter));
    type_str(&mut e, "b");
    e.handle_key(key(KeyCode::Enter));
    e.handle_key(key(KeyCode::Backspace));
    assert_eq!(e.buffer().text(), "    a\n    b\n   ");
    let v = e.view(20, 6);
    assert_eq!(v.cursor, (2, 3));
}

#[test]
fn enter_in_a_prompt_is_not_a_newline() {
    let mut e = Editor::new(Buffer::from_text("  a"));
    e.handle_key(key(KeyCode::End));
    e.handle_key(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::CONTROL));
    type_str(&mut e, "1");
    e.handle_key(key(KeyCode::Enter));
    assert_eq!(e.buffer().text(), "  a");
}

#[test]
fn enter_on_a_rust_line_keeps_highlighting_in_step() {
    let mut e = Editor::new(Buffer::from_text("    let x = 1; // c"));
    e.handle_key(key(KeyCode::End));
    e.handle_key(key(KeyCode::Enter));
    type_str(&mut e, "y");
    assert_eq!(e.buffer().text(), "    let x = 1; // c\n    y");
    let v = e.view(40, 6);
    assert_eq!(v.lines[1], "    y");
}
