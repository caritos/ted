use super::*;
use std::path::PathBuf;

fn mods(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
    KeyEvent::new(code, modifiers)
}

const ALT: KeyModifiers = KeyModifiers::ALT;
const CTRL: KeyModifiers = KeyModifiers::CONTROL;

fn ed(text: &str) -> Editor {
    Editor::new(Buffer::from_text(text))
}

fn named(text: &str, name: &str) -> Editor {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!("ted-lineops-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    std::fs::create_dir_all(&dir).unwrap();
    let path: PathBuf = dir.join(name);
    std::fs::write(&path, text).unwrap();
    Editor::new(Buffer::open(&path).unwrap())
}

#[test]
fn ctrl_d_duplicates_the_line() {
    let mut e = ed("one\ntwo\n");
    e.handle_key(mods(KeyCode::Char('d'), CTRL));
    assert_eq!(e.buffer().text(), "one\none\ntwo\n");
    assert_eq!(e.buffer().cursor(), (1, 0));
}

#[test]
fn alt_k_deletes_the_line() {
    let mut e = ed("one\ntwo\n");
    e.handle_key(mods(KeyCode::Char('k'), ALT));
    assert_eq!(e.buffer().text(), "two\n");
}

#[test]
fn alt_up_and_down_move_the_line() {
    let mut e = ed("a\nb\nc\n");
    e.handle_key(mods(KeyCode::Down, ALT));
    assert_eq!(e.buffer().text(), "b\na\nc\n");
    e.handle_key(mods(KeyCode::Down, ALT));
    assert_eq!(e.buffer().text(), "b\nc\na\n");
    e.handle_key(mods(KeyCode::Up, ALT));
    assert_eq!(e.buffer().text(), "b\na\nc\n");
    assert_eq!(e.buffer().cursor(), (1, 0));
}

#[test]
fn moving_past_the_end_says_nothing_and_changes_nothing() {
    let mut e = ed("a\nb");
    e.handle_key(mods(KeyCode::Up, ALT));
    assert_eq!(e.buffer().text(), "a\nb");
    assert!(!e.buffer().is_modified());
}

#[test]
fn the_view_follows_a_line_moved_off_screen() {
    let text: String = (0..30).map(|i| format!("line {i}\n")).collect();
    let mut e = ed(&text);
    e.view(40, 6);
    for _ in 0..12 {
        e.handle_key(mods(KeyCode::Down, ALT));
    }
    let view = e.view(40, 6);
    assert!(view.cursor.0 < 4, "the cursor row is on screen");
    assert!(view.lines[view.cursor.0].starts_with("line 0"));
}

#[test]
fn alt_slash_comments_by_file_type() {
    for (path, expect) in [("a.rs", "// x\n"), ("a.py", "# x\n"), ("a.sh", "# x\n"), ("a.sql", "-- x\n"), ("a.js", "// x\n")] {
        let mut e = named("x\n", path);
        e.handle_key(mods(KeyCode::Char('/'), ALT));
        assert_eq!(e.buffer().text(), expect, "{path}");
    }
}

#[test]
fn ctrl_slash_in_its_terminal_spellings_also_toggles() {
    for code in [KeyCode::Char('/'), KeyCode::Char('_'), KeyCode::Char('7')] {
        let mut e = named("x\n", "a.rs");
        e.handle_key(mods(code, CTRL));
        assert_eq!(e.buffer().text(), "// x\n", "{code:?}");
        e.handle_key(mods(code, CTRL));
        assert_eq!(e.buffer().text(), "x\n");
    }
}

#[test]
fn a_file_without_a_known_comment_syntax_says_so() {
    for mut e in [ed("x\n"), named("x\n", "notes.txt")] {
        e.handle_key(mods(KeyCode::Char('/'), ALT));
        assert_eq!(e.buffer().text(), "x\n");
        assert_eq!(e.view(60, 5).message, "No comment syntax for this file");
    }
}

#[test]
fn blank_lines_alone_say_nothing_to_comment() {
    let mut e = named("\n", "a.rs");
    e.handle_key(mods(KeyCode::Char('/'), ALT));
    assert_eq!(e.buffer().text(), "\n");
    assert_eq!(e.view(60, 5).message, "Nothing to comment");
}

#[test]
fn each_key_is_one_undo_step() {
    let mut e = named("a\nb\n", "a.rs");
    for key in [mods(KeyCode::Char('d'), CTRL), mods(KeyCode::Down, ALT), mods(KeyCode::Char('/'), ALT), mods(KeyCode::Char('k'), ALT)] {
        e.handle_key(key);
    }
    for _ in 0..4 {
        e.handle_key(mods(KeyCode::Char('z'), CTRL));
    }
    assert_eq!(e.buffer().text(), "a\nb\n");
    assert!(!e.buffer().is_modified());
}

#[test]
fn the_keys_do_nothing_while_a_prompt_is_open() {
    let mut e = ed("a\nb\n");
    e.handle_key(mods(KeyCode::Char('f'), CTRL));
    e.handle_key(mods(KeyCode::Char('d'), CTRL));
    e.handle_key(mods(KeyCode::Down, ALT));
    e.handle_key(mods(KeyCode::Esc, KeyModifiers::NONE));
    assert_eq!(e.buffer().text(), "a\nb\n");
}

#[test]
fn the_edit_marks_the_highlight_cache_dirty_so_colours_stay_right() {
    let mut e = named("let a = 1;\n/* c\n*/\nlet b = 2;\n", "a.rs");
    e.view(40, 6);
    e.handle_key(mods(KeyCode::Down, ALT));
    let moved = e.view(40, 6);
    let mut fresh = named(&e.buffer().text(), "a.rs");
    assert_eq!(moved.styles, fresh.view(40, 6).styles);
}
