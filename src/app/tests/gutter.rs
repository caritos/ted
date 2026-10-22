use super::help::{at, go};
use super::*;

fn f2() -> KeyEvent {
    key(KeyCode::F(2))
}

fn first_row(app: &mut App) -> String {
    row_text(&app.screen(40, 8, t(1000)), 0)
}

fn two_buffers() -> App {
    let mut app = text_app("alpha\nbeta", FakeGit::new());
    app.editors.push(Editor::new(Buffer::from_text("gamma")));
    app
}

#[test]
fn gutter_is_on_by_default_and_f2_toggles_it() {
    let mut app = text_app("alpha\nbeta", FakeGit::new());
    assert_eq!(first_row(&mut app), "  1 alpha");
    app.handle_key(f2(), t(1000));
    assert_eq!(first_row(&mut app), "alpha");
    app.handle_key(f2(), t(1000));
    assert_eq!(first_row(&mut app), "  1 alpha");
}

#[test]
fn f2_does_not_edit_the_buffer() {
    let mut app = text_app("alpha", FakeGit::new());
    app.handle_key(f2(), t(1000));
    assert_eq!(app.editor().buffer().text(), "alpha");
    assert!(!app.editor().buffer().is_modified());
}

#[test]
fn setting_applies_to_every_buffer_and_survives_switching() {
    let mut app = two_buffers();
    assert_eq!(first_row(&mut app), "  1 alpha");
    app.handle_key(f2(), t(1000));
    app.handle_key(ctrl('n'), t(1000));
    assert_eq!(first_row(&mut app), "gamma");
    app.handle_key(f2(), t(1000));
    assert_eq!(first_row(&mut app), "  1 gamma");
    app.handle_key(ctrl('p'), t(1000));
    assert_eq!(first_row(&mut app), "  1 alpha");
    // Toggled off again, then a switch through the buffer list keeps it off.
    app.handle_key(f2(), t(1000));
    app.handle_key(ctrl('o'), t(1000));
    app.handle_key(key(KeyCode::Down), t(1000));
    app.handle_key(key(KeyCode::Enter), t(1000));
    assert_eq!(first_row(&mut app), "gamma");
}

#[test]
fn closing_a_buffer_keeps_the_setting() {
    let mut app = two_buffers();
    app.handle_key(f2(), t(1000));
    app.handle_key(ctrl('w'), t(1000));
    assert_eq!(app.buffer_count(), 1);
    assert_eq!(first_row(&mut app), "gamma");
}

#[test]
fn f2_is_ignored_outside_the_editor() {
    for place in ["git", "tree", "list"] {
        let mut app = text_app("alpha", one_file());
        go(&mut app, place);
        assert!(at(&app, place));
        app.handle_key(f2(), t(1000));
        assert!(at(&app, place), "{place}");
        // Still on after returning.
        app.handle_key(key(KeyCode::Esc), t(1000));
        assert_eq!(first_row(&mut app), "  1 alpha", "{place}");
    }
    let mut app = text_app("alpha", FakeGit::new());
    app.handle_key(key(KeyCode::F(1)), t(1000));
    assert!(app.in_help());
    app.handle_key(f2(), t(1000));
    assert!(app.in_help());
    app.handle_key(key(KeyCode::F(1)), t(1000));
    assert_eq!(first_row(&mut app), "  1 alpha");
}

#[test]
fn f2_is_swallowed_by_an_open_prompt() {
    for open in [ctrl('f'), ctrl('l')] {
        let mut app = text_app("alpha", FakeGit::new());
        app.handle_key(open, t(1000));
        assert!(app.editor().in_prompt());
        app.handle_key(f2(), t(1000));
        assert!(app.editor().in_prompt());
        app.handle_key(key(KeyCode::Esc), t(1000));
        assert_eq!(first_row(&mut app), "  1 alpha");
    }
}

#[test]
fn f2_withdraws_pending_quit_and_close_warnings() {
    let mut app = text_app("alpha", FakeGit::new());
    type_str(&mut app, "x");
    app.handle_key(ctrl('q'), t(1000));
    assert!(app.screen(80, 8, t(1000)).message.contains("Unsaved"));
    app.handle_key(f2(), t(1000));
    assert_eq!(app.screen(80, 8, t(1000)).message, "");
    // The warning is gone: Ctrl-Q warns again instead of quitting.
    assert_eq!(app.handle_key(ctrl('q'), t(1000)), Flow::Continue);

    let mut app = text_app("alpha", FakeGit::new());
    type_str(&mut app, "x");
    app.handle_key(ctrl('w'), t(1000));
    app.handle_key(f2(), t(1000));
    assert_eq!(app.screen(80, 8, t(1000)).message, "");
    app.handle_key(ctrl('w'), t(1000));
    assert_eq!(app.buffer_count(), 1);
    assert!(app.editor().buffer().is_modified(), "close warning was not withdrawn");
}
