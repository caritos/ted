use super::*;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

fn temp_dir() -> PathBuf {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!("ted-grep-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn write(root: &Path, name: &str, bytes: &[u8]) {
    let p = root.join(name);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, bytes).unwrap();
}

fn names(files: &[&str]) -> Vec<String> {
    files.iter().map(|s| s.to_string()).collect()
}

fn summary(found: &Found) -> Vec<(String, usize, String)> {
    found.hits.iter().map(|h| (h.path.clone(), h.line, h.text.clone())).collect()
}

#[test]
fn finds_lines_in_listed_order_with_one_based_line_numbers() {
    let root = temp_dir();
    write(&root, "a.txt", b"one\nfoo bar\nthree\nfoo again\n");
    write(&root, "src/b.rs", b"fn foo() {}\n");
    let found = search(&root, &names(&["a.txt", "src/b.rs"]), "foo");
    assert_eq!(
        summary(&found),
        vec![
            ("a.txt".into(), 2, "foo bar".into()),
            ("a.txt".into(), 4, "foo again".into()),
            ("src/b.rs".into(), 1, "fn foo() {}".into()),
        ]
    );
    assert_eq!(found.files_with_hits, 2);
    assert!(!found.capped);
}

#[test]
fn a_line_with_two_matches_is_one_hit() {
    let root = temp_dir();
    write(&root, "a.txt", b"foo foo foo\n");
    assert_eq!(search(&root, &names(&["a.txt"]), "foo").hits.len(), 1);
}

#[test]
fn smart_case_lowercase_ignores_case_and_any_capital_is_exact() {
    let root = temp_dir();
    write(&root, "a.txt", b"Foo\nfoo\nFOO\n");
    let files = names(&["a.txt"]);
    assert_eq!(search(&root, &files, "foo").hits.len(), 3);
    let exact = search(&root, &files, "Foo");
    assert_eq!(summary(&exact), vec![("a.txt".into(), 1, "Foo".into())]);
}

#[test]
fn indent_and_line_break_are_trimmed_from_the_text_and_crlf_is_handled() {
    let root = temp_dir();
    write(&root, "a.txt", b"\t  indented foo  \r\nplain foo\n");
    let found = search(&root, &names(&["a.txt"]), "foo");
    assert_eq!(found.hits[0].text, "indented foo");
    assert_eq!(found.hits[1].text, "plain foo");
}

#[test]
fn long_lines_are_cut() {
    let root = temp_dir();
    write(&root, "a.txt", format!("foo{}\n", "x".repeat(1000)).as_bytes());
    let found = search(&root, &names(&["a.txt"]), "foo");
    assert_eq!(found.hits[0].text.chars().count(), MAX_TEXT_CHARS);
}

#[test]
fn empty_query_finds_nothing() {
    let root = temp_dir();
    write(&root, "a.txt", b"foo\n");
    assert_eq!(search(&root, &names(&["a.txt"]), ""), Found::default());
}

#[test]
fn binary_non_utf8_oversized_missing_and_directory_entries_are_skipped() {
    let root = temp_dir();
    write(&root, "bin.dat", b"foo\0bar\n");
    write(&root, "latin.txt", b"foo \xe9\n");
    write(&root, "big.txt", &vec![b'f'; MAX_FILE_BYTES as usize + 1]);
    fs::create_dir_all(root.join("dir")).unwrap();
    write(&root, "ok.txt", b"foo\n");
    let files = names(&["bin.dat", "latin.txt", "big.txt", "gone.txt", "dir", "ok.txt"]);
    let found = search(&root, &files, "f");
    assert_eq!(summary(&found), vec![("ok.txt".into(), 1, "foo".into())]);
}

#[test]
fn the_last_line_without_a_break_and_an_empty_file_work() {
    let root = temp_dir();
    write(&root, "a.txt", b"x\nfoo");
    write(&root, "empty.txt", b"");
    let found = search(&root, &names(&["empty.txt", "a.txt"]), "foo");
    assert_eq!(summary(&found), vec![("a.txt".into(), 2, "foo".into())]);
}

#[test]
fn stops_at_the_hit_cap_and_says_so() {
    let root = temp_dir();
    let body = "foo\n".repeat(MAX_HITS + 50);
    write(&root, "a.txt", body.as_bytes());
    write(&root, "b.txt", b"foo\n");
    let found = search(&root, &names(&["a.txt", "b.txt"]), "foo");
    assert_eq!(found.hits.len(), MAX_HITS);
    assert!(found.capped);
    assert_eq!(found.files_with_hits, 1);
}

#[test]
fn unicode_text_and_case_folding_do_not_panic() {
    let root = temp_dir();
    write(&root, "a.txt", "ÄÖÜ straße İstanbul\n".as_bytes());
    let files = names(&["a.txt"]);
    assert_eq!(search(&root, &files, "äöü").hits.len(), 1);
    assert_eq!(search(&root, &files, "straße").hits.len(), 1);
    assert_eq!(search(&root, &files, "i\u{307}").hits.len(), 1);
}
