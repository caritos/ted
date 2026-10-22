use super::*;
use crate::git::fake::FakeGit;
use crate::git::{Baseline, Change, ChangeKind, Commit, GitError};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::cell::RefCell;
use std::fs;
use std::rc::Rc;
use std::time::{Duration, UNIX_EPOCH};

/// A `FakeGit` the test can still change after handing it to the app.
#[derive(Clone)]
struct Shared(Rc<RefCell<FakeGit>>);

impl Shared {
    fn new(git: FakeGit) -> Self {
        Shared(Rc::new(RefCell::new(git)))
    }
}

impl Git for Shared {
    fn head(&self) -> Result<Baseline, GitError> {
        self.0.borrow().head()
    }
    fn baseline_exists(&self, base: &Baseline) -> bool {
        self.0.borrow().baseline_exists(base)
    }
    fn changes(&self, base: &Baseline) -> Result<Vec<Change>, GitError> {
        self.0.borrow().changes(base)
    }
    fn diff(&self, base: &Baseline, change: &Change) -> Result<String, GitError> {
        self.0.borrow().diff(base, change)
    }
    fn log(&self, limit: usize) -> Result<Vec<Commit>, GitError> {
        self.0.borrow().log(limit)
    }
    fn modified(&self, path: &str) -> Option<SystemTime> {
        self.0.borrow().modified(path)
    }
}

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn ch(c: char) -> KeyEvent {
    key(KeyCode::Char(c))
}

fn ctrl(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
}

fn t(secs: u64) -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(secs)
}

fn type_str(app: &mut App, text: &str) {
    for c in text.chars() {
        assert_eq!(app.handle_key(ch(c), t(1000)), Flow::Continue);
    }
}

fn temp_dir(name: &str) -> PathBuf {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "ted-app-test-{}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed),
        name
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn row_text(screen: &Screen, i: usize) -> String {
    screen.rows[i].iter().map(|s| s.text.as_str()).collect()
}

/// A repo with one modified file, `a.txt`.
fn one_file() -> FakeGit {
    let mut git = FakeGit::new();
    git.add(ChangeKind::Modified, "a.txt", Some(t(900)));
    git.diffs.insert("a.txt".to_string(), "+hello\n".to_string());
    git
}

fn app_with(editor: Editor, location: PathBuf, git: FakeGit) -> App {
    App::new(editor, location, Box::new(git))
}

fn text_app(text: &str, git: FakeGit) -> App {
    app_with(Editor::new(Buffer::from_text(text)), PathBuf::from("/nowhere"), git)
}

#[test]
fn ctrl_g_switches_to_git_view_and_q_returns_with_editor_state_intact() {
    let mut app = text_app("one\ntwo", one_file());
    app.handle_key(key(KeyCode::Down), t(1000));
    app.handle_key(key(KeyCode::Right), t(1000));
    type_str(&mut app, "X");
    assert!(!app.in_git_view());

    assert_eq!(app.handle_key(ctrl('g'), t(1000)), Flow::Continue);
    assert!(app.in_git_view());
    let screen = app.screen(60, 6, t(1000));
    assert_eq!(screen.cursor, None);
    assert!(row_text(&screen, 0).starts_with("M a.txt"), "{}", row_text(&screen, 0));
    // Ctrl-G did not reach the editor.
    assert_eq!(app.editor().buffer().text(), "one\ntXwo");

    assert_eq!(app.handle_key(ch('q'), t(1000)), Flow::Continue);
    assert!(!app.in_git_view());
    assert_eq!(app.editor().buffer().text(), "one\ntXwo");
    assert_eq!(app.editor().buffer().cursor(), (1, 2));
    assert!(app.editor().buffer().is_modified());

    // Esc also returns, and a second Ctrl-G re-enters.
    app.handle_key(ctrl('g'), t(1001));
    assert!(app.in_git_view());
    app.handle_key(key(KeyCode::Esc), t(1001));
    assert!(!app.in_git_view());
}

#[test]
fn ctrl_g_in_git_view_returns_to_editor_with_state_intact() {
    let mut app = text_app("one\ntwo", one_file());
    type_str(&mut app, "X");
    app.handle_key(ctrl('g'), t(1000));
    assert!(app.in_git_view());

    assert_eq!(app.handle_key(ctrl('g'), t(1000)), Flow::Continue);
    assert!(!app.in_git_view());
    assert_eq!(app.editor().buffer().text(), "Xone\ntwo");
    assert!(app.editor().buffer().is_modified());

    // And it toggles back in again.
    app.handle_key(ctrl('g'), t(1001));
    assert!(app.in_git_view());
}

#[test]
fn second_ctrl_g_refreshes_the_git_view() {
    let git = Shared::new(one_file());
    let mut app = App::new(Editor::new(Buffer::new()), PathBuf::from("/x"), Box::new(git.clone()));
    app.handle_key(ctrl('g'), t(1000));
    app.handle_key(ch('q'), t(1000));
    git.0.borrow_mut().add(ChangeKind::Added, "new.txt", Some(t(950)));
    app.handle_key(ctrl('g'), t(1001));
    let screen = app.screen(60, 6, t(1001));
    assert!(row_text(&screen, 0).starts_with("A new.txt"), "{}", row_text(&screen, 0));
}

#[test]
fn typing_in_editor_is_unaffected_before_and_after_git_view() {
    let mut app = text_app("", one_file());
    type_str(&mut app, "ab");
    app.handle_key(ctrl('g'), t(1000));
    // Keys in the git view never reach the editor.
    app.handle_key(ch('x'), t(1000));
    app.handle_key(key(KeyCode::Down), t(1000));
    app.handle_key(ch('q'), t(1000));
    type_str(&mut app, "cd");
    assert_eq!(app.editor().buffer().text(), "abcd");
}

#[test]
fn enter_loads_selected_file_into_editor() {
    let dir = temp_dir("open");
    fs::write(dir.join("a.txt"), "from disk\n").unwrap();
    let mut app = app_with(Editor::new(Buffer::new()), dir.clone(), one_file());
    app.handle_key(ctrl('g'), t(1000));
    assert_eq!(app.handle_key(key(KeyCode::Enter), t(1000)), Flow::Continue);
    assert!(!app.in_git_view());
    assert_eq!(app.editor().buffer().text(), "from disk\n");
    assert_eq!(app.editor().buffer().path(), Some(dir.join("a.txt").as_path()));
    assert!(!app.editor().buffer().is_modified());
}

#[test]
fn enter_on_unreadable_file_shows_error_and_stays_in_git_view() {
    let dir = temp_dir("unreadable");
    fs::write(dir.join("a.txt"), [0xff, 0xfe, 0x00]).unwrap();
    let mut app = app_with(Editor::new(Buffer::from_text("keep")), dir, one_file());
    app.handle_key(ctrl('g'), t(1000));
    app.handle_key(key(KeyCode::Enter), t(1000));
    assert!(app.in_git_view());
    let message = app.screen(80, 6, t(1000)).message;
    assert!(message.starts_with("Open failed: "), "{message}");
    assert_eq!(app.editor().buffer().text(), "keep");
}

#[test]
fn enter_on_file_missing_from_disk_refuses_and_creates_nothing() {
    let dir = temp_dir("missing");
    let mut app = app_with(Editor::new(Buffer::new()), dir.clone(), one_file());
    app.handle_key(ctrl('g'), t(1000));
    app.handle_key(key(KeyCode::Enter), t(1000));
    assert!(app.in_git_view());
    assert_eq!(app.screen(60, 6, t(1000)).message, "File no longer exists");
    assert_eq!(app.editor().buffer().text(), "");
    assert_eq!(app.editor().buffer().path(), None);
    assert!(!dir.join("a.txt").exists());
}

#[cfg(unix)]
#[test]
fn enter_on_dangling_symlink_refuses() {
    let dir = temp_dir("dangling");
    std::os::unix::fs::symlink(dir.join("target.txt"), dir.join("a.txt")).unwrap();
    let mut app = app_with(Editor::new(Buffer::new()), dir.clone(), one_file());
    app.handle_key(ctrl('g'), t(1000));
    app.handle_key(key(KeyCode::Enter), t(1000));
    assert!(app.in_git_view());
    assert_eq!(app.screen(60, 6, t(1000)).message, "File no longer exists");
    assert_eq!(app.editor().buffer().path(), None);
    assert!(!dir.join("target.txt").exists());
}

#[test]
fn quit_warning_armed_in_editor_is_cancelled_by_ctrl_g() {
    let mut app = text_app("", one_file());
    type_str(&mut app, "x");
    assert_eq!(app.handle_key(ctrl('q'), t(1000)), Flow::Continue);
    app.handle_key(ctrl('g'), t(1000));
    assert_eq!(app.handle_key(ctrl('q'), t(1000)), Flow::Continue);
    let message = app.screen(80, 6, t(1000)).message;
    assert!(message.contains("Unsaved"), "{message}");
    assert_eq!(app.handle_key(ctrl('q'), t(1000)), Flow::Quit);
}

#[test]
fn ctrl_q_in_git_view_follows_editor_quit_rules() {
    // Clean buffer: one press quits.
    let mut app = text_app("clean", one_file());
    app.handle_key(ctrl('g'), t(1000));
    assert_eq!(app.handle_key(ctrl('q'), t(1000)), Flow::Quit);

    // Modified buffer: the first press warns in the git view, the second quits.
    let mut app = text_app("", one_file());
    type_str(&mut app, "x");
    app.handle_key(ctrl('g'), t(1000));
    assert_eq!(app.handle_key(ctrl('q'), t(1000)), Flow::Continue);
    assert!(app.in_git_view());
    let message = app.screen(80, 6, t(1000)).message;
    assert!(message.contains("Unsaved"), "{message}");
    assert_eq!(app.handle_key(ctrl('q'), t(1000)), Flow::Quit);

    // Any other key in between cancels the warning, as in the editor.
    let mut app = text_app("", one_file());
    type_str(&mut app, "x");
    app.handle_key(ctrl('g'), t(1000));
    app.handle_key(ctrl('q'), t(1000));
    app.handle_key(key(KeyCode::Down), t(1000));
    assert_eq!(app.handle_key(ctrl('q'), t(1000)), Flow::Continue);

    // Leaving the git view cancels it too.
    let mut app = text_app("", one_file());
    type_str(&mut app, "x");
    app.handle_key(ctrl('g'), t(1000));
    app.handle_key(ctrl('q'), t(1000));
    app.handle_key(ch('q'), t(1000));
    assert!(!app.in_git_view());
    assert_eq!(app.handle_key(ctrl('q'), t(1000)), Flow::Continue);
}

#[test]
fn not_a_repo_location_shows_error_in_git_view_and_editor_still_works() {
    let mut git = FakeGit::new();
    git.fail = Some(GitError::NotARepo);
    let mut app = text_app("", git);
    type_str(&mut app, "a");
    app.handle_key(ctrl('g'), t(1000));
    assert!(app.in_git_view());
    let screen = app.screen(60, 6, t(1000));
    assert_eq!(row_text(&screen, 0), "not a git repository");
    app.handle_key(key(KeyCode::Enter), t(1000));
    assert!(app.in_git_view());

    app.handle_key(ch('q'), t(1000));
    assert!(!app.in_git_view());
    type_str(&mut app, "b");
    assert_eq!(app.editor().buffer().text(), "ab");
    let screen = app.screen(20, 4, t(1000));
    assert_eq!(row_text(&screen, 0), "ab");
}

#[test]
fn tick_refreshes_only_in_git_view() {
    let git = Shared::new(one_file());
    let mut app = App::new(Editor::new(Buffer::new()), PathBuf::from("/x"), Box::new(git.clone()));

    // Before the git view was ever opened, tick does nothing.
    app.tick(t(1000));
    assert!(git.0.borrow().diff_calls.borrow().is_empty());

    app.handle_key(ctrl('g'), t(1000));
    git.0.borrow_mut().add(ChangeKind::Added, "new.txt", Some(t(950)));
    app.tick(t(1001));
    let screen = app.screen(60, 6, t(1001));
    assert!(row_text(&screen, 0).starts_with("A new.txt"), "{}", row_text(&screen, 0));

    // Back in the editor, ticks do not call git.
    app.handle_key(ch('q'), t(1001));
    let calls = git.0.borrow().diff_calls.borrow().len();
    git.0.borrow_mut().add(ChangeKind::Added, "newer.txt", Some(t(990)));
    app.tick(t(1002));
    assert_eq!(git.0.borrow().diff_calls.borrow().len(), calls);
}

#[test]
fn screen_of_editor_matches_editor_view_text() {
    let mut app = text_app("one\ntwo", one_file());
    app.handle_key(key(KeyCode::Down), t(1000));
    let screen = app.screen(20, 6, t(1000));

    let mut editor = Editor::new(Buffer::from_text("one\ntwo"));
    editor.handle_key(key(KeyCode::Down));
    let view = editor.view(20, 6);

    let rows: Vec<String> = (0..screen.rows.len()).map(|i| row_text(&screen, i)).collect();
    assert_eq!(rows, view.lines);
    assert!(screen.rows.iter().all(|r| r.len() == 1 && r[0].style == Style::Normal));
    assert_eq!(screen.status, view.status);
    assert_eq!(screen.message, view.message);
    assert_eq!(screen.cursor, Some(view.cursor));
}

#[test]
fn screen_never_panics_for_tiny_sizes() {
    let mut app = text_app("abc\ndef", one_file());
    for in_git in [false, true] {
        if in_git {
            app.handle_key(ctrl('g'), t(1000));
        }
        assert_eq!(app.in_git_view(), in_git);
        for w in 0..=5 {
            for h in 0..=5 {
                let screen = app.screen(w, h, t(1000));
                assert!(screen.rows.len() <= h);
                assert!(screen.rows.iter().all(|r| r.iter().map(|s| s.text.chars().count()).sum::<usize>() <= w));
            }
        }
    }
}

// ---- File tree mode ----

/// A unique temp directory for a tree root, removed on drop.
struct Temp(PathBuf);

impl Temp {
    fn new(name: &str) -> Self {
        Temp(temp_dir(name))
    }

    fn file(&self, rel: &str, text: &str) {
        let p = self.0.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, text).unwrap();
    }
}

impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn tree_app(dir: &Temp, text: &str) -> App {
    app_with(Editor::new(Buffer::from_text(text)), dir.0.clone(), one_file())
}

fn rows(screen: &Screen) -> Vec<String> {
    (0..screen.rows.len()).map(|i| row_text(screen, i)).collect()
}

fn selected_row(screen: &Screen) -> Option<String> {
    screen
        .rows
        .iter()
        .find(|r| r.iter().any(|s| s.style == Style::Selected))
        .map(|r| r.iter().map(|s| s.text.as_str()).collect::<String>().trim_end().to_string())
}

#[test]
fn ctrl_t_opens_tree_and_ctrl_t_or_q_or_esc_returns_with_editor_state_intact() {
    let dir = Temp::new("tree-toggle");
    dir.file("a.txt", "x");
    let mut app = tree_app(&dir, "one\ntwo");
    app.handle_key(key(KeyCode::Down), t(1000));
    app.handle_key(key(KeyCode::Right), t(1000));
    type_str(&mut app, "X");
    assert!(!app.in_file_tree());

    for back in [ctrl('t'), ch('q'), key(KeyCode::Esc)] {
        assert_eq!(app.handle_key(ctrl('t'), t(1000)), Flow::Continue);
        assert!(app.in_file_tree());
        assert!(!app.in_git_view());
        let screen = app.screen(60, 6, t(1000));
        assert_eq!(screen.cursor, None);
        assert_eq!(selected_row(&screen).as_deref(), Some("  a.txt"));
        assert!(screen.status.starts_with("Files: "), "{}", screen.status);
        // Ctrl-T did not reach the editor.
        assert_eq!(app.editor().buffer().text(), "one\ntXwo");

        assert_eq!(app.handle_key(back, t(1000)), Flow::Continue);
        assert!(!app.in_file_tree());
        assert!(!app.in_git_view());
        assert_eq!(app.editor().buffer().text(), "one\ntXwo");
        assert_eq!(app.editor().buffer().cursor(), (1, 2));
        assert!(app.editor().buffer().is_modified());
    }
    // Typing still goes to the editor afterwards.
    type_str(&mut app, "Y");
    assert_eq!(app.editor().buffer().text(), "one\ntXYwo");
}

#[test]
fn ctrl_g_in_tree_switches_to_git_view_and_ctrl_t_in_git_view_switches_to_tree() {
    let dir = Temp::new("tree-git");
    dir.file("a.txt", "x");
    let git = Shared::new(one_file());
    let mut app = App::new(Editor::new(Buffer::new()), dir.0.clone(), Box::new(git.clone()));

    app.handle_key(ctrl('t'), t(1000));
    assert!(app.in_file_tree());
    // Ctrl-G from the tree enters (and refreshes) the git view.
    git.0.borrow_mut().add(ChangeKind::Added, "new.txt", Some(t(950)));
    assert_eq!(app.handle_key(ctrl('g'), t(1001)), Flow::Continue);
    assert!(app.in_git_view());
    assert!(!app.in_file_tree());
    let screen = app.screen(60, 6, t(1001));
    assert!(row_text(&screen, 0).starts_with("A new.txt"), "{}", row_text(&screen, 0));

    // Ctrl-T from the git view enters the tree.
    assert_eq!(app.handle_key(ctrl('t'), t(1001)), Flow::Continue);
    assert!(app.in_file_tree());
    assert!(!app.in_git_view());
    assert!(app.screen(60, 6, t(1001)).status.starts_with("Files: "));

    // Ctrl-G inside the git view still returns to the editor.
    app.handle_key(ctrl('g'), t(1002));
    assert!(app.in_git_view());
    app.handle_key(ctrl('g'), t(1002));
    assert!(!app.in_git_view());
    assert!(!app.in_file_tree());
    assert_eq!(app.editor().buffer().text(), "");
}

#[test]
fn ctrl_t_from_git_view_leaves_no_stale_message_or_open_picker() {
    let dir = Temp::new("tree-git-stale");
    dir.file("a.txt", "x");
    let mut git = one_file();
    git.log = vec![Commit {
        hash: "c1".to_string(),
        short: "c1".to_string(),
        subject: "first".to_string(),
        unix_time: 900,
        refs: Vec::new(),
    }];

    // An armed quit warning does not survive git -> tree -> git.
    let mut app = app_with(Editor::new(Buffer::new()), dir.0.clone(), git);
    type_str(&mut app, "x");
    app.handle_key(ctrl('g'), t(1000));
    app.handle_key(ctrl('q'), t(1000));
    assert!(app.screen(80, 6, t(1000)).message.contains("Unsaved"));
    app.handle_key(ctrl('t'), t(1000));
    assert!(app.in_file_tree());
    app.handle_key(ctrl('g'), t(1000));
    assert!(app.in_git_view());
    assert_eq!(app.screen(80, 6, t(1000)).message, "");

    // A refusal message does not survive it either.
    fs::remove_file(dir.0.join("a.txt")).unwrap();
    app.handle_key(key(KeyCode::Enter), t(1000));
    assert_eq!(app.screen(80, 6, t(1000)).message, "File no longer exists");
    app.handle_key(ctrl('t'), t(1000));
    app.handle_key(ctrl('g'), t(1000));
    assert_eq!(app.screen(80, 6, t(1000)).message, "");

    // An open picker is closed on return.
    app.handle_key(ch('b'), t(1000));
    assert!(app.git_view.as_ref().unwrap().picker_open());
    app.handle_key(ctrl('t'), t(1000));
    assert!(app.in_file_tree());
    app.handle_key(ctrl('g'), t(1000));
    assert!(app.in_git_view());
    assert!(!app.git_view.as_ref().unwrap().picker_open());
}

#[test]
fn tree_keeps_expansion_and_selection_between_visits() {
    let dir = Temp::new("tree-keep");
    dir.file("sub/inner.txt", "inner\n");
    dir.file("z.txt", "z");
    let mut app = tree_app(&dir, "");
    app.handle_key(ctrl('t'), t(1000));
    app.handle_key(key(KeyCode::Right), t(1000)); // expand sub
    app.handle_key(key(KeyCode::Down), t(1000)); // select sub/inner.txt
    app.handle_key(ch('q'), t(1000));
    assert!(!app.in_file_tree());

    app.handle_key(ctrl('t'), t(1001));
    let screen = app.screen(60, 6, t(1001));
    assert_eq!(rows(&screen)[0], "v sub/");
    assert_eq!(selected_row(&screen).as_deref(), Some("    inner.txt"));
    app.handle_key(key(KeyCode::Enter), t(1001));
    assert!(!app.in_file_tree());
    assert_eq!(app.editor().buffer().path(), Some(dir.0.join("sub/inner.txt").as_path()));
}

#[test]
fn enter_on_file_in_tree_loads_it_and_returns_to_editor() {
    let dir = Temp::new("tree-open");
    dir.file("a.txt", "from disk\n");
    let mut app = tree_app(&dir, "");
    app.handle_key(ctrl('t'), t(1000));
    assert_eq!(app.handle_key(key(KeyCode::Enter), t(1000)), Flow::Continue);
    assert!(!app.in_file_tree());
    assert!(!app.in_git_view());
    assert_eq!(app.editor().buffer().text(), "from disk\n");
    assert_eq!(app.editor().buffer().path(), Some(dir.0.join("a.txt").as_path()));
    assert!(!app.editor().buffer().is_modified());
}

#[test]
fn enter_on_file_that_vanished_shows_file_no_longer_exists_in_tree() {
    let dir = Temp::new("tree-vanished");
    dir.file("a.txt", "x");
    let mut app = tree_app(&dir, "");
    app.handle_key(ctrl('t'), t(1000));
    fs::remove_file(dir.0.join("a.txt")).unwrap();
    app.handle_key(key(KeyCode::Enter), t(1000));
    assert!(app.in_file_tree());
    assert_eq!(app.screen(60, 6, t(1000)).message, "File no longer exists");
    assert_eq!(app.editor().buffer().path(), None);
    assert!(!dir.0.join("a.txt").exists());
}

#[test]
fn enter_on_unreadable_file_in_tree_shows_open_failed() {
    let dir = Temp::new("tree-unreadable");
    fs::write(dir.0.join("a.txt"), [0xff, 0xfe, 0x00]).unwrap();
    let mut app = tree_app(&dir, "keep");
    app.handle_key(ctrl('t'), t(1000));
    app.handle_key(key(KeyCode::Enter), t(1000));
    assert!(app.in_file_tree());
    let message = app.screen(80, 6, t(1000)).message;
    assert!(message.starts_with("Open failed: "), "{message}");
    assert_eq!(app.editor().buffer().text(), "keep");
}

#[test]
fn ctrl_q_in_tree_follows_editor_quit_rules() {
    let dir = Temp::new("tree-quit");
    dir.file("a.txt", "x");

    // Clean buffer: one press quits.
    let mut app = tree_app(&dir, "clean");
    app.handle_key(ctrl('t'), t(1000));
    assert_eq!(app.handle_key(ctrl('q'), t(1000)), Flow::Quit);

    // Modified buffer: the first press warns in the tree, the second quits.
    let mut app = tree_app(&dir, "");
    type_str(&mut app, "x");
    app.handle_key(ctrl('t'), t(1000));
    assert_eq!(app.handle_key(ctrl('q'), t(1000)), Flow::Continue);
    assert!(app.in_file_tree());
    let message = app.screen(80, 6, t(1000)).message;
    assert!(message.contains("Unsaved"), "{message}");
    assert_eq!(app.handle_key(ctrl('q'), t(1000)), Flow::Quit);

    // Any other key in between cancels the warning, as in the editor.
    let mut app = tree_app(&dir, "");
    type_str(&mut app, "x");
    app.handle_key(ctrl('t'), t(1000));
    app.handle_key(ctrl('q'), t(1000));
    app.handle_key(key(KeyCode::Down), t(1000));
    assert_eq!(app.handle_key(ctrl('q'), t(1000)), Flow::Continue);

    // Leaving the tree cancels it too.
    let mut app = tree_app(&dir, "");
    type_str(&mut app, "x");
    app.handle_key(ctrl('t'), t(1000));
    app.handle_key(ctrl('q'), t(1000));
    app.handle_key(ch('q'), t(1000));
    assert!(!app.in_file_tree());
    assert_eq!(app.handle_key(ctrl('q'), t(1000)), Flow::Continue);

    // Switching to the git view cancels it as well.
    let mut app = tree_app(&dir, "");
    type_str(&mut app, "x");
    app.handle_key(ctrl('t'), t(1000));
    app.handle_key(ctrl('q'), t(1000));
    app.handle_key(ctrl('g'), t(1000));
    assert!(app.in_git_view());
    assert_eq!(app.handle_key(ctrl('q'), t(1000)), Flow::Continue);
}

#[test]
fn entering_tree_cancels_a_pending_quit_warning() {
    let dir = Temp::new("tree-cancel");
    dir.file("a.txt", "x");

    // Armed in the editor.
    let mut app = tree_app(&dir, "");
    type_str(&mut app, "x");
    assert_eq!(app.handle_key(ctrl('q'), t(1000)), Flow::Continue);
    app.handle_key(ctrl('t'), t(1000));
    assert_eq!(app.handle_key(ctrl('q'), t(1000)), Flow::Continue);
    let message = app.screen(80, 6, t(1000)).message;
    assert!(message.contains("Unsaved"), "{message}");
    assert_eq!(app.handle_key(ctrl('q'), t(1000)), Flow::Quit);

    // Armed in the git view, then Ctrl-T to the tree.
    let mut app = tree_app(&dir, "");
    type_str(&mut app, "x");
    app.handle_key(ctrl('g'), t(1000));
    app.handle_key(ctrl('q'), t(1000));
    app.handle_key(ctrl('t'), t(1000));
    assert!(app.in_file_tree());
    assert_eq!(app.handle_key(ctrl('q'), t(1000)), Flow::Continue);
}

#[test]
fn tick_refreshes_tree_so_new_files_appear() {
    let dir = Temp::new("tree-tick");
    dir.file("a.txt", "x");
    let git = Shared::new(one_file());
    let mut app = App::new(Editor::new(Buffer::new()), dir.0.clone(), Box::new(git.clone()));
    app.handle_key(ctrl('t'), t(1000));
    dir.file("b.txt", "y");
    assert!(!rows(&app.screen(60, 6, t(1000))).iter().any(|r| r.contains("b.txt")));
    app.tick(t(1001));
    assert!(rows(&app.screen(60, 6, t(1001))).iter().any(|r| r.contains("b.txt")));
    // The tree's tick does not poll git.
    assert!(git.0.borrow().diff_calls.borrow().is_empty());
}

#[test]
fn tick_does_nothing_in_editor_mode() {
    let dir = Temp::new("tree-tick-editor");
    dir.file("a.txt", "x");
    let mut app = tree_app(&dir, "");
    // Before the tree was ever opened, tick does not create it.
    app.tick(t(1000));
    assert!(app.file_tree.is_none());

    app.handle_key(ctrl('t'), t(1000));
    app.handle_key(ch('q'), t(1000));
    dir.file("b.txt", "y");
    app.tick(t(1001));
    let tree = app.file_tree.as_ref().unwrap();
    assert!(!tree.visible().iter().any(|r| r.name == "b.txt"));
    assert!(app.screen(20, 4, t(1001)).cursor.is_some());
}

#[test]
fn screen_in_tree_mode_never_panics_for_tiny_sizes() {
    let dir = Temp::new("tree-tiny");
    dir.file("sub/inner.txt", "x");
    dir.file("a long file name.txt", "x");
    let mut app = tree_app(&dir, "abc");
    app.handle_key(ctrl('t'), t(1000));
    app.handle_key(key(KeyCode::Right), t(1000));
    assert!(app.in_file_tree());
    for w in 0..=5 {
        for h in 0..=5 {
            let screen = app.screen(w, h, t(1000));
            assert!(screen.rows.len() <= h);
            assert!(screen.rows.iter().all(|r| r.iter().map(|s| s.text.chars().count()).sum::<usize>() <= w));
            assert!(screen.status.chars().count() <= w);
            assert!(screen.message.chars().count() <= w);
        }
    }
}

/// Opens the tree on `dir` (whose only entry is the one to open) with an
/// editor holding "keep", presses Enter, and returns (still in the tree,
/// message, editor text, editor has no path). Runs on a thread so a hang
/// fails the test instead of blocking it.
fn enter_in_tree_with_timeout(dir: PathBuf) -> (bool, String, String, bool) {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut app = app_with(Editor::new(Buffer::from_text("keep")), dir, one_file());
        app.handle_key(ctrl('t'), t(1000));
        app.handle_key(key(KeyCode::Enter), t(1000));
        let message = app.screen(80, 6, t(1000)).message;
        let _ = tx.send((
            app.in_file_tree(),
            message,
            app.editor().buffer().text(),
            app.editor().buffer().path().is_none(),
        ));
    });
    rx.recv_timeout(Duration::from_secs(5)).expect("Enter in the tree hung")
}

#[cfg(unix)]
#[test]
fn enter_on_socket_in_tree_is_refused_as_not_a_regular_file() {
    let dir = Temp::new("sock");
    let _listener = std::os::unix::net::UnixListener::bind(dir.0.join("s")).unwrap();
    let (in_tree, message, text, no_path) = enter_in_tree_with_timeout(dir.0.clone());
    assert!(in_tree);
    assert_eq!(message, "Not a regular file");
    assert_eq!(text, "keep");
    assert!(no_path);
}

#[cfg(unix)]
#[test]
fn enter_on_fifo_in_tree_is_refused_without_hanging() {
    let dir = Temp::new("fifo");
    let made = std::process::Command::new("mkfifo").arg(dir.0.join("f")).status();
    if !made.is_ok_and(|s| s.success()) {
        eprintln!("mkfifo unavailable; skipping");
        return;
    }
    let (in_tree, message, text, no_path) = enter_in_tree_with_timeout(dir.0.clone());
    assert!(in_tree);
    assert_eq!(message, "Not a regular file");
    assert_eq!(text, "keep");
    assert!(no_path);
}

#[test]
fn enter_on_file_over_64_mib_in_tree_is_refused_as_too_large() {
    let dir = Temp::new("big");
    let file = fs::File::create(dir.0.join("big.bin")).unwrap();
    // Sparse: cheap to create, no data written.
    file.set_len(64 * 1024 * 1024 + 1).unwrap();
    drop(file);
    let (in_tree, message, text, no_path) = enter_in_tree_with_timeout(dir.0.clone());
    assert!(in_tree);
    assert_eq!(message, "File too large");
    assert_eq!(text, "keep");
    assert!(no_path);
}

#[test]
fn app_groups_equal_styles_into_spans_and_keeps_text_identical() {
    let dir = temp_dir("spans");
    let path = dir.join("a.rs");
    fs::write(&path, "fn a() {} // hi\nplain\n").unwrap();
    let editor = Editor::new(Buffer::open(&path).unwrap());
    let mut app = app_with(editor, dir, one_file());
    let screen = app.screen(30, 6, t(1000));
    assert_eq!(row_text(&screen, 0), "fn a() {} // hi");
    let spans: Vec<(&str, Style)> =
        screen.rows[0].iter().map(|s| (s.text.as_str(), s.style)).collect();
    assert_eq!(
        spans,
        vec![("fn", Style::Keyword), (" a() {} ", Style::Normal), ("// hi", Style::Comment)]
    );
    assert_eq!(screen.rows[1], vec![Span { text: "plain".into(), style: Style::Normal }]);
    assert_eq!(screen.rows[3], vec![Span { text: "~".into(), style: Style::Normal }]);
}

#[test]
fn spans_never_drop_text_when_styles_are_short() {
    let row = spans("abcd", &[Style::Keyword]);
    assert_eq!(
        row,
        vec![
            Span { text: "a".into(), style: Style::Keyword },
            Span { text: "bcd".into(), style: Style::Normal },
        ]
    );
    assert_eq!(spans("xy", &[]), vec![Span { text: "xy".into(), style: Style::Normal }]);
}

#[test]
fn editor_screen_spans_carry_no_control_characters() {
    let text = "a \x1b[31mred\x1b[0m \x1b]0;t\x07\nb \x7f \u{9b} \tc\n";
    for name in ["n.txt", "n.rs"] {
        let dir = Temp::new("ctl");
        dir.file(name, text);
        let editor = Editor::new(Buffer::open(&dir.0.join(name)).unwrap());
        let mut app = app_with(editor, dir.0.clone(), one_file());
        let screen = app.screen(40, 6, t(1000));
        for row in &screen.rows {
            for span in row {
                assert!(!span.text.chars().any(char::is_control), "{name}: {:?}", span.text);
            }
        }
        assert_eq!(row_text(&screen, 0), "a ?[31mred?[0m ?]0;t?");
        assert_eq!(row_text(&screen, 1), "b ? ?  c");
        assert!(!screen.status.chars().any(char::is_control));
        assert!(!screen.message.chars().any(char::is_control));
    }
}

#[test]
fn editor_screen_survives_tiny_sizes_after_scrolling() {
    let text: String = (0..12).map(|i| format!("fn f{i}() {{ \"long string here\" }} // c\n")).collect();
    let dir = Temp::new("tinyscroll");
    dir.file("s.rs", &text);
    let editor = Editor::new(Buffer::open(&dir.0.join("s.rs")).unwrap());
    let mut app = app_with(editor, dir.0.clone(), one_file());
    for _ in 0..11 {
        app.handle_key(key(KeyCode::Down), t(1000));
    }
    for _ in 0..20 {
        app.handle_key(key(KeyCode::Right), t(1000));
    }
    app.screen(10, 5, t(1000));
    for _ in 0..11 {
        app.handle_key(key(KeyCode::Up), t(1000));
    }
    for _ in 0..40 {
        app.handle_key(key(KeyCode::Left), t(1000));
    }
    for w in 0..=6 {
        for h in 0..=6 {
            let screen = app.screen(w, h, t(1000));
            assert!(screen.rows.len() <= h.saturating_sub(2));
            assert!(screen.rows.iter().all(|r| r.iter().map(|s| s.text.chars().count()).sum::<usize>() <= w));
            assert!(screen.status.chars().count() <= w && screen.message.chars().count() <= w);
        }
    }
}

// ---- Several buffers ----

/// App whose first buffer is `a.txt` from `dir`, with `a.txt`, `b.txt`, `c.txt` on disk.
fn multi_app(dir: &Temp) -> App {
    dir.file("a.txt", "alpha\nsecond\n");
    dir.file("b.txt", "beta\n");
    dir.file("c.txt", "gamma\n");
    let buffer = Buffer::open(&dir.0.join("a.txt")).unwrap();
    app_with(Editor::new(buffer), dir.0.clone(), one_file())
}

/// Opens the `n`th (0-based) top-level file of the tree.
fn open_from_tree(app: &mut App, n: usize) {
    app.handle_key(ctrl('t'), t(1000));
    app.handle_key(key(KeyCode::Home), t(1000));
    for _ in 0..n {
        app.handle_key(key(KeyCode::Down), t(1000));
    }
    app.handle_key(key(KeyCode::Enter), t(1000));
}

#[test]
fn opening_while_modified_keeps_both_buffers() {
    let dir = Temp::new("multi-modified");
    let mut app = multi_app(&dir);
    type_str(&mut app, "X");
    open_from_tree(&mut app, 1);
    assert_eq!(app.buffer_count(), 2);
    assert_eq!(app.current_index(), 1);
    assert_eq!(app.editor().buffer().text(), "beta\n");
    app.handle_key(ctrl('p'), t(1000));
    assert_eq!(app.current_index(), 0);
    assert!(app.editor().buffer().is_modified());
    assert_eq!(app.editor().buffer().text(), "Xalpha\nsecond\n");
}

#[test]
fn opening_open_file_switches() {
    let dir = Temp::new("multi-switch");
    let mut app = multi_app(&dir);
    open_from_tree(&mut app, 1);
    open_from_tree(&mut app, 1);
    assert_eq!(app.buffer_count(), 2);
    assert_eq!(app.current_index(), 1);
    open_from_tree(&mut app, 0);
    assert_eq!(app.buffer_count(), 2);
    assert_eq!(app.current_index(), 0);
}

#[test]
fn opening_replaces_pristine_unnamed_buffer() {
    let dir = Temp::new("multi-pristine");
    dir.file("b.txt", "beta\n");
    let mut app = app_with(Editor::new(Buffer::new()), dir.0.clone(), one_file());
    open_from_tree(&mut app, 0);
    assert_eq!(app.buffer_count(), 1);
    assert_eq!(app.editor().buffer().text(), "beta\n");
}

#[test]
fn opening_keeps_modified_unnamed_buffer() {
    let dir = Temp::new("multi-unnamed-modified");
    dir.file("b.txt", "beta\n");
    let mut app = app_with(Editor::new(Buffer::new()), dir.0.clone(), one_file());
    type_str(&mut app, "x");
    open_from_tree(&mut app, 0);
    assert_eq!(app.buffer_count(), 2);
}

#[test]
fn opening_same_file_via_dot_path_does_not_duplicate() {
    let dir = Temp::new("multi-canon");
    let mut app = multi_app(&dir);
    // The first buffer was opened through a path with a `./` detour.
    let detour = dir.0.join(".").join("a.txt");
    app.editors[0] = Editor::new(Buffer::open(&detour).unwrap());
    open_from_tree(&mut app, 0);
    assert_eq!(app.buffer_count(), 1);
}

#[test]
fn ctrl_n_and_ctrl_p_wrap() {
    let dir = Temp::new("multi-wrap");
    let mut app = multi_app(&dir);
    app.handle_key(key(KeyCode::Down), t(1000)); // a.txt cursor row 1
    open_from_tree(&mut app, 1);
    open_from_tree(&mut app, 2);
    assert_eq!(app.buffer_count(), 3);
    assert_eq!(app.current_index(), 2);
    app.handle_key(ctrl('n'), t(1000));
    assert_eq!(app.current_index(), 0);
    assert_eq!(app.editor().buffer().cursor().0, 1);
    app.handle_key(ctrl('p'), t(1000));
    assert_eq!(app.current_index(), 2);
    assert_eq!(app.editor().buffer().cursor().0, 0);
}

#[test]
fn ctrl_n_in_a_view_switches_and_returns_to_editor() {
    let dir = Temp::new("multi-view");
    let mut app = multi_app(&dir);
    open_from_tree(&mut app, 1);
    app.handle_key(ctrl('g'), t(1000));
    app.handle_key(ctrl('n'), t(1000));
    assert!(!app.in_git_view());
    assert_eq!(app.current_index(), 0);
    app.handle_key(ctrl('t'), t(1000));
    app.handle_key(ctrl('p'), t(1000));
    assert!(!app.in_file_tree());
    assert_eq!(app.current_index(), 1);
}

#[test]
fn status_shows_position_tag_only_with_many() {
    let dir = Temp::new("multi-tag");
    let mut app = multi_app(&dir);
    assert!(!app.screen(40, 6, t(1000)).status.contains('/'));
    open_from_tree(&mut app, 1);
    open_from_tree(&mut app, 2);
    app.handle_key(ctrl('p'), t(1000));
    let status = app.screen(40, 6, t(1000)).status;
    assert!(status.contains("[2/3]"), "{status}");
}
