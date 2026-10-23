use super::*;
use std::fs;
use std::path::PathBuf;

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn ctrl(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
}

fn type_str(f: &mut Finder, text: &str) {
    for c in text.chars() {
        f.handle_key(key(KeyCode::Char(c)));
    }
}

fn finder() -> Finder {
    let files = ["README.md", "src/app.rs", "src/editor.rs", "src/editor/tests.rs"];
    Finder::new(files.iter().map(|s| s.to_string()).collect(), false)
}

fn text(screen: &Screen, i: usize) -> String {
    screen.rows[i].iter().map(|s| s.text.as_str()).collect()
}

#[test]
fn starts_with_every_file_in_order_and_the_first_selected() {
    let f = finder();
    assert_eq!(f.selected_path(), Some("README.md"));
    let screen = f.render(40, 8);
    assert_eq!(text(&screen, 0).trim_end(), "  README.md");
    assert_eq!(screen.rows[0][0].style, Style::Selected);
    assert_eq!(text(&screen, 3).trim_end(), "  src/editor/tests.rs");
    assert_eq!(screen.rows.len(), 6);
    assert!(screen.status.starts_with("Open file: 4 of 4 files"));
    assert_eq!(screen.message, "Open: _");
    assert_eq!(screen.cursor, None);
}

#[test]
fn typing_ranks_and_selects_the_best_match() {
    let mut f = finder();
    type_str(&mut f, "app");
    assert_eq!(f.selected_path(), Some("src/app.rs"));
    let screen = f.render(40, 8);
    assert_eq!(text(&screen, 0).trim_end(), "  src/app.rs");
    assert!(screen.status.starts_with("Open file: 1 of 4 files"));
    assert_eq!(screen.message, "Open: app_");
}

#[test]
fn typing_resets_the_selection_to_the_top() {
    let mut f = finder();
    f.handle_key(key(KeyCode::Down));
    f.handle_key(key(KeyCode::Down));
    assert_eq!(f.selected_path(), Some("src/editor.rs"));
    type_str(&mut f, "s");
    assert_eq!(f.selected, 0);
}

#[test]
fn backspace_widens_the_list_again() {
    let mut f = finder();
    type_str(&mut f, "appx");
    assert_eq!(f.selected_path(), None);
    assert!(text(&f.render(40, 6), 0).contains("No match"));
    f.handle_key(key(KeyCode::Backspace));
    assert_eq!(f.selected_path(), Some("src/app.rs"));
    for _ in 0..5 {
        f.handle_key(key(KeyCode::Backspace));
    }
    assert_eq!(f.query(), "");
    assert_eq!(f.render(40, 6).status.split(" of ").next(), Some("Open file: 4"));
}

#[test]
fn arrows_home_end_and_pages_move_and_clamp() {
    let mut f = finder();
    f.handle_key(key(KeyCode::Up));
    assert_eq!(f.selected, 0);
    f.handle_key(key(KeyCode::Down));
    assert_eq!(f.selected, 1);
    f.handle_key(key(KeyCode::End));
    assert_eq!(f.selected, 3);
    f.handle_key(key(KeyCode::Down));
    assert_eq!(f.selected, 3);
    f.handle_key(key(KeyCode::PageUp));
    assert_eq!(f.selected, 0);
    f.handle_key(key(KeyCode::PageDown));
    assert_eq!(f.selected, 3);
    f.handle_key(key(KeyCode::Home));
    assert_eq!(f.selected, 0);
}

#[test]
fn enter_opens_the_selection_and_does_nothing_without_one() {
    let mut f = finder();
    f.handle_key(key(KeyCode::Down));
    assert_eq!(f.handle_key(key(KeyCode::Enter)), FinderAction::Open("src/app.rs".to_string()));
    type_str(&mut f, "zzz");
    assert_eq!(f.handle_key(key(KeyCode::Enter)), FinderAction::None);
    let mut empty = Finder::new(Vec::new(), false);
    assert_eq!(empty.handle_key(key(KeyCode::Enter)), FinderAction::None);
    assert!(text(&empty.render(40, 6), 0).contains("No files"));
}

#[test]
fn esc_and_ctrl_e_go_back_and_other_ctrl_keys_are_not_text() {
    let mut f = finder();
    assert_eq!(f.handle_key(key(KeyCode::Esc)), FinderAction::Back);
    assert_eq!(f.handle_key(ctrl('e')), FinderAction::Back);
    assert_eq!(f.handle_key(ctrl('x')), FinderAction::None);
    assert_eq!(f.query(), "");
    // q and ? are text here, unlike in the other views.
    type_str(&mut f, "q");
    assert_eq!(f.query(), "q");
}

#[test]
fn a_paste_adds_text_without_control_chars() {
    let mut f = finder();
    f.paste("ed\nit\r\x1b[2Jor");
    assert_eq!(f.query(), "edit[2Jor");
}

#[test]
fn a_message_replaces_the_query_row_until_the_next_key() {
    let mut f = finder();
    f.set_message("File too large".to_string());
    assert_eq!(f.render(40, 6).message, "File too large");
    f.handle_key(key(KeyCode::Down));
    assert_eq!(f.render(40, 6).message, "Open: _");
}

#[test]
fn a_cut_list_says_so() {
    let f = Finder::new(vec!["a".to_string()], true);
    assert!(f.render(40, 6).status.contains("(list cut)"));
}

#[test]
fn a_long_query_shows_its_end() {
    let mut f = finder();
    type_str(&mut f, &"x".repeat(50));
    let message = f.render(20, 6).message;
    assert_eq!(message.chars().count(), 20);
    assert!(message.starts_with('\u{2026}') && message.ends_with("x_"), "{message:?}");
}

#[test]
fn the_selection_stays_visible_in_a_long_list_and_at_any_size() {
    let files: Vec<String> = (0..100).map(|i| format!("f{i:03}")).collect();
    let mut f = Finder::new(files, false);
    for _ in 0..50 {
        f.handle_key(key(KeyCode::Down));
    }
    let screen = f.render(20, 10);
    let shown: Vec<String> = (0..8).map(|i| text(&screen, i)).collect();
    assert!(shown.iter().any(|r| r.trim_end() == "  f050"), "{shown:?}");
    assert_eq!(screen.rows.iter().filter(|r| r.first().is_some_and(|s| s.style == Style::Selected)).count(), 1);
    for w in 0..=8 {
        for h in 0..=6 {
            let screen = f.render(w, h);
            assert_eq!(screen.rows.len(), h.saturating_sub(2));
            assert!(screen.rows.iter().all(|r| r.iter().all(|s| s.text.chars().count() <= w)));
            assert!(screen.status.chars().count() <= w && screen.message.chars().count() <= w);
        }
    }
}

#[test]
fn hostile_file_names_are_drawn_as_question_marks() {
    let f = Finder::new(vec!["bad\x1b[2Jname".to_string()], false);
    let screen = f.render(40, 6);
    assert_eq!(text(&screen, 0).trim_end(), "  bad?[2Jname");
}

// --- walk_files ---------------------------------------------------------------

fn temp_dir() -> PathBuf {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!("ted-finder-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn touch(root: &Path, rel: &str) {
    let path = root.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, "x").unwrap();
}

#[test]
fn walk_lists_files_sorted_with_slash_paths() {
    let root = temp_dir();
    for rel in ["b.txt", "a/z.rs", "a/b/c.rs", "README"] {
        touch(&root, rel);
    }
    let (files, cut) = walk_files(&root);
    assert_eq!(files, ["README", "a/b/c.rs", "a/z.rs", "b.txt"]);
    assert!(!cut);
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn walk_skips_hidden_target_and_node_modules() {
    let root = temp_dir();
    for rel in [".git/config", ".env", "target/debug/x", "node_modules/m/i.js", "src/target.rs", "src/.hidden/a", "keep.txt"] {
        touch(&root, rel);
    }
    assert_eq!(walk_files(&root).0, ["keep.txt", "src/target.rs"]);
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn walk_does_not_follow_directory_symlinks_but_lists_file_links() {
    let root = temp_dir();
    touch(&root, "real/a.txt");
    std::os::unix::fs::symlink(root.join("real"), root.join("linkdir")).unwrap();
    std::os::unix::fs::symlink(root.join("real/a.txt"), root.join("link.txt")).unwrap();
    std::os::unix::fs::symlink(root.join("missing"), root.join("dangling")).unwrap();
    assert_eq!(walk_files(&root).0, ["link.txt", "real/a.txt"]);
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn walk_of_a_missing_or_empty_directory_is_empty() {
    assert!(walk_files(Path::new("/definitely/not/here")).0.is_empty());
    let root = temp_dir();
    assert!(walk_files(&root).0.is_empty());
    fs::remove_dir_all(&root).unwrap();
}
