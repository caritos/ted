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

// ---- render ----

fn row_text(row: &Row) -> String {
    row.iter().map(|s| s.text.as_str()).collect()
}

fn render_git(git: &FakeGit, w: usize, h: usize) -> Screen {
    refreshed(git).render(w, h, t(1000))
}

fn two_files() -> FakeGit {
    let mut git = FakeGit::new();
    git.add(ChangeKind::Modified, "a.rs", Some(t(940)));
    git.add(ChangeKind::Untracked, "b.rs", Some(t(100)));
    git.diffs.insert(
        "a.rs".to_string(),
        "diff --git a/a.rs b/a.rs\nindex 1..2 100644\n--- a/a.rs\n+++ b/a.rs\n@@ -1 +1 @@\n-old\n+new\n ctx\n".to_string(),
    );
    git
}

#[test]
fn render_shows_status_letter_path_and_age() {
    let screen = render_git(&two_files(), 60, 10);
    assert_eq!(screen.rows.len(), 8);
    assert!(row_text(&screen.rows[0]).starts_with("M a.rs  1m"));
    assert!(row_text(&screen.rows[1]).starts_with("? b.rs  15m"));
    assert_eq!(screen.status, format!("{:<60}", "Baseline h1  2 changes"));
    assert_eq!(screen.cursor, None);
}

#[test]
fn render_negative_age_clamps_to_zero() {
    let mut git = FakeGit::new();
    git.add(ChangeKind::Modified, "a.rs", Some(t(5000)));
    let screen = render_git(&git, 40, 4);
    assert!(row_text(&screen.rows[0]).starts_with("M a.rs  0s"));
}

#[test]
fn render_styles_diff_lines() {
    let screen = render_git(&two_files(), 80, 12);
    let style_of = |i: usize| screen.rows[i].last().unwrap().style;
    assert_eq!(style_of(0), Style::Dim);
    assert_eq!(style_of(1), Style::Dim);
    assert_eq!(style_of(2), Style::Dim);
    assert_eq!(style_of(3), Style::Dim);
    assert_eq!(style_of(4), Style::Hunk);
    assert_eq!(style_of(5), Style::Removed);
    assert_eq!(style_of(6), Style::Added);
    assert_eq!(style_of(7), Style::Normal);
    assert!(row_text(&screen.rows[5]).ends_with("-old"));
}

#[test]
fn render_styles_dashes_and_pluses_after_hunk_as_changes() {
    let mut git = FakeGit::new();
    git.add(ChangeKind::Modified, "q.sql", Some(t(1)));
    git.diffs.insert(
        "q.sql".to_string(),
        "diff --git a/q.sql b/q.sql\n--- a/q.sql\n+++ b/q.sql\n@@ -1,2 +1,2 @@\n--- sql comment\n+++x\n-diff\n+index\n".to_string(),
    );
    let mut view = refreshed(&git);
    let styles = |screen: &Screen| -> Vec<Style> {
        screen.rows.iter().filter_map(|r| r.get(2)).map(|s| s.style).collect()
    };
    let screen = view.render(80, 10, t(1000));
    assert_eq!(
        styles(&screen),
        vec![
            Style::Dim,
            Style::Dim,
            Style::Dim,
            Style::Hunk,
            Style::Removed,
            Style::Added,
            Style::Removed,
            Style::Added,
        ]
    );
    // Scrolled past the hunk header, the styles still know it was there.
    let long: String = format!("--- a/q\n+++ b/q\n@@ -1 +1 @@\n{}", "--- c\n".repeat(30));
    git.diffs.insert("q.sql".to_string(), long);
    view.refresh(&git, t(1001));
    view.handle_key(key(KeyCode::PageDown), &git, t(1001));
    let screen = view.render(80, 6, t(1001));
    assert!(styles(&screen).iter().all(|s| *s == Style::Removed), "{:?}", styles(&screen));
}

#[test]
fn render_diff_respects_scroll() {
    let mut git = FakeGit::new();
    git.add(ChangeKind::Modified, "a.rs", Some(t(1)));
    git.diffs.insert("a.rs".to_string(), diff_of(100));
    let mut view = refreshed(&git);
    view.handle_key(key(KeyCode::PageDown), &git, t(1000));
    let screen = view.render(40, 6, t(1000));
    assert!(row_text(&screen.rows[0]).ends_with("+line 20"));
}

#[test]
fn render_selected_row_is_styled_selected() {
    let screen = render_git(&two_files(), 60, 10);
    assert_eq!(screen.rows[0][0].style, Style::Selected);
    assert_eq!(screen.rows[1][0].style, Style::Normal);
}

#[test]
fn render_keeps_selected_row_visible_in_short_list() {
    let mut git = FakeGit::new();
    for i in 0..10 {
        git.add(ChangeKind::Modified, &format!("f{i}.rs"), Some(t(900 - i)));
    }
    let mut view = refreshed(&git);
    for _ in 0..9 {
        view.handle_key(key(KeyCode::Down), &git, t(1000));
    }
    let screen = view.render(40, 5, t(1000));
    let selected: Vec<_> = screen.rows.iter().filter(|r| r[0].style == Style::Selected).collect();
    assert_eq!(selected.len(), 1);
    assert!(row_text(selected[0]).contains("f9.rs"));
}

#[test]
fn render_empty_state_text() {
    let screen = render_git(&FakeGit::new(), 60, 6);
    assert!(row_text(&screen.rows[0]).contains("No changes since"));
    assert_eq!(screen.status, format!("{:<60}", "Baseline h1  0 changes"));
}

#[test]
fn render_empty_repo_baseline_text() {
    let mut git = FakeGit::new();
    git.head = Baseline::Empty;
    assert_eq!(render_git(&git, 60, 6).status, format!("{:<60}", "Baseline: (no commits)  0 changes"));
}

#[test]
fn render_error_state_text() {
    let mut git = FakeGit::new();
    git.fail = Some(GitError::NotARepo);
    let screen = render_git(&git, 60, 6);
    assert_eq!(row_text(&screen.rows[0]), "not a git repository");
    assert!(screen.rows[1..].iter().all(|r| row_text(r).is_empty()));
    // No change count: the list is hidden, so a count would mislead.
    assert_eq!(screen.status, format!("{:<60}", "Error"));

    // Also when an error follows a good refresh with changes.
    let mut git = two_files();
    let mut view = refreshed(&git);
    git.fail = Some(GitError::Failed("boom".to_string()));
    view.refresh(&git, t(1001));
    assert_eq!(view.render(60, 6, t(1001)).status, format!("{:<60}", "Error"));
}

#[test]
fn render_shows_message_row() {
    let mut git = two_files();
    git.add(ChangeKind::Deleted, "gone.rs", None);
    let mut view = refreshed(&git);
    // The deleted file has no mtime, so it sorts first by first-seen time.
    assert_eq!(view.selected_path(), Some("gone.rs"));
    view.handle_key(key(KeyCode::Enter), &git, t(1000));
    assert_eq!(view.render(60, 6, t(1000)).message, "gone.rs was deleted; nothing to open");
}

#[test]
fn render_picker_rows_contain_short_hash_subject_time_and_refs() {
    let mut git = FakeGit::new();
    let mut c1 = commit("abcdef1234", "first");
    c1.refs = vec!["main".to_string(), "v1".to_string()];
    git.log = vec![c1, commit("1234567890", "second")];
    let mut view = refreshed(&git);
    view.handle_key(ch('b'), &git, t(1000));
    let screen = view.render(80, 8, t(1000));
    // Padded so the selection bar spans the whole row.
    assert_eq!(row_text(&screen.rows[0]), format!("{:<80}", "abcdef1  first  2023-11-14 22:13  [main, v1]"));
    assert_eq!(screen.rows[0][0].style, Style::Selected);
    assert!(row_text(&screen.rows[1]).starts_with("1234567  second"));
    assert_eq!(screen.rows[1][0].style, Style::Normal);
}

#[test]
fn render_picker_keeps_selection_visible() {
    let mut git = FakeGit::new();
    git.log = (0..10).map(|i| commit(&format!("{i}000000"), &format!("c{i}"))).collect();
    let mut view = refreshed(&git);
    view.handle_key(ch('b'), &git, t(1000));
    for _ in 0..9 {
        view.handle_key(key(KeyCode::Down), &git, t(1000));
    }
    let screen = view.render(60, 5, t(1000));
    assert_eq!(screen.rows.len(), 3);
    assert!(row_text(screen.rows.last().unwrap()).contains("c9"));
    assert_eq!(screen.rows.last().unwrap()[0].style, Style::Selected);
}

#[test]
fn format_commit_time_known_values() {
    assert_eq!(format_commit_time(1_700_000_000), "2023-11-14 22:13");
    assert_eq!(format_commit_time(0), "1970-01-01 00:00");
    assert_eq!(format_commit_time(951_782_400), "2000-02-29 00:00");
    assert_eq!(format_commit_time(-1), "1969-12-31 23:59");
    assert_eq!(format_commit_time(4_102_444_799), "2099-12-31 23:59");
}

fn assert_fits(screen: &Screen, w: usize, h: usize) {
    let total = screen.rows.len() + usize::from(h >= 2) * 2;
    assert!(total <= h, "{w}x{h}: {total} rows");
    for row in &screen.rows {
        assert!(row_text(row).chars().count() <= w, "{w}x{h}: row too wide");
    }
    assert!(screen.status.chars().count() <= w);
    assert!(screen.message.chars().count() <= w);
}

#[test]
fn render_never_exceeds_width_or_height() {
    let mut list = two_files();
    list.add(ChangeKind::Modified, "dir/\u{e9}\u{e9}\u{e9}/very_long_name_indeed.rs", Some(t(10)));
    let mut err = FakeGit::new();
    err.fail = Some(GitError::Failed("something went quite wrong here".to_string()));
    let mut pick = FakeGit::new();
    pick.log = vec![commit("abcdef1234", "a rather long subject line")];
    for w in 0..=40 {
        for h in 0..=8 {
            assert_fits(&render_git(&list, w, h), w, h);
            assert_fits(&render_git(&err, w, h), w, h);
            let mut view = refreshed(&pick);
            view.handle_key(ch('b'), &pick, t(1000));
            assert_fits(&view.render(w, h, t(1000)), w, h);
        }
    }
}

#[test]
fn render_status_is_padded_to_exactly_width() {
    let mut err = FakeGit::new();
    err.fail = Some(GitError::NotARepo);
    for git in [two_files(), FakeGit::new(), err] {
        let view = refreshed(&git);
        for w in [0, 1, 2, 5, 10, 22, 23, 40, 80, 200] {
            let screen = view.render(w, 6, t(1000));
            assert_eq!(screen.status.chars().count(), w, "width {w}: {:?}", screen.status);
        }
    }
}

/// Text with terminal escapes, a lone CR, DEL and a C1 control.
const NASTY: &str = "a\x1b[31mb\x1b]0;title\x07c\rd\x7fe\u{9b}f";

fn is_control(c: char) -> bool {
    c.is_control()
}

fn assert_no_controls(screen: &Screen) {
    for row in &screen.rows {
        for span in row {
            assert!(!span.text.chars().any(is_control), "span {:?}", span.text);
        }
    }
    assert!(!screen.status.chars().any(is_control), "status {:?}", screen.status);
    assert!(!screen.message.chars().any(is_control), "message {:?}", screen.message);
}

#[test]
fn sanitize_maps_controls_to_question_mark_and_keeps_the_rest() {
    assert_eq!(sanitize(NASTY), "a?[31mb?]0;title?c?d?e?f");
    assert_eq!(sanitize("caf\u{e9} \u{65e5}\u{672c} \u{a0}x"), "caf\u{e9} \u{65e5}\u{672c} \u{a0}x");
}

#[test]
fn render_never_passes_control_characters_to_the_terminal() {
    // File names and diff lines.
    let mut git = FakeGit::new();
    git.add(ChangeKind::Deleted, NASTY, None);
    git.diffs.insert(NASTY.to_string(), format!("+{NASTY}\n-{NASTY}\n {NASTY}\n"));
    let mut view = refreshed(&git);
    for w in [0, 1, 10, 40, 120] {
        assert_no_controls(&view.render(w, 8, t(1000)));
    }
    let wide = view.render(200, 8, t(1000));
    assert!(row_text(&wide.rows[0]).contains("a?[31mb?]0;title?c?d?e?f"));

    // The message row (built from the path).
    view.handle_key(key(KeyCode::Enter), &git, t(1000));
    assert!(view.message().contains('\x1b'));
    assert_no_controls(&view.render(200, 8, t(1000)));

    // Commit subjects and refs in the picker.
    let mut pick = FakeGit::new();
    let mut c = commit("abcdef1234", NASTY);
    c.refs = vec![NASTY.to_string()];
    pick.log = vec![c];
    let mut view = refreshed(&pick);
    view.handle_key(ch('b'), &pick, t(1000));
    for w in [0, 1, 10, 40, 200] {
        assert_no_controls(&view.render(w, 6, t(1000)));
    }

    // Error text from git stderr.
    let mut err = FakeGit::new();
    err.fail = Some(GitError::Failed(NASTY.to_string()));
    assert_no_controls(&refreshed(&err).render(80, 6, t(1000)));
}

#[test]
fn render_keeps_normal_unicode_text() {
    let name = "caf\u{e9}/\u{65e5}\u{672c}.rs";
    let mut git = FakeGit::new();
    git.add(ChangeKind::Modified, name, Some(t(1)));
    git.diffs.insert(name.to_string(), "+\u{fc}ber \u{2713}\n".to_string());
    let screen = render_git(&git, 80, 6);
    assert!(row_text(&screen.rows[0]).starts_with(&format!("M {name}")));
    assert!(row_text(&screen.rows[0]).ends_with("+\u{fc}ber \u{2713}"));
}

#[test]
fn render_with_unicode_paths_clips_by_char_not_byte() {
    let name = "\u{65e5}\u{672c}\u{8a9e}\u{30d5}\u{30a1}\u{30a4}\u{30eb}\u{540d}\u{524d}.rs";
    let mut git = FakeGit::new();
    git.add(ChangeKind::Modified, name, Some(t(1)));
    git.diffs.insert(name.to_string(), "+\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}\n".to_string());
    for w in 0..=30 {
        assert_fits(&render_git(&git, w, 6), w, 6);
    }
    let screen = render_git(&git, 24, 6);
    assert!(row_text(&screen.rows[0]).starts_with("M \u{65e5}\u{672c}"));
}

#[test]
fn set_message_shows_until_next_key() {
    let git = two_files();
    let mut view = refreshed(&git);
    view.set_message("Unsaved changes: save or quit first".to_string());
    assert_eq!(view.message(), "Unsaved changes: save or quit first");
    assert_eq!(view.render(60, 6, t(1000)).message, "Unsaved changes: save or quit first");
    view.handle_key(key(KeyCode::Down), &git, t(1000));
    assert_eq!(view.message(), "");
}
