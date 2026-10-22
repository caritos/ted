use super::*;

fn msg(app: &mut App) -> String {
    app.screen(80, 24, t(1000)).message
}

fn editor_app() -> App {
    text_app("alpha\nbeta ? gamma\ndelta", FakeGit::new())
}

#[test]
fn global_keys_go_into_an_open_prompt() {
    for c in ['g', 't', 'o', 'n', 'p', 'w'] {
        let mut app = editor_app();
        app.handle_key(ctrl('f'), t(1000));
        app.handle_key(ctrl(c), t(1000));
        assert!(app.editor().in_prompt(), "ctrl-{c} closed the prompt");
        assert!(!app.in_git_view() && !app.in_file_tree() && !app.in_buffer_list(), "ctrl-{c}");
        assert_eq!(app.buffer_count(), 1);
    }
    let mut app = editor_app();
    app.handle_key(ctrl('f'), t(1000));
    app.handle_key(key(KeyCode::F(1)), t(1000));
    assert!(!app.in_help() && app.editor().in_prompt());
    app.handle_key(ch('?'), t(1000));
    assert!(!app.in_help());
    assert!(msg(&mut app).contains("Find: ?"), "{:?}", msg(&mut app));
    // The typed `?` finds the one in the text.
    assert_eq!(app.editor().buffer().cursor(), (1, 5));
}

#[test]
fn ctrl_w_in_a_prompt_does_not_arm_a_close() {
    let mut app = editor_app();
    type_str(&mut app, "x");
    app.handle_key(ctrl('f'), t(1000));
    app.handle_key(ctrl('w'), t(1000));
    assert!(!msg(&mut app).contains("Unsaved"));
    assert_eq!(app.buffer_count(), 1);
}

#[test]
fn ctrl_q_cancels_the_prompt_then_quits_or_warns() {
    let mut app = editor_app();
    app.handle_key(ctrl('f'), t(1000));
    assert_eq!(app.handle_key(ctrl('q'), t(1000)), Flow::Quit);

    let mut app = editor_app();
    type_str(&mut app, "X");
    app.handle_key(ctrl('f'), t(1000));
    assert_eq!(app.handle_key(ctrl('q'), t(1000)), Flow::Continue);
    assert!(!app.editor().in_prompt());
    assert!(msg(&mut app).contains("Unsaved changes in 1 buffer"));
    assert_eq!(app.handle_key(ctrl('q'), t(1000)), Flow::Quit);
}

#[test]
fn any_other_key_in_a_prompt_withdraws_a_pending_quit_warning() {
    let mut app = editor_app();
    type_str(&mut app, "X");
    app.handle_key(ctrl('q'), t(1000));
    assert!(msg(&mut app).contains("Unsaved changes"));
    app.handle_key(ctrl('f'), t(1000));
    app.handle_key(ch('a'), t(1000));
    assert!(!msg(&mut app).contains("Unsaved"));
    // The warning is gone, so Ctrl-Q warns again instead of quitting.
    assert_eq!(app.handle_key(ctrl('q'), t(1000)), Flow::Continue);
    assert!(!app.editor().in_prompt());
    assert!(msg(&mut app).contains("Unsaved changes"));
}

#[test]
fn find_and_goto_do_nothing_outside_the_editor() {
    for place in ["git", "tree", "list"] {
        for k in [ctrl('f'), ctrl('l'), key(KeyCode::F(3))] {
            let mut app = editor_app();
            super::help::go(&mut app, place);
            app.handle_key(k, t(1000));
            assert!(super::help::at(&app, place), "{place} {k:?}");
            assert!(!app.editor().in_prompt(), "{place} {k:?}");
        }
    }
    for k in [ctrl('f'), ctrl('l'), key(KeyCode::F(3))] {
        let mut app = editor_app();
        app.handle_key(key(KeyCode::F(1)), t(1000));
        app.handle_key(k, t(1000));
        assert!(app.in_help(), "{k:?}");
        app.handle_key(ch('q'), t(1000));
        assert!(!app.editor().in_prompt(), "{k:?}");
    }
}

#[test]
fn f3_works_from_the_editor() {
    let mut app = text_app("ab\nab\nab", FakeGit::new());
    app.handle_key(ctrl('f'), t(1000));
    type_str(&mut app, "ab");
    app.handle_key(key(KeyCode::Enter), t(1000));
    let first = app.editor().buffer().cursor();
    app.handle_key(key(KeyCode::F(3)), t(1000));
    assert_ne!(app.editor().buffer().cursor(), first);
}

#[test]
fn ctrl_f_and_ctrl_l_open_prompts_in_the_editor() {
    let mut app = editor_app();
    app.handle_key(ctrl('l'), t(1000));
    assert!(msg(&mut app).starts_with("Go to line:"));
    app.handle_key(key(KeyCode::Esc), t(1000));
    app.handle_key(ctrl('f'), t(1000));
    assert!(msg(&mut app).starts_with("Find:"));
}
