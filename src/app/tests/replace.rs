use super::*;

fn msg(app: &mut App) -> String {
    app.screen(80, 24, t(1000)).message
}

fn replace(app: &mut App, term: &str, with: &str) {
    app.handle_key(ctrl('r'), t(1000));
    type_str(app, term);
    app.handle_key(key(KeyCode::Enter), t(1000));
    type_str(app, with);
    app.handle_key(key(KeyCode::Enter), t(1000));
}

#[test]
fn ctrl_r_replaces_and_one_ctrl_z_restores() {
    let mut app = text_app("foo bar\nfoo", FakeGit::new());
    replace(&mut app, "foo", "baz");
    assert_eq!(app.editor().buffer().text(), "baz bar\nbaz");
    assert_eq!(msg(&mut app), "Replaced 2 occurrences");
    app.handle_key(ctrl('z'), t(1000));
    assert_eq!(app.editor().buffer().text(), "foo bar\nfoo");
    assert!(!app.editor().buffer().is_modified());
}

#[test]
fn the_replace_term_becomes_the_shared_search_term() {
    let mut app = text_app("foo bar\nfoo", FakeGit::new());
    replace(&mut app, "foo", "foox");
    app.handle_key(ctrl('f'), t(1000));
    assert_eq!(msg(&mut app), "Find: foo_");
    app.handle_key(key(KeyCode::Esc), t(1000));
    // And a search term is the next replace's prefill.
    app.handle_key(ctrl('r'), t(1000));
    assert_eq!(msg(&mut app), "Replace: foo_");
}

#[test]
fn a_find_term_prefills_replace() {
    let mut app = text_app("foo bar", FakeGit::new());
    app.handle_key(ctrl('f'), t(1000));
    type_str(&mut app, "bar");
    app.handle_key(key(KeyCode::Enter), t(1000));
    app.handle_key(ctrl('r'), t(1000));
    assert_eq!(msg(&mut app), "Replace: bar_");
}

#[test]
fn the_replace_command_opens_the_same_prompt() {
    let mut app = text_app("foo", FakeGit::new());
    app.run_command("replace", t(1000));
    assert!(app.editor().in_prompt());
    assert_eq!(msg(&mut app), "Replace: _");
    // Through the prompt too.
    let mut app = text_app("foo", FakeGit::new());
    app.handle_key(ctrl('x'), t(1000));
    type_str(&mut app, "rep");
    app.handle_key(key(KeyCode::Enter), t(1000));
    assert_eq!(msg(&mut app), "Replace: _");
    type_str(&mut app, "foo");
    app.handle_key(key(KeyCode::Enter), t(1000));
    type_str(&mut app, "bar");
    app.handle_key(key(KeyCode::Enter), t(1000));
    assert_eq!(app.editor().buffer().text(), "bar");
}

#[test]
fn red_and_redo_still_run_redo_and_re_is_ambiguous() {
    let mut app = text_app("abc", FakeGit::new());
    type_str(&mut app, "X");
    app.handle_key(ctrl('z'), t(1000));
    app.run_command("re", t(1000));
    assert!(!app.editor().in_prompt());
    assert_eq!(msg(&mut app), "Ambiguous: redo, replace");
    app.run_command("red", t(1000));
    assert_eq!(app.editor().buffer().text(), "Xabc");
    app.handle_key(ctrl('z'), t(1000));
    app.run_command("redo", t(1000));
    assert_eq!(app.editor().buffer().text(), "Xabc");
}

#[test]
fn ctrl_r_does_nothing_in_the_views_and_help() {
    let mut app = text_app("foo", one_file());
    app.handle_key(ctrl('g'), t(1000));
    app.handle_key(ctrl('r'), t(1000));
    assert!(app.in_git_view());
    app.handle_key(key(KeyCode::Char('q')), t(1000));
    assert!(!app.editor().in_prompt());

    app.handle_key(ctrl('t'), t(1000));
    app.handle_key(ctrl('r'), t(1000));
    assert!(app.in_file_tree());
    app.handle_key(ctrl('t'), t(1000));

    app.handle_key(ctrl('o'), t(1000));
    app.handle_key(ctrl('r'), t(1000));
    assert!(app.in_buffer_list());
    app.handle_key(key(KeyCode::Esc), t(1000));

    app.handle_key(key(KeyCode::F(1)), t(1000));
    app.handle_key(ctrl('r'), t(1000));
    assert!(app.in_help());
    app.handle_key(key(KeyCode::Esc), t(1000));
    assert!(!app.editor().in_prompt());
    assert_eq!(app.editor().buffer().text(), "foo");
}

#[test]
fn ctrl_q_cancels_a_replace_prompt_then_quits() {
    let mut app = text_app("foo", FakeGit::new());
    app.handle_key(ctrl('r'), t(1000));
    type_str(&mut app, "foo");
    app.handle_key(key(KeyCode::Enter), t(1000));
    assert_eq!(app.handle_key(ctrl('q'), t(1000)), Flow::Quit);
    assert!(!app.editor().in_prompt());
    assert_eq!(app.editor().buffer().text(), "foo");
}

#[test]
fn replace_marks_the_buffer_modified_and_quit_warns() {
    let mut app = text_app("foo", FakeGit::new());
    replace(&mut app, "foo", "bar");
    assert_eq!(app.handle_key(ctrl('q'), t(1000)), Flow::Continue);
    assert!(msg(&mut app).contains("Unsaved"), "{}", msg(&mut app));
}

#[test]
fn the_highlight_reaches_the_screen_while_finding() {
    let mut app = text_app("foo bar foo", FakeGit::new());
    app.handle_key(ctrl('f'), t(1000));
    type_str(&mut app, "foo");
    let screen = app.screen(40, 6, t(1000));
    let marked: usize = screen.rows[0].iter().filter(|s| s.style == Style::Match).map(|s| s.text.chars().count()).sum();
    assert_eq!(marked, 6);
    app.handle_key(key(KeyCode::Enter), t(1000));
    let screen = app.screen(40, 6, t(1000));
    assert!(screen.rows.iter().flatten().all(|s| s.style != Style::Match));
}
