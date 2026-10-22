use super::*;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::fs;
use crate::screen::Style;
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

#[test]
fn editor_load_replaces_buffer_and_resets_scroll() {
    let text: Vec<String> = (0..10).map(|i| format!("line{i}")).collect();
    let mut e = Editor::new(Buffer::from_text(&text.join("\n")));
    for _ in 0..9 {
        e.handle_key(key(KeyCode::Down));
    }
    for _ in 0..4 {
        e.handle_key(key(KeyCode::Right));
    }
    e.view(3, 5); // scrolled down and right
    e.handle_key(key(KeyCode::Char('x')));
    e.handle_key(ctrl('q')); // arms the quit warning

    e.load(Buffer::from_text("new\ntext"));
    assert_eq!(e.buffer().text(), "new\ntext");
    assert!(!e.buffer().is_modified());
    assert_eq!(e.message(), "");
    let v = e.view(3, 5);
    assert_eq!(v.lines, vec!["new", "tex", "~"]);
    assert_eq!(v.cursor, (0, 0));
}

#[test]
fn cancel_quit_disarms_the_warning() {
    let mut e = empty();
    type_str(&mut e, "x");
    e.handle_key(ctrl('q'));
    e.cancel_quit();
    assert_eq!(e.message(), "");
    assert_eq!(e.handle_key(ctrl('q')), Flow::Continue);
    assert_eq!(e.handle_key(ctrl('q')), Flow::Quit);
}

fn named_editor(name: &str, text: &str) -> Editor {
    let dir = temp_file(name).parent().unwrap().to_path_buf();
    let path = dir.join(name);
    fs::write(&path, text).unwrap();
    Editor::new(Buffer::open(&path).unwrap())
}

fn assert_aligned(v: &View) {
    assert_eq!(v.styles.len(), v.lines.len());
    for (line, styles) in v.lines.iter().zip(&v.styles) {
        assert_eq!(line.chars().count(), styles.len(), "line {line:?}");
    }
}

#[test]
fn editor_view_styles_have_one_entry_per_displayed_char() {
    let src = "fn main() {\n    let s = \"héllo 😀 日本語 text\"; // done\n}\n/* open\nstill */ let x = 1;\n";
    for name in ["a.rs", "a.txt"] {
        for width in 0..40 {
            for height in 0..6 {
                let mut e = named_editor(name, src);
                let v = e.view(width, height);
                assert_aligned(&v);
                for _ in 0..30 {
                    e.handle_key(key(KeyCode::Right));
                }
                for _ in 0..3 {
                    e.handle_key(key(KeyCode::Down));
                }
                let v = e.view(width, height);
                assert_aligned(&v);
            }
        }
    }
}

#[test]
fn rust_buffer_styles_mark_keywords_strings_and_comments() {
    let mut e = named_editor("a.rs", "fn x() { \"s\" } // c\n");
    let v = e.view(40, 6);
    assert_eq!(v.lines[0], "fn x() { \"s\" } // c");
    let s = &v.styles[0];
    assert_eq!(s[0], Style::Keyword);
    assert_eq!(s[1], Style::Keyword);
    assert_eq!(s[2], Style::Normal);
    assert_eq!(s[3], Style::Normal);
    assert_eq!(s[9], Style::Str);
    assert_eq!(s[11], Style::Str);
    assert_eq!(s[16], Style::Comment);
    assert_eq!(s[18], Style::Comment);
    assert_eq!(v.lines[2], "~");
    assert_eq!(v.styles[2], vec![Style::Normal]);
}

#[test]
fn scrolling_in_the_middle_of_a_string_keeps_styles_aligned() {
    let mut e = named_editor("a.rs", "let a = \"0123456789\"; // tail\n");
    for _ in 0..24 {
        e.handle_key(key(KeyCode::Right));
    }
    let v = e.view(10, 3);
    assert_aligned(&v);
    assert_eq!(v.lines[0], "6789\"; // ");
    assert_eq!(&v.styles[0][..5], &[Style::Str; 5]);
    assert_eq!(v.styles[0][5], Style::Normal);
    assert_eq!(v.styles[0][8], Style::Comment);
}

#[test]
fn non_rust_and_unnamed_buffers_are_all_normal() {
    let mut e = named_editor("a.txt", "fn x() // c\n");
    let v = e.view(20, 4);
    assert!(v.styles.iter().flatten().all(|s| *s == Style::Normal));
    let mut e = Editor::new(Buffer::from_text("fn x() // c\n"));
    let v = e.view(20, 4);
    assert_aligned(&v);
    assert!(v.styles.iter().flatten().all(|s| *s == Style::Normal));
}

#[test]
fn multiline_block_comment_state_carries_into_visible_rows_when_scrolled_down() {
    let mut text = String::from("/* start\n");
    for _ in 0..10 {
        text.push_str("let x = 1;\n");
    }
    text.push_str("end */ let y;\n");
    let mut e = named_editor("a.rs", &text);
    for _ in 0..8 {
        e.handle_key(key(KeyCode::Down));
    }
    let v = e.view(30, 5);
    assert!(v.styles[0].iter().all(|s| *s == Style::Comment));
    for _ in 0..3 {
        e.handle_key(key(KeyCode::Down));
    }
    let v = e.view(30, 5);
    let last = v.lines.iter().position(|l| l.starts_with("end */")).unwrap();
    assert_eq!(v.styles[last][0], Style::Comment);
    assert_eq!(v.styles[last][5], Style::Comment);
    assert_eq!(v.styles[last][7], Style::Keyword);
}
