use super::*;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::fs;

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn plain(text: &str) -> Editor {
    Editor::new(Buffer::from_text(text))
}

fn rs_editor(text: &str) -> Editor {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!("ted-tabs-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("x.rs");
    fs::write(&path, text).unwrap();
    Editor::new(Buffer::open(&path).unwrap())
}

#[test]
fn a_leading_tab_draws_four_spaces() {
    let mut e = plain("\tx");
    let v = e.view(20, 4);
    assert_eq!(v.lines[0], "    x");
    assert_eq!(v.styles[0].len(), 5);
}

#[test]
fn a_tab_in_the_middle_goes_to_the_next_stop() {
    let mut e = plain("ab\tc\nabc\td\n\t\tz\n");
    let v = e.view(20, 6);
    assert_eq!(v.lines[0], "ab  c");
    assert_eq!(v.lines[1], "abc d");
    assert_eq!(v.lines[2], "        z");
}

#[test]
fn a_line_of_only_tabs() {
    let mut e = plain("\t\t");
    let v = e.view(20, 3);
    assert_eq!(v.lines[0], " ".repeat(8));
}

#[test]
fn cursor_sits_on_the_first_cell_of_a_tab_and_after_it_at_the_stop() {
    let mut e = plain("\tx");
    assert_eq!(e.view(20, 4).cursor, (0, 0));
    e.handle_key(key(KeyCode::Right));
    assert_eq!(e.view(20, 4).cursor, (0, 4));
    e.handle_key(key(KeyCode::Right));
    assert_eq!(e.view(20, 4).cursor, (0, 5));
}

#[test]
fn end_on_a_line_ending_in_a_tab() {
    let mut e = plain("x\t");
    e.handle_key(key(KeyCode::End));
    assert_eq!(e.view(20, 4).cursor, (0, 4));
}

#[test]
fn typing_right_after_a_tab_lands_at_the_stop() {
    let mut e = plain("\t");
    e.handle_key(key(KeyCode::End));
    e.handle_key(key(KeyCode::Char('q')));
    let v = e.view(20, 4);
    assert_eq!(v.lines[0], "    q");
    assert_eq!(v.cursor, (0, 5));
}

#[test]
fn tab_key_still_inserts_spaces() {
    let mut e = plain("");
    e.handle_key(key(KeyCode::Tab));
    assert_eq!(e.buffer.text(), "    ");
}

#[test]
fn a_tab_cut_by_the_left_edge_shows_its_remaining_cells() {
    let mut e = plain("a\tbcdefgh");
    e.scroll_col = 2;
    e.buffer.set_cursor(0, 3);
    let v = e.view(6, 4);
    assert_eq!(e.scroll_col, 2);
    assert_eq!(v.lines[0], "  bcde");
}

#[test]
fn cursor_stays_inside_the_text_width_with_tabs() {
    let lines = ["\t\t\t\tabc\t\tdef", "x\ty\tz\t\t\t\t", "\t", "", "plain text here"];
    let text = lines.join("\n");
    let mut seed = 12345u64;
    for gutter in [false, true] {
        for width in [4usize, 7, 10, 16, 30] {
            let mut e = plain(&text);
            e.set_line_numbers(gutter);
            for _ in 0..300 {
                seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                let code = match (seed >> 33) % 6 {
                    0 => KeyCode::Left,
                    1 => KeyCode::Right,
                    2 => KeyCode::Up,
                    3 => KeyCode::Down,
                    4 => KeyCode::End,
                    _ => KeyCode::Home,
                };
                e.handle_key(key(code));
                let v = e.view(width, 6);
                assert!(v.cursor.1 < width, "width {width} cursor {:?}", v.cursor);
                // The gutter is 4 columns here (<= 999 lines) when it fits.
                let g = if gutter && width >= 14 { 4 } else { 0 };
                for (i, line) in v.lines.iter().enumerate() {
                    assert!(line.chars().count() <= width, "width {width} row {i}: {line:?}");
                    assert!(line.chars().count() <= g + (width - g), "width {width} row {i}");
                    let text_len = line.chars().skip(g).count();
                    assert!(text_len <= width - g, "width {width} row {i}: {line:?}");
                    assert_eq!(v.styles[i].len(), line.chars().count());
                }
                // The cursor cell shows the cursor's char (a tab as a space),
                // or sits just past the end of the row at end of line.
                let (crow, ccol) = e.buffer.cursor();
                let chars: Vec<char> = e.buffer.line(crow).chars().collect();
                let drawn: Vec<char> = v.lines[v.cursor.0].chars().collect();
                let cell = v.cursor.1;
                match chars.get(ccol) {
                    Some(&c) => assert_eq!(drawn.get(cell), Some(&crate::text::sanitize_char(c)), "width {width} gutter {gutter}"),
                    None => assert_eq!(drawn.len(), cell, "width {width} gutter {gutter}"),
                }
            }
        }
    }
}

#[test]
fn indent_before_a_keyword_keeps_styles_on_the_right_cells() {
    let mut e = rs_editor("\tfn main() {}");
    let v = e.view(30, 4);
    assert_eq!(v.lines[0], "    fn main() {}");
    assert_eq!(&v.styles[0][0..4], &[Style::Normal; 4]);
    assert_eq!(&v.styles[0][4..6], &[Style::Keyword; 2]);
    assert_eq!(v.styles[0].len(), v.lines[0].chars().count());
}

#[test]
fn a_tab_inside_a_comment_is_comment_styled() {
    let mut e = rs_editor("a // x\ty");
    let v = e.view(30, 4);
    assert_eq!(v.lines[0], "a // x  y");
    assert!(v.styles[0][2..].iter().all(|s| *s == Style::Comment), "{:?}", v.styles[0]);
    assert_eq!(v.styles[0][0], Style::Normal);
}

#[test]
fn a_tab_inside_a_string_is_string_styled() {
    let mut e = rs_editor("let s = \"a\tb\";");
    let v = e.view(30, 4);
    assert_eq!(v.lines[0], "let s = \"a  b\";");
    // Quote is cell 8, `a` 9, tab cells 10-11, `b` 12, closing quote 13.
    assert!(v.styles[0][8..14].iter().all(|s| *s == Style::Str), "{:?}", v.styles[0]);
}

#[test]
fn line_numbers_shift_everything_and_the_gutter_is_unchanged() {
    let mut e = rs_editor("\tfn main() {}");
    e.set_line_numbers(true);
    let v = e.view(30, 4);
    assert_eq!(v.lines[0], "  1     fn main() {}");
    assert_eq!(&v.styles[0][0..4], &[Style::CurrentLineNo; 4]);
    assert_eq!(&v.styles[0][4..8], &[Style::Normal; 4]);
    assert_eq!(&v.styles[0][8..10], &[Style::Keyword; 2]);
    e.handle_key(key(KeyCode::Right));
    assert_eq!(e.view(30, 4).cursor, (0, 8));
}

#[test]
fn saved_bytes_keep_the_tabs() {
    let dir = std::env::temp_dir().join(format!("ted-tabs-save-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("t.txt");
    fs::write(&path, "a\tb\n\tx\n").unwrap();
    let mut e = Editor::new(Buffer::open(&path).unwrap());
    e.view(20, 5);
    e.handle_key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL));
    assert_eq!(fs::read(&path).unwrap(), b"a\tb\n\tx\n");
}

#[test]
fn tiny_sizes_with_tab_lines_never_panic() {
    for gutter in [false, true] {
        for w in 0..=16 {
            for h in 0..=6 {
                let mut e = plain("\t\tfoo\tbar\n\n\t\n x\t\ty");
                e.set_line_numbers(gutter);
                for code in [KeyCode::Down, KeyCode::End, KeyCode::Down, KeyCode::End, KeyCode::Right, KeyCode::Home] {
                    e.handle_key(key(code));
                    let v = e.view(w, h);
                    assert!(v.lines.iter().all(|l| l.chars().count() <= w));
                }
            }
        }
    }
}
