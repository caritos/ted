use super::*;
use std::fs;

fn numbered(n: usize) -> Editor {
    let text: Vec<String> = (1..=n).map(|i| format!("l{i}")).collect();
    let mut e = Editor::new(Buffer::from_text(&text.join("\n")));
    e.set_line_numbers(true);
    e
}

fn rs_editor(text: &str) -> Editor {
    let dir = std::env::temp_dir().join(format!("ted-gutter-{}-{}", std::process::id(), text.len()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("x.rs");
    fs::write(&path, text).unwrap();
    Editor::new(Buffer::open(&path).unwrap())
}

#[test]
fn gutter_is_three_digits_and_a_space_for_small_buffers() {
    for n in [5, 9, 10, 999] {
        let mut e = numbered(n);
        let v = e.view(40, 5);
        assert_eq!(v.lines[0], "  1 l1", "{n} lines");
        assert_eq!(v.lines[2], "  3 l3", "{n} lines");
    }
}

#[test]
fn gutter_grows_at_a_thousand_lines() {
    let mut e = numbered(1000);
    let v = e.view(40, 5);
    assert_eq!(v.lines[0], "   1 l1");
    e.buffer.go_to_line(1000);
    let v = e.view(40, 5);
    assert_eq!(v.lines[2], "1000 l1000");
}

#[test]
fn gutter_right_aligns_numbers() {
    let mut e = numbered(12);
    let v = e.view(40, 14);
    assert_eq!(v.lines[9], " 10 l10");
}

#[test]
fn rows_past_the_end_show_a_blank_gutter() {
    let mut e = numbered(2);
    let v = e.view(40, 6);
    assert_eq!(v.lines[2], "    ~");
    assert_eq!(v.lines[3], "    ~");
}

#[test]
fn gutter_chars_are_lineno_and_text_styles_stay_aligned() {
    let mut e = rs_editor("fn a() {}\nlet");
    e.set_line_numbers(true);
    let v = e.view(40, 5);
    assert_eq!(v.lines[0], "  1 fn a() {}");
    assert_eq!(v.styles[0].len(), v.lines[0].chars().count());
    assert!(v.styles[0][..4].iter().all(|s| *s == Style::LineNo));
    assert_eq!(v.styles[0][4], Style::Keyword);
    assert_eq!(v.styles[0][5], Style::Keyword);
    assert_eq!(v.styles[0][6], Style::Normal);
    assert!(v.styles[2][..4].iter().all(|s| *s == Style::LineNo));
}

#[test]
fn cursor_column_includes_the_gutter() {
    let mut e = Editor::new(Buffer::from_text("abc"));
    e.set_line_numbers(true);
    e.buffer.set_cursor(0, 2);
    assert_eq!(e.view(40, 5).cursor, (0, 6));
}

#[test]
fn horizontal_scroll_applies_to_the_text_area() {
    let mut e = Editor::new(Buffer::from_text(&"abcdefghijklmnopqrstuvwxyz".repeat(2)));
    e.set_line_numbers(true);
    e.buffer.set_cursor(0, 30);
    let v = e.view(20, 5); // text area is 16 wide
    assert_eq!(v.lines[0].chars().count(), 20);
    assert_eq!(v.cursor, (0, 19));
    assert_eq!(v.lines[0], "  1 pqrstuvwxyzabcdef".chars().take(20).collect::<String>());
}

#[test]
fn off_is_the_plain_layout() {
    let mut e = Editor::new(Buffer::from_text("abc\ndef"));
    e.buffer.set_cursor(1, 2);
    let v = e.view(40, 5);
    assert_eq!(v.lines, vec!["abc", "def", "~"]);
    assert_eq!(v.cursor, (1, 2));
    assert!(v.styles[0].iter().all(|s| *s == Style::Normal));
}

#[test]
fn toggling_mid_session_keeps_the_cursor_visible() {
    let mut e = Editor::new(Buffer::from_text(&"x".repeat(30)));
    e.buffer.set_cursor(0, 29);
    assert_eq!(e.view(20, 4).cursor, (0, 19));
    e.set_line_numbers(true);
    assert_eq!(e.view(20, 4).cursor, (0, 19));
    e.set_line_numbers(false);
    let v = e.view(20, 4);
    // Scroll is only moved as far as needed to show the cursor.
    assert_eq!(v.cursor, (0, 15));
    assert_eq!(v.lines[0], "x".repeat(16));
}

#[test]
fn gutter_hides_below_the_threshold() {
    // gutter is 4 wide for a short buffer
    let mut e = numbered(3);
    assert_eq!(e.view(4 + 9, 4).lines[0], "l1");
    assert_eq!(e.view(4 + 10, 4).lines[0], "  1 l1");
    // 1000 lines: gutter is 5 wide
    let mut e = numbered(1000);
    assert_eq!(e.view(5 + 9, 4).lines[0], "l1");
    assert_eq!(e.view(5 + 10, 4).lines[0], "   1 l1");
}

#[test]
fn tiny_sizes_do_not_panic_with_the_gutter_on_or_off() {
    for on in [false, true] {
        for w in 0..=6 {
            for h in 0..=6 {
                let mut e = Editor::new(Buffer::from_text("hello\nworld\nfoo"));
                e.set_line_numbers(on);
                e.buffer.set_cursor(2, 3);
                let v = e.view(w, h);
                assert!(v.lines.iter().all(|l| l.chars().count() <= w));
                assert_eq!(v.lines.len(), h.saturating_sub(2));
                assert!(w == 0 || v.cursor.1 < w);
            }
        }
    }
}

#[test]
fn cursor_never_leaves_the_width_around_the_threshold() {
    for w in 8..=20 {
        let mut e = numbered(3);
        e.buffer.set_cursor(2, 1);
        let v = e.view(w, 6);
        assert!(v.cursor.1 < w, "width {w}");
    }
}

#[test]
fn hostile_text_is_still_sanitized_one_for_one() {
    let mut e = Editor::new(Buffer::from_text("a\u{1b}[31mb\tc"));
    e.set_line_numbers(true);
    let v = e.view(40, 4);
    assert_eq!(v.lines[0], "  1 a?[31mb c");
    assert_eq!(v.styles[0].len(), v.lines[0].chars().count());
}
