use super::*;
use crate::screen::{Row, Screen, Style};
use std::fs;
use std::os::unix::fs::{symlink, PermissionsExt};
use std::path::Path;

/// A unique temp directory, removed on drop (permissions restored first so
/// removal works even when a test fails after a chmod).
struct Temp(PathBuf);

impl Temp {
    fn new(name: &str) -> Self {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "ted-tree-test-{}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed),
            name
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Temp(dir)
    }

    fn file(&self, rel: &str) {
        let p = self.0.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, "x").unwrap();
    }

    fn dir(&self, rel: &str) {
        fs::create_dir_all(self.0.join(rel)).unwrap();
    }

    fn tree(&self) -> FileTree {
        FileTree::new(self.0.clone())
    }
}

fn restore(dir: &Path) {
    let _ = fs::set_permissions(dir, fs::Permissions::from_mode(0o755));
    if let Ok(rd) = fs::read_dir(dir) {
        for e in rd.flatten() {
            if e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                restore(&e.path());
            }
        }
    }
}

impl Drop for Temp {
    fn drop(&mut self) {
        restore(&self.0);
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn ctrl(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
}

fn names(t: &FileTree) -> Vec<String> {
    t.visible().into_iter().map(|r| r.name).collect()
}

fn paths(t: &FileTree) -> Vec<String> {
    t.visible().into_iter().map(|r| r.path).collect()
}

fn press(t: &mut FileTree, codes: &[KeyCode]) {
    for c in codes {
        t.handle_key(key(*c));
    }
}

#[test]
fn lists_root_directories_first_then_files_case_insensitively() {
    let d = Temp::new("order");
    d.file("b.txt");
    d.file("A.txt");
    d.dir("zeta");
    d.dir("Alpha");
    d.file("c.txt");
    let t = d.tree();
    assert_eq!(names(&t), ["Alpha", "zeta", "A.txt", "b.txt", "c.txt"]);
    let rows = t.visible();
    assert_eq!(rows[0].kind, NodeKind::Dir);
    assert_eq!(rows[2].kind, NodeKind::File);
    assert!(rows.iter().all(|r| r.depth == 0 && !r.expanded && !r.unreadable));
}

#[test]
fn sort_key_ignores_surrounding_whitespace_and_ties_break_on_the_exact_name() {
    let d = Temp::new("order-trim");
    d.file("alpha.txt");
    d.file(" beta.txt"); // leading space: sorts as "beta.txt"
    d.file("Charlie.txt");
    d.file("x");
    d.file("x "); // same key as "x" and " x"; exact name breaks the tie
    d.file(" x");
    d.dir(" zdir");
    d.dir("Adir");
    let t = d.tree();
    assert_eq!(
        names(&t),
        ["Adir", " zdir", "alpha.txt", " beta.txt", "Charlie.txt", " x", "x", "x "]
    );
}

#[test]
fn symlink_to_a_file_is_a_file() {
    let d = Temp::new("filelink");
    d.file("real.txt");
    symlink(d.0.join("real.txt"), d.0.join("link.txt")).unwrap();
    let t = d.tree();
    assert_eq!(names(&t), ["link.txt", "real.txt"]);
    assert!(t.visible().iter().all(|r| r.kind == NodeKind::File));
}

#[test]
fn dot_git_is_hidden_but_other_dotfiles_are_shown() {
    let d = Temp::new("dotgit");
    d.dir(".git");
    d.file(".gitignore");
    d.file(".env");
    d.file("sub/.git"); // a .git file (worktree) is hidden too
    d.file("sub/keep");
    let mut t = d.tree();
    assert_eq!(names(&t), ["sub", ".env", ".gitignore"]);
    press(&mut t, &[KeyCode::Right]);
    assert_eq!(names(&t), ["sub", "keep", ".env", ".gitignore"]);
}

#[test]
fn names_with_spaces_unicode_and_newlines_are_kept_exactly() {
    let d = Temp::new("names");
    d.file("a b.txt");
    d.file("caf\u{e9} \u{65e5}\u{672c}.txt");
    d.file("line\nbreak");
    let t = d.tree();
    let n = names(&t);
    assert!(n.contains(&"a b.txt".to_string()));
    assert!(n.contains(&"caf\u{e9} \u{65e5}\u{672c}.txt".to_string()));
    assert!(n.contains(&"line\nbreak".to_string()));
}

#[test]
fn right_expands_and_left_collapses_a_directory() {
    let d = Temp::new("expand");
    d.file("dir/inner.txt");
    d.file("top.txt");
    let mut t = d.tree();
    assert_eq!(paths(&t), ["dir", "top.txt"]);
    press(&mut t, &[KeyCode::Right]);
    assert_eq!(paths(&t), ["dir", "dir/inner.txt", "top.txt"]);
    let rows = t.visible();
    assert!(rows[0].expanded);
    assert_eq!(rows[1].depth, 1);
    assert_eq!(t.selected_path(), Some("dir"));
    press(&mut t, &[KeyCode::Left]);
    assert_eq!(paths(&t), ["dir", "top.txt"]);
    assert_eq!(t.selected_path(), Some("dir"));
}

#[test]
fn right_on_expanded_directory_moves_to_first_child() {
    let d = Temp::new("firstchild");
    d.file("dir/a.txt");
    d.file("dir/b.txt");
    let mut t = d.tree();
    press(&mut t, &[KeyCode::Right]);
    assert_eq!(t.selected_path(), Some("dir"));
    press(&mut t, &[KeyCode::Right]);
    assert_eq!(t.selected_path(), Some("dir/a.txt"));
    // Right on a file does nothing.
    press(&mut t, &[KeyCode::Right]);
    assert_eq!(t.selected_path(), Some("dir/a.txt"));
    assert_eq!(paths(&t).len(), 3);
}

#[test]
fn left_on_a_file_selects_its_parent_and_does_nothing_at_depth_zero() {
    let d = Temp::new("left");
    d.file("dir/a.txt");
    d.file("top.txt");
    let mut t = d.tree();
    press(&mut t, &[KeyCode::Right, KeyCode::Down]);
    assert_eq!(t.selected_path(), Some("dir/a.txt"));
    press(&mut t, &[KeyCode::Left]);
    assert_eq!(t.selected_path(), Some("dir"));
    // Collapsed dir at depth 0: Left does nothing.
    press(&mut t, &[KeyCode::Left, KeyCode::Left]);
    assert_eq!(t.selected_path(), Some("dir"));
    press(&mut t, &[KeyCode::Down]);
    assert_eq!(t.selected_path(), Some("top.txt"));
    press(&mut t, &[KeyCode::Left]);
    assert_eq!(t.selected_path(), Some("top.txt"));
}

#[test]
fn enter_toggles_a_directory_and_opens_a_file_with_a_root_relative_slash_path() {
    let d = Temp::new("enter");
    d.file("dir/sub/deep.txt");
    d.file("top.txt");
    let mut t = d.tree();
    assert_eq!(t.handle_key(key(KeyCode::Enter)), TreeAction::None);
    assert_eq!(paths(&t), ["dir", "dir/sub", "top.txt"]);
    press(&mut t, &[KeyCode::Down]);
    assert_eq!(t.handle_key(key(KeyCode::Enter)), TreeAction::None);
    press(&mut t, &[KeyCode::Down]);
    assert_eq!(t.selected_path(), Some("dir/sub/deep.txt"));
    assert_eq!(
        t.handle_key(key(KeyCode::Enter)),
        TreeAction::Open("dir/sub/deep.txt".to_string())
    );
    press(&mut t, &[KeyCode::Up, KeyCode::Up]);
    assert_eq!(t.selected_path(), Some("dir"));
    assert_eq!(t.handle_key(key(KeyCode::Enter)), TreeAction::None);
    assert_eq!(paths(&t), ["dir", "top.txt"]);
}

#[test]
fn up_down_clamp_and_page_home_end_move_the_selection() {
    let d = Temp::new("move");
    for i in 0..25 {
        d.file(&format!("f{i:02}"));
    }
    let mut t = d.tree();
    press(&mut t, &[KeyCode::Up]);
    assert_eq!(t.selected_path(), Some("f00"));
    press(&mut t, &[KeyCode::Down]);
    assert_eq!(t.selected_path(), Some("f01"));
    press(&mut t, &[KeyCode::PageDown]);
    assert_eq!(t.selected_path(), Some("f11"));
    press(&mut t, &[KeyCode::PageDown, KeyCode::PageDown]);
    assert_eq!(t.selected_path(), Some("f24"));
    press(&mut t, &[KeyCode::Down]);
    assert_eq!(t.selected_path(), Some("f24"));
    press(&mut t, &[KeyCode::PageUp]);
    assert_eq!(t.selected_path(), Some("f14"));
    press(&mut t, &[KeyCode::Home]);
    assert_eq!(t.selected_path(), Some("f00"));
    press(&mut t, &[KeyCode::PageUp]);
    assert_eq!(t.selected_path(), Some("f00"));
    press(&mut t, &[KeyCode::End]);
    assert_eq!(t.selected_path(), Some("f24"));
}

#[test]
fn collapsing_a_directory_containing_the_selection_selects_the_directory() {
    let d = Temp::new("collapse");
    d.file("dir/sub/deep.txt");
    let mut t = d.tree();
    press(&mut t, &[KeyCode::Right, KeyCode::Down, KeyCode::Right, KeyCode::Down]);
    assert_eq!(t.selected_path(), Some("dir/sub/deep.txt"));
    // Collapse the grandparent through Enter on it.
    press(&mut t, &[KeyCode::Home]);
    t.handle_key(key(KeyCode::Enter));
    assert_eq!(paths(&t), ["dir"]);
    assert_eq!(t.selected_path(), Some("dir"));
    // Collapsing with the selection deeper than the directory.
    press(&mut t, &[KeyCode::Right, KeyCode::Down, KeyCode::Right, KeyCode::Down]);
    assert_eq!(t.selected_path(), Some("dir/sub/deep.txt"));
    press(&mut t, &[KeyCode::Up, KeyCode::Left]);
    assert_eq!(t.selected_path(), Some("dir/sub"));
    assert_eq!(paths(&t), ["dir", "dir/sub"]);
}

#[test]
fn refresh_picks_up_new_files_and_keeps_selection_by_path() {
    let d = Temp::new("refresh");
    d.file("b.txt");
    d.file("c.txt");
    let mut t = d.tree();
    press(&mut t, &[KeyCode::Down]);
    assert_eq!(t.selected_path(), Some("c.txt"));
    d.file("a.txt"); // sorts before the selection, shifting its row index
    t.refresh();
    assert_eq!(paths(&t), ["a.txt", "b.txt", "c.txt"]);
    assert_eq!(t.selected_path(), Some("c.txt"));
}

#[test]
fn refresh_after_selected_file_is_deleted_selects_nearest_ancestor() {
    let d = Temp::new("deleted");
    d.file("dir/sub/deep.txt");
    d.file("dir/other.txt");
    let mut t = d.tree();
    press(&mut t, &[KeyCode::Right, KeyCode::Down, KeyCode::Right, KeyCode::Down]);
    assert_eq!(t.selected_path(), Some("dir/sub/deep.txt"));
    fs::remove_file(d.0.join("dir/sub/deep.txt")).unwrap();
    t.refresh();
    assert_eq!(t.selected_path(), Some("dir/sub"));
    fs::remove_dir_all(d.0.join("dir")).unwrap();
    t.refresh();
    assert_eq!(t.selected_path(), None);
    assert!(paths(&t).is_empty());
}

#[test]
fn deleted_expanded_directory_is_dropped_on_refresh() {
    let d = Temp::new("dropped");
    d.file("dir/a.txt");
    let mut t = d.tree();
    press(&mut t, &[KeyCode::Right]);
    fs::remove_dir_all(d.0.join("dir")).unwrap();
    t.refresh();
    assert!(paths(&t).is_empty());
    // A new directory of the same name must not come back expanded.
    d.file("dir/b.txt");
    t.refresh();
    assert_eq!(paths(&t), ["dir"]);
    assert!(!t.visible()[0].expanded);
}

#[test]
fn unreadable_directory_sets_message_and_marks_unreadable() {
    let d = Temp::new("unreadable");
    d.file("locked/secret.txt");
    d.file("open.txt");
    fs::set_permissions(d.0.join("locked"), fs::Permissions::from_mode(0o000)).unwrap();
    if fs::read_dir(d.0.join("locked")).is_ok() {
        return; // running as root: permissions are ignored
    }
    let mut t = d.tree();
    assert_eq!(t.message(), "");
    press(&mut t, &[KeyCode::Right]);
    assert!(t.message().starts_with("Cannot read locked: "), "{}", t.message());
    let rows = t.visible();
    assert_eq!(paths(&t), ["locked", "open.txt"]);
    assert!(rows[0].unreadable);
    assert!(rows[0].expanded);
    assert!(!rows[1].unreadable);
}

#[test]
fn symlink_to_directory_is_a_directory_and_a_cycle_does_not_hang() {
    let d = Temp::new("symlink");
    d.file("real/f.txt");
    symlink(d.0.join("real"), d.0.join("link")).unwrap();
    d.dir("loop");
    symlink(&d.0, d.0.join("loop/up")).unwrap(); // points at the root, so it is a cycle
    let mut t = d.tree();
    let rows = t.visible();
    let link = rows.iter().find(|r| r.name == "link").unwrap();
    assert_eq!(link.kind, NodeKind::Dir);
    // Walk into loop/up/loop/up: each expansion is one more level.
    press(&mut t, &[KeyCode::Down]);
    assert_eq!(t.selected_path(), Some("loop"));
    press(&mut t, &[KeyCode::Right, KeyCode::Down, KeyCode::Right]);
    assert_eq!(t.selected_path(), Some("loop/up"));
    press(&mut t, &[KeyCode::Down, KeyCode::Down, KeyCode::Right]);
    assert_eq!(t.selected_path(), Some("loop/up/loop"));
    press(&mut t, &[KeyCode::Down]);
    assert_eq!(t.selected_path(), Some("loop/up/loop/up"));
    t.refresh();
    assert!(paths(&t).contains(&"loop/up/loop/up".to_string()));
}

#[test]
fn broken_symlink_is_a_file() {
    let d = Temp::new("broken");
    symlink(d.0.join("missing"), d.0.join("dangling")).unwrap();
    let mut t = d.tree();
    assert_eq!(t.visible()[0].kind, NodeKind::File);
    assert_eq!(
        t.handle_key(key(KeyCode::Enter)),
        TreeAction::Open("dangling".to_string())
    );
}

#[test]
fn back_keys_and_toggle_git_key() {
    let d = Temp::new("keys");
    d.file("a.txt");
    let mut t = d.tree();
    assert_eq!(t.handle_key(key(KeyCode::Char('q'))), TreeAction::Back);
    assert_eq!(t.handle_key(key(KeyCode::Esc)), TreeAction::Back);
    assert_eq!(t.handle_key(ctrl('t')), TreeAction::Back);
    assert_eq!(t.handle_key(ctrl('g')), TreeAction::ToggleGit);
    assert_eq!(t.handle_key(ctrl('q')), TreeAction::None);
    assert_eq!(t.handle_key(key(KeyCode::Char('x'))), TreeAction::None);
    assert_eq!(t.handle_key(key(KeyCode::Char('t'))), TreeAction::None);
    assert_eq!(t.handle_key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::ALT)), TreeAction::None);
}

#[test]
fn empty_directory_has_no_selection_and_keys_do_not_panic() {
    let d = Temp::new("empty");
    let mut t = d.tree();
    assert!(t.visible().is_empty());
    assert_eq!(t.selected_path(), None);
    for code in [
        KeyCode::Up,
        KeyCode::Down,
        KeyCode::PageUp,
        KeyCode::PageDown,
        KeyCode::Home,
        KeyCode::End,
        KeyCode::Left,
        KeyCode::Right,
        KeyCode::Enter,
    ] {
        assert_eq!(t.handle_key(key(code)), TreeAction::None);
    }
    t.refresh();
    assert_eq!(t.selected_path(), None);
}

#[test]
fn missing_root_shows_one_unreadable_marker_and_nothing_is_selectable() {
    let d = Temp::new("noroot");
    let mut t = FileTree::new(d.0.join("nope"));
    let rows = t.visible();
    assert_eq!(rows.len(), 1);
    assert!(rows[0].unreadable);
    assert_eq!(t.selected_path(), None);
    assert!(t.message().starts_with("Cannot read "), "{}", t.message());
    assert_eq!(t.handle_key(key(KeyCode::Enter)), TreeAction::None);
}

#[test]
fn set_message_replaces_the_message_and_the_next_key_clears_it() {
    let d = Temp::new("message");
    d.file("a.txt");
    let mut t = d.tree();
    t.set_message("hello".to_string());
    assert_eq!(t.message(), "hello");
    t.handle_key(key(KeyCode::Down));
    assert_eq!(t.message(), "");
}

// ---- render ----

fn row_text(r: &Row) -> String {
    r.iter().map(|s| s.text.as_str()).collect()
}

fn texts(s: &Screen) -> Vec<String> {
    s.rows.iter().map(row_text).collect()
}

fn root_name(t: &FileTree) -> String {
    t.display_name("")
}

#[test]
fn render_shows_indent_markers_names_and_trailing_slash_for_directories() {
    let d = Temp::new("r-layout");
    d.file("src/main.rs");
    d.dir("empty");
    d.file("a.txt");
    let mut t = d.tree();
    press(&mut t, &[KeyCode::Down, KeyCode::Right]); // expand src
    let s = t.render(40, 10);
    let rows: Vec<String> = texts(&s).iter().take(4).map(|r| r.trim_end().to_string()).collect();
    assert_eq!(rows, vec!["> empty/", "v src/", "    main.rs", "  a.txt"]);
}

#[test]
fn render_styles_directories_selection_and_unreadable_marker() {
    let d = Temp::new("r-style");
    d.dir("locked");
    d.file("a.txt");
    fs::set_permissions(d.0.join("locked"), fs::Permissions::from_mode(0o000)).unwrap();
    let mut t = d.tree();
    press(&mut t, &[KeyCode::Right]); // expand locked (fails)
    press(&mut t, &[KeyCode::Down]); // locked -> a.txt
    let s = t.render(30, 10);
    assert_eq!(texts(&s)[0].trim_end(), "v locked/");
    assert_eq!(s.rows[0][0].style, Style::Dir);
    assert_eq!(texts(&s)[1].trim_end(), "    (cannot read)");
    assert_eq!(s.rows[1][0].style, Style::Dim);
    assert_eq!(texts(&s)[2].trim_end(), "  a.txt");
    assert_eq!(s.rows[2][0].style, Style::Selected);
    // A selected directory is Selected, not Dir.
    let t2 = d.tree();
    assert_eq!(t2.render(30, 10).rows[0][0].style, Style::Selected);
}

#[test]
fn render_selected_row_is_padded_to_full_width() {
    let d = Temp::new("r-pad");
    d.file("a.txt");
    let s = d.tree().render(20, 5);
    assert_eq!(s.rows[0][0].text.chars().count(), 20);
    assert_eq!(s.rows[0][0].style, Style::Selected);
}

#[test]
fn render_status_is_exactly_width_and_shows_root_name_and_count() {
    let d = Temp::new("r-status");
    d.file("a.txt");
    d.file("b.txt");
    let t = d.tree();
    let want = format!("Files: {}  2 entries visible", root_name(&t));
    for w in [want.chars().count(), want.chars().count() + 7, 12] {
        let s = t.render(w, 6);
        assert_eq!(s.status.chars().count(), w);
        assert_eq!(s.status.trim_end(), want.chars().take(w).collect::<String>().trim_end());
    }
    assert!(t.render(200, 6).cursor.is_none());
}

#[test]
fn render_empty_and_unreadable_root_states() {
    let d = Temp::new("r-empty");
    let t = d.tree();
    let s = t.render(60, 6);
    assert_eq!(s.status.trim_end(), format!("Files: {}  (empty)", root_name(&t)));
    assert!(texts(&s).iter().all(|r| r.trim().is_empty()));

    let gone = FileTree::new(d.0.join("does-not-exist"));
    let s = gone.render(60, 6);
    assert_eq!(s.status.trim_end(), "Files: does-not-exist  Error");
    assert_eq!(s.message, gone.message().chars().take(60).collect::<String>());
    assert!(s.message.starts_with("Cannot read"));
}

#[test]
fn render_sanitizes_escape_and_control_characters_in_names() {
    let d = Temp::new("r-san");
    d.file("a\x1b[31mb\rc");
    let mut t = d.tree();
    t.set_message("bad\x1b[0m".to_string());
    let s = t.render(40, 6);
    let all = format!("{}{}{}", texts(&s).join(""), s.status, s.message);
    assert!(!all.contains('\x1b') && !all.contains('\r'));
    assert!(texts(&s)[0].contains("a?[31mb?c"));
}

#[test]
fn render_keeps_selection_visible_in_a_long_listing() {
    let d = Temp::new("r-long");
    for i in 0..30 {
        d.file(&format!("f{i:02}.txt"));
    }
    let mut t = d.tree();
    press(&mut t, &[KeyCode::End]);
    let s = t.render(30, 7); // 5 body rows
    assert_eq!(s.rows.len(), 5);
    assert!(texts(&s)[4].starts_with("  f29.txt"));
    assert_eq!(s.rows[4][0].style, Style::Selected);
    press(&mut t, &[KeyCode::Home]);
    assert!(texts(&t.render(30, 7))[0].starts_with("  f00.txt"));
}

#[test]
fn render_never_exceeds_width_or_height() {
    let d = Temp::new("r-bounds");
    d.file("one/two/three/deep.txt");
    d.file("one/ünïcödé-名前.txt");
    d.file("z.txt");
    let mut t = d.tree();
    press(&mut t, &[KeyCode::Right, KeyCode::Down, KeyCode::Right, KeyCode::Down, KeyCode::Right]);
    for w in 0..=40 {
        for h in 0..=8 {
            let s = t.render(w, h);
            assert_eq!(s.rows.len(), h.saturating_sub(2));
            for r in &s.rows {
                assert!(row_text(r).chars().count() <= w, "w={w} h={h}");
            }
            assert!(s.message.chars().count() <= w);
            if h >= 2 {
                assert_eq!(s.status.chars().count(), w);
            } else {
                assert!(s.status.is_empty() && s.message.is_empty());
            }
        }
    }
}
