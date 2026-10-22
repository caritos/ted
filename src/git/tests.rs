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
