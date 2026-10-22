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

fn press(e: &mut Editor, code: KeyCode, times: usize) {
    for _ in 0..times {
        e.handle_key(key(code));
    }
}

/// The visible styles of `v` computed from scratch, walking every line of
/// the buffer from line 0 without any cache.
fn reference_styles(e: &Editor, v: &View, width: usize) -> Vec<Vec<Style>> {
    let mut state = LineState::default();
    let mut all = Vec::new();
    for r in 0..e.buffer.line_count() {
        let (tokens, next) = highlight_line(Language::Rust, state, &e.buffer.line(r));
        state = next;
        all.push(tokens);
    }
    (0..v.lines.len())
        .map(|i| match all.get(e.scroll_row + i) {
            Some(tokens) => {
                tokens.iter().skip(e.scroll_col).take(width).map(|t| token_style(*t)).collect()
            }
            None => vec![Style::Normal; v.lines[i].chars().count()],
        })
        .collect()
}

/// Small deterministic generator so the edit sequences are reproducible.
struct Lcg(u64);

impl Lcg {
    fn below(&mut self, n: usize) -> usize {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((self.0 >> 33) as usize) % n
    }
}

#[test]
fn cached_styles_match_a_from_scratch_walk_after_every_edit() {
    let text: String = (0..60).map(|i| format!("let v{i} = \"s\"; // c\n")).collect();
    let chars = ['/', '*', '"', '\\', '#', '[', ']', 'r', '\'', 'a', ' '];
    let sizes = [(30, 8), (12, 5), (50, 20), (20, 3), (40, 1)];
    for seed in 1..=4u64 {
        let mut e = named_editor("a.rs", &text);
        let mut rng = Lcg(seed);
        for step in 0..400 {
            match rng.below(14) {
                0 => press(&mut e, KeyCode::Enter, 1),
                1 => press(&mut e, KeyCode::Backspace, 1),
                2 => press(&mut e, KeyCode::Delete, 1),
                3 => press(&mut e, KeyCode::Up, 1 + rng.below(25)),
                4 => press(&mut e, KeyCode::Down, 1 + rng.below(25)),
                5 => press(&mut e, KeyCode::Left, 1),
                6 => press(&mut e, KeyCode::Right, 1),
                _ => press(&mut e, KeyCode::Char(chars[rng.below(chars.len())]), 1),
            }
            let (w, h) = sizes[rng.below(sizes.len())];
            let v = e.view(w, h);
            assert_eq!(v.styles, reference_styles(&e, &v, w), "seed {seed} step {step}");
        }
    }
}

#[test]
fn opening_and_closing_a_comment_or_string_above_the_viewport_restyles_it() {
    let text: String = (0..100).map(|i| format!("let x{i} = 1;\n")).collect();
    let mut e = named_editor("a.rs", &text);
    let check = |e: &mut Editor, first: Style| {
        let v = e.view(30, 10);
        assert_eq!(v.styles, reference_styles(e, &v, 30));
        assert_eq!(v.styles[0][0], first);
    };
    press(&mut e, KeyCode::Down, 90);
    check(&mut e, Style::Keyword);
    // Each edit happens at row 10, far above the viewport at rows 81..=90.
    let edits: [(&[KeyCode], Style); 6] = [
        (&[KeyCode::Char('/'), KeyCode::Char('*')], Style::Comment),
        (&[KeyCode::Backspace, KeyCode::Backspace], Style::Keyword),
        (&[KeyCode::Char('"')], Style::Str),
        (&[KeyCode::Left, KeyCode::Delete], Style::Keyword),
        (&[KeyCode::Char('/'), KeyCode::Char('*'), KeyCode::Enter], Style::Comment),
        (&[KeyCode::Backspace, KeyCode::Backspace, KeyCode::Backspace], Style::Keyword),
    ];
    for (keys, first) in edits {
        press(&mut e, KeyCode::Up, 80);
        for k in keys {
            press(&mut e, *k, 1);
        }
        press(&mut e, KeyCode::Down, 80);
        check(&mut e, first);
    }
}

#[test]
fn the_highlight_cache_only_walks_lines_it_has_not_seen() {
    let text: String = (0..10_000).map(|i| format!("let x{i} = 1;\n")).collect();
    let mut e = named_editor("big.rs", &text);
    e.view(80, 40);
    assert_eq!(e.walked_lines, 0);
    press(&mut e, KeyCode::Down, 9_999);
    e.view(80, 40);
    let cold = e.walked_lines;
    assert!((9_900..=10_001).contains(&cold), "cold walk {cold}");

    // Moving and scrolling without edits walks nothing.
    e.walked_lines = 0;
    for code in [KeyCode::Up, KeyCode::Down, KeyCode::Right, KeyCode::Left] {
        for _ in 0..100 {
            press(&mut e, code, 1);
            e.view(80, 40);
        }
    }
    assert_eq!(e.walked_lines, 0);

    // An edit at the bottom walks only a few lines.
    press(&mut e, KeyCode::Char('x'), 1);
    press(&mut e, KeyCode::Enter, 1);
    press(&mut e, KeyCode::Backspace, 1);
    e.view(80, 40);
    assert!(e.walked_lines <= 3, "walked {}", e.walked_lines);

    // An edit at the top re-walks down to the viewport.
    press(&mut e, KeyCode::Up, 9_999);
    press(&mut e, KeyCode::Char('y'), 1);
    press(&mut e, KeyCode::Down, 9_999);
    e.walked_lines = 0;
    e.view(80, 40);
    assert!((9_900..=10_001).contains(&e.walked_lines), "walked {}", e.walked_lines);
}

#[test]
fn non_highlighted_buffers_walk_and_cache_nothing() {
    let text: String = (0..1_000).map(|i| format!("/* x{i}\n")).collect();
    let mut e = named_editor("big.txt", &text);
    press(&mut e, KeyCode::Down, 999);
    e.view(80, 40);
    assert_eq!(e.walked_lines, 0);
    assert_eq!(e.line_states.capacity(), 0);
}

#[test]
fn load_clears_the_highlight_cache() {
    let mut text = String::from("/*\n");
    text.push_str(&"let x = 1;\n".repeat(50));
    let mut e = named_editor("a.rs", &text);
    press(&mut e, KeyCode::Down, 50);
    let v = e.view(30, 10);
    assert_eq!(v.styles[0][0], Style::Comment);
    assert!(!e.line_states.is_empty());

    // Same language and shape, but no comment.
    let dir = temp_file("load").parent().unwrap().to_path_buf();
    let path = dir.join("b.rs");
    fs::write(&path, "let x = 1;\n".repeat(51)).unwrap();
    e.load(Buffer::open(&path).unwrap());
    assert!(e.line_states.is_empty());
    press(&mut e, KeyCode::Down, 50);
    let v = e.view(30, 10);
    assert_eq!(v.styles, reference_styles(&e, &v, 30));
    assert_eq!(v.styles[0][0], Style::Keyword);

    // Switching to a non-highlighted buffer leaves the cache empty.
    e.load(Buffer::from_text(&text));
    e.view(30, 10);
    assert!(e.line_states.is_empty());
}

// ---- Control characters never reach the terminal ----

/// Lines with CSI and OSC sequences, a lone CR (ropey splits the line
/// there), DEL, a C1 CSI and a tab.
const NASTY: &str =
    "red \x1b[31mX\x1b[0m end\nset \x1b]0;t\x07 title\ncr a\rb\ndel \x7f c1 \u{9b}2J\ttab\n";

fn has_control(s: &str) -> bool {
    s.chars().any(char::is_control)
}

#[test]
fn view_shows_control_characters_as_safe_one_column_stand_ins() {
    for name in ["a.txt", "a.rs"] {
        let mut e = named_editor(name, NASTY);
        e.message = "Save failed: \x1b]52;c;eA==\x07 \u{9d}".to_string();
        let v = e.view(60, 10);
        assert_aligned(&v);
        for (i, line) in v.lines.iter().enumerate() {
            assert!(!has_control(line), "{name} row {i}: {line:?}");
            if i < e.buffer.line_count() {
                // One display char per buffer char, so columns stay aligned.
                let raw = e.buffer.line(i);
                assert_eq!(line.chars().count(), raw.chars().count(), "{name} row {i}");
                assert_eq!(*line, crate::text::sanitize(&raw), "{name} row {i}");
            }
        }
        assert_eq!(v.lines[0], "red ?[31mX?[0m end");
        assert_eq!(v.lines[1], "set ?]0;t? title");
        assert_eq!(v.lines[4], "del ? c1 ?2J tab");
        assert!(!has_control(&v.status), "{:?}", v.status);
        assert!(!has_control(&v.message), "{:?}", v.message);
        assert_eq!(v.message, "Save failed: ?]52;c;eA==? ?");
    }
}

#[test]
fn control_characters_keep_cursor_and_horizontal_scroll_aligned() {
    let mut e = named_editor("a.rs", "\x1b[2Jlet \x7f = \"\x1b\";\n");
    press(&mut e, KeyCode::Right, 12);
    let v = e.view(6, 4);
    assert_aligned(&v);
    assert_eq!(v.lines[0], " ? = \"");
    assert_eq!(v.cursor, (0, 5));
    press(&mut e, KeyCode::Right, 3);
    let v = e.view(6, 4);
    assert_aligned(&v);
    assert_eq!(v.lines[0], "= \"?\";");
    assert_eq!(v.styles[0][3], Style::Str);
    assert_eq!(v.cursor, (0, 5));
    assert!(v.lines.iter().all(|l| !has_control(l)));
}

#[test]
fn file_name_with_an_escape_shows_a_question_mark_in_the_status_bar() {
    let dir = temp_file("escname").parent().unwrap().to_path_buf();
    let path = dir.join("a\x1b]0;x\x07b.txt");
    fs::write(&path, "hi\n").unwrap();
    let mut e = Editor::new(Buffer::open(&path).unwrap());
    for width in [0, 3, 10, 30, 80] {
        let v = e.view(width, 5);
        assert!(!has_control(&v.status), "{:?}", v.status);
        assert_eq!(v.status.chars().count(), width, "{:?}", v.status);
    }
    let status = e.view(80, 5).status;
    assert!(status.starts_with(" a?]0;x?b.txt "), "{status:?}");
}

#[test]
fn saving_a_file_with_control_characters_writes_the_original_bytes() {
    let path = temp_file("ctlsave");
    fs::write(&path, NASTY).unwrap();
    let mut e = Editor::new(Buffer::open(&path).unwrap());
    e.view(40, 10);
    e.handle_key(ctrl('s'));
    assert!(e.message().starts_with("Saved"), "{}", e.message());
    assert_eq!(fs::read(&path).unwrap(), NASTY.as_bytes());
}

// ---- Tiny terminals after scrolling ----

fn long_text() -> String {
    (0..12).map(|i| format!("let v{i} = \"abcdefghij\"; // c\n")).collect()
}

/// `v` fits a `width` x `height` terminal and the cursor is inside the text
/// area whenever there is one.
fn assert_fits(v: &View, width: usize, height: usize) {
    let at = format!("{width}x{height}");
    assert!(v.lines.len() <= height.saturating_sub(2), "{at}: {} rows", v.lines.len());
    assert!(v.lines.iter().all(|l| l.chars().count() <= width), "{at}: {:?}", v.lines);
    assert_aligned(v);
    assert!(v.status.chars().count() <= width, "{at}: {:?}", v.status);
    assert!(v.message.chars().count() <= width, "{at}: {:?}", v.message);
    if !v.lines.is_empty() && width > 0 {
        assert!(v.cursor.0 < v.lines.len() && v.cursor.1 < width, "{at}: cursor {:?}", v.cursor);
    }
}

#[test]
fn view_with_no_text_rows_after_scrolling_down_does_not_underflow() {
    let mut e = named_editor("a.txt", &long_text());
    press(&mut e, KeyCode::Down, 11);
    e.view(40, 6); // scroll_row > 0
    press(&mut e, KeyCode::Up, 11);
    for h in 0..3 {
        let v = e.view(40, h);
        assert_fits(&v, 40, h);
    }
    // Back at a normal size the cursor row scrolls into view again.
    let v = e.view(40, 6);
    assert_eq!(v.cursor, (0, 0));
    assert!(v.lines[0].starts_with("let v0 "), "{:?}", v.lines);
}

#[test]
fn view_with_zero_width_after_scrolling_right_does_not_underflow() {
    let mut e = named_editor("a.txt", &long_text());
    press(&mut e, KeyCode::Right, 20);
    e.view(5, 6); // scroll_col > 0
    press(&mut e, KeyCode::Left, 20);
    let v = e.view(0, 6);
    assert_fits(&v, 0, 6);
    let v = e.view(5, 6);
    assert_eq!(v.cursor, (0, 0));
    assert_eq!(v.lines[0], "let v");
}

#[test]
fn view_fits_every_tiny_size_after_scrolling_and_returning_to_the_top_left() {
    for name in ["a.rs", "a.txt"] {
        for width in 0..=6 {
            for height in 0..=6 {
                let mut e = named_editor(name, &long_text());
                e.message = "a long message that is wider than the terminal".to_string();
                press(&mut e, KeyCode::Down, 11);
                press(&mut e, KeyCode::Right, 25);
                assert_fits(&e.view(width, height), width, height);
                e.view(30, 8); // scrolled down and right at a normal size
                press(&mut e, KeyCode::Up, 11);
                press(&mut e, KeyCode::Left, 40);
                assert_eq!(e.buffer.cursor(), (0, 0));
                let v = e.view(width, height);
                assert_fits(&v, width, height);
                // Shrinking through every other size never panics either.
                for w in 0..=6 {
                    for h in 0..=6 {
                        assert_fits(&e.view(w, h), w, h);
                    }
                }
            }
        }
    }
}
