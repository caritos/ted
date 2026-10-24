use super::*;

fn type_keys(e: &mut Editor, keys: &[KeyCode]) {
    for &k in keys {
        e.handle_key(KeyEvent::new(k, KeyModifiers::NONE));
    }
}

fn chars(s: &str) -> Vec<KeyCode> {
    s.chars().map(|c| if c == '\n' { KeyCode::Enter } else { KeyCode::Char(c) }).collect()
}

#[test]
fn typing_a_block_indents_and_dedents_by_itself() {
    let mut e = Editor::new(Buffer::from_text(""));
    type_keys(&mut e, &chars("fn x() {\ny;\n}"));
    assert_eq!(e.buffer().text(), "fn x() {\n    y;\n}");
}

#[test]
fn enter_between_typed_braces_opens_the_block() {
    let mut e = Editor::new(Buffer::from_text(""));
    type_keys(&mut e, &chars("{}"));
    type_keys(&mut e, &[KeyCode::Left, KeyCode::Enter]);
    type_keys(&mut e, &chars("z"));
    assert_eq!(e.buffer().text(), "{\n    z\n}");
}

#[test]
fn a_closer_in_a_prompt_is_text() {
    let mut e = Editor::new(Buffer::from_text("    x"));
    e.handle_key(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::CONTROL));
    type_keys(&mut e, &chars("}"));
    assert_eq!(e.buffer().text(), "    x");
}
