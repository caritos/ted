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

#[test]
fn undo_and_redo_commands_work() {
    let mut app = text_app("abc", FakeGit::new());
    type_str(&mut app, "XY");
    run(&mut app, "undo");
    assert_eq!(app.editor().buffer().text(), "abc");
    run(&mut app, "redo");
    assert_eq!(app.editor().buffer().text(), "XYabc");
}

#[test]
fn unique_prefixes_run_undo_and_redo() {
    let mut app = text_app("abc", FakeGit::new());
    type_str(&mut app, "XY");
    run(&mut app, "u");
    assert_eq!(app.editor().buffer().text(), "abc");
    run(&mut app, "re");
    assert_eq!(app.editor().buffer().text(), "XYabc");
    run(&mut app, "UNDO");
    assert_eq!(app.editor().buffer().text(), "abc");
}

#[test]
fn undo_command_with_an_argument_is_a_usage_error() {
    let mut app = text_app("abc", FakeGit::new());
    type_str(&mut app, "X");
    run(&mut app, "undo now");
    assert_eq!(msg(&mut app), "Usage: undo");
    assert_eq!(app.editor().buffer().text(), "Xabc");
}

#[test]
fn undo_and_redo_commands_with_nothing_to_do_show_a_message() {
    let mut app = text_app("abc", FakeGit::new());
    run(&mut app, "undo");
    assert_eq!(msg(&mut app), "Nothing to undo");
    run(&mut app, "redo");
    assert_eq!(msg(&mut app), "Nothing to redo");
}

#[test]
fn ctrl_z_and_ctrl_y_are_ignored_outside_the_editor() {
    let mut app = text_app("abc", one_file());
    type_str(&mut app, "X");
    for open in [ctrl('g'), ctrl('t'), ctrl('o'), key(KeyCode::F(1))] {
        app.handle_key(open, t(1000));
        app.handle_key(ctrl('z'), t(1000));
        app.handle_key(ctrl('y'), t(1000));
        // Still in the same view, nothing undone.
        assert!(app.in_help() || app.in_git_view() || app.in_file_tree() || app.in_buffer_list());
        app.handle_key(ch('q'), t(1000));
        assert!(!app.in_help() && !app.in_git_view() && !app.in_file_tree() && !app.in_buffer_list());
        assert_eq!(app.editor().buffer().text(), "Xabc");
        assert!(app.editor().buffer().is_modified());
    }
    // The redo side too: undo in the editor, then Ctrl-Y in a view keeps it undone.
    app.handle_key(ctrl('z'), t(1000));
    assert_eq!(app.editor().buffer().text(), "abc");
    for open in [ctrl('g'), ctrl('t'), ctrl('o'), key(KeyCode::F(1))] {
        app.handle_key(open, t(1000));
        app.handle_key(ctrl('y'), t(1000));
        app.handle_key(ch('q'), t(1000));
        assert_eq!(app.editor().buffer().text(), "abc");
    }
}

#[test]
fn save_undo_warns_on_quit_then_redo_quits_at_once() {
    let dir = Temp::new("undo-quit");
    dir.file("a.txt", "abc\n");
    let buffer = Buffer::open(&dir.0.join("a.txt")).unwrap();
    let mut app = app_with(Editor::new(buffer), dir.0.clone(), one_file());
    type_str(&mut app, "X");
    app.handle_key(ctrl('s'), t(1000));
    assert!(!app.editor().buffer().is_modified());
    app.handle_key(ctrl('z'), t(1000));
    assert_eq!(app.handle_key(ctrl('q'), t(1000)), Flow::Continue);
    assert!(msg(&mut app).contains("Unsaved changes"), "{:?}", msg(&mut app));
    app.handle_key(ctrl('y'), t(1000));
    assert_eq!(app.handle_key(ctrl('q'), t(1000)), Flow::Quit);
}

#[test]
fn reopening_a_saved_buffer_with_the_same_text_keeps_history_and_cursor() {
    let dir = Temp::new("undo-reopen-same");
    dir.file("a.txt", "abc\n");
    dir.file("b.txt", "beta\n");
    let buffer = Buffer::open(&dir.0.join("a.txt")).unwrap();
    let mut app = app_with(Editor::new(buffer), dir.0.clone(), one_file());
    type_str(&mut app, "X");
    app.handle_key(ctrl('s'), t(1000));
    let cursor = app.editor().buffer().cursor();
    open_from_tree(&mut app, 0);
    assert_eq!(app.buffer_count(), 1);
    assert_eq!(app.editor().buffer().cursor(), cursor);
    app.handle_key(ctrl('z'), t(1000));
    assert_eq!(app.editor().buffer().text(), "abc\n");
}

#[test]
fn reopening_an_unmodified_buffer_whose_file_changed_reloads_and_drops_history() {
    let dir = Temp::new("undo-reopen-changed");
    dir.file("a.txt", "abc\n");
    dir.file("b.txt", "beta\n");
    let buffer = Buffer::open(&dir.0.join("a.txt")).unwrap();
    let mut app = app_with(Editor::new(buffer), dir.0.clone(), one_file());
    type_str(&mut app, "X");
    app.handle_key(ctrl('s'), t(1000));
    dir.file("a.txt", "changed\n");
    open_from_tree(&mut app, 0);
    assert_eq!(app.editor().buffer().text(), "changed\n");
    app.handle_key(ctrl('z'), t(1000));
    assert_eq!(msg(&mut app), "Nothing to undo");
    assert_eq!(app.editor().buffer().text(), "changed\n");
}

#[test]
fn opening_a_file_keeps_an_unnamed_buffer_that_can_still_redo() {
    let dir = Temp::new("undo-unnamed-redo");
    dir.file("a.txt", "abc\n");
    let mut app = app_with(Editor::new(Buffer::new()), dir.0.clone(), one_file());
    type_str(&mut app, "a paragraph");
    app.handle_key(ctrl('z'), t(1000));
    assert_eq!(app.editor().buffer().text(), "");
    open_from_tree(&mut app, 0);
    assert_eq!(app.buffer_count(), 2);
    app.handle_key(ctrl('p'), t(1000));
    app.handle_key(ctrl('y'), t(1000));
    assert_eq!(app.editor().buffer().text(), "a paragraph");
}
