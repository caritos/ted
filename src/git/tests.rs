use super::*;

fn ch(kind: ChangeKind, path: &str) -> Change {
    Change { kind, path: path.to_string() }
}

#[test]
fn name_status_parses_each_kind() {
    assert_eq!(
        parse_name_status_z("M\0a.rs\0A\0b.rs\0D\0c.rs\0"),
        vec![
            ch(ChangeKind::Modified, "a.rs"),
            ch(ChangeKind::Added, "b.rs"),
            ch(ChangeKind::Deleted, "c.rs"),
        ]
    );
}

#[test]
fn name_status_keeps_spaces_unicode_and_newlines() {
    let path = "my file\nwith é.rs";
    let input = format!("M\0{path}\0");
    assert_eq!(parse_name_status_z(&input), vec![ch(ChangeKind::Modified, path)]);
}

#[test]
fn name_status_unknown_letter_is_modified() {
    assert_eq!(parse_name_status_z("T\0x\0"), vec![ch(ChangeKind::Modified, "x")]);
}

#[test]
fn name_status_empty_and_truncated_input() {
    assert_eq!(parse_name_status_z(""), vec![]);
    assert_eq!(parse_name_status_z("M\0"), vec![]);
}

#[test]
fn untracked_parses_paths() {
    assert_eq!(
        parse_untracked_z("a\0b c\0"),
        vec![ch(ChangeKind::Untracked, "a"), ch(ChangeKind::Untracked, "b c")]
    );
}

#[test]
fn log_parses_fields_and_refs() {
    let commits =
        parse_log("H1\tabc\t1700000000\tHEAD -> main, origin/main, tag: v1\tAdd viewer");
    assert_eq!(commits.len(), 1);
    let c = &commits[0];
    assert_eq!(c.hash, "H1");
    assert_eq!(c.short, "abc");
    assert_eq!(c.unix_time, 1700000000);
    assert_eq!(c.refs, vec!["main", "origin/main", "v1"]);
    assert_eq!(c.subject, "Add viewer");
}

#[test]
fn log_subject_with_tabs_and_empty_refs() {
    let commits = parse_log("H\th\t1\t\ta\tb");
    assert_eq!(commits[0].subject, "a\tb");
    assert!(commits[0].refs.is_empty());
}

#[test]
fn log_drops_bare_head() {
    let commits = parse_log("H\th\t1\tHEAD\ts");
    assert!(commits[0].refs.is_empty());
}

#[test]
fn log_skips_malformed_lines() {
    let commits = parse_log("garbage\nH\th\tnotanumber\t\ts\nH\th\t1\t\tok\n\n");
    assert_eq!(commits.len(), 1);
    assert_eq!(commits[0].subject, "ok");
}

#[test]
fn empty_baseline_uses_empty_tree_hash() {
    assert_eq!(Baseline::Empty.rev(), "4b825dc642cb6eb9a060e54bf8d69288fbee4904");
    assert_eq!(Baseline::Commit("abc".into()).rev(), "abc");
}

#[test]
fn format_age_boundaries() {
    for (secs, want) in [
        (0, "0s"),
        (59, "59s"),
        (60, "1m"),
        (3599, "59m"),
        (3600, "1h"),
        (86399, "23h"),
        (86400, "1d"),
    ] {
        assert_eq!(format_age(secs), want);
    }
}

#[test]
fn not_a_repo_displays_message() {
    assert_eq!(GitError::NotARepo.to_string(), "not a git repository");
}

// ---- GitCli (real temp repos) ----

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> TempDir {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "ted-git-test-{}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed),
            name
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        TempDir(dir)
    }

    fn path(&self) -> &Path {
        &self.0
    }

    fn git(&self, args: &[&str]) {
        run_git(&self.0, args);
    }

    fn write(&self, name: &str, content: &str) {
        let p = self.0.join(name);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, content).unwrap();
    }

    fn commit_all(&self, msg: &str) {
        self.git(&["add", "-A"]);
        self.git(&["commit", "-q", "-m", msg]);
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn run_git(dir: &Path, args: &[&str]) {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .env("LC_ALL", "C")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

fn temp_repo(name: &str) -> TempDir {
    let t = TempDir::new(name);
    t.git(&["init", "-q"]);
    t.git(&["config", "user.name", "Test"]);
    t.git(&["config", "user.email", "test@example.com"]);
    t.git(&["config", "commit.gpgsign", "false"]);
    t
}

fn sorted(mut v: Vec<Change>) -> Vec<Change> {
    v.sort_by(|a, b| a.path.cmp(&b.path));
    v
}

#[test]
fn head_of_repo_without_commits_is_empty() {
    let t = temp_repo("head-empty");
    assert_eq!(GitCli::new(t.path()).head(), Ok(Baseline::Empty));
}

#[test]
fn head_with_corrupt_branch_ref_is_an_error_not_empty() {
    let t = temp_repo("head-corrupt");
    t.write("a", "1\n");
    t.commit_all("first");
    let head = fs::read_to_string(t.path().join(".git/HEAD")).unwrap();
    let branch_ref = head.trim().strip_prefix("ref: ").unwrap().to_string();
    fs::write(t.path().join(".git").join(branch_ref), "garbage\n").unwrap();
    let git = GitCli::new(t.path());
    assert!(matches!(git.head(), Err(GitError::Failed(_))), "{:?}", git.head());
}

#[test]
fn every_git_command_disables_optional_locks() {
    let t = temp_repo("optional-locks");
    let cmd = GitCli::new(t.path()).command();
    let envs: Vec<_> = cmd.get_envs().collect();
    assert!(envs.iter().any(|(k, v)| {
        *k == "GIT_OPTIONAL_LOCKS" && *v == Some(std::ffi::OsStr::new("0"))
    }));
}

#[test]
fn head_returns_current_commit_after_commit() {
    let t = temp_repo("head-commit");
    t.write("a", "1\n");
    t.commit_all("first");
    let Ok(Baseline::Commit(hash)) = GitCli::new(t.path()).head() else {
        panic!("expected a commit baseline");
    };
    assert_eq!(hash.len(), 40);
}

#[test]
fn changes_reports_modified_added_deleted_untracked() {
    let t = temp_repo("changes-kinds");
    t.write("a", "1\n");
    t.write("b", "1\n");
    t.commit_all("first");
    let git = GitCli::new(t.path());
    let base = git.head().unwrap();
    t.write("a", "2\n");
    fs::remove_file(t.path().join("b")).unwrap();
    t.write("c", "new\n");
    t.git(&["add", "c"]);
    t.write("d", "untracked\n");
    assert_eq!(
        sorted(git.changes(&base).unwrap()),
        vec![
            ch(ChangeKind::Modified, "a"),
            ch(ChangeKind::Deleted, "b"),
            ch(ChangeKind::Added, "c"),
            ch(ChangeKind::Untracked, "d"),
        ]
    );
}

#[test]
fn changes_respects_gitignore() {
    let t = temp_repo("changes-ignore");
    t.write(".gitignore", "ignored.txt\n");
    t.commit_all("first");
    let git = GitCli::new(t.path());
    let base = git.head().unwrap();
    t.write("ignored.txt", "x\n");
    t.write("shown.txt", "x\n");
    assert_eq!(
        git.changes(&base).unwrap(),
        vec![ch(ChangeKind::Untracked, "shown.txt")]
    );
}

#[test]
fn changes_with_no_commits_uses_empty_baseline() {
    let t = temp_repo("changes-nocommits");
    t.write("a", "1\n");
    t.write("b", "1\n");
    t.git(&["add", "a"]);
    let git = GitCli::new(t.path());
    let base = git.head().unwrap();
    assert_eq!(base, Baseline::Empty);
    assert_eq!(
        sorted(git.changes(&base).unwrap()),
        vec![ch(ChangeKind::Added, "a"), ch(ChangeKind::Untracked, "b")]
    );
}

#[test]
fn changes_in_subdirectory_are_scoped_and_relative() {
    let t = temp_repo("changes-sub");
    t.write("sub/x.txt", "1\n");
    t.write("other.txt", "1\n");
    t.commit_all("first");
    let git = GitCli::new(t.path().join("sub"));
    let base = git.head().unwrap();
    t.write("sub/x.txt", "2\n");
    t.write("other.txt", "2\n");
    t.write("sub/new.txt", "n\n");
    t.write("elsewhere.txt", "n\n");
    assert_eq!(
        sorted(git.changes(&base).unwrap()),
        vec![ch(ChangeKind::Untracked, "new.txt"), ch(ChangeKind::Modified, "x.txt")]
    );
}

#[test]
fn changes_include_commits_made_after_baseline() {
    let t = temp_repo("changes-after-base");
    t.write("a", "1\n");
    t.commit_all("first");
    let git = GitCli::new(t.path());
    let base = git.head().unwrap();
    t.write("a", "2\n");
    t.commit_all("second");
    assert_eq!(git.changes(&base).unwrap(), vec![ch(ChangeKind::Modified, "a")]);
}

#[test]
fn changes_with_odd_file_names() {
    let t = temp_repo("changes-odd");
    t.write("keep", "1\n");
    t.commit_all("first");
    let git = GitCli::new(t.path());
    let base = git.head().unwrap();
    t.write("my file.txt", "x\n");
    t.write("café é.txt", "x\n");
    assert_eq!(
        sorted(git.changes(&base).unwrap()),
        sorted(vec![
            ch(ChangeKind::Untracked, "my file.txt"),
            ch(ChangeKind::Untracked, "café é.txt"),
        ])
    );
}

#[test]
fn diff_of_modified_file_has_hunk_and_plus_minus_lines() {
    let t = temp_repo("diff-mod");
    t.write("a", "one\ntwo\n");
    t.commit_all("first");
    let git = GitCli::new(t.path());
    let base = git.head().unwrap();
    t.write("a", "one\nTWO\n");
    let d = git.diff(&base, &ch(ChangeKind::Modified, "a")).unwrap();
    assert!(d.contains("@@"), "{d}");
    assert!(d.contains("-two"), "{d}");
    assert!(d.contains("+TWO"), "{d}");
}

#[test]
fn diff_of_untracked_file_shows_all_added() {
    let t = temp_repo("diff-untracked");
    t.write("a", "1\n");
    t.commit_all("first");
    let git = GitCli::new(t.path());
    let base = git.head().unwrap();
    t.write("new.txt", "hello\nworld\n");
    let d = git.diff(&base, &ch(ChangeKind::Untracked, "new.txt")).unwrap();
    assert!(d.contains("+hello"), "{d}");
    assert!(d.contains("+world"), "{d}");
}

#[test]
fn diff_of_binary_file_says_binary() {
    let t = temp_repo("diff-binary");
    t.write("a", "1\n");
    t.commit_all("first");
    let git = GitCli::new(t.path());
    let base = git.head().unwrap();
    fs::write(t.path().join("bin.dat"), [0u8, 1, 2, 0, 255, 0]).unwrap();
    let d = git.diff(&base, &ch(ChangeKind::Untracked, "bin.dat")).unwrap();
    assert!(d.contains("Binary"), "{d}");
}

#[test]
fn diff_is_truncated_over_cap() {
    let t = temp_repo("diff-cap");
    t.write("a", "1\n");
    t.commit_all("first");
    let git = GitCli::new(t.path());
    let base = git.head().unwrap();
    t.write("big.txt", &"line of text\n".repeat(30_000));
    let d = git.diff(&base, &ch(ChangeKind::Untracked, "big.txt")).unwrap();
    assert!(d.ends_with("... diff truncated"), "{}", &d[d.len().saturating_sub(40)..]);
    assert!(d.len() < MAX_DIFF_BYTES + 100);
}

#[test]
fn log_lists_recent_commits_newest_first_with_refs() {
    let t = temp_repo("log");
    t.write("a", "1\n");
    t.commit_all("first");
    t.write("a", "2\n");
    t.commit_all("second");
    t.git(&["tag", "v1"]);
    t.write("a", "3\n");
    t.commit_all("third");
    let git = GitCli::new(t.path());
    let all = git.log(10).unwrap();
    let subjects: Vec<&str> = all.iter().map(|c| c.subject.as_str()).collect();
    assert_eq!(subjects, vec!["third", "second", "first"]);
    assert!(all[1].refs.contains(&"v1".to_string()), "{:?}", all[1].refs);
    assert_eq!(git.log(2).unwrap().len(), 2);
}

#[test]
fn not_a_repo_is_reported() {
    let t = TempDir::new("not-repo");
    let git = GitCli::new(t.path());
    assert_eq!(git.head(), Err(GitError::NotARepo));
    assert_eq!(git.changes(&Baseline::Empty), Err(GitError::NotARepo));
}

#[test]
fn baseline_exists_false_for_unknown_hash() {
    let t = temp_repo("baseline-exists");
    t.write("a", "1\n");
    t.commit_all("first");
    let git = GitCli::new(t.path());
    let head = git.head().unwrap();
    assert!(git.baseline_exists(&head));
    assert!(git.baseline_exists(&Baseline::Empty));
    let bogus = Baseline::Commit("0123456789abcdef0123456789abcdef01234567".into());
    assert!(!git.baseline_exists(&bogus));
}

#[test]
fn modified_returns_none_for_missing_file() {
    let t = temp_repo("modified");
    t.write("a", "1\n");
    let git = GitCli::new(t.path());
    assert!(git.modified("a").is_some());
    assert_eq!(git.modified("nope"), None);
}
