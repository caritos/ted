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

fn ed(text: &str) -> Editor {
    Editor::new(Buffer::from_text(text))
}

fn open(text: &str) -> Editor {
    let mut e = ed(text);
    e.handle_key(ctrl('x'));
    e
}

#[test]
fn ctrl_x_opens_the_prompt() {
    let mut e = ed("hello");
    e.handle_key(ctrl('x'));
    assert!(e.in_prompt());
    assert!(e.view(60, 5).message.starts_with("Command: _"));
}

#[test]
fn typing_and_backspace() {
    let mut e = open("hello");
    type_str(&mut e, "ab");
    assert_eq!(e.view(60, 5).message, "Command: ab_");
    e.handle_key(key(KeyCode::Backspace));
    assert_eq!(e.view(60, 5).message, "Command: a_");
    e.handle_key(key(KeyCode::Backspace));
    e.handle_key(key(KeyCode::Backspace));
    assert!(e.in_prompt());
    assert!(e.view(60, 5).message.starts_with("Command: _"));
    assert_eq!(e.buffer().text(), "hello");
}

#[test]
fn esc_cancels_without_a_command() {
    let mut e = open("hello");
    type_str(&mut e, "save");
    e.handle_key(key(KeyCode::Esc));
    assert!(!e.in_prompt());
    assert_eq!(e.take_command(), None);
}

#[test]
fn enter_stores_the_text_once() {
    let mut e = open("hello");
    type_str(&mut e, "goto 3");
    e.handle_key(key(KeyCode::Enter));
    assert!(!e.in_prompt());
    assert_eq!(e.take_command(), Some("goto 3".to_string()));
    assert_eq!(e.take_command(), None);
}

#[test]
fn enter_on_blank_stores_nothing() {
    let mut e = open("hello");
    e.handle_key(key(KeyCode::Enter));
    assert!(!e.in_prompt());
    assert_eq!(e.take_command(), None);
    e.handle_key(ctrl('x'));
    type_str(&mut e, "   ");
    e.handle_key(key(KeyCode::Enter));
    assert_eq!(e.take_command(), None);
}

#[test]
fn a_new_prompt_discards_an_untaken_command() {
    let mut e = open("x");
    type_str(&mut e, "save");
    e.handle_key(key(KeyCode::Enter));
    e.handle_key(ctrl('x'));
    e.handle_key(key(KeyCode::Esc));
    assert_eq!(e.take_command(), None);
}

#[test]
fn leading_spaces_keep_tab_and_the_match_list_working() {
    let mut e = open("hello");
    type_str(&mut e, "  go");
    assert!(e.view(60, 5).message.starts_with("Command:   go_  goto"), "{:?}", e.view(60, 5).message);
    e.handle_key(key(KeyCode::Tab));
    assert_eq!(e.view(60, 5).message, "Command:   goto _");
    e.handle_key(key(KeyCode::Tab));
    assert_eq!(e.view(60, 5).message, "Command:   goto _");
}

#[test]
fn tab_completes_the_word() {
    let mut e = open("x");
    type_str(&mut e, "go");
    e.handle_key(key(KeyCode::Tab));
    assert!(e.view(60, 5).message.starts_with("Command: goto _"));
    type_str(&mut e, "5");
    e.handle_key(key(KeyCode::Enter));
    assert_eq!(e.take_command(), Some("goto 5".to_string()));
}

#[test]
fn tab_does_nothing_after_a_space_or_without_match() {
    let mut e = open("x");
    type_str(&mut e, "goto g");
    e.handle_key(key(KeyCode::Tab));
    assert_eq!(e.view(60, 5).message, "Command: goto g_");
    e.handle_key(key(KeyCode::Esc));
    e.handle_key(ctrl('x'));
    type_str(&mut e, "zzz");
    e.handle_key(key(KeyCode::Tab));
    assert_eq!(e.view(60, 5).message, "Command: zzz_");
    assert_eq!(e.buffer().text(), "x");
}

#[test]
fn tab_with_several_matches_completes_the_common_prefix() {
    let mut e = open("x");
    type_str(&mut e, "g");
    e.handle_key(key(KeyCode::Tab));
    assert_eq!(e.view(60, 5).message, "Command: g_  goto git");
}

#[test]
fn message_row_is_clear_after_enter_or_esc_and_other_prompts_work() {
    for finish in [KeyCode::Enter, KeyCode::Esc] {
        let mut e = open("alpha\nbeta\ngamma");
        type_str(&mut e, "goto 2");
        e.handle_key(key(finish));
        assert!(!e.in_prompt());
        assert_eq!(e.view(60, 5).message, "", "{finish:?}");
        e.handle_key(ctrl('f'));
        type_str(&mut e, "gam");
        assert_eq!(e.view(60, 5).message, "Find: gam_");
        e.handle_key(key(KeyCode::Enter));
        assert_eq!(e.buffer().cursor(), (2, 0));
        e.handle_key(ctrl('l'));
        assert!(e.view(60, 5).message.starts_with("Go to line"), "{:?}", e.view(60, 5).message);
        type_str(&mut e, "1");
        e.handle_key(key(KeyCode::Enter));
        assert_eq!(e.buffer().cursor(), (0, 0));
        assert_eq!(e.view(60, 5).message, "");
    }
}

#[test]
fn suffix_lists_matches_until_a_space() {
    let mut e = open("x");
    type_str(&mut e, "g");
    assert_eq!(e.view(60, 5).message, "Command: g_  goto git");
    type_str(&mut e, "o");
    assert_eq!(e.view(60, 5).message, "Command: go_  goto");
    type_str(&mut e, "to ");
    assert_eq!(e.view(60, 5).message, "Command: goto _");
    e.handle_key(key(KeyCode::Esc));
    e.handle_key(ctrl('x'));
    type_str(&mut e, "zzz");
    assert_eq!(e.view(60, 5).message, "Command: zzz_");
}

#[test]
fn control_chars_in_the_prompt_are_drawn_as_question_marks() {
    let mut e = open("x");
    e.handle_key(key(KeyCode::Char('\u{1b}')));
    type_str(&mut e, "[2J");
    let v = e.view(60, 5);
    assert!(v.message.starts_with("Command: ?[2J_"), "{:?}", v.message);
    assert!(!v.message.contains('\u{1b}'));
}

#[test]
fn ctrl_f_and_ctrl_l_inside_the_prompt_are_not_actions() {
    let mut e = open("hello");
    e.handle_key(ctrl('f'));
    e.handle_key(ctrl('l'));
    assert!(e.in_prompt());
    assert!(e.view(60, 5).message.starts_with("Command: _"));
    assert_eq!(e.buffer().text(), "hello");
}

#[test]
fn tiny_sizes_with_the_prompt_open_do_not_panic() {
    let mut e = open("one\ntwo\nthree");
    type_str(&mut e, "g");
    for w in 0..=6 {
        for h in 0..=6 {
            let v = e.view(w, h);
            assert!(v.message.chars().count() <= w);
        }
    }
}
