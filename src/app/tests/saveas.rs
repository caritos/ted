use super::*;

fn msg(app: &mut App) -> String {
    app.screen(80, 24, t(1000)).message
}

fn run(app: &mut App, text: &str) -> Flow {
    app.handle_key(ctrl('x'), t(1000));
    type_str(app, text);
    app.handle_key(key(KeyCode::Enter), t(1000))
}

fn app_in(dir: &Temp, text: &str) -> App {
    app_with(Editor::new(Buffer::from_text(text)), dir.0.clone(), one_file())
}

#[test]
fn creates_the_file_under_the_location() {
    let dir = Temp::new("saveas");
    let mut app = app_in(&dir, "fn main() {}");
    app.handle_key(key(KeyCode::End), t(1000));
    type_str(&mut app, "x");
    assert!(app.screen(40, 6, t(1000)).status.contains("[+]"));
    run(&mut app, "saveas new.rs");
    assert_eq!(fs::read_to_string(dir.0.join("new.rs")).unwrap(), "fn main() {}x");
    assert!(msg(&mut app).starts_with("Saved 1 lines"), "{:?}", msg(&mut app));
    let screen = app.screen(40, 6, t(1000));
    assert!(screen.status.contains("new.rs"), "{:?}", screen.status);
    assert!(!screen.status.contains("[+]"));
    // The new extension turns highlighting on.
    assert!(screen.rows[0].iter().any(|s| s.style == Style::Keyword), "{:?}", screen.rows[0]);
}

#[test]
fn keeps_the_argument_case_and_spaces() {
    let dir = Temp::new("saveas-case");
    let mut app = app_in(&dir, "x");
    run(&mut app, "SAVEAS My File.TXT");
    assert!(dir.0.join("My File.TXT").exists());
}

#[test]
fn refuses_an_existing_target() {
    let dir = Temp::new("saveas-exists");
    dir.file("old.txt", "keep");
    let mut app = app_in(&dir, "new text");
    run(&mut app, "saveas old.txt");
    assert_eq!(msg(&mut app), "Exists: old.txt");
    assert_eq!(fs::read_to_string(dir.0.join("old.txt")).unwrap(), "keep");
    assert!(app.editor().buffer().path().is_none());
}

#[test]
fn refuses_a_file_open_in_another_buffer() {
    let dir = Temp::new("saveas-open");
    let mut app = multi_app(&dir);
    open_from_tree(&mut app, 1);
    run(&mut app, "saveas ./a.txt");
    assert_eq!(msg(&mut app), "Already open: a.txt");
    assert_eq!(fs::read_to_string(dir.0.join("a.txt")).unwrap(), "alpha\nsecond\n");
}

#[test]
fn without_a_path_shows_usage() {
    let dir = Temp::new("saveas-usage");
    let mut app = app_in(&dir, "x");
    run(&mut app, "saveas");
    assert_eq!(msg(&mut app), "Usage: saveas <path>");
    run(&mut app, "saveas   ");
    assert_eq!(msg(&mut app), "Usage: saveas <path>");
}

#[test]
fn into_a_missing_directory_fails_and_changes_nothing() {
    let dir = Temp::new("saveas-missing");
    let mut app = app_in(&dir, "x");
    type_str(&mut app, "Y");
    run(&mut app, "saveas nodir/a.txt");
    assert!(msg(&mut app).starts_with("Save failed:"), "{:?}", msg(&mut app));
    assert!(app.editor().buffer().path().is_none());
    assert!(app.editor().buffer().is_modified());
    assert!(!dir.0.join("nodir").exists());
}

#[test]
fn a_directory_is_refused() {
    let dir = Temp::new("saveas-dir");
    dir.file("sub/x.txt", "");
    let mut app = app_in(&dir, "x");
    run(&mut app, "saveas sub");
    assert_eq!(msg(&mut app), "Exists: sub");
    assert!(app.editor().buffer().path().is_none());
}

#[test]
fn dotdot_resolves_against_the_location() {
    let dir = Temp::new("saveas-dotdot");
    dir.file("sub/x.txt", "");
    let mut app = app_in(&dir, "x");
    run(&mut app, "saveas sub/../up.txt");
    assert!(dir.0.join("up.txt").exists());
}

#[test]
fn hostile_name_is_drawn_sanitized() {
    let dir = Temp::new("saveas-hostile");
    let mut app = app_in(&dir, "x");
    run(&mut app, "saveas a\u{1b}[2Jb.txt");
    let screen = app.screen(60, 6, t(1000));
    assert!(!screen.status.contains('\u{1b}'), "{:?}", screen.status);
    assert!(!screen.message.contains('\u{1b}'), "{:?}", screen.message);
    assert!(screen.status.contains("a?[2Jb.txt"), "{:?}", screen.status);
}

#[test]
fn refuses_a_missing_file_another_buffer_is_named_after() {
    let dir = Temp::new("saveas-missing-open");
    dir.file("b.txt", "beta\n");
    let notes = dir.0.join("notes.txt");
    let mut buffer = Buffer::open(&notes).unwrap();
    buffer.insert_char('x');
    let mut app = app_with(Editor::new(buffer), dir.0.clone(), one_file());
    open_from_tree(&mut app, 0);
    assert_eq!(app.buffer_count(), 2);
    for typed in ["saveas notes.txt", "saveas ./notes.txt", "saveas sub/../notes.txt"] {
        run(&mut app, typed);
        assert_eq!(msg(&mut app), "Already open: notes.txt", "{typed}");
    }
    assert!(!notes.exists());
}

#[test]
fn a_buffer_opened_by_a_relative_missing_path_is_found() {
    let dir = Temp::new("saveas-relative-open");
    dir.file("b.txt", "beta\n");
    // Relative to the process directory; the file does not exist.
    let name = format!("ted-rel-{}-missing.txt", std::process::id());
    let mut buffer = Buffer::open(Path::new(&name)).unwrap();
    buffer.insert_char('x');
    let cwd = std::env::current_dir().unwrap();
    let mut app = app_with(Editor::new(buffer), cwd, one_file());
    app.handle_key(ctrl('t'), t(1000));
    app.handle_key(ch('q'), t(1000));
    // A second, unnamed buffer cannot be made without the tree, so name the
    // other side through `open`-style reuse: saveas from this buffer onto
    // `./name` is its own file.
    run(&mut app, &format!("saveas ./{name}"));
    assert_eq!(msg(&mut app), format!("Exists: {name}"));
    assert!(!Path::new(&name).exists());
}

#[test]
fn saveas_onto_the_buffers_own_file_says_exists() {
    let dir = Temp::new("saveas-own");
    let mut app = multi_app(&dir);
    run(&mut app, "saveas a.txt");
    assert_eq!(msg(&mut app), "Exists: a.txt");
    assert_eq!(fs::read_to_string(dir.0.join("a.txt")).unwrap(), "alpha\nsecond\n");
}

#[test]
fn goto_with_an_overflowing_number_goes_to_the_last_line() {
    let mut app = text_app("one\ntwo\nthree", FakeGit::new());
    run(&mut app, "goto 99999999999999999999999999");
    assert_eq!(app.editor().buffer().cursor(), (2, 0));
    run(&mut app, "goto -1");
    assert_eq!(msg(&mut app), "Usage: goto <line>");
}
