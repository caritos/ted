use super::*;

fn run(app: &mut App, command: &str) {
    app.handle_key(ctrl('x'), t(1000));
    type_str(app, command);
    app.handle_key(key(KeyCode::Enter), t(1000));
}

#[test]
fn the_line_commands_work_from_the_command_prompt() {
    let mut app = text_app("a\nb\nc\n", one_file());
    run(&mut app, "duplicate");
    assert_eq!(app.editor().buffer().text(), "a\na\nb\nc\n");
    run(&mut app, "movedown");
    assert_eq!(app.editor().buffer().text(), "a\nb\na\nc\n");
    run(&mut app, "moveup");
    assert_eq!(app.editor().buffer().text(), "a\na\nb\nc\n");
    run(&mut app, "deleteline");
    assert_eq!(app.editor().buffer().text(), "a\nb\nc\n");
}

#[test]
fn ctrl_d_and_alt_keys_do_nothing_in_the_views() {
    let mut app = text_app("a\nb\n", one_file());
    app.handle_key(ctrl('t'), t(1000));
    app.handle_key(ctrl('d'), t(1000));
    app.handle_key(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::ALT), t(1000));
    app.handle_key(ctrl('t'), t(1000));
    assert_eq!(app.editor().buffer().text(), "a\nb\n");
}

#[test]
fn a_line_command_is_undone_with_one_ctrl_z() {
    let mut app = text_app("a\nb\n", one_file());
    run(&mut app, "duplicate");
    app.handle_key(ctrl('z'), t(1000));
    assert_eq!(app.editor().buffer().text(), "a\nb\n");
}
