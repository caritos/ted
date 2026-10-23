use super::*;

fn f1() -> KeyEvent {
    key(KeyCode::F(1))
}

fn status_of(app: &mut App) -> String {
    app.screen(80, 24, t(1000)).status
}

fn is_help(app: &mut App) -> bool {
    status_of(app).starts_with(" Help")
}

/// Puts `app` in the place named: "editor", "git", "tree" or "list".
pub(super) fn go(app: &mut App, place: &str) {
    match place {
        "git" => {
            app.handle_key(ctrl('g'), t(1000));
        }
        "tree" => {
            app.handle_key(ctrl('t'), t(1000));
        }
        "list" => {
            app.handle_key(ctrl('o'), t(1000));
        }
        _ => {}
    }
}

pub(super) fn at(app: &App, place: &str) -> bool {
    match place {
        "git" => app.in_git_view(),
        "tree" => app.in_file_tree(),
        "list" => app.in_buffer_list(),
        _ => !app.in_git_view() && !app.in_file_tree() && !app.in_buffer_list() && !app.in_help(),
    }
}

#[test]
fn f1_opens_help_from_every_place_and_each_close_key_returns_there() {
    for place in ["editor", "git", "tree", "list"] {
        for close in [ch('q'), key(KeyCode::Esc), f1()] {
            let mut app = text_app("one", one_file());
            go(&mut app, place);
            assert!(at(&app, place), "{place}");
            app.handle_key(f1(), t(1000));
            assert!(app.in_help(), "{place}");
            assert!(is_help(&mut app), "{place}");
            assert_eq!(app.screen(80, 24, t(1000)).cursor, None);
            app.handle_key(close, t(1000));
            assert!(!app.in_help(), "{place} {close:?}");
            assert!(at(&app, place), "back at {place} via {close:?}");
        }
    }
}

#[test]
fn question_mark_opens_help_from_views_but_types_in_the_editor() {
    for place in ["git", "tree", "list"] {
        let mut app = text_app("one", one_file());
        go(&mut app, place);
        app.handle_key(ch('?'), t(1000));
        assert!(app.in_help(), "{place}");
        app.handle_key(ch('?'), t(1000));
        assert!(at(&app, place), "{place}");
    }
    let mut app = text_app("", one_file());
    app.handle_key(ch('?'), t(1000));
    assert!(!app.in_help());
    assert_eq!(app.editor().buffer().text(), "?");
}

#[test]
fn f1_does_not_change_the_buffer_and_scroll_restarts_each_visit() {
    let mut app = text_app("one", one_file());
    app.handle_key(f1(), t(1000));
    for _ in 0..5 {
        app.handle_key(key(KeyCode::Down), t(1000));
    }
    let scrolled = app.screen(80, 8, t(1000)).rows;
    app.handle_key(f1(), t(1000));
    assert_eq!(app.editor().buffer().text(), "one");
    app.handle_key(f1(), t(1000));
    assert_ne!(app.screen(80, 8, t(1000)).rows, scrolled);
}

#[test]
fn ctrl_q_in_help_follows_quit_rules() {
    let mut app = text_app("one", one_file());
    app.handle_key(f1(), t(1000));
    assert_eq!(app.handle_key(ctrl('q'), t(1000)), Flow::Quit);

    let mut app = text_app("", one_file());
    type_str(&mut app, "x");
    app.handle_key(f1(), t(1000));
    assert_eq!(app.handle_key(ctrl('q'), t(1000)), Flow::Continue);
    assert!(app.in_help());
    assert!(app.screen(80, 24, t(1000)).message.contains("Unsaved changes in 1 buffer!"));
    // Any other key withdraws the warning.
    app.handle_key(key(KeyCode::Down), t(1000));
    assert_eq!(app.screen(80, 24, t(1000)).message, "");
    app.handle_key(ctrl('q'), t(1000));
    assert_eq!(app.handle_key(ctrl('q'), t(1000)), Flow::Quit);
}

#[test]
fn ctrl_n_leaves_help_for_the_next_buffer() {
    let dir = temp_dir("help-next");
    fs::write(dir.join("b.txt"), "bee").unwrap();
    let mut app = app_with(Editor::new(Buffer::from_text("a")), dir.clone(), one_file());
    app.open("b.txt");
    assert_eq!(app.current_index(), 1);
    app.handle_key(f1(), t(1000));
    app.handle_key(ctrl('n'), t(1000));
    assert!(at(&app, "editor"));
    assert_eq!(app.current_index(), 0);
    fs::remove_dir_all(dir).ok();
}

#[test]
fn ctrl_g_t_o_leave_help_and_ctrl_w_is_ignored() {
    let mut app = text_app("one", one_file());
    app.handle_key(f1(), t(1000));
    app.handle_key(ctrl('w'), t(1000));
    assert!(app.in_help());
    assert_eq!(app.buffer_count(), 1);
    app.handle_key(ctrl('g'), t(1000));
    assert!(app.in_git_view());
    app.handle_key(f1(), t(1000));
    app.handle_key(ctrl('t'), t(1000));
    assert!(app.in_file_tree());
    app.handle_key(f1(), t(1000));
    app.handle_key(ctrl('o'), t(1000));
    assert!(app.in_buffer_list());
    // Help opened from the list and left through Ctrl-G does not come back to the list.
    app.handle_key(f1(), t(1000));
    app.handle_key(ctrl('g'), t(1000));
    assert!(app.in_git_view());
    app.handle_key(ch('q'), t(1000));
    assert!(at(&app, "editor"));
}

#[test]
fn pending_close_warning_is_cancelled_by_f1() {
    let mut app = text_app("", one_file());
    type_str(&mut app, "x");
    app.handle_key(ctrl('w'), t(1000));
    assert!(app.editor().message().contains("Press Ctrl-W again"));
    app.handle_key(f1(), t(1000));
    assert_eq!(app.editor().message(), "");
    app.handle_key(f1(), t(1000));
    app.handle_key(ctrl('w'), t(1000));
    assert_eq!(app.buffer_count(), 1);
    assert!(app.editor().message().contains("Press Ctrl-W again"));
}

#[test]
fn hint_shows_initially_and_goes_after_one_key() {
    let mut app = text_app("one", one_file());
    assert_eq!(app.screen(80, 24, t(1000)).message, "F1: help");
    app.handle_key(key(KeyCode::Right), t(1000));
    assert_eq!(app.screen(80, 24, t(1000)).message, "");
}

#[test]
fn hint_goes_on_the_first_key_of_any_kind() {
    for first in [ctrl('g'), ctrl('t'), ctrl('o'), ctrl('n'), ctrl('s')] {
        let mut app = text_app("one", one_file());
        app.handle_key(first, t(1000));
        app.handle_key(ch('q'), t(1000));
        let message = app.screen(80, 24, t(1000)).message;
        assert!(!message.contains("F1: help"), "{first:?}: {message:?}");
    }
}

#[test]
fn help_from_the_commit_picker_returns_to_the_open_picker() {
    let mut git = one_file();
    git.log = vec![Commit {
        hash: "c1".to_string(),
        short: "c1".to_string(),
        subject: "first".to_string(),
        unix_time: 900,
        refs: Vec::new(),
    }];
    let mut app = text_app("one", git);
    app.handle_key(ctrl('g'), t(1000));
    app.handle_key(ch('b'), t(1000));
    assert!(app.git_view.as_ref().unwrap().picker_open());
    app.handle_key(ch('?'), t(1000));
    assert!(app.in_help());
    app.handle_key(ch('q'), t(1000));
    assert!(app.in_git_view());
    assert!(app.git_view.as_ref().unwrap().picker_open());
}

#[test]
fn help_lists_its_own_keys_and_printable_insert() {
    let mut app = text_app("one", one_file());
    app.handle_key(f1(), t(1000));
    let all: String = (0..70).map(|i| format!("{}\n", row_text_or_empty(&app.screen(80, 80, t(1000)), i))).collect();
    assert!(all.contains("Printable keys"), "{all}");
    assert!(all.contains("Help"), "{all}");
}

fn row_text_or_empty(screen: &Screen, i: usize) -> String {
    screen.rows.get(i).map(|r| r.iter().map(|s| s.text.as_str()).collect()).unwrap_or_default()
}

#[test]
fn help_tick_and_tiny_sizes_do_not_panic() {
    let mut app = text_app("one", one_file());
    app.handle_key(f1(), t(1000));
    app.tick(t(1001));
    for w in 0..=6 {
        for h in 0..=6 {
            let s = app.screen(w, h, t(1000));
            assert!(s.status.chars().count() <= w && s.message.chars().count() <= w);
        }
    }
}
