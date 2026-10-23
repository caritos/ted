use super::*;

fn shift(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::SHIFT)
}

fn two_buffers() -> App {
    let mut app = text_app("alpha\nbeta", FakeGit::new());
    app.editors.push(Editor::new(Buffer::from_text("gamma")));
    app
}

fn selected_cells(app: &mut App) -> usize {
    let s = app.screen(40, 8, t(1000));
    s.rows.iter().flatten().filter(|sp| sp.style == Style::Selected).map(|sp| sp.text.chars().count()).sum()
}

#[test]
fn shift_right_and_ctrl_a_select_through_the_app() {
    let mut app = text_app("alpha", FakeGit::new());
    app.handle_key(shift(KeyCode::Right), t(1000));
    app.handle_key(shift(KeyCode::Right), t(1000));
    assert_eq!(selected_cells(&mut app), 2);
    app.handle_key(ctrl('a'), t(1000));
    assert_eq!(selected_cells(&mut app), 5);
}

#[test]
fn switching_buffers_clears_the_selection_of_both() {
    let mut app = two_buffers();
    app.handle_key(ctrl('a'), t(1000));
    app.editors[1].handle_key(ctrl('a'));
    app.handle_key(ctrl('n'), t(1000));
    assert!(app.editors[0].buffer().selection().is_none());
    assert!(app.editors[1].buffer().selection().is_none());
    app.editors[0].handle_key(ctrl('a'));
    app.handle_key(ctrl('p'), t(1000));
    assert!(app.editors[0].buffer().selection().is_none());
}

#[test]
fn the_buffer_list_switch_clears_the_selection() {
    let mut app = two_buffers();
    app.handle_key(ctrl('a'), t(1000));
    app.editors[1].handle_key(ctrl('a'));
    app.handle_key(ctrl('o'), t(1000));
    app.handle_key(key(KeyCode::Down), t(1000));
    app.handle_key(key(KeyCode::Enter), t(1000));
    assert_eq!(app.current_index(), 1);
    assert!(app.editors[0].buffer().selection().is_none());
    assert!(app.editors[1].buffer().selection().is_none());
}

#[test]
fn closing_a_buffer_leaves_no_selection_in_the_one_shown() {
    let mut app = two_buffers();
    app.editors[0].handle_key(ctrl('a'));
    app.handle_key(ctrl('n'), t(1000));
    app.editors[0].handle_key(ctrl('a'));
    app.handle_key(ctrl('w'), t(1000));
    assert!(app.editor().buffer().selection().is_none());
}

#[test]
fn shifted_keys_do_nothing_new_in_the_views_and_help() {
    for enter in [ctrl('g'), ctrl('t'), ctrl('o'), key(KeyCode::F(1))] {
        let mut app = two_buffers();
        app.handle_key(enter, t(1000));
        for code in [KeyCode::Left, KeyCode::Right, KeyCode::Home, KeyCode::End, KeyCode::PageUp, KeyCode::PageDown] {
            app.handle_key(shift(code), t(1000));
        }
        app.handle_key(ctrl('a'), t(1000));
        assert!(app.editors.iter().all(|e| e.buffer().selection().is_none()));
        assert!(!app.editors.iter().any(|e| e.buffer().is_modified()));
    }
}
