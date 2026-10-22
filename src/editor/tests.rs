use super::*;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::fs;
use std::path::PathBuf;

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

fn temp_file(name: &str) -> PathBuf {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "ted-editor-test-{}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed),
        name
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir.join("file.txt")
}

fn empty() -> Editor {
    Editor::new(Buffer::new())
}

#[test]
fn typing_inserts_characters() {
    let mut e = empty();
    type_str(&mut e, "hi");
    assert_eq!(e.buffer().text(), "hi");
}

#[test]
fn shifted_characters_are_inserted() {
    let mut e = empty();
    e.handle_key(KeyEvent::new(KeyCode::Char('A'), KeyModifiers::SHIFT));
    assert_eq!(e.buffer().text(), "A");
}

#[test]
fn control_characters_are_not_inserted() {
    let mut e = empty();
    e.handle_key(ctrl('a'));
    assert_eq!(e.buffer().text(), "");
}

#[test]
fn enter_backspace_delete_and_tab() {
    let mut e = empty();
    type_str(&mut e, "ab");
    e.handle_key(key(KeyCode::Enter));
    type_str(&mut e, "cd");
    assert_eq!(e.buffer().text(), "ab\ncd");
    e.handle_key(key(KeyCode::Backspace));
    assert_eq!(e.buffer().text(), "ab\nc");
    e.handle_key(key(KeyCode::Left));
    e.handle_key(key(KeyCode::Delete));
    assert_eq!(e.buffer().text(), "ab\n");
    e.handle_key(key(KeyCode::Tab));
    assert_eq!(e.buffer().text(), "ab\n    ");
}

#[test]
fn arrow_keys_move_the_cursor() {
    let mut e = Editor::new(Buffer::from_text("ab\ncd"));
    e.handle_key(key(KeyCode::Right));
    e.handle_key(key(KeyCode::Down));
    assert_eq!(e.buffer().cursor(), (1, 1));
    e.handle_key(key(KeyCode::Left));
    e.handle_key(key(KeyCode::Up));
    assert_eq!(e.buffer().cursor(), (0, 0));
}

#[test]
fn ctrl_s_saves_and_reports() {
    let path = temp_file("save");
    let mut e = Editor::new(Buffer::open(&path).unwrap());
    type_str(&mut e, "hi");
    assert_eq!(e.handle_key(ctrl('s')), Flow::Continue);
    assert_eq!(fs::read_to_string(&path).unwrap(), "hi");
    assert!(!e.buffer().is_modified());
    assert!(e.message().starts_with("Saved"), "{}", e.message());
}

#[test]
fn ctrl_s_without_a_file_name_reports_and_keeps_going() {
    let mut e = empty();
    type_str(&mut e, "hi");
    assert_eq!(e.handle_key(ctrl('s')), Flow::Continue);
    assert!(e.message().contains("No file name"), "{}", e.message());
    assert!(e.buffer().is_modified());
}

#[test]
fn failed_save_reports_the_error() {
    let path = temp_file("fail").parent().unwrap().join("nope").join("f.txt");
    let mut e = Editor::new(Buffer::open(&path).unwrap());
    type_str(&mut e, "hi");
    assert_eq!(e.handle_key(ctrl('s')), Flow::Continue);
    assert!(e.message().starts_with("Save failed"), "{}", e.message());
    assert!(e.buffer().is_modified());
}

#[test]
fn ctrl_q_quits_when_clean() {
    let mut e = empty();
    assert_eq!(e.handle_key(ctrl('q')), Flow::Quit);
}

#[test]
fn ctrl_q_warns_before_discarding_changes() {
    let mut e = empty();
    type_str(&mut e, "x");
    assert_eq!(e.handle_key(ctrl('q')), Flow::Continue);
    assert!(e.message().contains("Unsaved"), "{}", e.message());
    assert_eq!(e.handle_key(ctrl('q')), Flow::Quit);
}

#[test]
fn any_other_key_cancels_the_quit_warning() {
    let mut e = empty();
    type_str(&mut e, "x");
    e.handle_key(ctrl('q'));
    e.handle_key(key(KeyCode::Left));
    assert_eq!(e.handle_key(ctrl('q')), Flow::Continue);
}

#[test]
fn saving_after_the_warning_lets_quit_through() {
    let path = temp_file("saveq");
    let mut e = Editor::new(Buffer::open(&path).unwrap());
    type_str(&mut e, "x");
    e.handle_key(ctrl('q'));
    e.handle_key(ctrl('s'));
    assert_eq!(e.handle_key(ctrl('q')), Flow::Quit);
}

#[test]
fn view_has_text_rows_status_and_message() {
    let mut e = Editor::new(Buffer::from_text("one\ntwo"));
    let v = e.view(20, 6);
    assert_eq!(v.lines, vec!["one", "two", "~", "~"]);
    assert_eq!(v.status.chars().count(), 20);
    assert_eq!(v.cursor, (0, 0));
}

#[test]
fn status_shows_name_modified_marker_and_position() {
    let path = temp_file("status");
    let mut e = Editor::new(Buffer::open(&path).unwrap());
    type_str(&mut e, "ab");
    let v = e.view(40, 5);
    assert!(v.status.contains("file.txt"), "{}", v.status);
    assert!(v.status.contains("[+]"), "{}", v.status);
    assert!(v.status.trim_end().ends_with("1:3"), "{}", v.status);
}

#[test]
fn status_for_unnamed_buffer() {
    let mut e = empty();
    let v = e.view(40, 5);
    assert!(v.status.contains("[No Name]"), "{}", v.status);
}

#[test]
fn view_scrolls_down_to_keep_the_cursor_visible() {
    let text: Vec<String> = (0..10).map(|i| format!("line{i}")).collect();
    let mut e = Editor::new(Buffer::from_text(&text.join("\n")));
    for _ in 0..9 {
        e.handle_key(key(KeyCode::Down));
    }
    let v = e.view(20, 5); // 3 text rows
    assert_eq!(v.lines, vec!["line7", "line8", "line9"]);
    assert_eq!(v.cursor, (2, 0));
}

#[test]
fn view_scrolls_back_up() {
    let text: Vec<String> = (0..10).map(|i| format!("line{i}")).collect();
    let mut e = Editor::new(Buffer::from_text(&text.join("\n")));
    for _ in 0..9 {
        e.handle_key(key(KeyCode::Down));
    }
    e.view(20, 5);
    for _ in 0..9 {
        e.handle_key(key(KeyCode::Up));
    }
    let v = e.view(20, 5);
    assert_eq!(v.lines, vec!["line0", "line1", "line2"]);
    assert_eq!(v.cursor, (0, 0));
}

#[test]
fn view_scrolls_horizontally_and_clips_to_width() {
    let mut e = Editor::new(Buffer::from_text("0123456789"));
    for _ in 0..10 {
        e.handle_key(key(KeyCode::Right));
    }
    let v = e.view(5, 4);
    assert_eq!(v.lines[0], "6789");
    assert_eq!(v.cursor, (0, 4));
    for _ in 0..10 {
        e.handle_key(key(KeyCode::Left));
    }
    let v = e.view(5, 4);
    assert_eq!(v.lines[0], "01234");
    assert_eq!(v.cursor, (0, 0));
}

#[test]
fn view_survives_tiny_terminals() {
    let mut e = Editor::new(Buffer::from_text("abc"));
    for (w, h) in [(0, 0), (1, 1), (1, 2), (0, 5), (5, 0)] {
        let v = e.view(w, h);
        assert!(v.lines.len() <= h);
    }
}
