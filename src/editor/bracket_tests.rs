use super::*;

const CTRL: KeyModifiers = KeyModifiers::CONTROL;

fn ed(text: &str) -> Editor {
    Editor::new(Buffer::from_text(text))
}

fn key(e: &mut Editor, code: KeyCode, m: KeyModifiers) {
    e.handle_key(KeyEvent::new(code, m));
}

#[test]
fn ctrl_close_bracket_jumps_to_the_partner() {
    let mut e = ed("(a)");
    key(&mut e, KeyCode::Char(']'), CTRL);
    assert_eq!(e.buffer().cursor(), (0, 2));
}

#[test]
fn ctrl_5_is_the_same_key_as_a_terminal_sends_it() {
    // 0x1d reaches crossterm as Ctrl-5.
    let mut e = ed("(a)");
    key(&mut e, KeyCode::Char('5'), CTRL);
    assert_eq!(e.buffer().cursor(), (0, 2));
}

#[test]
fn shift_selects_up_to_the_partner() {
    let mut e = ed("(a)");
    key(&mut e, KeyCode::Char(']'), CTRL | KeyModifiers::SHIFT);
    assert_eq!(e.buffer().selection(), Some(((0, 0), (0, 2))));
}

#[test]
fn without_a_partner_it_says_so() {
    let mut e = ed("abc");
    key(&mut e, KeyCode::Char(']'), CTRL);
    assert_eq!(e.message(), "No matching bracket");
    assert_eq!(e.buffer().cursor(), (0, 0));
}

#[test]
fn the_view_scrolls_to_a_far_partner() {
    let text = format!("{{\n{}}}", "x\n".repeat(50));
    let mut e = ed(&text);
    e.view(20, 6);
    key(&mut e, KeyCode::Char(']'), CTRL);
    let v = e.view(20, 6);
    assert_eq!(e.buffer().cursor(), (51, 0));
    assert!(v.cursor.1 < 6, "cursor row {} is off screen", v.cursor.1);
}

#[test]
fn a_prompt_gets_the_key_as_text_not_a_jump() {
    let mut e = ed("(a)");
    key(&mut e, KeyCode::Char('l'), CTRL);
    key(&mut e, KeyCode::Char(']'), CTRL);
    assert_eq!(e.buffer().cursor(), (0, 0));
}
