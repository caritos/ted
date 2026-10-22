use super::*;

fn msg(app: &mut App) -> String {
    app.screen(80, 24, t(1000)).message
}

/// Opens the command prompt, types `text` and presses Enter.
fn run(app: &mut App, text: &str) -> Flow {
    app.handle_key(ctrl('x'), t(1000));
    type_str(app, text);
    app.handle_key(key(KeyCode::Enter), t(1000))
}

fn editor_app() -> App {
    text_app("one\ntwo\nthree\nfour", FakeGit::new())
}

#[test]
fn goto_moves_the_cursor() {
    let mut app = editor_app();
    assert_eq!(run(&mut app, "goto 3"), Flow::Continue);
    assert_eq!(app.editor().buffer().cursor(), (2, 0));
    assert!(!app.editor().in_prompt());
}

#[test]
fn lines_toggles_and_sets() {
    let mut app = editor_app();
    let gutter = |app: &mut App| app.screen(40, 6, t(1000)).rows[0].iter().any(|s| s.style == Style::LineNo);
    assert!(gutter(&mut app));
    run(&mut app, "lines");
    assert!(!gutter(&mut app));
    run(&mut app, "lines");
    assert!(gutter(&mut app));
    run(&mut app, "lines off");
    assert!(!gutter(&mut app));
    run(&mut app, "lines off");
    assert!(!gutter(&mut app));
    run(&mut app, "lines on");
    assert!(gutter(&mut app));
}

#[test]
fn next_and_prev_cycle() {
    let dir = Temp::new("cmd-cycle");
    let mut app = multi_app(&dir);
    open_from_tree(&mut app, 1);
    assert_eq!(app.current_index(), 1);
    run(&mut app, "prev");
    assert_eq!(app.current_index(), 0);
    run(&mut app, "next");
    assert_eq!(app.current_index(), 1);
    run(&mut app, "next");
    assert_eq!(app.current_index(), 0);
}

#[test]
fn view_commands_switch_mode() {
    let mut app = text_app("a", one_file());
    run(&mut app, "buffers");
    assert!(app.in_buffer_list());
    app.handle_key(ch('q'), t(1000));
    run(&mut app, "tree");
    assert!(app.in_file_tree());
    app.handle_key(ch('q'), t(1000));
    run(&mut app, "git");
    assert!(app.in_git_view());
    app.handle_key(ch('q'), t(1000));
    run(&mut app, "help");
    assert!(app.in_help());
    app.handle_key(ch('q'), t(1000));
    assert!(!app.in_help());
}

#[test]
fn close_a_clean_buffer() {
    let dir = Temp::new("cmd-close-clean");
    let mut app = multi_app(&dir);
    open_from_tree(&mut app, 1);
    assert_eq!(app.buffer_count(), 2);
    run(&mut app, "close");
    assert_eq!(app.buffer_count(), 1);
}

#[test]
fn close_a_modified_buffer_warns_then_ctrl_w_closes() {
    let mut app = editor_app();
    type_str(&mut app, "X");
    run(&mut app, "close");
    assert!(app.editor().buffer().is_modified());
    assert!(msg(&mut app).contains("Unsaved changes! Press Ctrl-W again"), "{:?}", msg(&mut app));
    app.handle_key(ctrl('w'), t(1000));
    assert!(!app.editor().buffer().is_modified());
    assert!(app.editor().buffer().text().is_empty());
}

#[test]
fn another_key_withdraws_the_close_warning() {
    let mut app = editor_app();
    type_str(&mut app, "X");
    run(&mut app, "close");
    app.handle_key(key(KeyCode::Right), t(1000));
    assert!(!msg(&mut app).contains("Unsaved"));
    app.handle_key(ctrl('w'), t(1000));
    assert!(msg(&mut app).contains("Unsaved changes!"));
    assert!(app.editor().buffer().is_modified());
}

#[test]
fn quit_clean_quits() {
    let mut app = editor_app();
    assert_eq!(run(&mut app, "quit"), Flow::Quit);
    let mut app = editor_app();
    assert_eq!(run(&mut app, "q"), Flow::Quit);
}

#[test]
fn quit_warns_about_a_modified_buffer_in_another_tab() {
    let dir = Temp::new("cmd-quit");
    let mut app = multi_app(&dir);
    type_str(&mut app, "X");
    open_from_tree(&mut app, 1);
    assert_eq!(app.current_index(), 1);
    assert_eq!(run(&mut app, "quit"), Flow::Continue);
    assert!(msg(&mut app).contains("Unsaved changes in 1 buffer"), "{:?}", msg(&mut app));
    assert_eq!(app.handle_key(ctrl('q'), t(1000)), Flow::Quit);
}

#[test]
fn another_key_withdraws_the_quit_warning() {
    let mut app = editor_app();
    type_str(&mut app, "X");
    assert_eq!(run(&mut app, "quit"), Flow::Continue);
    app.handle_key(key(KeyCode::Right), t(1000));
    assert!(!msg(&mut app).contains("Unsaved"));
    assert_eq!(app.handle_key(ctrl('q'), t(1000)), Flow::Continue);
}

#[test]
fn save_saves() {
    let dir = Temp::new("cmd-save");
    let mut app = multi_app(&dir);
    type_str(&mut app, "X");
    run(&mut app, "save");
    assert!(msg(&mut app).starts_with("Saved 3 lines"), "{:?}", msg(&mut app));
    assert!(!app.editor().buffer().is_modified());
    assert_eq!(fs::read_to_string(dir.0.join("a.txt")).unwrap(), "Xalpha\nsecond\n");
}

#[test]
fn wq_saves_and_quits() {
    let dir = Temp::new("cmd-wq");
    let mut app = multi_app(&dir);
    type_str(&mut app, "X");
    assert_eq!(run(&mut app, "wq"), Flow::Quit);
    assert_eq!(fs::read_to_string(dir.0.join("a.txt")).unwrap(), "Xalpha\nsecond\n");
}

#[test]
fn wq_on_an_unnamed_buffer_does_not_quit() {
    let mut app = editor_app();
    type_str(&mut app, "X");
    assert_eq!(run(&mut app, "wq"), Flow::Continue);
    assert!(msg(&mut app).contains("No file name"), "{:?}", msg(&mut app));
}

#[test]
fn errors_show_in_the_message_row() {
    let mut app = editor_app();
    run(&mut app, "foo");
    assert_eq!(msg(&mut app), "Unknown command: foo");
    run(&mut app, "goto x");
    assert_eq!(msg(&mut app), "Usage: goto <line>");
    run(&mut app, "");
    assert_eq!(msg(&mut app), "");
}

#[test]
fn esc_leaves_everything_unchanged() {
    let mut app = editor_app();
    app.handle_key(key(KeyCode::Down), t(1000));
    app.handle_key(ctrl('x'), t(1000));
    type_str(&mut app, "goto 4");
    app.handle_key(key(KeyCode::Esc), t(1000));
    assert!(!app.editor().in_prompt());
    assert_eq!(app.editor().buffer().cursor(), (1, 0));
    assert_eq!(msg(&mut app), "");
}

#[test]
fn ctrl_x_is_ignored_outside_the_editor() {
    let mut app = text_app("a", one_file());
    for open in [ctrl('g'), ctrl('t'), ctrl('o'), key(KeyCode::F(1))] {
        app.handle_key(open, t(1000));
        app.handle_key(ctrl('x'), t(1000));
        app.handle_key(ch('q'), t(1000));
        // Back in the editor with no prompt open and no text typed.
        assert!(!app.editor().in_prompt());
        assert_eq!(app.editor().buffer().text(), "a");
    }
    assert!(!app.in_help() && !app.in_git_view() && !app.in_file_tree() && !app.in_buffer_list());
}

#[test]
fn ctrl_q_with_the_prompt_open_cancels_it_and_follows_the_quit_rules() {
    let mut app = editor_app();
    app.handle_key(ctrl('x'), t(1000));
    assert_eq!(app.handle_key(ctrl('q'), t(1000)), Flow::Quit);

    let mut app = editor_app();
    type_str(&mut app, "X");
    app.handle_key(ctrl('x'), t(1000));
    assert_eq!(app.handle_key(ctrl('q'), t(1000)), Flow::Continue);
    assert!(!app.editor().in_prompt());
    assert!(msg(&mut app).contains("Unsaved changes in 1 buffer"));
    assert_eq!(app.handle_key(ctrl('q'), t(1000)), Flow::Quit);
}

#[test]
fn tiny_sizes_with_the_prompt_open() {
    for w in 0..=6 {
        for h in 0..=6 {
            let mut app = editor_app();
            app.handle_key(ctrl('x'), t(1000));
            type_str(&mut app, "goto 2");
            let _ = app.screen(w, h, t(1000));
            app.handle_key(key(KeyCode::Enter), t(1000));
            let _ = app.screen(w, h, t(1000));
            run(&mut app, "foo");
            let _ = app.screen(w, h, t(1000));
        }
    }
}
