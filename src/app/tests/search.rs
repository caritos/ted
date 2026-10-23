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

// ---- One search term for all buffers ----

fn shared_app(dir: &Temp) -> App {
    dir.file("a.txt", "one\nfoo a\nthree\n");
    dir.file("b.txt", "zero\nfoo b\nfoo c\n");
    let buffer = Buffer::open(&dir.0.join("a.txt")).unwrap();
    app_with(Editor::new(buffer), dir.0.clone(), one_file())
}

fn accept(app: &mut App, term: &str) {
    app.handle_key(ctrl('f'), t(1000));
    // Clear a prefilled term first.
    for _ in 0..10 {
        app.handle_key(key(KeyCode::Backspace), t(1000));
    }
    type_str(app, term);
    app.handle_key(key(KeyCode::Enter), t(1000));
}

/// a.txt and b.txt open, term `foo` accepted in a.txt, b.txt current.
fn two_buffers(dir: &Temp) -> App {
    let mut app = shared_app(dir);
    accept(&mut app, "foo");
    open_from_tree(&mut app, 1);
    app
}

#[test]
fn term_accepted_in_one_buffer_prefills_in_the_next() {
    let dir = Temp::new("shared-ctrl-n");
    let mut app = two_buffers(&dir);
    app.handle_key(ctrl('p'), t(1000));
    app.handle_key(ctrl('n'), t(1000));
    assert_eq!(app.current_index(), 1);
    app.handle_key(ctrl('f'), t(1000));
    assert_eq!(msg(&mut app), "Find: foo_");
    app.handle_key(key(KeyCode::Enter), t(1000));
    assert_eq!(app.editor().buffer().cursor(), (1, 0));
}

#[test]
fn f3_in_another_buffer_uses_the_shared_term_from_its_own_cursor() {
    let dir = Temp::new("shared-f3");
    let mut app = two_buffers(&dir);
    app.handle_key(key(KeyCode::F(3)), t(1000));
    assert_eq!(app.editor().buffer().cursor(), (1, 0));
    app.handle_key(key(KeyCode::F(3)), t(1000));
    assert_eq!(app.editor().buffer().cursor(), (2, 0));
}

#[test]
fn shared_term_after_switching_with_the_buffer_list() {
    let dir = Temp::new("shared-list");
    let mut app = two_buffers(&dir);
    app.handle_key(ctrl('o'), t(1000));
    app.handle_key(key(KeyCode::Home), t(1000));
    app.handle_key(key(KeyCode::Enter), t(1000));
    assert_eq!(app.current_index(), 0);
    accept(&mut app, "three");
    app.handle_key(ctrl('o'), t(1000));
    app.handle_key(key(KeyCode::End), t(1000));
    app.handle_key(key(KeyCode::Enter), t(1000));
    assert_eq!(app.current_index(), 1);
    app.handle_key(ctrl('f'), t(1000));
    assert!(msg(&mut app).starts_with("Find: three"));
}

#[test]
fn second_file_opened_from_the_tree_gets_the_term() {
    let dir = Temp::new("shared-open");
    let mut app = two_buffers(&dir);
    assert_eq!(app.current_index(), 1);
    app.handle_key(ctrl('f'), t(1000));
    assert_eq!(msg(&mut app), "Find: foo_");
}

#[test]
fn closing_the_buffer_that_had_the_term_keeps_it() {
    let dir = Temp::new("shared-close");
    let mut app = two_buffers(&dir);
    app.handle_key(ctrl('p'), t(1000));
    app.handle_key(ctrl('w'), t(1000));
    assert_eq!(app.buffer_count(), 1);
    app.handle_key(ctrl('f'), t(1000));
    assert_eq!(msg(&mut app), "Find: foo_");
}

#[test]
fn reloading_an_unmodified_buffer_from_the_tree_keeps_the_term() {
    let dir = Temp::new("shared-reload");
    let mut app = two_buffers(&dir);
    open_from_tree(&mut app, 0);
    assert_eq!(app.current_index(), 0);
    app.handle_key(ctrl('f'), t(1000));
    assert_eq!(msg(&mut app), "Find: foo_");
    app.handle_key(key(KeyCode::Esc), t(1000));
    open_from_tree(&mut app, 1);
    app.handle_key(ctrl('f'), t(1000));
    assert_eq!(msg(&mut app), "Find: foo_");
}

#[test]
fn esc_keeps_the_previous_term() {
    let dir = Temp::new("shared-esc");
    let mut app = two_buffers(&dir);
    app.handle_key(ctrl('f'), t(1000));
    app.handle_key(key(KeyCode::Backspace), t(1000));
    type_str(&mut app, "x");
    app.handle_key(key(KeyCode::Esc), t(1000));
    app.handle_key(ctrl('p'), t(1000));
    app.handle_key(ctrl('f'), t(1000));
    assert_eq!(msg(&mut app), "Find: foo_");
}

#[test]
fn a_new_term_in_b_prefills_in_a() {
    let dir = Temp::new("shared-new");
    let mut app = two_buffers(&dir);
    accept(&mut app, "zero");
    app.handle_key(ctrl('p'), t(1000));
    app.handle_key(ctrl('f'), t(1000));
    assert!(msg(&mut app).starts_with("Find: zero"));
}

#[test]
fn no_term_anywhere_says_no_previous_search_in_every_buffer() {
    let dir = Temp::new("shared-none");
    let mut app = shared_app(&dir);
    open_from_tree(&mut app, 1);
    for _ in 0..2 {
        app.handle_key(key(KeyCode::F(3)), t(1000));
        assert_eq!(msg(&mut app), "No previous search");
        app.handle_key(ctrl('n'), t(1000));
    }
}

#[test]
fn views_do_not_change_the_term() {
    let dir = Temp::new("shared-views");
    let mut app = shared_app(&dir);
    accept(&mut app, "foo");
    app.handle_key(ctrl('g'), t(1000));
    type_str(&mut app, "zzz");
    app.handle_key(key(KeyCode::Esc), t(1000));
    app.handle_key(ctrl('t'), t(1000));
    type_str(&mut app, "zzz");
    app.handle_key(key(KeyCode::Esc), t(1000));
    app.handle_key(key(KeyCode::F(1)), t(1000));
    type_str(&mut app, "zzz");
    app.handle_key(key(KeyCode::Esc), t(1000));
    open_from_tree(&mut app, 1);
    app.handle_key(ctrl('f'), t(1000));
    assert_eq!(msg(&mut app), "Find: foo_");
}
