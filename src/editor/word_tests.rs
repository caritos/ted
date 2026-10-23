use super::*;

fn mods(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
    KeyEvent::new(code, modifiers)
}

const ALT: KeyModifiers = KeyModifiers::ALT;
const CTRL: KeyModifiers = KeyModifiers::CONTROL;

fn ed(text: &str) -> Editor {
    Editor::new(Buffer::from_text(text))
}

#[test]
fn alt_and_ctrl_arrows_move_by_words() {
    for modifier in [ALT, CTRL] {
        let mut e = ed("foo bar baz");
        e.handle_key(mods(KeyCode::Right, modifier));
        assert_eq!(e.buffer().cursor(), (0, 3), "{modifier:?}");
        e.handle_key(mods(KeyCode::Right, modifier));
        assert_eq!(e.buffer().cursor(), (0, 7));
        e.handle_key(mods(KeyCode::Left, modifier));
        assert_eq!(e.buffer().cursor(), (0, 4));
        assert_eq!(e.buffer().text(), "foo bar baz");
    }
}

#[test]
fn alt_b_and_alt_f_are_the_terminals_option_arrows() {
    let mut e = ed("foo bar");
    e.handle_key(mods(KeyCode::Char('f'), ALT));
    e.handle_key(mods(KeyCode::Char('f'), ALT));
    assert_eq!(e.buffer().cursor(), (0, 7));
    e.handle_key(mods(KeyCode::Char('b'), ALT));
    assert_eq!(e.buffer().cursor(), (0, 4));
    assert_eq!(e.buffer().text(), "foo bar", "nothing was typed");
}

#[test]
fn shift_adds_the_selection() {
    let mut e = ed("foo bar baz");
    let both = ALT | KeyModifiers::SHIFT;
    e.handle_key(mods(KeyCode::Right, both));
    e.handle_key(mods(KeyCode::Right, both));
    assert_eq!(e.selected_text().as_deref(), Some("foo bar"));
    e.handle_key(mods(KeyCode::Left, both));
    assert_eq!(e.selected_text().as_deref(), Some("foo "));
    let v = e.view(20, 5);
    assert!(v.styles[0].iter().take(4).all(|s| *s == Style::Selected));
}

#[test]
fn alt_backspace_and_alt_d_delete_words() {
    let mut e = ed("foo bar baz");
    e.handle_key(key_end());
    e.handle_key(mods(KeyCode::Backspace, ALT));
    assert_eq!(e.buffer().text(), "foo bar ");
    e.handle_key(mods(KeyCode::Char('z'), CTRL));
    assert_eq!(e.buffer().text(), "foo bar baz", "one undo step");
    e.handle_key(mods(KeyCode::Home, KeyModifiers::NONE));
    e.handle_key(mods(KeyCode::Char('d'), ALT));
    assert_eq!(e.buffer().text(), " bar baz");
    e.handle_key(mods(KeyCode::Delete, CTRL));
    assert_eq!(e.buffer().text(), " baz");
}

fn key_end() -> KeyEvent {
    mods(KeyCode::End, KeyModifiers::NONE)
}

#[test]
fn word_keys_do_nothing_inside_a_prompt() {
    let mut e = ed("foo bar");
    e.handle_key(mods(KeyCode::Char('l'), CTRL));
    e.handle_key(mods(KeyCode::Right, ALT));
    e.handle_key(mods(KeyCode::Char('b'), ALT));
    e.handle_key(mods(KeyCode::Backspace, ALT));
    assert!(e.in_prompt());
    e.cancel_prompt();
    assert_eq!(e.buffer().cursor(), (0, 0));
    assert_eq!(e.buffer().text(), "foo bar");
}
