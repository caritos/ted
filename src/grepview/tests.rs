use super::*;

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn type_str(v: &mut GrepView, text: &str) {
    for c in text.chars() {
        v.handle_key(key(KeyCode::Char(c)));
    }
}

fn hit(path: &str, line: usize, text: &str) -> Hit {
    Hit { path: path.into(), line, text: text.into() }
}

fn found(hits: Vec<Hit>) -> Found {
    let files_with_hits = hits.iter().map(|h| h.path.as_str()).collect::<std::collections::BTreeSet<_>>().len();
    Found { hits, capped: false, files_with_hits }
}

fn searched() -> GrepView {
    let mut v = GrepView::new("foo");
    assert_eq!(v.handle_key(key(KeyCode::Enter)), GrepAction::Search("foo".into()));
    v.set_found(found(vec![hit("a.rs", 3, "let foo = 1;"), hit("a.rs", 9, "foo()"), hit("src/b.rs", 20, "// foo")]));
    v
}

fn text(screen: &Screen, i: usize) -> String {
    screen.rows[i].iter().map(|s| s.text.as_str()).collect()
}

#[test]
fn starts_with_the_prefill_and_no_results() {
    let v = GrepView::new("needle");
    let screen = v.render(50, 6);
    assert_eq!(screen.message, "Find in files: needle_");
    assert_eq!(screen.status.trim_end(), "Find in files: Enter searches");
    assert_eq!(screen.rows.len(), 4);
    assert!(screen.rows.iter().all(|r| r.is_empty()));
    assert_eq!(screen.cursor, None);
}

#[test]
fn enter_asks_for_a_search_and_an_empty_query_asks_for_nothing() {
    let mut v = GrepView::new("");
    assert_eq!(v.handle_key(key(KeyCode::Enter)), GrepAction::None);
    type_str(&mut v, "ab");
    assert_eq!(v.handle_key(key(KeyCode::Enter)), GrepAction::Search("ab".into()));
}

#[test]
fn results_list_path_line_and_text_with_the_first_selected() {
    let v = searched();
    let screen = v.render(50, 8);
    assert_eq!(text(&screen, 0).trim_end(), "  a.rs:3: let foo = 1;");
    assert_eq!(screen.rows[0][0].style, Style::Selected);
    assert_eq!(text(&screen, 2).trim_end(), "  src/b.rs:20: // foo");
    assert_eq!(screen.rows[2][0].style, Style::Normal);
    assert_eq!(screen.status.trim_end(), "Find in files: 3 hits in 2 files");
}

#[test]
fn singular_wording_and_the_cap_note() {
    let mut v = GrepView::new("x");
    v.set_found(found(vec![hit("a", 1, "x")]));
    assert_eq!(v.render(50, 6).status.trim_end(), "Find in files: 1 hit in 1 file");
    v.set_found(Found { hits: vec![hit("a", 1, "x")], capped: true, files_with_hits: 1 });
    assert!(v.render(60, 6).status.contains("(first 2000)"));
}

#[test]
fn no_hits_says_so() {
    let mut v = GrepView::new("zzz");
    v.set_found(found(vec![]));
    let screen = v.render(40, 6);
    assert_eq!(text(&screen, 0).trim_end(), "  No matches");
    assert_eq!(v.handle_key(key(KeyCode::Enter)), GrepAction::None);
}

#[test]
fn arrows_move_the_selection_and_enter_opens_it() {
    let mut v = searched();
    v.handle_key(key(KeyCode::Down));
    v.handle_key(key(KeyCode::Down));
    v.handle_key(key(KeyCode::Down));
    assert_eq!(v.handle_key(key(KeyCode::Enter)), GrepAction::Open { path: "src/b.rs".into(), line: 20 });
    v.handle_key(key(KeyCode::Home));
    assert_eq!(v.handle_key(key(KeyCode::Enter)), GrepAction::Open { path: "a.rs".into(), line: 3 });
    v.handle_key(key(KeyCode::End));
    v.handle_key(key(KeyCode::Up));
    assert_eq!(v.handle_key(key(KeyCode::Enter)), GrepAction::Open { path: "a.rs".into(), line: 9 });
    v.handle_key(key(KeyCode::PageUp));
    assert_eq!(v.handle_key(key(KeyCode::Enter)), GrepAction::Open { path: "a.rs".into(), line: 3 });
    v.handle_key(key(KeyCode::PageDown));
    assert_eq!(v.handle_key(key(KeyCode::Enter)), GrepAction::Open { path: "src/b.rs".into(), line: 20 });
}

#[test]
fn editing_the_query_makes_enter_search_again_and_marks_the_results_old() {
    let mut v = searched();
    v.handle_key(key(KeyCode::Backspace));
    let screen = v.render(60, 6);
    assert_eq!(screen.message, "Find in files: fo_");
    assert!(screen.status.contains("Enter searches again"), "{}", screen.status);
    assert_eq!(text(&screen, 0).trim_end(), "  a.rs:3: let foo = 1;");
    assert_eq!(v.handle_key(key(KeyCode::Enter)), GrepAction::Search("fo".into()));
    // Back to the searched text: the old results are current again.
    let mut w = searched();
    w.handle_key(key(KeyCode::Char('x')));
    w.handle_key(key(KeyCode::Backspace));
    assert_eq!(w.handle_key(key(KeyCode::Enter)), GrepAction::Open { path: "a.rs".into(), line: 3 });
}

#[test]
fn new_results_reset_the_selection() {
    let mut v = searched();
    v.handle_key(key(KeyCode::End));
    v.handle_key(key(KeyCode::Char('o')));
    v.set_found(found(vec![hit("c", 1, "fooo"), hit("d", 2, "fooo")]));
    assert_eq!(v.handle_key(key(KeyCode::Enter)), GrepAction::Open { path: "c".into(), line: 1 });
}

#[test]
fn esc_and_f4_go_back_and_q_is_text() {
    let mut v = searched();
    assert_eq!(v.handle_key(key(KeyCode::Esc)), GrepAction::Back);
    assert_eq!(v.handle_key(key(KeyCode::F(4))), GrepAction::Back);
    v.handle_key(key(KeyCode::Char('q')));
    assert_eq!(v.query(), "fooq");
}

#[test]
fn a_paste_is_added_without_line_breaks() {
    let mut v = GrepView::new("");
    v.paste("a\nb\r\tc");
    assert_eq!(v.query(), "abc");
}

#[test]
fn hostile_text_is_drawn_as_question_marks() {
    let mut v = GrepView::new("x\u{1b}[2J");
    v.set_found(found(vec![hit("a\u{1b}b", 1, "\u{1b}[31mred")]));
    let screen = v.render(60, 6);
    let all: String = (0..4).map(|i| text(&screen, i)).chain([screen.status.clone(), screen.message.clone()]).collect();
    assert!(!all.contains('\u{1b}'));
    assert!(text(&screen, 0).contains("a?b:1: ?[31mred"));
}

#[test]
fn the_selection_stays_in_view_and_every_size_renders() {
    let mut v = GrepView::new("x");
    v.set_found(found((1..=30).map(|i| hit("f", i, &format!("line {i}"))).collect()));
    for _ in 0..25 {
        v.handle_key(key(KeyCode::Down));
    }
    let screen = v.render(40, 8);
    assert!(screen.rows.iter().any(|r| text_of(r).contains("f:26: line 26") && r[0].style == Style::Selected));
    for w in 0..=8 {
        for h in 0..=6 {
            let screen = v.render(w, h);
            assert_eq!(screen.rows.len(), h.saturating_sub(2));
            for row in &screen.rows {
                assert!(text_of(row).chars().count() <= w);
            }
        }
    }
}

fn text_of(row: &[Span]) -> String {
    row.iter().map(|s| s.text.as_str()).collect()
}
