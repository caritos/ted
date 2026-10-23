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

fn status(app: &mut App) -> String {
    app.screen(60, 10, t(1000)).status
}

fn f4() -> KeyEvent {
    key(KeyCode::F(4))
}

fn enter(app: &mut App) {
    app.handle_key(key(KeyCode::Enter), t(1000));
}

fn project(name: &str) -> (Temp, App) {
    let dir = Temp::new(name);
    dir.file("a.txt", "alpha\nbeta needle\ngamma\n");
    dir.file("src/b.rs", "fn main() {}\n// needle here\nneedle again\n");
    dir.file("c.txt", "nothing\n");
    let app = listed(&dir, &["a.txt", "src/b.rs", "c.txt"]);
    (dir, app)
}

#[test]
fn f4_opens_the_view_and_typing_goes_to_the_query_not_the_buffer() {
    let (_dir, mut app) = project("grep-open");
    assert!(!app.in_grep());
    app.handle_key(f4(), t(1000));
    assert!(app.in_grep());
    assert_eq!(msg(&mut app), "Find in files: _");
    type_str(&mut app, "needle");
    assert_eq!(msg(&mut app), "Find in files: needle_");
    assert_eq!(app.editor().buffer().text(), "scratch");
}

#[test]
fn enter_searches_the_project_and_lists_the_hits() {
    let (_dir, mut app) = project("grep-search");
    app.handle_key(f4(), t(1000));
    type_str(&mut app, "needle");
    enter(&mut app);
    assert!(app.in_grep());
    assert_eq!(row(&mut app, 0).trim_end(), "  a.txt:2: beta needle");
    assert_eq!(row(&mut app, 1).trim_end(), "  src/b.rs:2: // needle here");
    assert_eq!(row(&mut app, 2).trim_end(), "  src/b.rs:3: needle again");
    assert_eq!(status(&mut app).trim_end(), "Find in files: 3 hits in 2 files");
}

#[test]
fn enter_on_a_hit_opens_the_file_at_that_line_and_returns_to_the_editor() {
    let (_dir, mut app) = project("grep-goto");
    app.handle_key(f4(), t(1000));
    type_str(&mut app, "needle");
    enter(&mut app);
    app.handle_key(key(KeyCode::Down), t(1000));
    app.handle_key(key(KeyCode::Down), t(1000));
    enter(&mut app);
    assert!(!app.in_grep());
    assert_eq!(app.buffer_count(), 2);
    assert_eq!(app.editor().buffer().cursor().0, 2, "line 3 is row 2");
    assert!(app.screen(40, 6, t(1000)).status.contains("b.rs"));
}

#[test]
fn opening_a_hit_in_an_open_buffer_switches_to_it_and_jumps() {
    let (_dir, mut app) = project("grep-switch");
    for round in 0..2 {
        app.handle_key(f4(), t(1000));
        if round == 0 {
            type_str(&mut app, "needle");
        }
        enter(&mut app);
        enter(&mut app);
        assert!(!app.in_grep());
    }
    assert_eq!(app.buffer_count(), 2, "scratch plus a.txt, once");
    assert_eq!(app.editor().buffer().cursor().0, 1);
}

#[test]
fn the_opened_term_becomes_the_shared_search_term() {
    let (_dir, mut app) = project("grep-term");
    app.handle_key(f4(), t(1000));
    type_str(&mut app, "needle");
    enter(&mut app);
    enter(&mut app);
    app.handle_key(ctrl('f'), t(1000));
    assert_eq!(msg(&mut app), "Find: needle_");
}

#[test]
fn the_query_starts_with_the_shared_search_term() {
    let (_dir, mut app) = project("grep-prefill");
    app.handle_key(ctrl('f'), t(1000));
    type_str(&mut app, "alpha");
    enter(&mut app);
    app.handle_key(f4(), t(1000));
    assert_eq!(msg(&mut app), "Find in files: alpha_");
    enter(&mut app);
    assert_eq!(row(&mut app, 0).trim_end(), "  a.txt:1: alpha");
}

#[test]
fn no_match_says_so_and_stays() {
    let (_dir, mut app) = project("grep-none");
    app.handle_key(f4(), t(1000));
    type_str(&mut app, "zzzz");
    enter(&mut app);
    assert!(app.in_grep());
    assert_eq!(row(&mut app, 0).trim_end(), "  No matches");
    enter(&mut app);
    assert!(app.in_grep());
}

#[test]
fn a_file_that_vanished_shows_a_message_and_stays_in_the_view() {
    let (dir, mut app) = project("grep-gone");
    app.handle_key(f4(), t(1000));
    type_str(&mut app, "alpha");
    enter(&mut app);
    std::fs::remove_file(dir.0.join("a.txt")).unwrap();
    enter(&mut app);
    assert!(app.in_grep());
    assert_eq!(msg(&mut app), "File no longer exists");
    assert_eq!(app.buffer_count(), 1);
}

#[test]
fn esc_and_f4_go_back_with_the_editor_state_intact() {
    let (_dir, mut app) = project("grep-back");
    app.handle_key(key(KeyCode::End), t(1000));
    type_str(&mut app, "!");
    for back in [key(KeyCode::Esc), f4()] {
        app.handle_key(f4(), t(1000));
        assert!(app.in_grep());
        app.handle_key(back, t(1000));
        assert!(!app.in_grep());
        assert_eq!(app.editor().buffer().text(), "scratch!");
        assert_eq!(app.editor().buffer().cursor(), (0, 8));
    }
}

#[test]
fn the_grep_command_opens_the_view() {
    let (_dir, mut app) = project("grep-cmd");
    app.handle_key(ctrl('x'), t(1000));
    type_str(&mut app, "grep");
    enter(&mut app);
    assert!(app.in_grep());
}

#[test]
fn outside_a_repo_the_files_come_from_a_directory_walk() {
    let dir = Temp::new("grep-walk");
    dir.file("one.txt", "find me\n");
    dir.file("sub/two.txt", "and me: find\n");
    dir.file(".hidden", "find hidden\n");
    let mut app = app_with(Editor::new(Buffer::from_text("")), dir.0.clone(), one_file());
    app.handle_key(f4(), t(1000));
    type_str(&mut app, "find");
    enter(&mut app);
    assert_eq!(row(&mut app, 0).trim_end(), "  one.txt:1: find me");
    assert_eq!(row(&mut app, 1).trim_end(), "  sub/two.txt:1: and me: find");
    assert_eq!(row(&mut app, 2).trim_end(), "");
}

#[test]
fn question_mark_and_q_are_text_but_f1_still_opens_help() {
    let (_dir, mut app) = project("grep-keys");
    app.handle_key(f4(), t(1000));
    type_str(&mut app, "q?");
    assert!(app.in_grep() && !app.in_help());
    assert_eq!(msg(&mut app), "Find in files: q?_");
    app.handle_key(key(KeyCode::F(1)), t(1000));
    assert!(app.in_help());
    app.handle_key(key(KeyCode::Esc), t(1000));
    assert!(app.in_grep(), "help returns to the view");
}

#[test]
fn f4_does_nothing_inside_a_prompt_and_ctrl_w_is_ignored_in_the_view() {
    let (_dir, mut app) = project("grep-ignored");
    app.handle_key(ctrl('f'), t(1000));
    app.handle_key(f4(), t(1000));
    assert!(!app.in_grep());
    app.handle_key(key(KeyCode::Esc), t(1000));
    app.handle_key(f4(), t(1000));
    app.handle_key(ctrl('w'), t(1000));
    assert!(app.in_grep());
    assert_eq!(app.buffer_count(), 1);
}

#[test]
fn ctrl_q_follows_the_quit_rules_and_warns_inside_the_view() {
    let (_dir, mut app) = project("grep-quit");
    type_str(&mut app, "x");
    app.handle_key(f4(), t(1000));
    assert_eq!(app.handle_key(ctrl('q'), t(1000)), Flow::Continue);
    assert!(msg(&mut app).starts_with("Unsaved changes in 1 buffer"), "{:?}", msg(&mut app));
    assert_eq!(app.handle_key(ctrl('q'), t(1000)), Flow::Quit);
}

#[test]
fn a_bracketed_paste_goes_into_the_query() {
    let (_dir, mut app) = project("grep-paste");
    app.handle_key(f4(), t(1000));
    app.paste("need\nle");
    assert_eq!(msg(&mut app), "Find in files: needle_");
    assert_eq!(app.editor().buffer().text(), "scratch");
}

#[test]
fn every_visit_starts_without_old_results() {
    let (_dir, mut app) = project("grep-fresh");
    app.handle_key(f4(), t(1000));
    type_str(&mut app, "needle");
    enter(&mut app);
    app.handle_key(key(KeyCode::Esc), t(1000));
    app.handle_key(f4(), t(1000));
    assert_eq!(row(&mut app, 0).trim_end(), "");
    assert_eq!(status(&mut app).trim_end(), "Find in files: Enter searches");
}

#[test]
fn the_view_draws_at_every_size() {
    let (_dir, mut app) = project("grep-sizes");
    app.handle_key(f4(), t(1000));
    type_str(&mut app, "needle");
    enter(&mut app);
    for w in 0..=12 {
        for h in 0..=6 {
            let screen = app.screen(w, h, t(1000));
            assert_eq!(screen.rows.len(), h.saturating_sub(2));
        }
    }
}
