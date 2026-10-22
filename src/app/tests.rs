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
fn enter_with_unsaved_changes_refuses_and_keeps_buffer() {
    let dir = temp_dir("unsaved");
    fs::write(dir.join("a.txt"), "from disk\n").unwrap();
    let mut app = app_with(Editor::new(Buffer::new()), dir, one_file());
    type_str(&mut app, "draft");
    app.handle_key(ctrl('g'), t(1000));
    app.handle_key(key(KeyCode::Enter), t(1000));
    assert!(app.in_git_view());
    assert_eq!(app.screen(60, 6, t(1000)).message, "Unsaved changes: save or quit first");
    assert_eq!(app.editor().buffer().text(), "draft");
    assert!(app.editor().buffer().is_modified());
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
