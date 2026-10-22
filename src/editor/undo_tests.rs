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

#[test]
fn ctrl_z_undoes_typing_and_clears_the_modified_marker() {
    let mut e = Editor::new(Buffer::from_text("abc"));
    type_str(&mut e, "XY");
    assert_eq!(e.buffer().text(), "XYabc");
    assert!(e.view(40, 5).status.contains("[+]"));
    e.handle_key(ctrl('z'));
    assert_eq!(e.buffer().text(), "abc");
    let v = e.view(40, 5);
    assert!(!v.status.contains("[+]"), "{}", v.status);
    assert_eq!(v.message, "");
    e.handle_key(ctrl('y'));
    assert_eq!(e.buffer().text(), "XYabc");
    let v = e.view(40, 5);
    assert!(v.status.contains("[+]"), "{}", v.status);
    assert_eq!(v.message, "");
}

#[test]
fn empty_history_shows_messages() {
    let mut e = Editor::new(Buffer::from_text("abc"));
    e.handle_key(ctrl('z'));
    assert_eq!(e.view(40, 5).message, "Nothing to undo");
    e.handle_key(ctrl('y'));
    assert_eq!(e.view(40, 5).message, "Nothing to redo");
    assert_eq!(e.buffer().text(), "abc");
}

#[test]
fn ctrl_z_and_ctrl_y_are_ignored_inside_a_prompt() {
    for prompt in ['f', 'l', 'x'] {
        let mut e = Editor::new(Buffer::from_text("abc"));
        type_str(&mut e, "X");
        e.handle_key(ctrl(prompt));
        assert!(e.in_prompt());
        e.handle_key(ctrl('z'));
        e.handle_key(ctrl('y'));
        assert!(e.in_prompt(), "{prompt}");
        assert_eq!(e.buffer().text(), "Xabc", "{prompt}");
    }
}

#[test]
fn styles_agree_with_a_fresh_editor_after_undo_and_redo() {
    let text = "fn main() {\n    let s = \"hi\"; // c\n}\n";
    let dir = std::env::temp_dir().join(format!("ted-undo-style-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("x.rs");
    std::fs::write(&path, text).unwrap();
    let mut fresh = Editor::new(Buffer::open(&path).unwrap());
    let mut e = Editor::new(Buffer::open(&path).unwrap());
    let _ = e.view(40, 6);
    e.handle_key(key(KeyCode::Home));
    type_str(&mut e, "/* ");
    let _ = e.view(40, 6);
    e.handle_key(ctrl('z'));
    assert_eq!(e.buffer().text(), text);
    let (a, b) = (e.view(40, 6), fresh.view(40, 6));
    assert_eq!(a.lines, b.lines);
    assert_eq!(a.styles, b.styles);
    e.handle_key(ctrl('y'));
    let a = e.view(40, 6);
    assert_eq!(a.lines[0], "/* fn main() {");
    let mut commented = Editor::new(Buffer::open(&path).unwrap());
    commented.handle_key(key(KeyCode::Home));
    type_str(&mut commented, "/* ");
    assert_eq!(a.styles, commented.view(40, 6).styles);
    let _ = std::fs::remove_dir_all(&dir);
}
