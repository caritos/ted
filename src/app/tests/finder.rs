use super::*;

fn listed(dir: &Temp, files: &[&str]) -> App {
    let mut git = one_file();
    git.files = Some(files.iter().map(|s| s.to_string()).collect());
    app_with(Editor::new(Buffer::from_text("scratch")), dir.0.clone(), git)
}

fn row(app: &mut App, i: usize) -> String {
    row_text(&app.screen(60, 10, t(1000)), i)
}

fn msg(app: &mut App) -> String {
    app.screen(60, 10, t(1000)).message
}

#[test]
fn ctrl_e_lists_the_files_and_typing_ranks_them() {
    let dir = Temp::new("finder-list");
    let mut app = listed(&dir, &["README.md", "src/app.rs", "src/editor.rs"]);
    assert!(!app.in_finder());
    assert_eq!(app.handle_key(ctrl('e'), t(1000)), Flow::Continue);
    assert!(app.in_finder());
    assert_eq!(row(&mut app, 0).trim_end(), "  README.md");
    assert_eq!(app.screen(60, 10, t(1000)).cursor, None);
    type_str(&mut app, "edi");
    assert_eq!(row(&mut app, 0).trim_end(), "  src/editor.rs");
    assert_eq!(msg(&mut app), "Open: edi_");
    // The keys went to the query, not the buffer.
    assert_eq!(app.editor().buffer().text(), "scratch");
}

#[test]
fn enter_opens_the_file_as_a_new_buffer_and_returns_to_the_editor() {
    let dir = Temp::new("finder-open");
    dir.file("src/app.rs", "fn app() {}\n");
    let mut app = listed(&dir, &["README.md", "src/app.rs"]);
    app.handle_key(ctrl('e'), t(1000));
    type_str(&mut app, "app");
    app.handle_key(key(KeyCode::Enter), t(1000));
    assert!(!app.in_finder());
    assert_eq!(app.buffer_count(), 2);
    assert_eq!(app.editor().buffer().text(), "fn app() {}\n");
    assert!(app.screen(40, 6, t(1000)).status.contains("app.rs"));
}

#[test]
fn an_already_open_file_is_switched_to_not_opened_twice() {
    let dir = Temp::new("finder-twice");
    dir.file("a.txt", "A\n");
    let mut app = listed(&dir, &["a.txt"]);
    for _ in 0..2 {
        app.handle_key(ctrl('e'), t(1000));
        app.handle_key(key(KeyCode::Enter), t(1000));
    }
    assert_eq!(app.buffer_count(), 2, "scratch plus a.txt, once");
}

#[test]
fn esc_and_ctrl_e_go_back_with_the_editor_state_intact() {
    let dir = Temp::new("finder-back");
    let mut app = listed(&dir, &["a.txt"]);
    app.handle_key(key(KeyCode::End), t(1000));
    type_str(&mut app, "!");
    for back in [key(KeyCode::Esc), ctrl('e')] {
        app.handle_key(ctrl('e'), t(1000));
        assert!(app.in_finder());
        app.handle_key(back, t(1000));
        assert!(!app.in_finder());
        assert_eq!(app.editor().buffer().text(), "scratch!");
        assert_eq!(app.editor().buffer().cursor(), (0, 8));
    }
}

#[test]
fn the_open_command_starts_the_finder() {
    let dir = Temp::new("finder-cmd");
    let mut app = listed(&dir, &["a.txt"]);
    app.handle_key(ctrl('x'), t(1000));
    type_str(&mut app, "open");
    app.handle_key(key(KeyCode::Enter), t(1000));
    assert!(app.in_finder());
}

#[test]
fn outside_a_repo_the_files_come_from_a_directory_walk() {
    let dir = Temp::new("finder-walk");
    dir.file("one.txt", "1");
    dir.file("sub/two.txt", "2");
    dir.file(".hidden", "h");
    // `one_file()` has no file list: NotARepo, so the app walks.
    let mut app = app_with(Editor::new(Buffer::from_text("")), dir.0.clone(), one_file());
    app.handle_key(ctrl('e'), t(1000));
    assert_eq!(row(&mut app, 0).trim_end(), "  one.txt");
    assert_eq!(row(&mut app, 1).trim_end(), "  sub/two.txt");
    assert_eq!(row(&mut app, 2).trim_end(), "");
}

#[test]
fn a_file_that_vanished_shows_a_message_and_stays_in_the_finder() {
    let dir = Temp::new("finder-gone");
    let mut app = listed(&dir, &["gone.txt"]);
    app.handle_key(ctrl('e'), t(1000));
    app.handle_key(key(KeyCode::Enter), t(1000));
    assert!(app.in_finder());
    assert_eq!(msg(&mut app), "File no longer exists");
    assert_eq!(app.buffer_count(), 1);
}

#[test]
fn question_mark_and_q_are_text_but_f1_still_opens_help() {
    let dir = Temp::new("finder-keys");
    let mut app = listed(&dir, &["a.txt"]);
    app.handle_key(ctrl('e'), t(1000));
    type_str(&mut app, "q?");
    assert!(app.in_finder() && !app.in_help());
    assert_eq!(msg(&mut app), "Open: q?_");
    app.handle_key(key(KeyCode::F(1)), t(1000));
    assert!(app.in_help());
    app.handle_key(key(KeyCode::Esc), t(1000));
    assert!(app.in_finder(), "help returns to the finder");
}

#[test]
fn ctrl_e_does_nothing_inside_a_prompt_and_ctrl_w_is_ignored_in_the_finder() {
    let dir = Temp::new("finder-ignored");
    let mut app = listed(&dir, &["a.txt"]);
    app.handle_key(ctrl('f'), t(1000));
    app.handle_key(ctrl('e'), t(1000));
    assert!(!app.in_finder());
    app.handle_key(key(KeyCode::Esc), t(1000));
    app.handle_key(ctrl('e'), t(1000));
    type_str(&mut app, "x");
    app.handle_key(ctrl('w'), t(1000));
    assert!(app.in_finder());
    assert_eq!(app.buffer_count(), 1);
}

#[test]
fn ctrl_q_follows_the_quit_rules_and_warns_inside_the_finder() {
    let dir = Temp::new("finder-quit");
    let mut app = listed(&dir, &["a.txt"]);
    type_str(&mut app, "x");
    app.handle_key(ctrl('e'), t(1000));
    assert_eq!(app.handle_key(ctrl('q'), t(1000)), Flow::Continue);
    assert!(msg(&mut app).starts_with("Unsaved changes in 1 buffer"), "{:?}", msg(&mut app));
    assert_eq!(app.handle_key(ctrl('q'), t(1000)), Flow::Quit);
}

#[test]
fn a_bracketed_paste_goes_into_the_query() {
    let dir = Temp::new("finder-paste");
    let mut app = listed(&dir, &["src/app.rs", "README.md"]);
    app.handle_key(ctrl('e'), t(1000));
    app.paste("app\n");
    assert_eq!(msg(&mut app), "Open: app_");
    assert_eq!(row(&mut app, 0).trim_end(), "  src/app.rs");
    assert_eq!(app.editor().buffer().text(), "scratch");
}

#[test]
fn every_visit_lists_the_files_again() {
    let dir = Temp::new("finder-fresh");
    let mut app = listed(&dir, &["a.txt"]);
    app.handle_key(ctrl('e'), t(1000));
    type_str(&mut app, "zz");
    app.handle_key(key(KeyCode::Esc), t(1000));
    app.handle_key(ctrl('e'), t(1000));
    assert_eq!(msg(&mut app), "Open: _", "the query starts empty");
}

#[test]
fn the_finder_draws_at_every_size() {
    let dir = Temp::new("finder-sizes");
    let mut app = listed(&dir, &["a.txt", "b/c.rs"]);
    app.handle_key(ctrl('e'), t(1000));
    for w in 0..=12 {
        for h in 0..=6 {
            let screen = app.screen(w, h, t(1000));
            assert_eq!(screen.rows.len(), h.saturating_sub(2));
        }
    }
}

#[test]
fn recently_used_files_come_first_when_the_query_is_empty() {
    let dir = Temp::new("finder-recent");
    for f in ["a.rs", "b.rs", "c.rs", "d.rs"] {
        dir.file(f, "x\n");
    }
    let mut app = listed(&dir, &["a.rs", "b.rs", "c.rs", "d.rs"]);
    for name in ["c", "b"] {
        app.handle_key(ctrl('e'), t(1000));
        type_str(&mut app, &format!("{name}.rs"));
        app.handle_key(key(KeyCode::Enter), t(1000));
    }
    app.handle_key(ctrl('e'), t(1000));
    let rows: Vec<String> = (0..4).map(|i| row(&mut app, i).trim().to_string()).collect();
    assert_eq!(rows, ["b.rs", "c.rs", "a.rs", "d.rs"]);
}

#[test]
fn recency_breaks_ties_between_equal_matches() {
    let dir = Temp::new("finder-recent-tie");
    for f in ["x1.rs", "x2.rs"] {
        dir.file(f, "x\n");
    }
    let mut app = listed(&dir, &["x1.rs", "x2.rs"]);
    app.handle_key(ctrl('e'), t(1000));
    type_str(&mut app, "x2.rs");
    app.handle_key(key(KeyCode::Enter), t(1000));
    app.handle_key(ctrl('e'), t(1000));
    type_str(&mut app, "rs");
    assert_eq!(row(&mut app, 0).trim(), "x2.rs");
}
