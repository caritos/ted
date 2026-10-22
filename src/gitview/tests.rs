use super::*;
use crate::git::fake::FakeGit;
use crate::git::{ChangeKind, Commit, GitError};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::time::{Duration, UNIX_EPOCH};

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn ch(c: char) -> KeyEvent {
    key(KeyCode::Char(c))
}

fn t(secs: u64) -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(secs)
}

fn commit(hash: &str, subject: &str) -> Commit {
    Commit {
        hash: hash.to_string(),
        short: hash.chars().take(7).collect(),
        subject: subject.to_string(),
        unix_time: 1_700_000_000,
        refs: Vec::new(),
    }
}

fn paths(view: &GitView) -> Vec<&str> {
    view.paths()
}

/// A view refreshed once against `git` at time 1000.
fn refreshed(git: &FakeGit) -> GitView {
    let mut view = GitView::new();
    view.refresh(git, t(1000));
    view
}

fn diff_of(lines: usize) -> String {
    (0..lines).map(|i| format!("+line {i}\n")).collect()
}

#[test]
fn first_refresh_sets_baseline_to_head() {
    let mut git = FakeGit::new();
    git.head = Baseline::Commit("abc".to_string());
    let mut view = GitView::new();
    assert_eq!(view.baseline(), None);
    view.refresh(&git, t(1000));
    assert_eq!(view.baseline(), Some(&Baseline::Commit("abc".to_string())));

    // Later refreshes keep the baseline even when HEAD moves.
    git.head = Baseline::Commit("def".to_string());
    view.refresh(&git, t(1001));
    assert_eq!(view.baseline(), Some(&Baseline::Commit("abc".to_string())));
}

#[test]
fn list_sorted_newest_first() {
    let mut git = FakeGit::new();
    git.add(ChangeKind::Modified, "old.rs", Some(t(10)));
    git.add(ChangeKind::Modified, "new.rs", Some(t(30)));
    git.add(ChangeKind::Untracked, "mid.rs", Some(t(20)));
    git.add(ChangeKind::Added, "b-tie.rs", Some(t(15)));
    git.add(ChangeKind::Added, "a-tie.rs", Some(t(15)));
    let view = refreshed(&git);
    assert_eq!(paths(&view), vec!["new.rs", "mid.rs", "a-tie.rs", "b-tie.rs", "old.rs"]);
    assert_eq!(view.selected_path(), Some("new.rs"));
}

#[test]
fn selection_follows_path_when_list_reorders() {
    let mut git = FakeGit::new();
    git.add(ChangeKind::Modified, "a.rs", Some(t(30)));
    git.add(ChangeKind::Modified, "b.rs", Some(t(20)));
    let mut view = refreshed(&git);
    view.handle_key(key(KeyCode::Down), &git, t(1000));
    assert_eq!(view.selected_path(), Some("b.rs"));

    // b.rs becomes newest and moves to the top row; selection stays on it.
    git.mtimes.insert("b.rs".to_string(), t(40));
    view.refresh(&git, t(1001));
    assert_eq!(paths(&view), vec!["b.rs", "a.rs"]);
    assert_eq!(view.selected_path(), Some("b.rs"));
}

#[test]
fn selection_falls_back_to_first_when_selected_file_disappears() {
    let mut git = FakeGit::new();
    git.add(ChangeKind::Modified, "a.rs", Some(t(30)));
    git.add(ChangeKind::Modified, "b.rs", Some(t(20)));
    git.add(ChangeKind::Modified, "c.rs", Some(t(10)));
    let mut view = refreshed(&git);
    view.handle_key(key(KeyCode::Down), &git, t(1000));
    assert_eq!(view.selected_path(), Some("b.rs"));

    git.remove("b.rs");
    view.refresh(&git, t(1001));
    assert_eq!(view.selected_path(), Some("a.rs"));

    git.changes.clear();
    view.refresh(&git, t(1002));
    assert_eq!(view.selected_path(), None);
}

#[test]
fn empty_changes_gives_empty_list_without_error() {
    let git = FakeGit::new();
    let view = refreshed(&git);
    assert!(paths(&view).is_empty());
    assert_eq!(view.error(), None);
    assert_eq!(view.selected_path(), None);
}

#[test]
fn git_error_sets_error_and_not_empty_state() {
    let mut git = FakeGit::new();
    git.fail = Some(GitError::NotARepo);
    let view = refreshed(&git);
    assert_eq!(view.error(), Some("not a git repository"));

    // An error after a good refresh hides, but keeps, the last good list.
    let mut git = FakeGit::new();
    git.add(ChangeKind::Modified, "a.rs", Some(t(10)));
    let mut view = refreshed(&git);
    git.fail = Some(GitError::Failed("boom".to_string()));
    view.refresh(&git, t(1001));
    assert_eq!(view.error(), Some("boom"));

    git.fail = None;
    view.refresh(&git, t(1002));
    assert_eq!(view.error(), None);
    assert_eq!(paths(&view), vec!["a.rs"]);
    assert_eq!(view.selected_path(), Some("a.rs"));
}

#[test]
fn deleted_file_sorts_by_first_noticed_time_and_keeps_it_after_reset() {
    let mut git = FakeGit::new();
    git.add(ChangeKind::Modified, "old.rs", Some(t(500)));
    git.add(ChangeKind::Deleted, "gone.rs", None);
    let mut view = GitView::new();
    view.refresh(&git, t(1000));
    // First noticed at 1000, newer than old.rs.
    assert_eq!(paths(&view), vec!["gone.rs", "old.rs"]);
    assert_eq!(view.entry_time("gone.rs"), Some(t(1000)));

    // A file modified after that sorts above it; the deleted file keeps 1000.
    git.add(ChangeKind::Modified, "fresh.rs", Some(t(1500)));
    view.refresh(&git, t(2000));
    assert_eq!(paths(&view), vec!["fresh.rs", "gone.rs", "old.rs"]);
    assert_eq!(view.entry_time("gone.rs"), Some(t(1000)));

    // Resetting the baseline does not forget when it was first noticed.
    view.handle_key(ch('r'), &git, t(3000));
    assert_eq!(view.entry_time("gone.rs"), Some(t(1000)));
}

#[test]
fn diff_scroll_kept_when_diff_unchanged_and_reset_when_it_changes() {
    let mut git = FakeGit::new();
    git.add(ChangeKind::Modified, "a.rs", Some(t(30)));
    git.add(ChangeKind::Modified, "b.rs", Some(t(20)));
    git.diffs.insert("a.rs".to_string(), diff_of(100));
    git.diffs.insert("b.rs".to_string(), diff_of(5));
    let mut view = refreshed(&git);
    // Only the selected file's diff was fetched.
    assert_eq!(*git.diff_calls.borrow(), vec!["a.rs".to_string()]);
    assert_eq!(view.diff(), diff_of(100));

    view.handle_key(key(KeyCode::PageDown), &git, t(1000));
    let scrolled = view.diff_scroll();
    assert!(scrolled > 0);

    view.refresh(&git, t(1001));
    assert_eq!(view.diff_scroll(), scrolled);

    git.diffs.insert("a.rs".to_string(), diff_of(101));
    view.refresh(&git, t(1002));
    assert_eq!(view.diff_scroll(), 0);
    assert_eq!(view.diff(), diff_of(101));
    assert!(git.diff_calls.borrow().iter().all(|p| p == "a.rs"));
}

#[test]
fn page_down_and_up_scroll_diff_never_below_zero() {
    let mut git = FakeGit::new();
    git.add(ChangeKind::Modified, "a.rs", Some(t(30)));
    git.diffs.insert("a.rs".to_string(), diff_of(100));
    let mut view = refreshed(&git);
    view.handle_key(key(KeyCode::PageUp), &git, t(1000));
    assert_eq!(view.diff_scroll(), 0);

    view.handle_key(key(KeyCode::PageDown), &git, t(1000));
    let one = view.diff_scroll();
    assert!(one > 0);
    view.handle_key(key(KeyCode::PageDown), &git, t(1000));
    assert!(view.diff_scroll() > one);
    view.handle_key(key(KeyCode::PageUp), &git, t(1000));
    assert_eq!(view.diff_scroll(), one);
    for _ in 0..5 {
        view.handle_key(key(KeyCode::PageUp), &git, t(1000));
    }
    assert_eq!(view.diff_scroll(), 0);

    // Scrolling stops at the last diff line.
    for _ in 0..50 {
        view.handle_key(key(KeyCode::PageDown), &git, t(1000));
    }
    assert_eq!(view.diff_scroll(), 99);
}

#[test]
fn up_down_move_selection_and_clamp_at_ends() {
    let mut git = FakeGit::new();
    git.add(ChangeKind::Modified, "a.rs", Some(t(30)));
    git.add(ChangeKind::Modified, "b.rs", Some(t(20)));
    git.add(ChangeKind::Modified, "c.rs", Some(t(10)));
    git.diffs.insert("b.rs".to_string(), "diff of b\n".to_string());
    let mut view = refreshed(&git);

    view.handle_key(key(KeyCode::Up), &git, t(1000));
    assert_eq!(view.selected_path(), Some("a.rs"));
    view.handle_key(key(KeyCode::Down), &git, t(1000));
    assert_eq!(view.selected_path(), Some("b.rs"));
    // Moving fetches the newly selected file's diff.
    assert_eq!(view.diff(), "diff of b\n");
    view.handle_key(key(KeyCode::Down), &git, t(1000));
    view.handle_key(key(KeyCode::Down), &git, t(1000));
    assert_eq!(view.selected_path(), Some("c.rs"));
    view.handle_key(key(KeyCode::Up), &git, t(1000));
    assert_eq!(view.selected_path(), Some("b.rs"));
}

#[test]
fn r_resets_baseline_to_head() {
    let mut git = FakeGit::new();
    git.head = Baseline::Commit("old".to_string());
    let mut view = refreshed(&git);
    git.head = Baseline::Commit("new".to_string());

    // Ctrl-R and Alt-R are not the reset key.
    view.handle_key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL), &git, t(1000));
    view.handle_key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::ALT), &git, t(1000));
    assert_eq!(view.baseline(), Some(&Baseline::Commit("old".to_string())));

    assert_eq!(view.handle_key(ch('r'), &git, t(1000)), GitAction::None);
    assert_eq!(view.baseline(), Some(&Baseline::Commit("new".to_string())));
}

#[test]
fn b_opens_picker_with_log_rows_and_enter_picks_baseline() {
    let mut git = FakeGit::new();
    git.log = vec![commit("c3", "third"), commit("c2", "second"), commit("c1", "first")];
    let mut view = refreshed(&git);
    assert!(!view.picker_open());

    view.handle_key(ch('b'), &git, t(1000));
    assert!(view.picker_open());
    assert_eq!(view.picker_commits().len(), 3);
    assert_eq!(view.picker_selected(), Some(0));

    view.handle_key(key(KeyCode::Down), &git, t(1000));
    view.handle_key(key(KeyCode::Down), &git, t(1000));
    view.handle_key(key(KeyCode::Down), &git, t(1000));
    assert_eq!(view.picker_selected(), Some(2));
    view.handle_key(key(KeyCode::Up), &git, t(1000));
    assert_eq!(view.picker_selected(), Some(1));

    assert_eq!(view.handle_key(key(KeyCode::Enter), &git, t(1000)), GitAction::None);
    assert!(!view.picker_open());
    assert_eq!(view.baseline(), Some(&Baseline::Commit("c2".to_string())));
}

#[test]
fn esc_closes_picker_without_changing_baseline() {
    let mut git = FakeGit::new();
    git.log = vec![commit("c2", "second"), commit("c1", "first")];
    let mut view = refreshed(&git);
    view.handle_key(ch('b'), &git, t(1000));
    view.handle_key(key(KeyCode::Down), &git, t(1000));
    // Esc closes the picker only; it does not leave the git view.
    assert_eq!(view.handle_key(key(KeyCode::Esc), &git, t(1000)), GitAction::None);
    assert!(!view.picker_open());
    assert_eq!(view.baseline(), Some(&Baseline::Commit("h1".to_string())));
}

#[test]
fn empty_log_picker_shows_message_and_closes_on_esc() {
    let mut git = FakeGit::new();
    git.head = Baseline::Empty;
    let mut view = refreshed(&git);
    view.handle_key(ch('b'), &git, t(1000));
    assert!(view.picker_open());
    assert_eq!(view.picker_selected(), None);
    assert_eq!(view.message(), "No commits yet");

    // Enter on an empty picker changes nothing and does not panic.
    view.handle_key(key(KeyCode::Enter), &git, t(1000));
    view.handle_key(key(KeyCode::Down), &git, t(1000));
    assert!(view.picker_open());
    assert_eq!(view.baseline(), Some(&Baseline::Empty));

    view.handle_key(key(KeyCode::Esc), &git, t(1000));
    assert!(!view.picker_open());
}

#[test]
fn vanished_baseline_falls_back_to_head_with_message() {
    let mut git = FakeGit::new();
    git.head = Baseline::Commit("old".to_string());
    let mut view = refreshed(&git);
    git.head = Baseline::Commit("new".to_string());
    git.existing = false;
    view.refresh(&git, t(1001));
    assert_eq!(view.baseline(), Some(&Baseline::Commit("new".to_string())));
    assert_eq!(view.message(), "Baseline commit is gone; reset to HEAD");
}

#[test]
fn enter_on_file_returns_open_with_path() {
    let mut git = FakeGit::new();
    git.add(ChangeKind::Modified, "src/a b.rs", Some(t(30)));
    let mut view = refreshed(&git);
    assert_eq!(
        view.handle_key(key(KeyCode::Enter), &git, t(1000)),
        GitAction::Open("src/a b.rs".to_string())
    );
}

#[test]
fn enter_on_deleted_file_does_nothing_and_sets_message() {
    let mut git = FakeGit::new();
    git.add(ChangeKind::Deleted, "gone.rs", None);
    let mut view = refreshed(&git);
    assert_eq!(view.handle_key(key(KeyCode::Enter), &git, t(1000)), GitAction::None);
    assert_eq!(view.message(), "gone.rs was deleted; nothing to open");
}

#[test]
fn enter_with_empty_list_does_nothing() {
    let git = FakeGit::new();
    let mut view = refreshed(&git);
    assert_eq!(view.handle_key(key(KeyCode::Enter), &git, t(1000)), GitAction::None);
}

#[test]
fn q_and_esc_return_back() {
    let git = FakeGit::new();
    let mut view = refreshed(&git);
    assert_eq!(view.handle_key(ch('q'), &git, t(1000)), GitAction::Back);
    assert_eq!(view.handle_key(key(KeyCode::Esc), &git, t(1000)), GitAction::Back);
    // Ctrl-Q belongs to the editor, not the git view.
    assert_eq!(
        view.handle_key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::CONTROL), &git, t(1000)),
        GitAction::None
    );
}

#[test]
fn keys_with_no_selection_do_not_panic() {
    let codes = [
        KeyCode::Up,
        KeyCode::Down,
        KeyCode::PageUp,
        KeyCode::PageDown,
        KeyCode::Enter,
        KeyCode::Char('r'),
        KeyCode::Char('b'),
        KeyCode::Esc,
        KeyCode::Char('x'),
    ];
    // Never refreshed, refreshed with nothing, and in an error state.
    let empty = FakeGit::new();
    let mut failing = FakeGit::new();
    failing.fail = Some(GitError::NotARepo);
    for git in [&empty, &failing] {
        for refresh in [false, true] {
            let mut view = GitView::new();
            if refresh {
                view.refresh(git, t(1000));
            }
            for code in codes {
                let action = view.handle_key(key(code), git, t(1000));
                if code == KeyCode::Enter {
                    assert_eq!(action, GitAction::None);
                }
            }
            assert_eq!(view.selected_path(), None);
        }
    }
}

#[test]
fn diff_failure_shows_in_diff_pane_and_keeps_list() {
    let mut git = FakeGit::new();
    git.add(ChangeKind::Modified, "a.rs", Some(t(30)));
    git.diffs.insert("a.rs".to_string(), "+real diff\n".to_string());
    git.fail_diff = Some(GitError::Failed("diff exploded".to_string()));
    let mut view = refreshed(&git);
    assert_eq!(view.error(), None);
    assert_eq!(paths(&view), vec!["a.rs"]);
    assert_eq!(view.selected_path(), Some("a.rs"));
    assert_eq!(view.diff(), "Diff failed: diff exploded");

    git.fail_diff = None;
    view.refresh(&git, t(1001));
    assert_eq!(view.error(), None);
    assert_eq!(view.diff(), "+real diff\n");
}

#[test]
fn deleted_file_keeps_first_noticed_time_when_it_leaves_and_reenters_list() {
    let mut git = FakeGit::new();
    git.add(ChangeKind::Deleted, "gone.rs", None);
    let mut view = GitView::new();
    view.refresh(&git, t(1000));
    assert_eq!(view.entry_time("gone.rs"), Some(t(1000)));

    // Another baseline: the deleted file is not a change any more.
    git.remove("gone.rs");
    git.add(ChangeKind::Modified, "mid.rs", Some(t(1500)));
    view.refresh(&git, t(2000));
    assert_eq!(paths(&view), vec!["mid.rs"]);

    // Back again: it keeps 1000, so it sorts below mid.rs (1500).
    git.add(ChangeKind::Deleted, "gone.rs", None);
    view.refresh(&git, t(3000));
    assert_eq!(paths(&view), vec!["mid.rs", "gone.rs"]);
    assert_eq!(view.entry_time("gone.rs"), Some(t(1000)));
}
